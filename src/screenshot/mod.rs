// 截图管理器模块
//
// 负责屏幕捕获、选择区域管理、保存和导出功能

use crate::interaction::InteractionController;
use crate::message::{Command, ScreenshotMessage};
use crate::platform::{PlatformError, PlatformRenderer};
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap;
use windows::Win32::Graphics::Gdi::{HBITMAP, HDC};
// use windows::core::Result; // 不需要，会与std::result::Result冲突

pub mod capture;
pub mod save;
pub mod selection;

use selection::SelectionState;

/// 截图管理器
pub struct ScreenshotManager {
    /// 选择状态
    selection: SelectionState,
    /// 交互控制器（阶段1，仅用于选择框）
    selection_interaction: InteractionController,
    /// 当前截图数据
    current_screenshot: Option<ScreenshotData>,

    // 从WindowState迁移的字段
    /// 传统GDI资源（用于屏幕捕获）
    screenshot_dc: Option<HDC>,
    gdi_screenshot_bitmap: Option<HBITMAP>,

    /// Direct2D截图位图
    screenshot_bitmap: Option<ID2D1Bitmap>,

    /// 屏幕尺寸
    screen_width: i32,
    screen_height: i32,

    /// UI隐藏状态（截图时隐藏UI元素）
    hide_ui_for_capture: bool,

    /// 是否显示选择框手柄（用于与绘图工具联动）
    show_selection_handles: bool,

    /// 窗口检测器（从原始代码迁移）
    window_detector: crate::system::window_detection::WindowDetectionManager,
    /// 自动高亮是否启用（从原始代码迁移）
    auto_highlight_enabled: bool,

    /// 当前窗口句柄（用于排除自己的窗口）
    current_window: Option<windows::Win32::Foundation::HWND>,
}

/// 截图数据
pub struct ScreenshotData {
    /// 图像数据
    pub data: Vec<u8>,
    /// 宽度
    pub width: u32,
    /// 高度
    pub height: u32,
}

impl ScreenshotManager {
    /// 创建新的截图管理器
    pub fn new() -> Result<Self, ScreenshotError> {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::Graphics::Gdi::{
            CreateCompatibleBitmap, CreateCompatibleDC, GetDC, ReleaseDC, SelectObject,
        };
        let (screen_width, screen_height) = crate::platform::windows::system::get_screen_size();

        // 初始化GDI资源（从原始代码迁移）
        let (screenshot_dc, gdi_screenshot_bitmap) = unsafe {
            let screen_dc = GetDC(Some(HWND(std::ptr::null_mut())));
            if screen_dc.is_invalid() {
                return Err(ScreenshotError::InitError(
                    "Failed to get screen DC".to_string(),
                ));
            }

            let screenshot_dc = CreateCompatibleDC(Some(screen_dc));
            if screenshot_dc.is_invalid() {
                ReleaseDC(Some(HWND(std::ptr::null_mut())), screen_dc);
                return Err(ScreenshotError::InitError(
                    "Failed to create compatible DC".to_string(),
                ));
            }

            let gdi_screenshot_bitmap =
                CreateCompatibleBitmap(screen_dc, screen_width, screen_height);
            if gdi_screenshot_bitmap.is_invalid() {
                let _ = windows::Win32::Graphics::Gdi::DeleteDC(screenshot_dc);
                ReleaseDC(Some(HWND(std::ptr::null_mut())), screen_dc);
                return Err(ScreenshotError::InitError(
                    "Failed to create compatible bitmap".to_string(),
                ));
            }

            SelectObject(screenshot_dc, gdi_screenshot_bitmap.into());
            ReleaseDC(Some(HWND(std::ptr::null_mut())), screen_dc);

            (Some(screenshot_dc), Some(gdi_screenshot_bitmap))
        };

        Ok(Self {
            selection: SelectionState::new(),
            current_screenshot: None,
            selection_interaction: InteractionController::new(),

            // 初始化从WindowState迁移的字段
            screenshot_dc,
            gdi_screenshot_bitmap,
            screenshot_bitmap: None,
            screen_width,
            screen_height,
            hide_ui_for_capture: false,
            show_selection_handles: true,
            window_detector: {
                let mut detector = crate::system::window_detection::WindowDetectionManager::new()?;
                detector.start_detection()?; // 启用窗口检测
                detector
            },
            auto_highlight_enabled: true, // 默认启用自动高亮
            current_window: None,
        })
    }

