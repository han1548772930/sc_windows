// 应用程序协调器
//
// App结构体是整个应用程序的核心协调器，负责：
// 1. 管理各个业务领域的管理器
// 2. 协调组件间的消息传递
// 3. 统一的渲染流程
// 4. 错误处理和状态管理

use crate::drawing::DrawingManager;
use crate::message::{Command, DrawingMessage, Message, ScreenshotMessage};
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

    /// 初始化Direct2D渲染器（从原始代码迁移）
    pub fn initialize_renderer(
        &mut self,
        hwnd: windows::Win32::Foundation::HWND,
        width: i32,
        height: i32,
    ) -> Result<(), AppError> {
        // 将platform转换为Direct2DRenderer并初始化
        // 这是一个临时的解决方案，因为我们知道platform是Direct2DRenderer
        // TODO: 改进架构以更好地处理这种情况

        // 暂时跳过Direct2D初始化，因为类型转换比较复杂
        // 在实际渲染时会处理
        Ok(())
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

        // 重置系统管理器状态
        self.system.reset_state();
    }

    /// 获取当前的GDI截图位图句柄（用于简单显示）
    pub fn get_gdi_screenshot_bitmap(&self) -> Option<windows::Win32::Graphics::Gdi::HBITMAP> {
        self.screenshot.get_gdi_bitmap()
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

        // 渲染绘图元素（添加边界检查）
        let selection_rect = self.screenshot.get_selection();
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

        self.drawing
            .render(
                &mut *self.platform,
                selection_rect.as_ref(),
                screen_width,
                screen_height,
            )
            .map_err(|e| AppError::RenderError(format!("Failed to render drawing: {:?}", e)))?;

        // 渲染UI覆盖层
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
                let commands = self.screenshot.handle_message(msg.clone());

                // 如果是StartCapture消息且截图成功，创建D2D位图
                if let ScreenshotMessage::StartCapture = msg {
                    if commands.contains(&Command::ShowOverlay) {
                        // 截图成功，现在创建D2D位图
                        if let Some(d2d_renderer) =
                            self.platform
                                .as_any_mut()
                                .downcast_mut::<crate::platform::windows::d2d::Direct2DRenderer>()
                        {
                            if let Err(e) = self.screenshot.create_d2d_bitmap_from_gdi(d2d_renderer)
                            {
                                eprintln!("Failed to create D2D bitmap: {:?}", e);
                            } else {
                                eprintln!("D2D bitmap created successfully");
                            }
                        }
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

    /// 获取Direct2D渲染器的引用（临时解决方案）
    #[allow(dead_code)]
    fn get_d2d_renderer(
        &self,
    ) -> Result<&crate::platform::windows::d2d::Direct2DRenderer, AppError> {
        // 这是一个临时的解决方案，因为我们知道platform是Direct2DRenderer
        // TODO: 改进架构以更好地处理这种情况

        // 由于Rust的借用检查器限制，我们暂时无法安全地转换
        // 让我们使用另一种方法
        Err(AppError::RenderError("Cannot access renderer".to_string()))
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
    pub fn reload_settings(&mut self) {
        // 重新加载设置并更新各个管理器
        self.system.reload_settings();
        // 可以在这里添加其他管理器的设置重新加载逻辑
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
        match key {
            0x1B => {
                // VK_ESCAPE - ESC键清除所有状态并隐藏窗口
                self.reset_to_initial_state();
                vec![Command::HideWindow]
            }
            0x20 => {
                // VK_SPACE - 空格键触发截图
                self.handle_message(Message::Screenshot(ScreenshotMessage::StartCapture))
            }
            0x52 => {
                // 'R' 键 - 矩形工具
                self.handle_message(Message::Drawing(DrawingMessage::SelectTool(
                    crate::types::DrawingTool::Rectangle,
                )))
            }
            0x43 => {
                // 'C' 键 - 圆形工具
                self.handle_message(Message::Drawing(DrawingMessage::SelectTool(
                    crate::types::DrawingTool::Circle,
                )))
            }
            0x41 => {
                // 'A' 键 - 箭头工具
                self.handle_message(Message::Drawing(DrawingMessage::SelectTool(
                    crate::types::DrawingTool::Arrow,
                )))
            }
            0x50 => {
                // 'P' 键 - 画笔工具
                self.handle_message(Message::Drawing(DrawingMessage::SelectTool(
                    crate::types::DrawingTool::Pen,
                )))
            }
            0x54 => {
                // 'T' 键 - 文本工具
                self.handle_message(Message::Drawing(DrawingMessage::SelectTool(
                    crate::types::DrawingTool::Text,
                )))
            }
            0x5A => {
                // 'Z' 键 - 撤销
                self.handle_message(Message::Drawing(DrawingMessage::Undo))
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

    /// 创建D2D位图（从原始代码迁移）
    pub fn create_d2d_bitmap_from_gdi(&mut self) -> Result<(), AppError> {
        if let Some(d2d_renderer) =
            self.platform
                .as_any_mut()
                .downcast_mut::<crate::platform::windows::d2d::Direct2DRenderer>()
        {
            self.screenshot
                .create_d2d_bitmap_from_gdi(d2d_renderer)
                .map_err(|e| AppError::RenderError(format!("Failed to create D2D bitmap: {:?}", e)))
        } else {
            Err(AppError::RenderError(
                "Cannot access D2D renderer".to_string(),
            ))
        }
    }

    /// 处理鼠标移动
    pub fn handle_mouse_move(&mut self, x: i32, y: i32) -> Vec<Command> {
        let mut commands = Vec::new();

        // 1) UI优先
        let ui_commands = self.ui.handle_mouse_move(x, y);
        if !ui_commands.is_empty() {
            commands.extend(ui_commands);
            return commands;
        }

        // 2) 如果绘图处于拖拽（移动/调整）中，则只让Drawing处理，避免选择框跟随移动
        let selection_rect = self.screenshot.get_selection();
        commands.extend(self.drawing.handle_mouse_move(x, y, selection_rect));

        // 3) 让Screenshot处理（用于自动高亮/选择框拖拽）
        // 注意：由于mouse_down阶段我们已避免冲突，这里可以始终让Screenshot检测高亮
        commands.extend(self.screenshot.handle_mouse_move(x, y));

        commands
    }
    /// 内部辅助：绘图管理器是否处于鼠标按下（用于拖拽元素）
    fn drawing_mouse_pressed(&self) -> bool {
        // 通过检查拖拽模式是否为元素拖拽或绘制
        // 暴露一个最小接口：当前简化为检测是否存在正在绘制的元素或内部标志
        // 这里采用保守策略：若有选中元素且App层没有更细粒度信息，则认为可能在拖拽
        // 更精确可通过给DrawingManager添加只读查询方法（后续优化）
        // 目前返回false以保持兼容，真正的冲突通过mouse_down路由修复
        false
    }

    /// 处理鼠标按下
    pub fn handle_mouse_down(&mut self, x: i32, y: i32) -> Vec<Command> {
        let mut commands = Vec::new();

        // UI层优先处理（按钮点击等）
        commands.extend(self.ui.handle_mouse_down(x, y));

        // 如果UI没有处理，则传递给其他管理器
        if commands.is_empty() {
            let selection_rect = self.screenshot.get_selection();
            // 先让Drawing尝试接管（元素手柄/移动/绘制）
            let drawing_cmds = self.drawing.handle_mouse_down(x, y, selection_rect);
            let drawing_consumed = !drawing_cmds.is_empty();
            commands.extend(drawing_cmds);

            // 若未被Drawing消费，再交给Screenshot（选择框手柄/新建选择等）
            if !drawing_consumed {
                commands.extend(self.screenshot.handle_mouse_down(x, y));
            }
        }

        commands
    }

    /// 处理鼠标释放
    pub fn handle_mouse_up(&mut self, x: i32, y: i32) -> Vec<Command> {
        let mut commands = Vec::new();

        commands.extend(self.ui.handle_mouse_up(x, y));
        // 先结束Drawing的拖拽，再让Screenshot基于点击/拖拽状态更新
        let drawing_cmds = self.drawing.handle_mouse_up(x, y);
        commands.extend(drawing_cmds.clone());
        commands.extend(self.screenshot.handle_mouse_up(x, y));

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

        // 如果绘图管理器没有处理，可能是UI元素需要处理
        if commands.is_empty() {
            commands.extend(self.ui.handle_text_input(character));
        }

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
        let current_tool = self.drawing.get_current_tool();
        self.ui.update_toolbar_selected_tool(current_tool);
    }

    /// 保存选择区域到剪贴板（严格遵循原始同步逻辑）
    pub fn save_selection_to_clipboard(
        &mut self,
        hwnd: windows::Win32::Foundation::HWND,
    ) -> Result<(), AppError> {
        use windows::Win32::Foundation::{HANDLE, HWND};
        use windows::Win32::Graphics::Gdi::{
            BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC,
            ReleaseDC, SRCCOPY, SelectObject,
        };
        use windows::Win32::System::DataExchange::{
            CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
        };

        // 临时隐藏UI元素
        self.screenshot.hide_ui_for_capture(hwnd);

        let result = unsafe {
            let Some(selection_rect) = self.screenshot.get_selection() else {
                // 恢复UI元素
                self.screenshot.show_ui_after_capture(hwnd);
                return Ok(());
            };

            let width = selection_rect.right - selection_rect.left;
            let height = selection_rect.bottom - selection_rect.top;
            if width <= 0 || height <= 0 {
                // 恢复UI元素
                self.screenshot.show_ui_after_capture(hwnd);
                return Ok(());
            }

            // 截取屏幕的完整选择区域（包含所有内容但不包含UI元素）
            let screen_dc = GetDC(Some(HWND(std::ptr::null_mut())));
            let mem_dc = CreateCompatibleDC(Some(screen_dc));
            let bitmap = CreateCompatibleBitmap(screen_dc, width, height);
            let old_bitmap = SelectObject(mem_dc, bitmap.into());

            // 从屏幕复制选择区域
            let _ = BitBlt(
                mem_dc,
                0,
                0,
                width,
                height,
                Some(screen_dc),
                selection_rect.left,
                selection_rect.top,
                SRCCOPY,
            );

            // 复制到剪贴板（CF_BITMAP = 2）
            if OpenClipboard(Some(HWND(std::ptr::null_mut()))).is_ok() {
                let _ = EmptyClipboard();
                let _ = SetClipboardData(2u32, Some(HANDLE(bitmap.0 as *mut std::ffi::c_void)));
                let _ = CloseClipboard();
            } else {
                let _ = DeleteObject(bitmap.into());
            }

            // 清理DC资源（位图句柄如已放入剪贴板，则由系统管理）
            SelectObject(mem_dc, old_bitmap);
            let _ = DeleteDC(mem_dc);
            // 释放屏幕DC
            let _ = ReleaseDC(Some(HWND(std::ptr::null_mut())), screen_dc);

            Ok(())
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
        use windows::Win32::Foundation::HWND;
        use windows::Win32::Graphics::Gdi::{
            BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC,
            ReleaseDC, SRCCOPY, SelectObject,
        };

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

        let result = unsafe {
            // 截取屏幕选择区域
            let screen_dc = GetDC(Some(HWND(std::ptr::null_mut())));
            let mem_dc = CreateCompatibleDC(Some(screen_dc));
            let bitmap = CreateCompatibleBitmap(screen_dc, width, height);
            let old_bitmap = SelectObject(mem_dc, bitmap.into());

            // 从屏幕复制选择区域
            let _ = BitBlt(
                mem_dc,
                0,
                0,
                width,
                height,
                Some(screen_dc),
                selection_rect.left,
                selection_rect.top,
                SRCCOPY,
            );

            // 保存位图到文件（使用备份代码中的实现）
            let save_res = Self::save_bitmap_to_file_internal(bitmap, &file_path, width, height);

            // 清理资源
            SelectObject(mem_dc, old_bitmap);
            let _ = DeleteDC(mem_dc);
            let _ = ReleaseDC(Some(HWND(std::ptr::null_mut())), screen_dc);
            let _ = DeleteObject(bitmap.into());

            save_res
        };

        // 恢复UI元素
        self.screenshot.show_ui_after_capture(hwnd);
        result.map(|_| true).map_err(|e| AppError::InitError(e))
    }

    /// 保存位图到文件（从原始代码迁移的内部实现）
    fn save_bitmap_to_file_internal(
        bitmap: windows::Win32::Graphics::Gdi::HBITMAP,
        file_path: &str,
        width: i32,
        height: i32,
    ) -> Result<(), String> {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::Graphics::Gdi::*;

        unsafe {
            // 获取位图信息
            let mut bitmap_info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    biHeight: -height, // 负值表示自上而下的位图
                    biPlanes: 1,
                    biBitCount: 24, // 24位RGB
                    biCompression: BI_RGB.0,
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [RGBQUAD::default(); 1],
            };

            // 计算位图数据大小
            let bytes_per_line = ((width * 3 + 3) / 4) * 4; // 4字节对齐
            let data_size = bytes_per_line * height;
            let mut bitmap_data = vec![0u8; data_size as usize];

            // 获取屏幕DC
            let screen_dc = GetDC(Some(HWND(std::ptr::null_mut())));

            // 获取位图数据
            let _ = GetDIBits(
                screen_dc,
                bitmap,
                0,
                height as u32,
                Some(bitmap_data.as_mut_ptr() as *mut _),
                &mut bitmap_info,
                DIB_RGB_COLORS,
            );

            // 创建BMP文件头
            let file_header = BITMAPFILEHEADER {
                bfType: 0x4D42, // "BM"
                bfSize: (std::mem::size_of::<BITMAPFILEHEADER>()
                    + std::mem::size_of::<BITMAPINFOHEADER>()
                    + data_size as usize) as u32,
                bfReserved1: 0,
                bfReserved2: 0,
                bfOffBits: (std::mem::size_of::<BITMAPFILEHEADER>()
                    + std::mem::size_of::<BITMAPINFOHEADER>()) as u32,
            };

            // 写入文件
            use std::fs::File;
            use std::io::Write;

            let mut file = File::create(file_path).map_err(|e| e.to_string())?;

            // 写入文件头
            let file_header_bytes = std::slice::from_raw_parts(
                &file_header as *const _ as *const u8,
                std::mem::size_of::<BITMAPFILEHEADER>(),
            );
            file.write_all(file_header_bytes)
                .map_err(|e| e.to_string())?;

            // 写入信息头
            let info_header_bytes = std::slice::from_raw_parts(
                &bitmap_info.bmiHeader as *const _ as *const u8,
                std::mem::size_of::<BITMAPINFOHEADER>(),
            );
            file.write_all(info_header_bytes)
                .map_err(|e| e.to_string())?;

            // 写入位图数据
            file.write_all(&bitmap_data).map_err(|e| e.to_string())?;

            // 清理资源
            let _ = ReleaseDC(Some(HWND(std::ptr::null_mut())), screen_dc);
        }

        Ok(())
    }

    /// 从选择区域提取文本（从原始代码迁移）
    pub fn extract_text_from_selection(
        &mut self,
        hwnd: windows::Win32::Foundation::HWND,
    ) -> Result<(), AppError> {
        // 检查是否有选择区域
        let Some(selection_rect) = self.screenshot.get_selection() else {
            return Ok(());
        };

        // 检查OCR引擎是否可用（从原始代码迁移）
        if !self.is_ocr_engine_available() {
            // 如果OCR引擎不可用，显示错误消息
            use windows::Win32::UI::WindowsAndMessaging::*;
            let message = "OCR引擎不可用。\n\n请确保PaddleOCR引擎正常运行。";
            let message_w: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();
            let title_w: Vec<u16> = "OCR错误".encode_utf16().chain(std::iter::once(0)).collect();

            unsafe {
                MessageBoxW(
                    Some(hwnd),
                    windows::core::PCWSTR(message_w.as_ptr()),
                    windows::core::PCWSTR(title_w.as_ptr()),
                    MB_OK | MB_ICONERROR,
                );
            }
            return Ok(());
        }

        // 临时隐藏UI元素
        self.screenshot.hide_ui_for_capture(hwnd);

        let result = unsafe {
            // 直接使用原始OCR模块的extract_text_from_selection函数
            // 这个函数会自动处理图像数据创建和OCR结果窗口显示
            match crate::ocr::extract_text_from_selection(
                self.screenshot.get_screenshot_dc(),
                selection_rect,
                Some(hwnd),
            ) {
                Ok(ocr_results) => {
                    // 原始函数已经处理了OCR结果窗口显示
                    // 这里只需要处理空结果的情况
                    if ocr_results.is_empty() {
                        // 显示"未识别到文本"消息
                        use windows::Win32::UI::WindowsAndMessaging::*;
                        let message = "未识别到文本内容。\n\n请确保选择区域包含清晰的文字。";
                        let message_w: Vec<u16> =
                            message.encode_utf16().chain(std::iter::once(0)).collect();
                        let title_w: Vec<u16> =
                            "OCR结果".encode_utf16().chain(std::iter::once(0)).collect();

                        MessageBoxW(
                            Some(hwnd),
                            windows::core::PCWSTR(message_w.as_ptr()),
                            windows::core::PCWSTR(title_w.as_ptr()),
                            MB_OK | MB_ICONINFORMATION,
                        );
                    }
                    Ok(())
                }
                Err(e) => {
                    // 显示错误消息
                    use windows::Win32::UI::WindowsAndMessaging::*;
                    let message = format!("OCR识别失败：{}\n\n请确保PaddleOCR引擎正常运行。", e);
                    let message_w: Vec<u16> =
                        message.encode_utf16().chain(std::iter::once(0)).collect();
                    let title_w: Vec<u16> =
                        "OCR错误".encode_utf16().chain(std::iter::once(0)).collect();

                    MessageBoxW(
                        Some(hwnd),
                        windows::core::PCWSTR(message_w.as_ptr()),
                        windows::core::PCWSTR(title_w.as_ptr()),
                        MB_OK | MB_ICONERROR,
                    );
                    Err(AppError::InitError(format!("OCR failed: {}", e)))
                }
            }
        };

        // 恢复UI元素
        self.screenshot.show_ui_after_capture(hwnd);
        result
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
            unsafe {
                use windows::Win32::UI::WindowsAndMessaging::{SW_HIDE, ShowWindow};
                let _ = ShowWindow(hwnd, SW_HIDE);
            }

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
            // 获取选择区域的屏幕截图（包含绘图内容）
            let screen_dc = GetDC(Some(HWND(std::ptr::null_mut())));
            let mem_dc = CreateCompatibleDC(Some(screen_dc));
            let bitmap = CreateCompatibleBitmap(screen_dc, width, height);
            let old_bitmap = SelectObject(mem_dc, bitmap.into());

            // 直接从屏幕复制选择区域（包含窗口内容和绘图）
            let _ = BitBlt(
                mem_dc,
                0,
                0,
                width,
                height,
                Some(screen_dc),
                selection_rect.left,
                selection_rect.top,
                SRCCOPY,
            );

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

            // 清理临时DC资源，但保留位图
            SelectObject(mem_dc, old_bitmap);
            let _ = DeleteDC(mem_dc);
            ReleaseDC(Some(HWND(std::ptr::null_mut())), screen_dc);

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

    /// 检查OCR引擎是否可用（从原始代码迁移）
    pub fn is_ocr_engine_available(&self) -> bool {
        // 使用原始代码的OCR引擎状态检查
        crate::ocr::PaddleOcrEngine::is_engine_available()
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
