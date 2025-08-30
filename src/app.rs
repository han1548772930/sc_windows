use crate::drawing::DrawingManager;
use crate::error::{AppError, AppResult};
use crate::event_handler::{
    KeyboardEventHandler, MouseEventHandler, SystemEventHandler, WindowEventHandler,
};
use crate::message::{Command, Message, ScreenshotMessage};
use crate::platform::{PlatformError, PlatformRenderer};
use crate::screenshot::ScreenshotManager;
use crate::system::SystemManager;
use crate::ui::UIManager;

/// 应用程序主结构体
pub struct App {
    /// 截图管理器
    screenshot: ScreenshotManager,
    /// 绘图管理器
    drawing: DrawingManager,
    /// UI管理器
    ui: UIManager,
    /// 系统管理器
    system: SystemManager,
    /// 平台渲染器
    platform: Box<dyn PlatformRenderer<Error = PlatformError> + Send + Sync>,
}

impl App {
    pub fn new(
        platform: Box<dyn PlatformRenderer<Error = PlatformError> + Send + Sync>,
    ) -> AppResult<Self> {
        let screenshot = ScreenshotManager::new()?;

        Ok(Self {
            screenshot,
            drawing: DrawingManager::new()?,
            ui: UIManager::new()?,
            system: SystemManager::new()?,
            platform,
        })
    }

    /// 重置到初始状态
    pub fn reset_to_initial_state(&mut self) {
        self.screenshot.reset_state();
        self.drawing.reset_state();
        self.ui.reset_state();
    }

    /// 绘制窗口内容
    pub fn paint(&mut self, hwnd: windows::Win32::Foundation::HWND) -> AppResult<()> {
        use windows::Win32::Graphics::Gdi::{BeginPaint, EndPaint, PAINTSTRUCT};

        unsafe {
            let mut ps = PAINTSTRUCT::default();
            BeginPaint(hwnd, &mut ps);

            let _ = self.render();

            let _ = EndPaint(hwnd, &ps);
        }

        Ok(())
    }

    /// 渲染所有组件
    pub fn render(&mut self) -> AppResult<()> {
        self.platform
            .begin_frame()
            .map_err(|e| AppError::Render(format!("Failed to begin frame: {e:?}")))?;

        self.platform
            .clear(crate::platform::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            })
            .map_err(|e| AppError::Render(format!("Failed to clear: {e:?}")))?;

        self.screenshot
            .render(&mut *self.platform)
            .map_err(|e| AppError::Render(format!("Failed to render screenshot: {e:?}")))?;

        let selection_rect = self.screenshot.get_selection();

        self.drawing
            .render(&mut *self.platform, selection_rect.as_ref())
            .map_err(|e| AppError::Render(format!("Failed to render drawing: {e:?}")))?;

        let screen_size = (
            self.screenshot.get_screen_width(),
            self.screenshot.get_screen_height(),
        );
        let show_handles = self.screenshot.should_show_selection_handles();
        let hide_ui_for_capture = self.screenshot.is_hiding_ui_for_capture();
        let has_auto_highlight = self.screenshot.has_auto_highlight();

        self.ui
            .render_selection_ui(
                &mut *self.platform,
                screen_size,
                selection_rect.as_ref(),
                show_handles,
                hide_ui_for_capture,
                has_auto_highlight,
            )
            .map_err(|e| AppError::Render(format!("Failed to render selection UI: {e:?}")))?;

        self.ui
            .render(&mut *self.platform)
            .map_err(|e| AppError::Render(format!("Failed to render UI: {e:?}")))?;

        self.platform
            .end_frame()
            .map_err(|e| AppError::Render(format!("Failed to end frame: {e:?}")))?;