    /// 与绘图工具联动：控制是否显示选择框手柄
    pub fn set_show_selection_handles(&mut self, show: bool) {
        self.show_selection_handles = show;
    }

    /// 获取屏幕宽度
    pub fn get_screen_width(&self) -> i32 {
        self.screen_width
    }

    /// 获取屏幕高度
    pub fn get_screen_height(&self) -> i32 {
        self.screen_height
    }

    /// 是否应该显示选择框手柄
    pub fn should_show_selection_handles(&self) -> bool {
        self.show_selection_handles
    }

    /// 是否因截图而隐藏UI
    pub fn is_hiding_ui_for_capture(&self) -> bool {
        self.hide_ui_for_capture
    }

    /// 是否有自动高亮
    pub fn has_auto_highlight(&self) -> bool {
        self.selection.has_auto_highlight()
    }

    /// 处理截图消息
    pub fn handle_message(&mut self, message: ScreenshotMessage) -> Vec<Command> {
        match message {
            ScreenshotMessage::StartCapture => {
                // 执行屏幕捕获（使用已有的capture_screen方法）
                match self.capture_screen() {
                    Ok(()) => {
                        vec![Command::ShowOverlay]
                    }
                    Err(_) => {
                        vec![Command::ShowError("Failed to capture screen".to_string())]
                    }
                }
            }
            ScreenshotMessage::UpdateSelection(rect) => {
                self.selection.update(rect);
                vec![Command::RequestRedraw]
            }
            ScreenshotMessage::ConfirmSelection => {
                if let Some(rect) = self.selection.get_selection() {
                    // 处理选择确认
                    vec![Command::ShowSaveDialog]
                } else {
                    vec![Command::None]
                }
            }
            ScreenshotMessage::CancelCapture => {
                self.current_screenshot = None;
                self.selection.clear();
                vec![Command::HideOverlay]
            }
            ScreenshotMessage::StartSelection(x, y) => {
                // 开始选择区域（从原始handle_left_button_down迁移）
                if self.current_screenshot.is_some() {
                    self.selection.start_selection(x, y);
                    vec![Command::RequestRedraw]
                } else {
                    vec![Command::None]
                }
            }
            ScreenshotMessage::EndSelection(_x, _y) => {
                // 注意：此消息处理已废弃，选择结束逻辑已移至 handle_mouse_up 方法中
                // 保留此分支以避免编译错误，但不执行任何操作
                vec![Command::None]
            }
        }
    }

    /// 渲染截图内容（按照原始代码逻辑）
    pub fn render(
        &mut self,
        renderer: &mut dyn PlatformRenderer<Error = PlatformError>,
    ) -> Result<(), ScreenshotError> {
        if let Some(d2d_renderer) = renderer
            .as_any_mut()
            .downcast_mut::<crate::platform::windows::d2d::Direct2DRenderer>(
        ) {
            unsafe {
                if let Some(render_target) = &d2d_renderer.render_target {
                    // 绘制截图背景（如果有D2D位图）
                    if let Some(screenshot_bitmap) = &self.screenshot_bitmap {
                        let dest_rect = windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F {
                            left: 0.0,
                            top: 0.0,
                            right: d2d_renderer.get_screen_width() as f32,
                            bottom: d2d_renderer.get_screen_height() as f32,
                        };
                        render_target.DrawBitmap(
                            screenshot_bitmap,
                            Some(&dest_rect),
                            1.0,
                            windows::Win32::Graphics::Direct2D::D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                            None,
                        );
                    }

                    // UI渲染（遮罩、边框、手柄）现在由UIManager负责
                    // ScreenshotManager只负责截图背景绘制
                }
            }
        }
        Ok(())
    }

    /// 获取GDI位图句柄（用于简单显示）
    pub fn get_gdi_bitmap(&self) -> Option<HBITMAP> {
        self.gdi_screenshot_bitmap
    }

    /// 重置状态（从原始reset_to_initial_state迁移）
    pub fn reset_state(&mut self) {
        // 清除当前截图
        self.current_screenshot = None;

        // 重置选择状态
        self.selection.reset();

        // 工具状态由Drawing模块管理

        // 重置屏幕尺寸（如果之前被pin功能修改过）
        let (w, h) = crate::platform::windows::system::get_screen_size();
        self.screen_width = w;
        self.screen_height = h;

        // 重置UI隐藏状态
        self.hide_ui_for_capture = false;

        // 默认显示选择框手柄（新一轮截图开始时应可见）
        self.show_selection_handles = true;

        // 重新启用自动窗口高亮功能（从原始代码迁移）
        self.auto_highlight_enabled = true;
    }

