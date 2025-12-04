//! 截图模块
//!
//! 提供屏幕捕获和选区管理功能。
//!
//! # 主要组件
//! - [`ScreenshotManager`]: 截图管理器，统一管理截图流程
//! - [`SelectionState`](selection::SelectionState): 选区状态管理
//! - [`save`]: 截图保存功能
//!
//! # 功能特点
//! - 支持全屏捕获和区域选择
//! - 支持窗口自动检测和高亮
//! - 支持 GDI 和 DXGI 两种捕获方式

use crate::interaction::InteractionController;
use crate::message::{Command, ScreenshotMessage};
use crate::platform::windows::SafeHwnd;
use crate::platform::{PlatformError, PlatformRenderer};
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap;
use windows::Win32::Graphics::Gdi::HBITMAP;

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
    /// Direct2D截图位图
    screenshot_bitmap: Option<ID2D1Bitmap>,

    /// 屏幕尺寸
    screen_width: i32,
    screen_height: i32,

    /// UI隐藏状态（截图时隐藏UI元素）
    hide_ui_for_capture: bool,

    /// 是否显示选择框手柄（用于与绘图工具联动）
    show_selection_handles: bool,

    /// 窗口检测器
    window_detector: crate::system::window_detection::WindowDetectionManager,
    /// 自动高亮是否启用
    auto_highlight_enabled: bool,

    /// 当前窗口句柄（用于排除自己的窗口）
    current_window: SafeHwnd,
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
    /// 便捷：外部直接设置选择矩形
    pub fn update_selection(&mut self, rect: windows::Win32::Foundation::RECT) {
        self.selection.update(rect);
    }

    /// 检查是否有截图数据（D2D位图或原始数据）
    pub fn has_screenshot(&self) -> bool {
        self.screenshot_bitmap.is_some() || self.current_screenshot.is_some()
    }

    /// 创建新的截图管理器
    pub fn new() -> Result<Self, ScreenshotError> {
        let (screen_width, screen_height) = crate::platform::windows::system::get_screen_size();

        Ok(Self {
            selection: SelectionState::new(),
            current_screenshot: None,
            selection_interaction: InteractionController::new(),

            // 初始化从WindowState迁移的字段
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
            current_window: SafeHwnd::default(),
        })
    }

    /// 与绘图工具联动：控制是否显示选择框手柄
    pub fn set_show_selection_handles(&mut self, show: bool) {
        self.show_selection_handles = show;
    }

    pub fn get_screen_width(&self) -> i32 {
        self.screen_width
    }

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
                if let Some(_rect) = self.selection.get_selection() {
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
                // 开始选择区域
                if self.current_screenshot.is_some() {
                    self.selection.start_selection(x, y);
                    vec![Command::RequestRedraw]
                } else {
                    vec![Command::None]
                }
            }
        }
    }

    /// 渲染截图内容
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

    /// 按需捕获屏幕并返回GDI位图句柄
    /// 调用方负责释放返回的HBITMAP
    pub fn capture_screen_to_gdi_bitmap(&self) -> Result<HBITMAP, ScreenshotError> {
        let screen_rect = windows::Win32::Foundation::RECT {
            left: 0,
            top: 0,
            right: self.screen_width,
            bottom: self.screen_height,
        };

        unsafe {
            crate::platform::windows::gdi::capture_screen_region_to_hbitmap(screen_rect)
                .map_err(|e| ScreenshotError::CaptureError(format!("GDI capture failed: {e:?}")))
        }
    }

    /// 重置状态
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

        // 重新启用自动窗口高亮功能
        self.auto_highlight_enabled = true;
    }

    /// 设置当前窗口句柄（用于排除自己的窗口）
    pub fn set_current_window(&mut self, hwnd: windows::Win32::Foundation::HWND) {
        self.current_window.set(Some(hwnd));
    }

    // 注意: 绘图工具管理已移至Drawing模块

    /// 重新截取当前屏幕
    pub fn capture_screen(&mut self) -> std::result::Result<(), ScreenshotError> {
        let exclude_hwnd = self
            .current_window
            .get()
            .unwrap_or(windows::Win32::Foundation::HWND(std::ptr::null_mut()));
        self.capture_screen_with_hwnd(exclude_hwnd)
    }

    /// 重新截取当前屏幕（带窗口句柄，用于排除自己的窗口）
    pub fn capture_screen_with_hwnd(
        &mut self,
        _exclude_hwnd: windows::Win32::Foundation::HWND,
    ) -> std::result::Result<(), ScreenshotError> {
        // 获取当前屏幕尺寸（可能在pin后发生了变化）
        let (current_screen_width, current_screen_height) =
            crate::platform::windows::system::get_screen_size();

        // 更新屏幕尺寸
        self.screen_width = current_screen_width;
        self.screen_height = current_screen_height;

        // 标记截图数据已更新（实际捕获将按需进行）
        self.current_screenshot = Some(ScreenshotData {
            width: self.screen_width as u32,
            height: self.screen_height as u32,
            data: vec![], // 暂时为空，实际数据将按需捕获
        });

        // 刷新窗口列表
        if let Err(e) = self.window_detector.refresh_windows() {
            eprintln!("Warning: Failed to refresh windows: {e:?}");
            // 继续运行，不退出程序
        }

        Ok(())
    }

    /// 从GDI位图创建D2D位图
    pub fn create_d2d_bitmap_from_gdi(
        &mut self,
        renderer: &mut dyn crate::platform::PlatformRenderer<Error = crate::platform::PlatformError>,
    ) -> std::result::Result<(), ScreenshotError> {
        // 按需捕获屏幕到GDI位图
        let gdi_bitmap = self.capture_screen_to_gdi_bitmap()?;

            // 使用 downcast 获取 Direct2DRenderer
        if let Some(d2d_renderer) = renderer
            .as_any_mut()
            .downcast_mut::<crate::platform::windows::d2d::Direct2DRenderer>(
        ) {
            // 创建临时DC来使用GDI位图，使用 RAII 封装自动管理资源
            use crate::platform::windows::resources::{ManagedBitmap, ManagedDC};
            use windows::Win32::Foundation::HWND;
            use windows::Win32::Graphics::Gdi::{
                CreateCompatibleDC, GetDC, ReleaseDC, SelectObject,
            };

            // 使用 RAII 封装管理 GDI 位图
            let managed_bitmap = ManagedBitmap::new(gdi_bitmap);

            unsafe {
                let screen_dc = GetDC(Some(HWND(std::ptr::null_mut())));
                let temp_dc = ManagedDC::new(CreateCompatibleDC(Some(screen_dc)));
                SelectObject(temp_dc.handle(), managed_bitmap.handle().into());

                // 创建D2D位图并存储
                let d2d_bitmap = d2d_renderer
                    .create_d2d_bitmap_from_gdi(temp_dc.handle(), self.screen_width, self.screen_height)
                    .map_err(|e| {
                        ScreenshotError::RenderError(format!("Failed to create D2D bitmap: {e:?}"))
                    })?;

                // 提取位图数据供OCR使用（避免重复截图）
                if let Ok(bmp_data) = crate::ocr::bitmap_to_bmp_data(temp_dc.handle(), managed_bitmap.handle(), self.screen_width, self.screen_height)
                     && let Some(ref mut screenshot) = self.current_screenshot {
                        screenshot.data = bmp_data;
                    }

                // 释放 screen_dc（通过 GetDC 获取的需要用 ReleaseDC）
                ReleaseDC(Some(HWND(std::ptr::null_mut())), screen_dc);
                // temp_dc 和 managed_bitmap 会在离开作用域时自动释放

                // 存储 D2D 位图
                self.screenshot_bitmap = Some(d2d_bitmap);
                Ok(())
            }
        } else {
            // gdi_bitmap 使用 RAII 封装自动释放
            let _managed = crate::platform::windows::resources::ManagedBitmap::new(gdi_bitmap);
            Err(ScreenshotError::RenderError(
                "Cannot access D2D renderer".to_string(),
            ))
        }
    }

    /// 获取当前截图的原始图像数据（如果可用）
    pub fn get_current_image_data(&self) -> Option<&[u8]> {
        self.current_screenshot.as_ref().map(|s| s.data.as_slice())
    }

    /// 处理鼠标移动（包含拖拽检测）
    /// 返回 (命令列表, 是否消费了事件)
    pub fn handle_mouse_move(&mut self, x: i32, y: i32) -> (Vec<Command>, bool) {
        // 绘图工具处理已移至Drawing模块

        // 第二优先级：检测拖拽开始
        if self.selection.is_mouse_pressed() && self.auto_highlight_enabled {
            let drag_start = self.selection.get_interaction_start_pos();

            if crate::utils::is_drag_threshold_exceeded(drag_start.x, drag_start.y, x, y) {
                // 开始拖拽，禁用自动高亮
                self.auto_highlight_enabled = false;

                // 如果之前有自动高亮的选择，清除它并开始新的手动选择
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
            // 窗口自动高亮检测（仅在启用自动高亮且没有按下鼠标时）
            if self.auto_highlight_enabled && !self.selection.is_mouse_pressed() {
                // 同时检测窗口和子控件
                let (window_info, control_info) = self.window_detector.detect_at_point(x, y);

                if let Some(control) = control_info {
                    // 优先显示子控件高亮，并限制在屏幕范围内
                    let limited_rect = crate::utils::clamp_rect_to_screen(
                        control.rect,
                        self.screen_width,
                        self.screen_height,
                    );
                    // 直接设置 selection_rect，而不是 auto_highlight_rect
                    self.selection.set_selection_rect(limited_rect);
                    (vec![Command::RequestRedraw], true)
                } else if let Some(window) = window_info {
                    // 如果没有子控件，显示窗口高亮，并限制在屏幕范围内
                    let limited_rect = crate::utils::clamp_rect_to_screen(
                        window.rect,
                        self.screen_width,
                        self.screen_height,
                    );
                    // 直接设置 selection_rect，而不是 auto_highlight_rect
                    self.selection.set_selection_rect(limited_rect);
                    (vec![Command::RequestRedraw], true)
                } else {
                    // 如果没有检测到窗口或子控件，清除自动高亮
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

    /// 处理鼠标按下
    /// 返回 (命令列表, 是否消费了事件)
    pub fn handle_mouse_down(&mut self, x: i32, y: i32) -> (Vec<Command>, bool) {
        if self.current_screenshot.is_none() {
            return (vec![], false);
        }

        // 设置鼠标按下状态
        self.selection.set_mouse_pressed(true);
        self.selection.set_interaction_start_pos(x, y);

        // 绘图工具和元素点击检查已移至Drawing模块

        // 第三优先级：检查工具栏点击
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

        // 第四优先级：开始新的选择框创建
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

    /// 开始拖拽操作
    fn start_drag_internal(&mut self, x: i32, y: i32) {
        // 如果已经有选择框，不允许在外面重新框选
        if self.selection.has_selection() {
            let _ = self
                .selection_interaction
                .mouse_down(&mut self.selection, x, y);
            // 未命中时不开始新选择，保持原行为
        } else {
            // 只有在没有选择框时才允许创建新的选择框
            self.selection.start_selection(x, y);
        }
    }

    /// 处理鼠标释放
    /// 返回 (命令列表, 是否消费了事件)
    pub fn handle_mouse_up(&mut self, x: i32, y: i32) -> (Vec<Command>, bool) {
        let mut commands = Vec::new();

        // 绘图工具完成处理已移至Drawing模块

        // 检查是否是单击（没有拖拽）
        let start_pos = self.selection.get_interaction_start_pos();
        let is_click = self.selection.is_mouse_pressed()
            && !crate::utils::is_drag_threshold_exceeded(start_pos.x, start_pos.y, x, y);

        // 处理选择框创建和拖拽结束
        if self.selection.is_selecting() {
            // 结束选择框创建
            self.selection.end_selection(x, y);
            commands.push(Command::RequestRedraw);

            // 如果选择框创建成功且有效，显示工具栏（仅对手动拖拽创建的选择框）
            if !is_click
                && let Some(rect) = self.selection.get_selection() {
                    commands.push(Command::UI(crate::message::UIMessage::ShowToolbar(rect)));
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

        // 如果没有选择区域，重新启用自动高亮以便下次使用
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

    /// 获取当前选择区域
    pub fn get_selection(&self) -> Option<windows::Win32::Foundation::RECT> {
        self.selection.get_selection()
    }

    /// 是否隐藏UI用于截图
    pub fn is_ui_hidden_for_capture(&self) -> bool {
        self.hide_ui_for_capture
    }

    /// 临时隐藏UI元素进行截图
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

    /// 恢复UI元素显示
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

    /// 处理双击事件
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
            ScreenshotError::CaptureError(msg) => write!(f, "Capture error: {msg}"),
            ScreenshotError::SaveError(msg) => write!(f, "Save error: {msg}"),
            ScreenshotError::InitError(msg) => write!(f, "Init error: {msg}"),
            ScreenshotError::RenderError(msg) => write!(f, "Render error: {msg}"),
            ScreenshotError::SystemError(err) => write!(f, "System error: {err}"),
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

impl ScreenshotManager {
    /// 获取选区图像数据（用于前置条件检查）
    pub fn get_selection_image(&self) -> Option<Vec<u8>> {
        if self.has_selection() && self.has_screenshot() {
            self.current_screenshot.as_ref().map(|s| s.data.clone())
        } else {
            None
        }
    }
}