        Ok(())
    }

    /// 处理消息并返回需要执行的命令
    pub fn handle_message(&mut self, message: Message) -> Vec<Command> {
        match message {
            Message::Screenshot(msg) => {
                let mut commands = self.screenshot.handle_message(msg.clone());

                if let ScreenshotMessage::StartCapture = msg {
                    if commands.contains(&Command::ShowOverlay) {
                        let _ = self
                            .screenshot
                            .create_d2d_bitmap_from_gdi(&mut *self.platform);

                        self.drawing.reset_state();
                        self.update_toolbar_state();
                        commands.push(Command::UpdateToolbar);
                        commands.push(Command::RequestRedraw);
                    }
                }

                commands
            }
            Message::Drawing(msg) => self.drawing.handle_message(msg),
            Message::UI(msg) => {
                let screen_width = unsafe {
                    windows::Win32::UI::WindowsAndMessaging::GetSystemMetrics(
                        windows::Win32::UI::WindowsAndMessaging::SM_CXSCREEN,
                    )
                };
                let screen_height = unsafe {
                    windows::Win32::UI::WindowsAndMessaging::GetSystemMetrics(
                        windows::Win32::UI::WindowsAndMessaging::SM_CYSCREEN,
                    )
                };
                self.ui.handle_message(msg, screen_width, screen_height)
            }
            Message::System(msg) => self.system.handle_message(msg),
        }
    }

    /// 初始化系统托盘
    pub fn init_system_tray(
        &mut self,
        hwnd: windows::Win32::Foundation::HWND,
    ) -> AppResult<()> {
        self.system
            .initialize(hwnd)
            .map_err(|e| AppError::Init(format!("Failed to initialize system tray: {e}")))
    }

    /// 启动异步OCR引擎状态检查
    pub fn start_async_ocr_check(&mut self, hwnd: windows::Win32::Foundation::HWND) {
        self.system.start_async_ocr_check(hwnd);
    }

    /// 处理光标定时器（用于文本输入光标闪烁）
    pub fn handle_cursor_timer(&mut self, timer_id: u32) -> Vec<Command> {
        self.drawing.handle_cursor_timer(timer_id)
    }

    /// 异步停止OCR引擎
    pub fn stop_ocr_engine_async(&mut self) {
        self.system.stop_ocr_engine_async();
    }

    /// 重新加载设置
    pub fn reload_settings(&mut self) -> Vec<Command> {
        self.system.reload_settings();
        self.drawing.reload_drawing_properties();
        vec![Command::UpdateToolbar, Command::RequestRedraw]
    }

    /// 重新注册热键
    pub fn reregister_hotkey(
        &mut self,
        hwnd: windows::Win32::Foundation::HWND,
    ) -> windows::core::Result<()> {
        self.system.reregister_hotkey(hwnd)
    }

    /// 更新OCR引擎状态
    pub fn update_ocr_engine_status(
        &mut self,
        available: bool,
        hwnd: windows::Win32::Foundation::HWND,
    ) {
        self.system.update_ocr_engine_status(available, hwnd);
    }

    /// 处理键盘输入
    pub fn handle_key_input(&mut self, key: u32) -> Vec<Command> {
        use windows::Win32::UI::Input::KeyboardAndMouse::VK_ESCAPE;
        match key {
            k if k == VK_ESCAPE.0 as u32 => {
                // ESC键清除所有状态并隐藏窗口
                self.reset_to_initial_state();
                vec![Command::HideWindow]
            }

            _ => {
                // 其他按键传递给各个管理器处理
                let mut commands = Vec::new();
                commands.extend(self.system.handle_key_input(key));
                if commands.is_empty() {
                    commands.extend(self.drawing.handle_key_input(key));
                }
                if commands.is_empty() {
                    commands.extend(self.ui.handle_key_input(key));
                }
                commands
            }
        }
    }

    /// 选择绘图工具
    pub fn select_drawing_tool(
        &mut self,
        tool: crate::types::DrawingTool,
    ) -> Vec<crate::message::Command> {
        let message = crate::message::DrawingMessage::SelectTool(tool);
        self.drawing.handle_message(message)
    }

    /// 创建D2D位图
    pub fn create_d2d_bitmap_from_gdi(&mut self) -> AppResult<()> {
        self.screenshot
            .create_d2d_bitmap_from_gdi(&mut *self.platform)
            .map_err(|e| AppError::Render(format!("Failed to create D2D bitmap: {e:?}")))
    }

    // execute_command 方法已经移动到 command_executor.rs

    /// 处理鼠标移动
    pub fn handle_mouse_move(&mut self, x: i32, y: i32) -> Vec<Command> {
        let mut commands = Vec::new();

        // 1) UI优先 - 使用事件消费机制
        let (ui_commands, ui_consumed) = self.ui.handle_mouse_move(x, y);
        commands.extend(ui_commands);

        if !ui_consumed {
            // 2) 将事件优先交给 Drawing（元素拖拽/调整/绘制）
            let selection_rect = self.screenshot.get_selection();
            let (drawing_commands, drawing_consumed) =
                self.drawing.handle_mouse_move(x, y, selection_rect);
            commands.extend(drawing_commands);

            if !drawing_consumed && !self.drawing.is_dragging() {
                // 3) 若Drawing未消费且未拖拽，再交给Screenshot（用于自动高亮/选择框拖拽）
                let (screenshot_commands, _screenshot_consumed) =
                    self.screenshot.handle_mouse_move(x, y);
                commands.extend(screenshot_commands);
            }
        }

        // 4) 统一设置鼠标指针（使用光标管理器）
        let cursor_id = {
            let hovered_button = self.ui.get_hovered_button();
            let is_button_disabled = self.ui.is_button_disabled(hovered_button);
            let is_text_editing = self.drawing.is_text_editing();

            // 获取文本编辑元素信息
            let editing_element_info = if is_text_editing {
                if let Some(edit_idx) = self.drawing.get_editing_element_index() {
                    self.drawing
                        .get_element_ref(edit_idx)
                        .map(|el| (el.clone(), edit_idx))
                } else {
                    None
                }
            } else {
                None
            };

            // 获取选中元素信息
            let selected_element_info =
                if let Some(sel_idx) = self.drawing.get_selected_element_index() {
                    self.drawing
                        .get_element_ref(sel_idx)
                        .map(|el| (el.clone(), sel_idx))
                } else {
                    None
                };

            let selection_rect = self.screenshot.get_selection();
            let current_tool = self.drawing.get_current_tool();
            let selection_handle_mode = self.screenshot.get_handle_at_position(x, y);

            crate::ui::cursor::CursorManager::determine_cursor(
                x,
                y,
                hovered_button,
                is_button_disabled,
                is_text_editing,
                editing_element_info,
                current_tool,
                selection_rect,
                selected_element_info,
                selection_handle_mode,
                &self.drawing,
            )
        };

        crate::ui::cursor::CursorManager::set_cursor(cursor_id);

        commands
    }

    /// 处理鼠标按下
    pub fn handle_mouse_down(&mut self, x: i32, y: i32) -> Vec<Command> {
        let mut commands = Vec::new();

        // UI层优先处理（按钮点击等）- 使用事件消费机制
        let (ui_commands, ui_consumed) = self.ui.handle_mouse_down(x, y);
        commands.extend(ui_commands);

        // 如果UI没有消费事件，则传递给其他管理器
        if !ui_consumed {
            let selection_rect = self.screenshot.get_selection();
            // 先让Drawing尝试接管（元素手柄/移动/绘制）
            let (drawing_commands, drawing_consumed) =
                self.drawing.handle_mouse_down(x, y, selection_rect);
            commands.extend(drawing_commands);

            // 若未被Drawing消费，再交给Screenshot（选择框手柄/新建选择等）
            if !drawing_consumed {
                let (screenshot_commands, screenshot_consumed) =
                    self.screenshot.handle_mouse_down(x, y);
                commands.extend(screenshot_commands);

                // 如果Drawing和Screenshot都没有消耗事件，清除元素选中状态（保持与原始逻辑一致）
                if !screenshot_consumed {
                    commands.extend(
                        self.drawing
                            .handle_message(crate::message::DrawingMessage::SelectElement(None)),
                    );
                }
            }
        }

        commands
    }

    /// 处理鼠标释放
    pub fn handle_mouse_up(&mut self, x: i32, y: i32) -> Vec<Command> {
        let mut commands = Vec::new();

        // 使用事件消费机制处理鼠标释放
        let (ui_commands, ui_consumed) = self.ui.handle_mouse_up(x, y);
        commands.extend(ui_commands);

        if !ui_consumed {
            // 先结束Drawing的拖拽，再让Screenshot基于点击/拖拽状态更新
            let (drawing_commands, drawing_consumed) = self.drawing.handle_mouse_up(x, y);
            commands.extend(drawing_commands);

            if !drawing_consumed {
                let (screenshot_commands, _screenshot_consumed) =
                    self.screenshot.handle_mouse_up(x, y);
                commands.extend(screenshot_commands);
            }
        }

        commands
    }

    /// 处理双击事件
    pub fn handle_double_click(&mut self, x: i32, y: i32) -> Vec<Command> {
        let mut commands = Vec::new();

        // UI层优先处理
        commands.extend(self.ui.handle_double_click(x, y));

        if commands.is_empty() {
            let selection_rect = self.screenshot.get_selection();
            // 优先让Drawing处理（双击文本进入编辑）
            let dcmds = self
                .drawing
                .handle_double_click(x, y, selection_rect.as_ref());
            if dcmds.is_empty() {
                // 若未消费，再交给Screenshot（双击确认选择保存）
                commands.extend(self.screenshot.handle_double_click(x, y));
            } else {
                commands.extend(dcmds);
            }
        }

        commands
    }

    /// 处理文本输入
    pub fn handle_text_input(&mut self, character: char) -> Vec<Command> {
        let mut commands = Vec::new();

        // 文本输入主要由绘图管理器处理（文本工具）
        commands.extend(self.drawing.handle_text_input(character));

        commands
    }

    /// 处理托盘消息
    pub fn handle_tray_message(&mut self, wparam: u32, lparam: u32) -> Vec<Command> {
        // 通过系统管理器处理托盘消息
        self.system.handle_tray_message(wparam, lparam)
    }

    /// 执行截图
    pub fn take_screenshot(
        &mut self,
        hwnd: windows::Win32::Foundation::HWND,
    ) -> AppResult<()> {
        use crate::utils::win_api;

        // 重置状态并开始截图（按照原始代码逻辑）
        self.screenshot.reset_state();

        // 设置当前窗口并捕获屏幕
        self.screenshot.set_current_window(hwnd);
        self.screenshot.capture_screen()?;

        // 显示窗口进入选择模式
        let _ = win_api::show_window(hwnd);

        Ok(())
    }

    /// 直接捕获屏幕（用于热键处理，与原始代码一致）
    pub fn capture_screen_direct(&mut self) -> AppResult<()> {
        self.screenshot
            .capture_screen()
            .map_err(|e| AppError::Screenshot(format!("Failed to capture screen: {e:?}")))
    }

    /// 更新工具栏状态以反映当前选中的绘图工具
    pub fn update_toolbar_state(&mut self) {
        use crate::types::DrawingTool;
        use crate::types::ToolbarButton;
        use std::collections::HashSet;

        // 工具栏始终显示当前绘图工具（与原代码保持一致，选中元素时已经更新了当前工具）
        let current_tool = self.drawing.get_current_tool();

        // 更新工具栏选中状态
        self.ui.update_toolbar_selected_tool(current_tool);

        // 根据当前工具状态决定是否显示选择框手柄：仅当未选择绘图工具时显示
        let current_tool = self.drawing.get_current_tool();
        let show_handles = matches!(current_tool, DrawingTool::None);
        self.screenshot.set_show_selection_handles(show_handles);

        // 更新禁用按钮集合
        let mut disabled: HashSet<ToolbarButton> = HashSet::new();
        if !self.can_undo() {
            disabled.insert(ToolbarButton::Undo);
        }
        if !self.is_ocr_engine_available() {
            disabled.insert(ToolbarButton::ExtractText);
        }
        self.ui.set_toolbar_disabled(disabled);
    }

    /// 保存选择区域到剪贴板（使用 screenshot 模块的统一接口）
    pub fn save_selection_to_clipboard(
        &mut self,
        hwnd: windows::Win32::Foundation::HWND,
    ) -> AppResult<()> {
        use windows::Win32::Graphics::Gdi::DeleteObject;

        // 获取选择区域
        let Some(selection_rect) = self.screenshot.get_selection() else {
            return Ok(());
        };

        let width = selection_rect.right - selection_rect.left;
        let height = selection_rect.bottom - selection_rect.top;
        if width <= 0 || height <= 0 {
            return Ok(());
        }

        // 临时隐藏UI元素
        self.screenshot.hide_ui_for_capture(hwnd);

        let result = {
            // 使用 screenshot 模块捕获区域
            let bitmap = match crate::screenshot::capture::capture_region_to_hbitmap(selection_rect)
            {
                Ok(b) => b,
                Err(e) => {
                    self.screenshot.show_ui_after_capture(hwnd);
                    return Err(AppError::Screenshot(format!(
                        "Failed to capture region: {e:?}"
                    )));
                }
            };

            // 使用 screenshot 模块复制到剪贴板
            let copy_result = crate::screenshot::save::copy_hbitmap_to_clipboard(bitmap);

            // 清理位图资源
            unsafe {
                let _ = DeleteObject(bitmap.into());
            }

            copy_result
                .map_err(|e| AppError::Screenshot(format!("Failed to copy to clipboard: {e:?}")))
        };

        // 恢复UI元素
        self.screenshot.show_ui_after_capture(hwnd);
        result
    }

    /// 保存选择区域到文件（严格遵循原始同步逻辑）
    /// 返回 Ok(true) 表示保存成功，Ok(false) 表示用户取消，Err 表示错误
    pub fn save_selection_to_file(
        &mut self,
        hwnd: windows::Win32::Foundation::HWND,
    ) -> Result<bool, AppError> {
        use windows::Win32::Graphics::Gdi::DeleteObject;

        // 没有有效选择则直接返回
        let Some(selection_rect) = self.screenshot.get_selection() else {
            return Ok(false);
        };

        let width = selection_rect.right - selection_rect.left;
        let height = selection_rect.bottom - selection_rect.top;
        if width <= 0 || height <= 0 {
            return Ok(false);
        }

        // 显示文件保存对话框
        let Some(file_path) = crate::file_dialog::show_image_save_dialog(hwnd, "screenshot.png")
        else {
            return Ok(false); // 用户取消了对话框
        };

        // 临时隐藏UI元素
        self.screenshot.hide_ui_for_capture(hwnd);

        let result = {
            // 使用 screenshot 模块捕获区域
            let bitmap = match crate::screenshot::capture::capture_region_to_hbitmap(selection_rect)
            {
                Ok(b) => b,
                Err(e) => {
                    self.screenshot.show_ui_after_capture(hwnd);
                    return Err(AppError::Screenshot(format!(
                        "Failed to capture region: {e:?}"
                    )));
                }
            };

            // 使用 screenshot 模块保存到文件
            let save_result =
                crate::screenshot::save::save_hbitmap_to_file(bitmap, &file_path, width, height);

            // 清理资源（保存后释放位图）
            unsafe {
                let _ = DeleteObject(bitmap.into());
            }

            save_result.map_err(|e| format!("Failed to save file: {e:?}"))
        };

        // 恢复UI元素
        self.screenshot.show_ui_after_capture(hwnd);
        result.map(|_| true).map_err(|e| AppError::File(e))
    }

    /// 从选择区域提取文本（简化版本 - 委托给OcrManager）
    pub fn extract_text_from_selection(
        &mut self,
        hwnd: windows::Win32::Foundation::HWND,
    ) -> AppResult<()> {
        // 检查是否有选择区域
        let Some(selection_rect) = self.screenshot.get_selection() else {
            return Ok(());
        };

        // 委托给SystemManager处理整个OCR流程
        self.system
            .recognize_text_from_selection(selection_rect, hwnd, &mut self.screenshot)
            .map_err(|e| AppError::System(format!("OCR识别失败: {e}")))
    }

    /// 固定选择区域
    pub fn pin_selection(
        &mut self,
        hwnd: windows::Win32::Foundation::HWND,
    ) -> AppResult<()> {
        // 检查是否有选择区域
        let Some(selection_rect) = self.screenshot.get_selection() else {
            return Ok(());
        };

        let width = selection_rect.right - selection_rect.left;
        let height = selection_rect.bottom - selection_rect.top;

        if width <= 0 || height <= 0 {
            return Ok(());
        }

        // 临时隐藏UI元素进行截图
        self.screenshot.hide_ui_for_capture(hwnd);

        // 创建固钉窗口
        let result = self.create_pin_window(hwnd, width, height, selection_rect);

        // 恢复UI元素显示（虽然窗口即将隐藏，但保持一致性）
        self.screenshot.show_ui_after_capture(hwnd);

        // 隐藏原始截屏窗口
        let _ = crate::utils::win_api::hide_window(hwnd);

        // 重置原始窗口状态，准备下次截屏
        self.reset_to_initial_state();

        result
    }

    /// 创建固钉窗口
    fn create_pin_window(
        &self,
        _parent_hwnd: windows::Win32::Foundation::HWND,
        width: i32,
        height: i32,
        selection_rect: windows::Win32::Foundation::RECT,
    ) -> AppResult<()> {
        use windows::Win32::Foundation::*;
        use windows::Win32::Graphics::Gdi::*;
        use windows::Win32::System::LibraryLoader::GetModuleHandleW;
        use windows::Win32::UI::WindowsAndMessaging::*;

        unsafe {
            // 获取选择区域的屏幕截图（通过 screenshot 统一入口，包含绘图内容）
            let bitmap = match crate::screenshot::capture::capture_region_to_hbitmap(selection_rect)
            {
                Ok(b) => b,
                Err(e) => {
                    return Err(AppError::Screenshot(format!(
                        "Failed to capture pin image: {e:?}"
                    )));
                }
            };

            // 注册固钉窗口类
            let hinstance = GetModuleHandleW(None)
                .map_err(|e| AppError::WinApi(format!("Failed to get module handle: {e:?}")))?;

            let class_name: Vec<u16> = "PinWindow\0".encode_utf16().collect();

            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(pin_window_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: hinstance.into(),
                hIcon: HICON::default(),
                hCursor: LoadCursorW(Some(HINSTANCE(std::ptr::null_mut())), IDC_ARROW)
                    .unwrap_or_default(),
                hbrBackground: HBRUSH::default(),
                lpszMenuName: windows::core::PCWSTR::null(),
                lpszClassName: windows::core::PCWSTR(class_name.as_ptr()),
                hIconSm: HICON::default(),
            };

            RegisterClassExW(&wc);

            // 创建固钉窗口
            let pin_hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
                windows::core::PCWSTR(class_name.as_ptr()),
                windows::core::w!("Pin Window"),
                WS_POPUP | WS_VISIBLE,
                selection_rect.left,
                selection_rect.top,
                width,
                height,
                None,
                None,
                Some(hinstance.into()),
                None,
            )
            .map_err(|e| AppError::WinApi(format!("Failed to create pin window: {e:?}")))?;

            if pin_hwnd.0.is_null() {
                return Err(AppError::WinApi("Pin window handle is null".to_string()));
            }

            // 将位图句柄存储到窗口数据中
            SetWindowLongPtrW(pin_hwnd, GWLP_USERDATA, bitmap.0 as isize);

            // 显示固钉窗口
            let _ = ShowWindow(pin_hwnd, SW_SHOW);
            let _ = UpdateWindow(pin_hwnd);

            Ok(())
        }
    }

    /// 检查是否可以撤销
    pub fn can_undo(&self) -> bool {
        self.drawing.can_undo()
    }

    /// 检查OCR引擎是否可用（非阻塞，基于缓存状态）
    pub fn is_ocr_engine_available(&self) -> bool {
        // 使用 SystemManager 中缓存的引擎可用状态，避免阻塞 UI
        self.system.ocr_is_available()
    }
}