    /// 设置当前窗口句柄（用于排除自己的窗口）
    pub fn set_current_window(&mut self, hwnd: windows::Win32::Foundation::HWND) {
        self.current_window = Some(hwnd);
    }

    /// 绘图工具管理已移至Drawing模块

    /// 重新截取当前屏幕（从WindowState迁移）
    pub fn capture_screen(&mut self) -> std::result::Result<(), ScreenshotError> {
        let exclude_hwnd = self
            .current_window
            .unwrap_or(windows::Win32::Foundation::HWND(std::ptr::null_mut()));
        self.capture_screen_with_hwnd(exclude_hwnd)
    }

    /// 重新截取当前屏幕（带窗口句柄，用于排除自己的窗口）
    pub fn capture_screen_with_hwnd(
        &mut self,
        exclude_hwnd: windows::Win32::Foundation::HWND,
    ) -> std::result::Result<(), ScreenshotError> {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::Graphics::Gdi::{
            BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC,
            ReleaseDC, SRCCOPY, SelectObject,
        };
        unsafe {
            // 获取当前屏幕尺寸（可能在pin后发生了变化）
            let (current_screen_width, current_screen_height) =
                crate::platform::windows::system::get_screen_size();

            // 如果屏幕尺寸发生了变化，需要重新创建资源
            if current_screen_width != self.screen_width
                || current_screen_height != self.screen_height
            {
                self.screen_width = current_screen_width;
                self.screen_height = current_screen_height;

                // 重新创建GDI资源
                let screen_dc = GetDC(Some(HWND(std::ptr::null_mut())));
                let new_screenshot_dc = CreateCompatibleDC(Some(screen_dc));
                let new_gdi_bitmap =
                    CreateCompatibleBitmap(screen_dc, self.screen_width, self.screen_height);
                SelectObject(new_screenshot_dc, new_gdi_bitmap.into());

                // 清理旧资源
                if let Some(old_dc) = self.screenshot_dc {
                    let _ = DeleteDC(old_dc);
                }
                if let Some(old_bitmap) = self.gdi_screenshot_bitmap {
                    let _ = DeleteObject(old_bitmap.into());
                }

                // 更新资源
                self.screenshot_dc = Some(new_screenshot_dc);
                self.gdi_screenshot_bitmap = Some(new_gdi_bitmap);

                ReleaseDC(Some(HWND(std::ptr::null_mut())), screen_dc);
            }

            // 获取屏幕DC
            let screen_dc = GetDC(Some(HWND(std::ptr::null_mut())));

            // 重新捕获屏幕到GDI位图
            if let Some(screenshot_dc) = self.screenshot_dc {
                BitBlt(
                    screenshot_dc,
                    0,
                    0,
                    self.screen_width,
                    self.screen_height,
                    Some(screen_dc),
                    0,
                    0,
                    SRCCOPY,
                )
                .map_err(|e| ScreenshotError::CaptureError(format!("BitBlt failed: {}", e)))?;
            }

            // 释放屏幕DC
            ReleaseDC(Some(HWND(std::ptr::null_mut())), screen_dc);

            // 从更新的GDI位图重新创建D2D位图（从原始代码迁移）
            // 注意：这里需要渲染器来创建D2D位图，但当前架构中ScreenshotManager无法直接访问
            // 我们需要重构这个方法，让它接收渲染器参数
            // 暂时先标记截图数据已更新
            self.current_screenshot = Some(ScreenshotData {
                width: self.screen_width as u32,
                height: self.screen_height as u32,
                data: vec![], // 暂时为空，实际数据在GDI位图中
            });

            // 刷新窗口列表（从原始代码迁移）
            if let Err(e) = self.window_detector.refresh_windows() {
                eprintln!("Warning: Failed to refresh windows: {:?}", e);
                // 继续运行，不退出程序
            }

            Ok(())
        }
    }

