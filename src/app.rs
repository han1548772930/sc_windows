use std::collections::HashSet;

use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Gdi::{BeginPaint, EndPaint, PAINTSTRUCT};
use windows::Win32::UI::Input::KeyboardAndMouse::VK_ESCAPE;

use crate::command_executor::CommandExecutor;
use crate::constants::*;
use crate::drawing::{DrawingError, DrawingManager, DrawingTool};
use crate::error::{AppError, AppResult};
use crate::message::{Command, DrawingMessage, Message, ScreenshotMessage};
use crate::ocr::OcrResult;
use crate::platform::windows::d2d::Direct2DRenderer;
use crate::platform::{Color, EventConverter, InputEvent, MouseButton, PlatformError, Rectangle};
use crate::rendering::{DirtyRectTracker, DirtyType};
use crate::screenshot::{ScreenshotError, ScreenshotManager, save::copy_bmp_data_to_clipboard};
use crate::settings::ConfigManager;
use crate::state::{AppState, AppStateHandler, ProcessingOperation, StateContext, StateTransition, create_state};
use crate::system::{SystemError, SystemManager};
use crate::ui::{cursor::CursorManager, file_dialog, PreviewWindow, ToolbarButton, UIError, UIManager};
use crate::platform::windows::system::get_screen_size;
use crate::utils::win_api;

/// 应用程序主结构体
pub struct App {
    /// 应用状态机（保留用于状态查询）
    state: AppState,
    /// 当前状态处理器
    current_state: Box<dyn AppStateHandler>,
    /// 配置管理器
    config: ConfigManager,
    /// 截图管理器
    screenshot: ScreenshotManager,
    /// 绘图管理器
    drawing: DrawingManager,
    /// UI管理器
    ui: UIManager,
    /// 系统管理器
    system: SystemManager,
    /// Direct2D 渲染器
    platform: Direct2DRenderer,
    /// 缓存的屏幕尺寸 (宽度, 高度)
    screen_size: (i32, i32),
    /// 脏矩形追踪器（用于渲染优化）
    dirty_tracker: DirtyRectTracker,
}

impl App {
    pub fn new(platform: Direct2DRenderer) -> AppResult<Self> {
        // 初始化配置管理器（仅加载一次配置文件）
        let config = ConfigManager::new();

        // 获取共享的配置引用，供子模块使用
        let shared_settings = config.get_shared();

        let screenshot = ScreenshotManager::new()?;

        // 缓存屏幕尺寸，避免重复调用系统API
        let screen_size = get_screen_size();

        let state = AppState::Idle;
        let current_state = create_state(&state);

        // 初始化脏矩形追踪器
        let dirty_tracker = DirtyRectTracker::new(screen_size.0 as f32, screen_size.1 as f32);

        Ok(Self {
            state,
            current_state,
            config,
            screenshot,
            drawing: DrawingManager::new(std::sync::Arc::clone(&shared_settings))?,
            ui: UIManager::new()?,
            system: SystemManager::new(shared_settings)?,
            platform,
            screen_size,
            dirty_tracker,
        })
    }

    /// 获取缓存的屏幕尺寸
    pub fn get_screen_size(&self) -> (i32, i32) {
        self.screen_size
    }

    /// 重置到初始状态
    pub fn reset_to_initial_state(&mut self) {
        self.apply_transition(StateTransition::ToIdle);
    }

    // ========== 状态机方法 ==========

    /// 获取当前应用状态
    pub fn get_state(&self) -> &AppState {
        &self.state
    }

    /// 应用状态转换
    fn apply_transition(&mut self, transition: StateTransition) {
        let new_state = match transition {
            StateTransition::None => return,
            StateTransition::ToIdle => AppState::Idle,
            StateTransition::ToSelecting => AppState::Selecting,
            StateTransition::ToEditing { selection, tool } => AppState::Editing { selection, tool },
            StateTransition::ToProcessing { operation } => AppState::Processing { operation },
        };
        self.transition_to(new_state);
    }