// 实现EventHandler traits
impl MouseEventHandler for App {
    fn handle_mouse_move(&mut self, x: i32, y: i32) -> Vec<Command> {
        self.handle_mouse_move(x, y)
    }

    fn handle_mouse_down(&mut self, x: i32, y: i32) -> Vec<Command> {
        self.handle_mouse_down(x, y)
    }

    fn handle_mouse_up(&mut self, x: i32, y: i32) -> Vec<Command> {
        self.handle_mouse_up(x, y)
    }

    fn handle_double_click(&mut self, x: i32, y: i32) -> Vec<Command> {
        self.handle_double_click(x, y)
    }
}

impl KeyboardEventHandler for App {
    fn handle_key_input(&mut self, key: u32) -> Vec<Command> {
        self.handle_key_input(key)
    }

    fn handle_text_input(&mut self, character: char) -> Vec<Command> {
        self.handle_text_input(character)
    }
}

impl SystemEventHandler for App {
    fn handle_tray_message(&mut self, wparam: u32, lparam: u32) -> Vec<Command> {
        self.handle_tray_message(wparam, lparam)
    }

    fn handle_cursor_timer(&mut self, timer_id: u32) -> Vec<Command> {
        self.handle_cursor_timer(timer_id)
    }
}

impl WindowEventHandler for App {
    fn paint(&mut self, hwnd: windows::Win32::Foundation::HWND) -> Result<(), AppError> {
        self.paint(hwnd)
    }