    /// 从GDI位图创建D2D位图（从原始代码迁移，恢复原有逻辑）
    pub fn create_d2d_bitmap_from_gdi(
        &mut self,
        renderer: &mut dyn crate::platform::PlatformRenderer<Error = crate::platform::PlatformError>,
    ) -> std::result::Result<(), ScreenshotError> {
        if let Some(screenshot_dc) = self.screenshot_dc {
            // 使用downcast获取Direct2DRenderer（与旧代码保持一致）
            if let Some(d2d_renderer) = renderer
                .as_any_mut()
                .downcast_mut::<crate::platform::windows::d2d::Direct2DRenderer>(
            ) {
                // 创建D2D位图并存储（与旧代码逻辑一致）
                let d2d_bitmap = d2d_renderer
                    .create_d2d_bitmap_from_gdi(
                        screenshot_dc,
                        self.screen_width,
                        self.screen_height,
                    )
                    .map_err(|e| {
                        ScreenshotError::RenderError(format!(
                            "Failed to create D2D bitmap: {:?}",
                            e
                        ))
                    })?;

                // 存储D2D位图（关键：与旧代码保持一致）
                self.screenshot_bitmap = Some(d2d_bitmap);

                Ok(())
            } else {
                Err(ScreenshotError::RenderError(
                    "Cannot access D2D renderer".to_string(),
                ))
            }
        } else {
            Err(ScreenshotError::CaptureError(
                "No screenshot DC available".to_string(),
            ))
        }
    }

    /// 处理鼠标移动（完全按照原始代码逻辑，包含拖拽检测）
    /// 返回 (命令列表, 是否消费了事件)
    pub fn handle_mouse_move(&mut self, x: i32, y: i32) -> (Vec<Command>, bool) {
        // 绘图工具处理已移至Drawing模块

        // 第二优先级：检测拖拽开始（从原始代码迁移）
        if self.selection.is_mouse_pressed() && self.auto_highlight_enabled {
            let drag_start = self.selection.get_interaction_start_pos();
            let dx = (x - drag_start.x).abs();
            let dy = (y - drag_start.y).abs();
            const DRAG_THRESHOLD: i32 = 5; // 拖拽阈值

            if dx > DRAG_THRESHOLD || dy > DRAG_THRESHOLD {
                // 开始拖拽，禁用自动高亮（从原始代码迁移）
                self.auto_highlight_enabled = false;

                // 如果之前有自动高亮的选择，清除它并开始新的手动选择（从原始代码迁移）
                if self.selection.has_selection() {
                    self.selection.clear_selection();
                    self.selection.start_selection(drag_start.x, drag_start.y);
                }
                return (vec![Command::RequestRedraw], true);
            }
        }

        if self.selection.is_selecting() {
            // 正在创建新选择框
            self.selection.update_end_point(x, y);
            (vec![Command::RequestRedraw], true)
        } else if self.selection.is_dragging() {
            // 正在拖拽选择框或调整大小（统一交互控制器）
            if self
                .selection_interaction
                .mouse_move(&mut self.selection, x, y)
            {
                let mut commands = vec![Command::RequestRedraw];

                // 拖拽已有选择框时，更新工具栏位置
                if let Some(selection_rect) = self.selection.get_selection() {
                    commands.push(Command::UI(
                        crate::message::UIMessage::UpdateToolbarPosition(selection_rect),
                    ));
                }

                (commands, true)
            } else {
                (vec![], false)
            }
        } else {
            // 窗口自动高亮检测（仅在启用自动高亮且没有按下鼠标时）（完全按照原始代码逻辑）
            if self.auto_highlight_enabled && !self.selection.is_mouse_pressed() {
                // 同时检测窗口和子控件（按照原始代码逻辑）
                let (window_info, control_info) = self.window_detector.detect_at_point(x, y);

                if let Some(control) = control_info {
                    // 优先显示子控件高亮，并限制在屏幕范围内（按照原始代码逻辑）
                    let limited_rect = windows::Win32::Foundation::RECT {
                        left: control.rect.left.max(0),
                        top: control.rect.top.max(0),
                        right: control.rect.right.min(self.screen_width),
                        bottom: control.rect.bottom.min(self.screen_height),
                    };
                    // 按照原始代码：直接设置selection_rect，而不是auto_highlight_rect
                    self.selection.set_selection_rect(limited_rect);
                    (vec![Command::RequestRedraw], true)
                } else if let Some(window) = window_info {
                    // 如果没有子控件，显示窗口高亮，并限制在屏幕范围内（按照原始代码逻辑）
                    let limited_rect = windows::Win32::Foundation::RECT {
                        left: window.rect.left.max(0),
                        top: window.rect.top.max(0),
                        right: window.rect.right.min(self.screen_width),
                        bottom: window.rect.bottom.min(self.screen_height),
                    };
                    // 按照原始代码：直接设置selection_rect，而不是auto_highlight_rect
                    self.selection.set_selection_rect(limited_rect);
                    (vec![Command::RequestRedraw], true)
                } else {
                    // 如果没有检测到窗口或子控件，清除自动高亮（按照原始代码逻辑）
                    if self.selection.has_selection() {
                        self.selection.clear_selection();
                        (vec![Command::RequestRedraw], true)
                    } else {
                        (vec![], false)
                    }
                }
            } else {
                (vec![], false)
            }
        }
    }