    /// 状态转换
    ///
    /// 将应用转换到新状态，并执行相应的状态进入/退出逻辑。
    pub fn transition_to(&mut self, new_state: AppState) {
        // 状态转换时标记全屏重绘
        self.dirty_tracker.mark_full_redraw();

        // 可选: 添加调试输出
        #[cfg(debug_assertions)]
        eprintln!("State transition: {:?} -> {:?}", self.state, new_state);

        // 调用旧状态的 on_exit
        {
            let mut ctx = StateContext {
                screenshot: &mut self.screenshot,
                drawing: &mut self.drawing,
                ui: &mut self.ui,
                system: &mut self.system,
            };
            self.current_state.on_exit(&mut ctx);
        }

        // 更新状态
        self.state = new_state.clone();
        self.current_state = create_state(&new_state);

        // 调用新状态的 on_enter
        {
            let mut ctx = StateContext {
                screenshot: &mut self.screenshot,
                drawing: &mut self.drawing,
                ui: &mut self.ui,
                system: &mut self.system,
            };
            self.current_state.on_enter(&mut ctx);
        }
    }

    /// 进入编辑状态（便捷方法）
    pub fn enter_editing_state(&mut self, selection: RECT) {
        self.transition_to(AppState::Editing {
            selection,
            tool: DrawingTool::None,
        });
    }

    /// 更新编辑状态中的工具
    pub fn update_editing_tool(&mut self, tool: DrawingTool) {
        if let AppState::Editing { selection, .. } = self.state {
            self.state = AppState::Editing { selection, tool };
        }
    }

    /// 进入处理中状态（便捷方法）
    pub fn enter_processing_state(&mut self, operation: ProcessingOperation) {
        self.transition_to(AppState::Processing { operation });
    }

    /// 检查是否可以进行绘图操作
    pub fn can_draw(&self) -> bool {
        matches!(self.state, AppState::Editing { .. })
    }

    /// 检查是否处于空闲状态
    pub fn is_idle(&self) -> bool {
        self.state.is_idle()
    }

    /// 检查是否有有效的选区图像
    pub fn has_valid_selection(&self) -> bool {
        self.screenshot.get_selection_image().is_some()
    }

    /// 标记局部脏区域（用于光标闪烁等局部更新场景）
    pub fn mark_dirty_rect(&mut self, rect: &RECT) {
        self.dirty_tracker.mark_dirty(Rectangle {
            x: rect.left as f32,
            y: rect.top as f32,
            width: (rect.right - rect.left) as f32,
            height: (rect.bottom - rect.top) as f32,
        });
    }

    /// 标记全屏重绘
    pub fn mark_full_redraw(&mut self) {
        self.dirty_tracker.mark_full_redraw();
    }

    /// 绘制窗口内容
    pub fn paint(&mut self, hwnd: windows::Win32::Foundation::HWND) -> AppResult<()> {
        unsafe {
            let mut ps = PAINTSTRUCT::default();
            BeginPaint(hwnd, &mut ps);

            let _ = self.render();

            let _ = EndPaint(hwnd, &ps);
        }

        Ok(())
    }

    /// 渲染所有组件
    ///
    /// 使用 DirtyRectTracker 追踪脏区域，为后续局部渲染优化做准备。
    pub fn render(&mut self) -> AppResult<()> {
        // 检查脏区域类型（当前用于调试/日志，后续可用于优化）
        let dirty_type = self.dirty_tracker.dirty_type();
        
        #[cfg(debug_assertions)]
        if dirty_type == DirtyType::Partial {
            if let Some(rect) = self.dirty_tracker.get_combined_dirty_rect() {
                eprintln!(
                    "Partial redraw: ({}, {}) {}x{}",
                    rect.x, rect.y, rect.width, rect.height
                );
            }
        }

        self.platform
            .begin_frame()
            .map_err(|e| AppError::Render(format!("Failed to begin frame: {e:?}")))?;

        self.platform
            .clear(Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            })
            .map_err(|e| AppError::Render(format!("Failed to clear: {e:?}")))?;

        self.screenshot
            .render(&mut self.platform)
            .map_err(|e| AppError::Render(format!("Failed to render screenshot: {e:?}")))?;

        let selection_rect = self.screenshot.get_selection();

