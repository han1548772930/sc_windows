// 应用程序协调器
//
// App结构体是整个应用程序的核心协调器，负责：
// 1. 管理各个业务领域的管理器
// 2. 协调组件间的消息传递
// 3. 统一的渲染流程
// 4. 错误处理和状态管理

use crate::drawing::DrawingManager;
use crate::message::{Command, Message, ScreenshotMessage};
use crate::platform::{PlatformError, PlatformRenderer};
use crate::screenshot::ScreenshotManager;
use crate::settings::SettingsWindow;
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
    platform: Box<dyn PlatformRenderer<Error = PlatformError>>,
}

impl App {
    /// 创建新的应用程序实例
    pub fn new(
        platform: Box<dyn PlatformRenderer<Error = PlatformError>>,
    ) -> Result<Self, AppError> {
        // 创建截图管理器，但不立即捕获屏幕（按照原始代码逻辑）
        // 原始代码中，WindowState::new只是初始化状态，截图在热键触发时才进行
        let screenshot = ScreenshotManager::new()?;

        Ok(Self {
            screenshot,
            drawing: DrawingManager::new()?,
            ui: UIManager::new()?,
            system: SystemManager::new()?,
            platform,
        })
    }

    /// 设置当前窗口句柄（用于排除自己的窗口）
    pub fn set_current_window(&mut self, hwnd: windows::Win32::Foundation::HWND) {
        self.screenshot.set_current_window(hwnd);
    }

    /// 重置到初始状态（从原始reset_to_initial_state迁移）
    pub fn reset_to_initial_state(&mut self) {
        // 重置截图管理器状态
        self.screenshot.reset_state();

        // 重置绘图管理器状态
        self.drawing.reset_state();

        // 重置UI管理器状态
        self.ui.reset_state();
    }

    /// 获取当前的GDI截图位图句柄（用于简单显示）
    /// 注意：调用方负责释放返回的HBITMAP
    pub fn get_gdi_screenshot_bitmap(&self) -> Option<windows::Win32::Graphics::Gdi::HBITMAP> {
        // 按需捕获屏幕并返回GDI位图
        self.screenshot.capture_screen_to_gdi_bitmap().ok()
    }

    /// 绘制窗口内容（从原始代码迁移）
    pub fn paint(&mut self, hwnd: windows::Win32::Foundation::HWND) -> Result<(), AppError> {
        use windows::Win32::Graphics::Gdi::{BeginPaint, EndPaint, PAINTSTRUCT};

        unsafe {
            let mut ps = PAINTSTRUCT::default();
            BeginPaint(hwnd, &mut ps);

            // 执行渲染
            if let Err(e) = self.render() {
                eprintln!("Render error: {}", e);
            }

            let _ = EndPaint(hwnd, &ps);
        }

        Ok(())
    }

    /// 渲染所有组件（从原始代码迁移）
    pub fn render(&mut self) -> Result<(), AppError> {
        // 开始渲染帧
        self.platform
            .begin_frame()
            .map_err(|e| AppError::RenderError(format!("Failed to begin frame: {:?}", e)))?;

        // 清除背景（透明）
        self.platform
            .clear(crate::platform::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            })
            .map_err(|e| AppError::RenderError(format!("Failed to clear: {:?}", e)))?;

        // 渲染截图内容
        self.screenshot
            .render(&mut *self.platform)
            .map_err(|e| AppError::RenderError(format!("Failed to render screenshot: {:?}", e)))?;

        // 渲染绘图元素
        let selection_rect = self.screenshot.get_selection();

        self.drawing
            .render(&mut *self.platform, selection_rect.as_ref())
            .map_err(|e| AppError::RenderError(format!("Failed to render drawing: {:?}", e)))?;

        // 渲染选区相关的UI元素（遮罩、边框、手柄）- 在工具栏之前渲染
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
            .map_err(|e| {
                AppError::RenderError(format!("Failed to render selection UI: {:?}", e))
            })?;