    fn reset_to_initial_state(&mut self) {
        self.reset_to_initial_state()
    }
}

// AppError已在error.rs中定义，这里仅导入额外的转换实现

// 错误转换实现
impl From<crate::screenshot::ScreenshotError> for AppError {
    fn from(err: crate::screenshot::ScreenshotError) -> Self {
        AppError::Screenshot(err.to_string())
    }
}

impl From<crate::drawing::DrawingError> for AppError {
    fn from(err: crate::drawing::DrawingError) -> Self {
        AppError::Drawing(err.to_string())
    }
}

impl From<crate::ui::UIError> for AppError {
    fn from(err: crate::ui::UIError) -> Self {
        AppError::UI(err.to_string())
    }
}

impl From<crate::system::SystemError> for AppError {
    fn from(err: crate::system::SystemError) -> Self {
        AppError::System(err.to_string())
    }
}

impl From<PlatformError> for AppError {
    fn from(err: PlatformError) -> Self {
        AppError::Platform(err.to_string())
    }
}

/// 固钉窗口过程
unsafe extern "system" fn pin_window_proc(
    hwnd: windows::Win32::Foundation::HWND,
    msg: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::Foundation::*;
    use windows::Win32::Graphics::Gdi::*;
    use windows::Win32::UI::Input::KeyboardAndMouse::VK_ESCAPE;
    use windows::Win32::UI::WindowsAndMessaging::*;

    unsafe {
        match msg {
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);

                // 获取存储的位图句柄
                let bitmap_handle = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut std::ffi::c_void;
                if !bitmap_handle.is_null() {
                    let bitmap = HBITMAP(bitmap_handle);

                    // 创建兼容DC并绘制位图
                    let mem_dc = CreateCompatibleDC(Some(hdc));
                    let old_bitmap = SelectObject(mem_dc, bitmap.into());

                    // 获取窗口尺寸
                    let mut rect = RECT::default();
                    let _ = GetClientRect(hwnd, &mut rect);

                    // 绘制位图到窗口
                    let _ = BitBlt(
                        hdc,
                        0,
                        0,
                        rect.right - rect.left,
                        rect.bottom - rect.top,
                        Some(mem_dc),
                        0,
                        0,
                        SRCCOPY,
                    );

                    // 清理
                    SelectObject(mem_dc, old_bitmap);
                    let _ = DeleteDC(mem_dc);
                }

                let _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }
            WM_LBUTTONDOWN => {
                // 开始拖拽窗口
                let _ = SendMessageW(
                    hwnd,
                    WM_NCLBUTTONDOWN,
                    Some(WPARAM(HTCAPTION as usize)),
                    Some(lparam),
                );
                LRESULT(0)
            }
            WM_RBUTTONUP => {
                // 右键菜单：关闭窗口
                let _ = DestroyWindow(hwnd);
                LRESULT(0)
            }
            WM_KEYDOWN => {
                // 处理键盘按键（从原始代码迁移）
                if wparam.0 == VK_ESCAPE.0 as usize {
                    // ESC键关闭固钉窗口
                    let _ = DestroyWindow(hwnd);
                }
                LRESULT(0)
            }
            WM_DESTROY => {
                // 清理位图资源
                let bitmap_handle = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut std::ffi::c_void;
                if !bitmap_handle.is_null() {
                    let bitmap = HBITMAP(bitmap_handle);
                    let _ = DeleteObject(bitmap.into());
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}