    /// 处理鼠标按下（完全按照原始代码逻辑）
    /// 返回 (命令列表, 是否消费了事件)
    pub fn handle_mouse_down(&mut self, x: i32, y: i32) -> (Vec<Command>, bool) {
        if self.current_screenshot.is_none() {
            return (vec![], false);
        }

        // 设置鼠标按下状态（从原始代码迁移）
        self.selection.set_mouse_pressed(true);
        self.selection.set_interaction_start_pos(x, y);

        // 绘图工具和元素点击检查已移至Drawing模块

        // 第三优先级：检查工具栏点击（从原始代码迁移）
        if self.selection.has_selection() {
            // 工具栏点击检测需要通过UI管理器处理
            // 这里暂时跳过，让UI管理器在App层处理工具栏点击
        }

        // 检查手柄/内部点击：通过统一交互控制器
        if self.selection.has_selection() {
            let consumed = self
                .selection_interaction
                .mouse_down(&mut self.selection, x, y);
            if consumed {
                return (vec![Command::RequestRedraw], true);
            } else {
                // 有选区但未命中手柄/内部：保持原行为，忽略此次点击
                self.selection.set_mouse_pressed(false);
                return (vec![], false);
            }
        }

        // 第四优先级：开始新的选择框创建（从原始代码迁移）
        // 修复：当启用自动高亮时，不要立即开始选择框创建，
        // 等待在 handle_mouse_move 中超过拖拽阈值后再开始，
        // 以避免小幅移动被误判为“单击确认”。
        if !self.auto_highlight_enabled {
            self.start_drag_internal(x, y);
            (vec![Command::RequestRedraw], true)
        } else {
            (vec![], false)
        }
    }

    /// 开始拖拽操作（完全按照原始代码的start_drag逻辑）
    fn start_drag_internal(&mut self, x: i32, y: i32) {
        // 如果已经有选择框，不允许在外面重新框选（从原始代码迁移）
        if self.selection.has_selection() {
            let _ = self
                .selection_interaction
                .mouse_down(&mut self.selection, x, y);
            // 未命中时不开始新选择，保持原行为
        } else {
            // 只有在没有选择框时才允许创建新的选择框（从原始代码迁移）
            self.selection.start_selection(x, y);
        }
    }

    /// 处理鼠标释放（完全按照原始代码逻辑）
    /// 返回 (命令列表, 是否消费了事件)
    pub fn handle_mouse_up(&mut self, x: i32, y: i32) -> (Vec<Command>, bool) {
        let mut commands = Vec::new();

        // 绘图工具完成处理已移至Drawing模块

        // 检查是否是单击（没有拖拽）（从原始代码迁移）
        let is_click = self.selection.is_mouse_pressed()
            && (x - self.selection.get_interaction_start_pos().x).abs() < 5
            && (y - self.selection.get_interaction_start_pos().y).abs() < 5;

        // 处理选择框创建和拖拽结束
        if self.selection.is_selecting() {
            // 结束选择框创建
            self.selection.end_selection(x, y);
            commands.push(Command::RequestRedraw);

            // 如果选择框创建成功且有效，显示工具栏（仅对手动拖拽创建的选择框）
            if !is_click {
                if let Some(rect) = self.selection.get_selection() {
                    commands.push(Command::UI(crate::message::UIMessage::ShowToolbar(rect)));
                }
            }
        } else if self.selection.is_dragging() {
            // 结束拖拽操作（统一交互控制器）
            let _ = self
                .selection_interaction
                .mouse_up(&mut self.selection, x, y);
            commands.push(Command::RequestRedraw);

            // 更新工具栏位置（如果有选择框）
            if let Some(rect) = self.selection.get_selection() {
                commands.push(Command::UI(
                    crate::message::UIMessage::UpdateToolbarPosition(rect),
                ));
            }
        }

        // 处理单击确认逻辑（独立于选择框创建和拖拽逻辑）
        if is_click && self.selection.has_selection() {
            // 单击确认：无论是自动高亮还是手动框选后的单击，都显示工具栏
            if let Some(rect) = self.selection.get_selection() {
                commands.push(Command::UI(crate::message::UIMessage::ShowToolbar(rect)));
            }
            // 单击确认后进入已选择状态，禁用自动高亮
            self.auto_highlight_enabled = false;
        } else if is_click && !self.selection.has_selection() {
            // 如果是单击但没有选择区域，重新启用自动高亮
            self.auto_highlight_enabled = true;
        }

        // 如果没有选择区域，重新启用自动高亮以便下次使用（从原始代码迁移）
        if !self.selection.has_selection() {
            self.auto_highlight_enabled = true;
        }

        // 重置鼠标按下状态
        self.selection.set_mouse_pressed(false);

        let consumed = !commands.is_empty();
        (commands, consumed)
    }