        // 渲染UI覆盖层（工具栏、对话框等）- 在遮罩之后渲染，确保工具栏在最上层
        self.ui
            .render(&mut *self.platform)
            .map_err(|e| AppError::RenderError(format!("Failed to render UI: {:?}", e)))?;

        // 结束渲染帧
        self.platform
            .end_frame()
            .map_err(|e| AppError::RenderError(format!("Failed to end frame: {:?}", e)))?;

        Ok(())
    }

    /// 处理消息并返回需要执行的命令
    pub fn handle_message(&mut self, message: Message) -> Vec<Command> {
        match message {
            Message::Screenshot(msg) => {
                // 处理截图消息，并在截图成功后创建D2D位图
                let mut commands = self.screenshot.handle_message(msg.clone());

                // 如果是StartCapture消息且截图成功，创建D2D位图并重置绘图与手柄显示
                if let ScreenshotMessage::StartCapture = msg {
                    if commands.contains(&Command::ShowOverlay) {
                        // 截图成功，现在创建D2D位图（使用平台无关接口）
                        if let Err(e) = self
                            .screenshot
                            .create_d2d_bitmap_from_gdi(&mut *self.platform)
                        {
                            eprintln!("Failed to create D2D bitmap: {:?}", e);
                        } else {
                            eprintln!("D2D bitmap created successfully");
                        }

                        // 新一轮截图：重置绘图状态为None，确保显示选择框手柄
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
                // 获取屏幕尺寸（从原始代码迁移）
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

    /// 初始化系统托盘（从原始代码迁移）
    pub fn init_system_tray(
        &mut self,
        hwnd: windows::Win32::Foundation::HWND,
    ) -> Result<(), AppError> {
        // 通过系统管理器初始化托盘
        self.system
            .initialize(hwnd)
            .map_err(|e| AppError::InitError(format!("Failed to initialize system tray: {}", e)))
    }

    /// 启动异步OCR引擎状态检查（从原始代码迁移）
    pub fn start_async_ocr_check(&mut self, hwnd: windows::Win32::Foundation::HWND) {
        // 通过系统管理器启动OCR引擎状态检查
        self.system.start_async_ocr_check(hwnd);
    }

    /// 处理光标定时器（从原始代码迁移）
    pub fn handle_cursor_timer(&mut self, timer_id: u32) -> Vec<Command> {
        // 通过绘图管理器处理光标定时器（主要用于文本输入光标闪烁）
        self.drawing.handle_cursor_timer(timer_id)
    }

    /// 异步停止OCR引擎（从原始代码迁移）
    pub fn stop_ocr_engine_async(&mut self) {
        self.system.stop_ocr_engine_async();
    }

    /// 重新加载设置（从原始代码迁移）
    pub fn reload_settings(&mut self) -> Vec<Command> {
        // 重新加载设置并更新各个管理器
        self.system.reload_settings();

        // 重新加载绘图属性（与旧代码保持一致）
        self.drawing.reload_drawing_properties();

        // 返回需要执行的命令
        vec![Command::UpdateToolbar, Command::RequestRedraw]
    }

    /// 重新注册热键（从原始代码迁移）
    pub fn reregister_hotkey(
        &mut self,
        hwnd: windows::Win32::Foundation::HWND,
    ) -> windows::core::Result<()> {
        self.system.reregister_hotkey(hwnd)
    }

    /// 更新OCR引擎状态（从原始代码迁移）
    pub fn update_ocr_engine_status(
        &mut self,
        available: bool,
        hwnd: windows::Win32::Foundation::HWND,
    ) {
        self.system.update_ocr_engine_status(available, hwnd);
    }

    /// 处理键盘输入（从原始代码迁移，统一处理所有按键）
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

    /// 选择绘图工具（从原始代码迁移）
    pub fn select_drawing_tool(
        &mut self,
        tool: crate::types::DrawingTool,
    ) -> Vec<crate::message::Command> {
        // 通过绘图管理器选择工具
        let message = crate::message::DrawingMessage::SelectTool(tool);
        let commands = self.drawing.handle_message(message);

        // 返回绘图管理器产生的命令
        commands
    }

    /// 创建D2D位图（从原始代码迁移，使用平台无关接口）
    pub fn create_d2d_bitmap_from_gdi(&mut self) -> Result<(), AppError> {
        self.screenshot
            .create_d2d_bitmap_from_gdi(&mut *self.platform)
            .map_err(|e| AppError::RenderError(format!("Failed to create D2D bitmap: {:?}", e)))
    }

    /// 执行单个命令（从main.rs的handle_commands迁移）
    /// 这个方法负责处理所有类型的命令，并返回可能产生的新命令
    pub fn execute_command(
        &mut self,
        command: Command,
        hwnd: windows::Win32::Foundation::HWND,
    ) -> Vec<Command> {
        use crate::message::{Command, DrawingMessage};
        use windows::Win32::Foundation::*;
        use windows::Win32::Graphics::Gdi::InvalidateRect;
        use windows::Win32::UI::WindowsAndMessaging::*;

        match command {
            Command::RequestRedraw => {
                unsafe {
                    let _ = InvalidateRect(Some(hwnd), None, FALSE.into());
                }
                vec![]
            }
            Command::UI(ui_message) => {
                // 处理UI消息
                self.handle_message(Message::UI(ui_message))
            }
            Command::Drawing(drawing_message) => {
                // 检查是否应该禁用某些绘图命令（从原始代码迁移）
                let should_execute = match &drawing_message {
                    DrawingMessage::Undo => self.can_undo(),
                    _ => true,
                };

                if should_execute {
                    // 处理绘图消息
                    self.handle_message(Message::Drawing(drawing_message))
                } else {
                    vec![]
                }
            }
            Command::SelectDrawingTool(tool) => {
                // 处理绘图工具选择
                let mut commands = self.select_drawing_tool(tool);
                commands.push(Command::RequestRedraw);
                commands
            }
            Command::ShowOverlay => {
                // 显示覆盖层（截图成功）
                // 覆盖层显示时再次异步预热OCR引擎，避免后续再次使用时冷启动卡顿
                crate::ocr::PaddleOcrEngine::start_ocr_engine_async();
                self.start_async_ocr_check(hwnd);
                vec![]
            }
            Command::HideOverlay => {
                // 隐藏覆盖层（取消或确认）
                unsafe {
                    let _ = ShowWindow(hwnd, SW_HIDE);
                }
                vec![]
            }
            Command::HideWindow => {
                // 隐藏窗口（从原始代码迁移）
                unsafe {
                    let _ = ShowWindow(hwnd, SW_HIDE);
                }
                vec![]
            }
            Command::SaveSelectionToFile => {
                // 保存选择区域到文件（从原始代码迁移）
                match self.save_selection_to_file(hwnd) {
                    Ok(true) => {
                        // 保存成功，隐藏窗口并重置状态
                        unsafe {
                            let _ = ShowWindow(hwnd, SW_HIDE);
                        }
                        self.reset_to_initial_state();
                        vec![]
                    }
                    Ok(false) => {
                        // 用户取消，不做任何操作
                        vec![]
                    }
                    Err(e) => {
                        eprintln!("Failed to save selection to file: {}", e);
                        vec![Command::ShowError(format!("保存失败: {}", e))]
                    }
                }
            }
            Command::SaveSelectionToClipboard => {
                // 复制选择区域到剪贴板（从原始代码迁移）
                match self.save_selection_to_clipboard(hwnd) {
                    Ok(_) => {
                        unsafe {
                            let _ = ShowWindow(hwnd, SW_HIDE);
                        }
                        self.reset_to_initial_state();
                        vec![]
                    }
                    Err(e) => {
                        eprintln!("Failed to copy selection to clipboard: {}", e);
                        vec![Command::ShowError(format!("复制失败: {}", e))]
                    }
                }
            }
            Command::UpdateToolbar => {
                // 更新工具栏状态
                self.update_toolbar_state();
                vec![]
            }
            Command::ShowSettings => {
                // 显示设置窗口（使用传统的 Win32 实现）
                let _ = SettingsWindow::show(windows::Win32::Foundation::HWND::default());
                vec![]
            }
            Command::TakeScreenshot => {
                // 执行截图（从原始代码迁移）
                match self.take_screenshot(hwnd) {
                    Ok(_) => vec![],
                    Err(e) => {
                        eprintln!("Failed to take screenshot: {}", e);
                        vec![Command::ShowError(format!("截图失败: {}", e))]
                    }
                }
            }
            Command::ExtractText => {
                // 提取文本（从原始代码迁移）
                match self.extract_text_from_selection(hwnd) {
                    Ok(_) => {
                        // OCR完成后隐藏截屏窗口（从原始代码迁移）
                        unsafe {
                            let _ = ShowWindow(hwnd, SW_HIDE);
                        }
                        self.reset_to_initial_state();
                        vec![]
                    }
                    Err(e) => {
                        eprintln!("Failed to extract text: {}", e);
                        vec![Command::ShowError(format!("文本提取失败: {}", e))]
                    }
                }
            }
            Command::PinSelection => {
                // 固定选择区域（从原始代码迁移）
                match self.pin_selection(hwnd) {
                    Ok(_) => vec![],
                    Err(e) => {
                        eprintln!("Failed to pin selection: {}", e);
                        vec![Command::ShowError(format!("固定失败: {}", e))]
                    }
                }
            }
            Command::ResetToInitialState => {
                // 重置到初始状态（从原始代码迁移）
                self.reset_to_initial_state();
                vec![]
            }
            Command::CopyToClipboard => {
                // 复制到剪贴板（重定向到 SaveSelectionToClipboard）
                self.execute_command(Command::SaveSelectionToClipboard, hwnd)
            }
            Command::ShowSaveDialog => {
                // 显示保存对话框（从原始代码迁移）
                match self.save_selection_to_file(hwnd) {
                    Ok(true) => {
                        unsafe {
                            let _ = ShowWindow(hwnd, SW_HIDE);
                        }
                        self.reset_to_initial_state();
                        vec![]
                    }
                    Ok(false) => {
                        // 用户取消，不做任何操作
                        vec![]
                    }
                    Err(e) => {
                        eprintln!("Failed to show save dialog: {}", e);
                        vec![Command::ShowError(format!("保存失败: {}", e))]
                    }
                }
            }
            Command::StartTimer(timer_id, interval_ms) => {
                // 启动定时器（从原始代码迁移，用于文本编辑光标闪烁）
                unsafe {
                    let _ = SetTimer(Some(hwnd), timer_id as usize, interval_ms, None);
                }
                vec![]
            }
            Command::StopTimer(timer_id) => {
                // 停止定时器（从原始代码迁移）
                unsafe {
                    let _ = KillTimer(Some(hwnd), timer_id as usize);
                }
                vec![]
            }
            Command::ReloadSettings => {
                // 重新加载设置（从原始代码迁移）
                let commands = self.reload_settings();
                commands
            }
            Command::ShowError(msg) => {
                // 显示错误消息
                eprintln!("Error: {}", msg);
                vec![]
            }
            Command::Quit => {
                // 退出应用
                unsafe {
                    PostQuitMessage(0);
                }
                vec![]
            }
            Command::None => {
                // 无操作，忽略
                vec![]
            }
        }
    }

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
                    if let Some(el) = self.drawing.get_element_ref(edit_idx) {
                        Some((el.clone(), edit_idx))
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            };

            // 获取选中元素信息
            let selected_element_info =
                if let Some(sel_idx) = self.drawing.get_selected_element_index() {
                    if let Some(el) = self.drawing.get_element_ref(sel_idx) {
                        Some((el.clone(), sel_idx))
                    } else {
                        None
                    }
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

    /// 处理双击事件（从原始代码迁移）
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

    /// 处理文本输入（从原始代码迁移）
    pub fn handle_text_input(&mut self, character: char) -> Vec<Command> {
        let mut commands = Vec::new();

        // 文本输入主要由绘图管理器处理（文本工具）
        commands.extend(self.drawing.handle_text_input(character));

        commands
    }

    /// 处理托盘消息（从原始代码迁移）
    pub fn handle_tray_message(&mut self, wparam: u32, lparam: u32) -> Vec<Command> {
        // 通过系统管理器处理托盘消息
        self.system.handle_tray_message(wparam, lparam)
    }

    /// 执行截图（从原始代码迁移）
    pub fn take_screenshot(
        &mut self,
        hwnd: windows::Win32::Foundation::HWND,
    ) -> Result<(), AppError> {
        // 重置状态并开始截图（按照原始代码逻辑）
        self.screenshot.reset_state();

        // 设置当前窗口并捕获屏幕
        self.screenshot.set_current_window(hwnd);
        self.screenshot.capture_screen()?;

        // 显示窗口进入选择模式
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::ShowWindow(
                hwnd,
                windows::Win32::UI::WindowsAndMessaging::SW_SHOW,
            );
            let _ = windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow(hwnd);
        }

        Ok(())
    }

    /// 直接捕获屏幕（用于热键处理，与原始代码一致）
    pub fn capture_screen_direct(&mut self) -> Result<(), AppError> {
        self.screenshot
            .capture_screen()
            .map_err(|e| AppError::ScreenshotError(e))
    }

    /// 更新工具栏状态以反映当前选中的绘图工具（从原始代码迁移）
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
    ) -> Result<(), AppError> {
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
                    return Err(AppError::InitError(format!(
                        "Failed to capture region: {:?}",
                        e
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
                .map_err(|e| AppError::InitError(format!("Failed to copy to clipboard: {:?}", e)))
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
                    return Err(AppError::InitError(format!(
                        "Failed to capture region: {:?}",
                        e
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

            save_result.map_err(|e| format!("Failed to save file: {:?}", e))
        };

        // 恢复UI元素
        self.screenshot.show_ui_after_capture(hwnd);
        result.map(|_| true).map_err(|e| AppError::InitError(e))
    }

    /// 从选择区域提取文本（简化版本 - 委托给OcrManager）
    pub fn extract_text_from_selection(
        &mut self,
        hwnd: windows::Win32::Foundation::HWND,
    ) -> Result<(), AppError> {
        // 检查是否有选择区域
        let Some(selection_rect) = self.screenshot.get_selection() else {
            return Ok(());
        };

        // 委托给SystemManager处理整个OCR流程
        self.system
            .recognize_text_from_selection(selection_rect, hwnd, &mut self.screenshot)
            .map_err(|e| AppError::InitError(format!("OCR识别失败: {}", e)))
    }

    /// 固定选择区域（从原始代码迁移）
    pub fn pin_selection(
        &mut self,
        hwnd: windows::Win32::Foundation::HWND,
    ) -> Result<(), AppError> {
        // 检查是否有选择区域
        let Some(selection_rect) = self.screenshot.get_selection() else {
            return Ok(());
        };

        let width = selection_rect.right - selection_rect.left;
        let height = selection_rect.bottom - selection_rect.top;

        if width <= 0 || height <= 0 {
            return Ok(());
        }

        unsafe {
            // 临时隐藏UI元素进行截图
            self.screenshot.hide_ui_for_capture(hwnd);

            // 创建固钉窗口
            let result = self.create_pin_window(hwnd, width, height, selection_rect);

            // 恢复UI元素显示（虽然窗口即将隐藏，但保持一致性）
            self.screenshot.show_ui_after_capture(hwnd);

            // 隐藏原始截屏窗口
            use windows::Win32::UI::WindowsAndMessaging::{SW_HIDE, ShowWindow};
            let _ = ShowWindow(hwnd, SW_HIDE);

            // 重置原始窗口状态，准备下次截屏
            self.reset_to_initial_state();

            result
        }
    }

    /// 创建固钉窗口（从原始代码迁移）
    fn create_pin_window(
        &self,
        _parent_hwnd: windows::Win32::Foundation::HWND,
        width: i32,
        height: i32,
        selection_rect: windows::Win32::Foundation::RECT,
    ) -> Result<(), AppError> {
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
                    return Err(AppError::InitError(format!(
                        "Failed to capture pin image: {:?}",
                        e
                    )));
                }
            };

            // 注册固钉窗口类
            let hinstance = GetModuleHandleW(None).map_err(|e| {
                AppError::InitError(format!("Failed to get module handle: {:?}", e))
            })?;

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
            .map_err(|e| AppError::InitError(format!("Failed to create pin window: {:?}", e)))?;

            if pin_hwnd.0.is_null() {
                return Err(AppError::InitError("Pin window handle is null".to_string()));
            }

            // 将位图句柄存储到窗口数据中
            SetWindowLongPtrW(pin_hwnd, GWLP_USERDATA, bitmap.0 as isize);

            // 显示固钉窗口
            let _ = ShowWindow(pin_hwnd, SW_SHOW);
            let _ = UpdateWindow(pin_hwnd);

            Ok(())
        }
    }

    /// 检查是否可以撤销（从原始代码迁移）
    pub fn can_undo(&self) -> bool {
        self.drawing.can_undo()
    }

    /// 检查OCR引擎是否可用（非阻塞，基于缓存状态）
    pub fn is_ocr_engine_available(&self) -> bool {
        // 使用 SystemManager 中缓存的引擎可用状态，避免阻塞 UI
        self.system.ocr_is_available()
    }

    /// 检查工具栏按钮是否应该被禁用（从原始代码迁移）
    pub fn is_toolbar_button_disabled(&self, button: crate::types::ToolbarButton) -> bool {
        use crate::types::ToolbarButton;
        match button {
            ToolbarButton::Undo => !self.can_undo(),
            ToolbarButton::ExtractText => !self.is_ocr_engine_available(),
            _ => false,
        }
    }
}

/// 应用程序错误类型
#[derive(Debug)]
pub enum AppError {
    /// 渲染错误
    RenderError(String),
    /// 初始化错误
    InitError(String),
    /// 平台错误
    PlatformError(String),
    /// 截图错误
    ScreenshotError(crate::screenshot::ScreenshotError),
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::RenderError(msg) => write!(f, "Render error: {}", msg),
            AppError::InitError(msg) => write!(f, "Init error: {}", msg),
            AppError::PlatformError(msg) => write!(f, "Platform error: {}", msg),
            AppError::ScreenshotError(err) => write!(f, "Screenshot error: {}", err),
        }
    }
}

impl std::error::Error for AppError {}

// 错误转换实现
impl From<crate::screenshot::ScreenshotError> for AppError {
    fn from(err: crate::screenshot::ScreenshotError) -> Self {
        AppError::InitError(err.to_string())
    }
}

impl From<crate::drawing::DrawingError> for AppError {
    fn from(err: crate::drawing::DrawingError) -> Self {
        AppError::InitError(err.to_string())
    }
}

impl From<crate::ui::UIError> for AppError {
    fn from(err: crate::ui::UIError) -> Self {
        AppError::InitError(err.to_string())
    }
}

impl From<crate::system::SystemError> for AppError {
    fn from(err: crate::system::SystemError) -> Self {
        AppError::InitError(err.to_string())
    }
}

impl From<PlatformError> for AppError {
    fn from(err: PlatformError) -> Self {
        AppError::RenderError(err.to_string())
    }
}

/// 固钉窗口过程（从原始代码迁移）
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