        self.drawing
            .render(&mut self.platform, selection_rect.as_ref())
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
                &mut self.platform,
                screen_size,
                selection_rect.as_ref(),
                show_handles,
                hide_ui_for_capture,
                has_auto_highlight,
            )
            .map_err(|e| AppError::Render(format!("Failed to render selection UI: {e:?}")))?;

        self.ui
            .render(&mut self.platform)
            .map_err(|e| AppError::Render(format!("Failed to render UI: {e:?}")))?;

        self.platform
            .end_frame()
            .map_err(|e| AppError::Render(format!("Failed to end frame: {e:?}")))?;

        // 渲染完成后清除脏区域
        self.dirty_tracker.clear();

        Ok(())
    }

    /// 处理消息并返回需要执行的命令
    pub fn handle_message(&mut self, message: Message) -> Vec<Command> {
        match message {
            Message::Screenshot(msg) => {
                let mut commands = self.screenshot.handle_message(msg.clone());

                if let ScreenshotMessage::StartCapture = msg
                    && commands.contains(&Command::ShowOverlay)
                {
                    let _ = self
                        .screenshot
                        .create_d2d_bitmap_from_gdi(&mut self.platform);

                    self.drawing.reset_state();
                    self.update_toolbar_state();
                    commands.push(Command::UpdateToolbar);
                    commands.push(Command::RequestRedraw);
                }

                commands
            }
            Message::Drawing(msg) => self.drawing.handle_message(msg),
            Message::UI(msg) => {
                // 使用缓存的屏幕尺寸，避免重复的系统API调用
                let (screen_width, screen_height) = self.screen_size;
                self.ui.handle_message(msg, screen_width, screen_height)
            }
            Message::System(msg) => self.system.handle_message(msg),
        }
    }

    /// 初始化系统托盘
    pub fn init_system_tray(&mut self, hwnd: windows::Win32::Foundation::HWND) -> AppResult<()> {
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
        // 重新加载配置
        self.config.reload();

        self.system.reload_settings();
        self.drawing.reload_drawing_properties();
        vec![Command::UpdateToolbar, Command::RequestRedraw]
    }

    /// 获取配置管理器引用
    pub fn config(&self) -> &ConfigManager {
        &self.config
    }

    /// 重新注册热键
    pub fn reregister_hotkey(
        &mut self,
        hwnd: windows::Win32::Foundation::HWND,
    ) -> windows::core::Result<()> {
        self.system.reregister_hotkey(hwnd)
    }

    /// OCR引擎状态变更回调
    pub fn on_ocr_engine_status_changed(
        &mut self,
        available: bool,
        hwnd: windows::Win32::Foundation::HWND,
    ) {
        self.system.on_ocr_engine_status_changed(available, hwnd);
    }

    /// 处理键盘输入
    pub fn handle_key_input(&mut self, key: u32) -> Vec<Command> {
        // ESC键在任何状态下都可以退出（保持在App层处理）
        if key == VK_ESCAPE.0 as u32 {
            self.reset_to_initial_state();
            return vec![Command::HideWindow];
        }

        // 其他按键委托给当前状态处理器
        let (commands, transition) = {
            let mut ctx = StateContext {
                screenshot: &mut self.screenshot,
                drawing: &mut self.drawing,
                ui: &mut self.ui,
                system: &mut self.system,
            };
            self.current_state.handle_key_input(key, &mut ctx)
        };

        // 应用状态转换
        self.apply_transition(transition);
        commands
    }

    /// 选择绘图工具
    pub fn select_drawing_tool(
        &mut self,
        tool: DrawingTool,
    ) -> Vec<Command> {
        let message = DrawingMessage::SelectTool(tool);
        self.drawing.handle_message(message)
    }
    
    /// 创建D2D位图
    pub fn create_d2d_bitmap_from_gdi(&mut self) -> AppResult<()> {
        self.screenshot
            .create_d2d_bitmap_from_gdi(&mut self.platform)
            .map_err(|e| AppError::Render(format!("Failed to create D2D bitmap: {e:?}")))
    }

    /// 处理鼠标移动
    pub fn handle_mouse_move(&mut self, x: i32, y: i32) -> Vec<Command> {
        // 委托给当前状态处理器
        let (commands, _consumed, transition) = {
            let mut ctx = StateContext {
                screenshot: &mut self.screenshot,
                drawing: &mut self.drawing,
                ui: &mut self.ui,
                system: &mut self.system,
            };
            self.current_state.handle_mouse_move(x, y, &mut ctx)
        };

        // 应用状态转换
        self.apply_transition(transition);

        // 统一设置鼠标指针（保持原逻辑）
        let cursor_id = {
            let hovered_button = self.ui.get_hovered_button();
            let is_button_disabled = self.ui.is_button_disabled(hovered_button);
            let is_text_editing = self.drawing.is_text_editing();

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

            CursorManager::determine_cursor(
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

        CursorManager::set_cursor(cursor_id);
        commands
    }

    /// 处理鼠标按下
    pub fn handle_mouse_down(&mut self, x: i32, y: i32) -> Vec<Command> {
        // 委托给当前状态处理器
        let (commands, _consumed, transition) = {
            let mut ctx = StateContext {
                screenshot: &mut self.screenshot,
                drawing: &mut self.drawing,
                ui: &mut self.ui,
                system: &mut self.system,
            };
            self.current_state.handle_mouse_down(x, y, &mut ctx)
        };

        // 应用状态转换
        self.apply_transition(transition);
        commands
    }

    /// 处理鼠标释放
    pub fn handle_mouse_up(&mut self, x: i32, y: i32) -> Vec<Command> {
        // 委托给当前状态处理器
        let (commands, _consumed, transition) = {
            let mut ctx = StateContext {
                screenshot: &mut self.screenshot,
                drawing: &mut self.drawing,
                ui: &mut self.ui,
                system: &mut self.system,
            };
            self.current_state.handle_mouse_up(x, y, &mut ctx)
        };

        // 应用状态转换
        self.apply_transition(transition);
        commands
    }

    /// 处理双击事件
    pub fn handle_double_click(&mut self, x: i32, y: i32) -> Vec<Command> {
        // 委托给当前状态处理器
        let (commands, transition) = {
            let mut ctx = StateContext {
                screenshot: &mut self.screenshot,
                drawing: &mut self.drawing,
                ui: &mut self.ui,
                system: &mut self.system,
            };
            self.current_state.handle_double_click(x, y, &mut ctx)
        };

        // 应用状态转换
        self.apply_transition(transition);
        commands
    }

    /// 处理文本输入
    pub fn handle_text_input(&mut self, character: char) -> Vec<Command> {
        // 委托给当前状态处理器
        let mut ctx = StateContext {
            screenshot: &mut self.screenshot,
            drawing: &mut self.drawing,
            ui: &mut self.ui,
            system: &mut self.system,
        };
        self.current_state.handle_text_input(character, &mut ctx)
    }

    /// 处理托盘消息
    pub fn handle_tray_message(&mut self, wparam: u32, lparam: u32) -> Vec<Command> {
        // 通过系统管理器处理托盘消息
        self.system.handle_tray_message(wparam, lparam)
    }

    /// 执行截图
    pub fn take_screenshot(&mut self, hwnd: windows::Win32::Foundation::HWND) -> AppResult<()> {
        // 重置状态并开始截图
        self.screenshot.reset_state();

        // 设置当前窗口并捕获屏幕
        self.screenshot.set_current_window(hwnd);
        self.screenshot.capture_screen()?;

        // 显示窗口进入选择模式
        let _ = win_api::show_window(hwnd);

        Ok(())
    }

    /// 直接捕获屏幕（用于热键处理）
    pub fn capture_screen_direct(&mut self) -> AppResult<()> {
        self.screenshot
            .capture_screen()
            .map_err(|e| AppError::Screenshot(format!("Failed to capture screen: {e:?}")))
    }

    /// 更新工具栏状态以反映当前选中的绘图工具
    pub fn update_toolbar_state(&mut self) {
        // 工具栏始终显示当前绘图工具
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

    /// 合成选择区域图像和绘图元素，返回BMP数据
    fn compose_selection_with_drawings(
        &mut self,
        selection_rect: &RECT,
    ) -> AppResult<Vec<u8>> {
        // 获取D2D位图
        let Some(source_bitmap) = self.screenshot.get_d2d_bitmap() else {
            return Err(AppError::Screenshot("No D2D bitmap available".to_string()));
        };
        let source_bitmap = source_bitmap.clone();

        // 克隆selection_rect以便在闭包中使用
        let sel_rect = *selection_rect;

        // 使用闭包来渲染元素
        let drawing_ref = &self.drawing;

        let bmp_data = self.platform
            .render_selection_to_bmp(&source_bitmap, &sel_rect, |render_target, renderer| {
                drawing_ref
                    .render_elements_to_target(render_target, renderer, &sel_rect)
                    .map_err(|e| PlatformError::RenderError(format!("{e}")))
            })
            .map_err(|e| AppError::Render(format!("Failed to compose image: {e:?}")))?;

        Ok(bmp_data)
    }

    /// 保存选择区域到剪贴板（包含绘图元素）
    pub fn save_selection_to_clipboard(
        &mut self,
        _hwnd: windows::Win32::Foundation::HWND,
    ) -> AppResult<()> {
        // 获取选择区域
        let Some(selection_rect) = self.screenshot.get_selection() else {
            return Ok(());
        };

        let width = selection_rect.right - selection_rect.left;
        let height = selection_rect.bottom - selection_rect.top;
        if width <= 0 || height <= 0 {
            return Ok(());
        }

        // 合成图像（截图 + 绘图元素）
        let bmp_data = self.compose_selection_with_drawings(&selection_rect)?;

        // 将 BMP 数据复制到剪贴板
        copy_bmp_data_to_clipboard(&bmp_data)
            .map_err(|e| AppError::Screenshot(format!("Failed to copy to clipboard: {e:?}")))
    }

    /// 保存选择区域到文件（包含绘图元素）
    /// 返回 Ok(true) 表示保存成功，Ok(false) 表示用户取消，Err 表示错误
    pub fn save_selection_to_file(
        &mut self,
        hwnd: windows::Win32::Foundation::HWND,
    ) -> Result<bool, AppError> {
        use std::fs::File;
        use std::io::Write;

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
        let Some(file_path) = file_dialog::show_image_save_dialog(hwnd, "screenshot.png")
        else {
            return Ok(false); // 用户取消了对话框
        };

        // 合成图像（截图 + 绘图元素）
        let bmp_data = self.compose_selection_with_drawings(&selection_rect)?;

        // 将 BMP 数据写入文件
        let mut file = File::create(&file_path)
            .map_err(|e| AppError::File(format!("Failed to create file: {e}")))?;
        file.write_all(&bmp_data)
            .map_err(|e| AppError::File(format!("Failed to write file: {e}")))?;

        Ok(true)
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

    /// 固定选择区域（包含绘图元素）
    pub fn pin_selection(&mut self, hwnd: windows::Win32::Foundation::HWND) -> AppResult<()> {
        // 检查是否有选择区域
        let Some(selection_rect) = self.screenshot.get_selection() else {
            return Ok(());
        };

        let width = selection_rect.right - selection_rect.left;
        let height = selection_rect.bottom - selection_rect.top;

        if width <= 0 || height <= 0 {
            return Ok(());
        }

        // 合成图像（截图 + 绘图元素）
        let bmp_data = self.compose_selection_with_drawings(&selection_rect)?;

        // 创建固钉窗口
        if let Err(e) = PreviewWindow::show(bmp_data, vec![], selection_rect, true) {
            return Err(AppError::WinApi(format!(
                "Failed to show pin window: {e:?}"
            )));
        }

        // 隐藏原始截屏窗口
        let _ = win_api::hide_window(hwnd);

        // 重置原始窗口状态，准备下次截屏
        self.reset_to_initial_state();

        Ok(())
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

    /// 处理平台无关的输入事件
    ///
    /// 这是新的抽象层 API，接受平台无关的 InputEvent
    pub fn handle_input_event(
        &mut self,
        event: InputEvent,
        _hwnd: windows::Win32::Foundation::HWND,
    ) -> Vec<Command> {
        match event {
            InputEvent::MouseMove { x, y } => self.handle_mouse_move(x, y),

            InputEvent::MouseDown { x, y, button: MouseButton::Left } => {
                self.handle_mouse_down(x, y)
            }

            InputEvent::MouseUp { x, y, button: MouseButton::Left } => {
                self.handle_mouse_up(x, y)
            }

            InputEvent::DoubleClick { x, y, button: MouseButton::Left } => {
                self.handle_double_click(x, y)
            }

            InputEvent::KeyDown { key, .. } => self.handle_key_input(key.0),

            InputEvent::TextInput { character } => self.handle_text_input(character),

            InputEvent::Timer { id } => self.handle_cursor_timer(id),

            // 其他鼠标按键暂不处理
            InputEvent::MouseDown { .. }
            | InputEvent::MouseUp { .. }
            | InputEvent::DoubleClick { .. } => vec![],

            // KeyUp 和 MouseWheel 暂不处理
            InputEvent::KeyUp { .. } | InputEvent::MouseWheel { .. } => vec![],
        }
    }

    /// 统一处理窗口消息
    /// 返回 Some(LRESULT) 表示消息已处理，None 表示需要默认处理
    pub fn handle_window_message(
        &mut self,
        hwnd: windows::Win32::Foundation::HWND,
        msg: u32,
        wparam: windows::Win32::Foundation::WPARAM,
        lparam: windows::Win32::Foundation::LPARAM,
    ) -> Option<windows::Win32::Foundation::LRESULT> {
        use windows::Win32::Foundation::LRESULT;
        use windows::Win32::UI::WindowsAndMessaging::*;

        // 先尝试转换为平台无关的输入事件
        if let Some(input_event) = EventConverter::convert(msg, wparam, lparam) {
            let commands = self.handle_input_event(input_event, hwnd);
            self.execute_command_chain(commands, hwnd);
            return Some(LRESULT(0));
        }

        // 非输入事件的窗口消息
        match msg {
            WM_CLOSE => {
                if !win_api::is_window_visible(hwnd) {
                    let _ = win_api::destroy_window(hwnd);
                } else {
                    self.reset_to_initial_state();
                    let _ = win_api::hide_window(hwnd);
                }
                Some(LRESULT(0))
            }

            WM_PAINT => {
                if let Err(e) = self.paint(hwnd) {
                    eprintln!("Paint failed: {e}");
                }
                Some(LRESULT(0))
            }

            WM_SETCURSOR => Some(LRESULT(1)),

            val if val == WM_TRAY_MESSAGE => {
                let commands = self.handle_tray_message(wparam.0 as u32, lparam.0 as u32);
                self.execute_command_chain(commands, hwnd);
                Some(LRESULT(0))
            }

            val if val == WM_HIDE_WINDOW_CUSTOM => {
                self.stop_ocr_engine_async();
                let _ = win_api::hide_window(hwnd);
                Some(LRESULT(0))
            }

            val if val == WM_RELOAD_SETTINGS => {
                let commands = self.reload_settings();
                self.execute_command_chain(commands, hwnd);
                let _ = self.reregister_hotkey(hwnd);
                Some(LRESULT(0))
            }

            val if val == WM_OCR_STATUS_UPDATE => {
                let available = wparam.0 != 0;
                self.on_ocr_engine_status_changed(available, hwnd);
                let commands = vec![
                    Command::UpdateToolbar,
                    Command::RequestRedraw,
                ];
                self.execute_command_chain(commands, hwnd);
                Some(LRESULT(0))
            }

            _ => None, // 返回 None 让调用者使用 DefWindowProcW
        }
    }

    /// 处理OCR完成消息
    /// 返回: (has_results, is_ocr_failed, ocr_text)
    pub fn handle_ocr_completed(
        &mut self,
        ocr_results: Vec<OcrResult>,
        image_data: Vec<u8>,
        selection_rect: RECT,
    ) -> (bool, bool, String) {
        let has_results = !ocr_results.is_empty();
        let is_ocr_failed = ocr_results.len() == 1 && ocr_results[0].text == "OCR识别失败";

        // 显示OCR结果窗口
        if let Err(e) =
            PreviewWindow::show(image_data, ocr_results.clone(), selection_rect, false)
        {
            eprintln!("Failed to show OCR result window: {:?}", e);
        }

        // 提取文本
        let text: String = if has_results {
            ocr_results
                .iter()
                .map(|r| r.text.clone())
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            String::new()
        };

        (has_results, is_ocr_failed, text)
    }
}


impl From<ScreenshotError> for AppError {
    fn from(err: ScreenshotError) -> Self {
        AppError::Screenshot(err.to_string())
    }
}

impl From<DrawingError> for AppError {
    fn from(err: DrawingError) -> Self {
        AppError::Drawing(err.to_string())
    }
}

impl From<UIError> for AppError {
    fn from(err: UIError) -> Self {
        AppError::UI(err.to_string())
    }
}

impl From<SystemError> for AppError {
    fn from(err: SystemError) -> Self {
        AppError::System(err.to_string())
    }
}

impl From<PlatformError> for AppError {
    fn from(err: PlatformError) -> Self {
        AppError::Platform(err.to_string())
    }
}