    /// 获取D2D位图
    pub fn get_d2d_bitmap(&self) -> Option<&windows::Win32::Graphics::Direct2D::ID2D1Bitmap> {
        self.screenshot_bitmap.as_ref()
    }

    /// 是否有选择区域
    pub fn has_selection(&self) -> bool {
        self.selection.has_selection()
    }

    /// 获取当前选择区域（从原始代码迁移）
    pub fn get_selection(&self) -> Option<windows::Win32::Foundation::RECT> {
        self.selection.get_selection()
    }

    /// 是否隐藏UI用于截图
    pub fn is_ui_hidden_for_capture(&self) -> bool {
        self.hide_ui_for_capture
    }

    /// 临时隐藏UI元素进行截图（从原始代码迁移）
    pub fn hide_ui_for_capture(&mut self, hwnd: windows::Win32::Foundation::HWND) {
        self.hide_ui_for_capture = true;
        unsafe {
            use windows::Win32::Foundation::FALSE;
            use windows::Win32::Graphics::Gdi::{InvalidateRect, UpdateWindow};

            // 强制重绘以隐藏UI元素
            let _ = InvalidateRect(Some(hwnd), None, FALSE.into());
            let _ = UpdateWindow(hwnd);
            // 等待重绘完成
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }

    /// 恢复UI元素显示（从原始代码迁移）
    pub fn show_ui_after_capture(&mut self, hwnd: windows::Win32::Foundation::HWND) {
        self.hide_ui_for_capture = false;
        unsafe {
            use windows::Win32::Foundation::FALSE;
            use windows::Win32::Graphics::Gdi::{InvalidateRect, UpdateWindow};

            // 强制重绘以显示UI元素
            let _ = InvalidateRect(Some(hwnd), None, FALSE.into());
            let _ = UpdateWindow(hwnd);
        }
    }

    /// 获取截图DC（从原始代码迁移）
    pub fn get_screenshot_dc(&self) -> windows::Win32::Graphics::Gdi::HDC {
        self.screenshot_dc.unwrap_or_default()
    }

    /// 处理双击事件（从原始代码迁移）
    pub fn handle_double_click(&mut self, _x: i32, _y: i32) -> Vec<Command> {
        // 双击可能用于确认选择或快速操作
        // 如果有选择区域，双击可能表示确认选择
        if let Some(_selection_rect) = self.selection.get_selection() {
            // 双击确认选择，可以触发保存到剪贴板
            vec![Command::SaveSelectionToClipboard, Command::HideWindow]
        } else {
            vec![]
        }
    }
}

/// 截图错误类型
#[derive(Debug)]
pub enum ScreenshotError {
    /// 捕获失败
    CaptureError(String),
    /// 保存失败
    SaveError(String),
    /// 初始化失败
    InitError(String),
    /// 渲染失败
    RenderError(String),
    /// 系统错误
    SystemError(crate::system::SystemError),
}

impl From<crate::system::SystemError> for ScreenshotError {
    fn from(error: crate::system::SystemError) -> Self {
        ScreenshotError::SystemError(error)
    }
}

impl std::fmt::Display for ScreenshotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScreenshotError::CaptureError(msg) => write!(f, "Capture error: {}", msg),
            ScreenshotError::SaveError(msg) => write!(f, "Save error: {}", msg),
            ScreenshotError::InitError(msg) => write!(f, "Init error: {}", msg),
            ScreenshotError::RenderError(msg) => write!(f, "Render error: {}", msg),
            ScreenshotError::SystemError(err) => write!(f, "System error: {}", err),
        }
    }
}

impl std::error::Error for ScreenshotError {}

impl ScreenshotManager {
    /// 代理选择状态的手柄命中检测（方便App层统一处理光标）
    pub fn get_handle_at_position(&self, x: i32, y: i32) -> crate::types::DragMode {
        self.selection.get_handle_at_position(x, y)
    }
}
