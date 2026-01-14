use std::collections::HashSet;
use std::fs::File;
use std::io::Write;

use sc_platform::WindowId;

use crate::command_executor::CommandExecutor;
use crate::constants::*;
use crate::core_bridge;
use crate::error::{AppError, AppResult};
use crate::screenshot::{ScreenshotError, ScreenshotManager};
use crate::system::{SystemError, SystemManager};
use sc_app::AppModel;
use sc_app::{Action as CoreAction, selection as core_selection};
use sc_drawing::Rect;
use sc_drawing_host::{DrawingConfig, DrawingError, DrawingManager, DrawingTool};
use sc_host_protocol::{Command, DrawingMessage, UIMessage};
use sc_ocr::{OcrCompletionData, OcrResult};
use sc_platform::{
    Color, HostPlatform, InputEvent, KeyCode, MouseButton, PlatformError, PlatformServicesError,
    WindowEvent,
};
use sc_platform_windows::windows::bmp::crop_bmp;
use sc_platform_windows::windows::{Direct2DRenderer, UserEventSender};
use sc_rendering::Rectangle;
use sc_rendering::{DirtyRectTracker, DirtyType};
use sc_settings::{ConfigManager, Settings};
use sc_ui_windows::cursor::CursorContext;
use sc_ui_windows::{CursorManager, PreviewWindow, ToolbarButton, UIError, UIManager};

use crate::HostEvent;

/// 应用程序主结构体
pub struct App {
    /// Core state/actions/effects (platform-neutral).
    core: AppModel,

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

    /// Host-facing platform side effects (window ops, timers, clipboard, dialogs, etc.).
    host_platform: Box<dyn HostPlatform<WindowHandle = WindowId>>,

    /// Direct2D 渲染器
    platform: Direct2DRenderer,
    /// 缓存的屏幕尺寸 (宽度, 高度)
    screen_size: (i32, i32),
    /// 脏矩形追踪器（用于渲染优化）
    dirty_tracker: DirtyRectTracker,

    /// Cached OCR availability (updated via `HostEvent::OcrAvailabilityChanged`).
    ocr_available: bool,

    /// Cached OCR completion payload (set by `HostEvent::OcrCompleted`, consumed by `Command::ShowOcrPreview`).
    last_ocr_completion: Option<OcrCompletionData>,
}

impl App {
    pub fn new(
        platform: Direct2DRenderer,
        events: UserEventSender<HostEvent>,
        host_platform: Box<dyn HostPlatform<WindowHandle = WindowId>>,
    ) -> AppResult<Self> {
        // 初始化配置管理器（仅加载一次配置文件）
        let config = ConfigManager::new();

        // 获取共享的配置引用，供子模块使用
        let shared_settings = config.get_shared();

        // 缓存屏幕尺寸，避免重复调用系统API
        let screen_size = host_platform.screen_size();

        let screenshot = ScreenshotManager::new(screen_size)?;

        // 初始化脏矩形追踪器
        let dirty_tracker = DirtyRectTracker::new(screen_size.0 as f32, screen_size.1 as f32);

        let drawing_config = Self::drawing_config_from_settings(&config.get());

        Ok(Self {
            core: AppModel::new(),
            config,
            screenshot,
            drawing: DrawingManager::new(drawing_config)?,
            ui: UIManager::new()?,
            system: SystemManager::new(shared_settings, events)?,
            host_platform,
            platform,
            screen_size,
            dirty_tracker,
            ocr_available: false,
            last_ocr_completion: None,
        })
    }

    /// 获取缓存的屏幕尺寸
    pub fn get_screen_size(&self) -> (i32, i32) {
        self.screen_size
    }

    fn update_screen_size_cache(&mut self) -> (i32, i32) {
        let screen_size = self.host_platform.screen_size();
        if self.screen_size != screen_size {
            self.screen_size = screen_size;
            self.dirty_tracker
                .set_screen_size(screen_size.0 as f32, screen_size.1 as f32);
        }
        screen_size
    }

    pub(crate) fn host_platform(&self) -> &dyn HostPlatform<WindowHandle = WindowId> {
        self.host_platform.as_ref()
    }

    /// 重置到初始状态
    pub fn reset_to_initial_state(&mut self) -> Vec<Command> {
        // Zed-style: host state is explicit; core state is a pure model we can reset.
        self.dirty_tracker.mark_full_redraw();

        self.core = AppModel::new();

        let screen_size = self.update_screen_size_cache();

        self.platform.clear_background_bitmap();
        self.screenshot.reset_state(screen_size);
        self.drawing.reset_state();
        self.ui.reset_state();
        self.last_ocr_completion = None;

        vec![]
    }

    /// 检查是否可以进行绘图操作
    pub fn can_draw(&self) -> bool {
        matches!(
            self.core.selection().phase(),
            core_selection::Phase::Editing { .. }
        )
    }

    fn confirmed_selection_rect(&self) -> Option<core_selection::RectI32> {
        match self.core.selection().phase() {
            core_selection::Phase::Editing { selection } => Some(*selection),
            core_selection::Phase::Idle | core_selection::Phase::Selecting { .. } => None,
        }
    }

    /// 检查是否有有效的选区图像
    pub fn has_valid_selection(&self) -> bool {
        self.confirmed_selection_rect().is_some() && self.screenshot.has_screenshot()
    }

    /// 标记局部脏区域（用于光标闪烁等局部更新场景）
    pub fn mark_dirty_rect(&mut self, rect: core_selection::RectI32) {
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
    pub fn paint(&mut self) -> AppResult<()> {
        // The WM_PAINT cycle is managed by the platform runner.
        self.render()
    }

    /// 渲染所有组件
    ///
    /// 使用 DirtyRectTracker 追踪脏区域
    pub fn render(&mut self) -> AppResult<()> {
        // 检查脏区域类型
        let dirty_type = self.dirty_tracker.dirty_type();

        #[cfg(debug_assertions)]
        if dirty_type == DirtyType::Partial
            && let Some(rect) = self.dirty_tracker.get_combined_dirty_rect()
        {
            eprintln!(
                "Partial redraw: ({}, {}) {}x{}",
                rect.x, rect.y, rect.width, rect.height
            );
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

        let selection_rect_core = self.core.selection().visible_selection();
        let selection_rect_drawing: Option<Rect> = selection_rect_core.map(Into::into);

        self.drawing
            .render(&mut self.platform, selection_rect_drawing.as_ref())
            .map_err(|e| AppError::Render(format!("Failed to render drawing: {e:?}")))?;

        let screen_size = (
            self.screenshot.get_screen_width(),
            self.screenshot.get_screen_height(),
        );
        let show_handles = self.screenshot.should_show_selection_handles();
        let hide_ui_for_capture = self.screenshot.is_hiding_ui_for_capture();
        let has_auto_highlight = self.core.selection().has_auto_highlight();

        self.ui
            .render_selection_ui(
                &mut self.platform,
                screen_size,
                selection_rect_core,
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

    pub(crate) fn handle_ui_message(&mut self, message: UIMessage) -> Vec<Command> {
        // 使用缓存的屏幕尺寸，避免重复的系统API调用
        let (screen_width, screen_height) = self.screen_size;
        self.ui.handle_message(message, screen_width, screen_height)
    }

    pub(crate) fn handle_drawing_message(&mut self, message: DrawingMessage) -> Vec<Command> {
        self.drawing.handle_message(message)
    }

    /// 初始化系统托盘
    pub fn init_system_tray(&mut self, window: WindowId) -> AppResult<()> {
        let host_platform = self.host_platform.as_ref();
        self.system
            .initialize(window, host_platform)
            .map_err(|e| AppError::Init(format!("Failed to initialize system tray: {e}")))
    }

    /// 启动异步OCR引擎状态检查
    pub fn start_async_ocr_check(&mut self) {
        self.system.start_async_ocr_check();
    }

    /// 处理光标定时器（用于文本输入光标闪烁）
    pub fn handle_cursor_timer(&mut self, timer_id: u32) -> Vec<Command> {
        self.drawing.handle_cursor_timer(timer_id)
    }

    /// 异步停止OCR引擎
    pub fn stop_ocr_engine_async(&self) {
        self.system.stop_ocr_engine_async();
    }

    /// 异步启动 OCR 引擎
    pub fn start_ocr_engine_async(&self) {
        self.system.start_ocr_engine_async();
    }

    /// 重新加载设置
    pub fn reload_settings(&mut self) -> Vec<Command> {
        self.config.reload();

        self.system.reload_settings();

        // Inject updated drawing config (no Settings dependency inside sc_drawing_host).
        let drawing_config = Self::drawing_config_from_settings(&self.config.get());
        self.drawing.update_config(drawing_config);

        vec![Command::UpdateToolbar, Command::RequestRedraw]
    }

    /// 获取配置管理器引用
    pub fn config(&self) -> &ConfigManager {
        &self.config
    }

    pub(crate) fn current_drawing_config(&self) -> DrawingConfig {
        Self::drawing_config_from_settings(&self.config.get())
    }

    fn drawing_config_from_settings(settings: &Settings) -> DrawingConfig {
        DrawingConfig {
            line_thickness: settings.line_thickness,
            drawing_color: (
                settings.drawing_color_red,
                settings.drawing_color_green,
                settings.drawing_color_blue,
            ),
            font_size: settings.font_size,
            font_name: settings.font_name.clone(),
            font_weight: settings.font_weight,
            font_italic: settings.font_italic,
            font_underline: settings.font_underline,
            font_strikeout: settings.font_strikeout,
            font_color: settings.font_color,
        }
    }

    /// 重新注册热键
    pub fn reregister_hotkey(&mut self, window: WindowId) -> Result<(), PlatformServicesError> {
        let host_platform = self.host_platform.as_ref();
        self.system.reregister_hotkey(window, host_platform)
    }

    pub(crate) fn cleanup_before_quit(&mut self) {
        let host_platform = self.host_platform.as_ref();
        self.system.cleanup_platform(host_platform);
    }

    /// 执行一条 core action，并返回需要由宿主执行的命令。
    pub(crate) fn dispatch_core_action(&mut self, action: sc_app::Action) -> Vec<Command> {
        let is_selection_mouse_up = matches!(
            action,
            sc_app::Action::Selection(core_selection::Action::MouseUp { .. })
        );

        let mut commands = core_bridge::dispatch(&mut self.core, action);

        // Keep the host-side confirmed selection rect in sync with the core phase.
        let confirmed_selection = match self.core.selection().phase() {
            core_selection::Phase::Editing { selection } => Some(*selection),
            core_selection::Phase::Idle | core_selection::Phase::Selecting { .. } => None,
        };
        self.screenshot
            .sync_confirmed_selection_from_core(confirmed_selection);

        // Auto-highlight should only be updated after core has confirmed/rejected selection.
        if is_selection_mouse_up {
            let is_click = self
                .screenshot
                .take_pending_mouse_up_is_click()
                .unwrap_or(false);
            let selection_has_selection = self.screenshot.get_selection().is_some();
            if self
                .screenshot
                .handle_auto_highlight_mouse_up(is_click, selection_has_selection)
            {
                commands.push(Command::RequestRedraw);
            }
        }

        commands
    }

    /// 处理键盘输入
    pub fn handle_key_input(&mut self, key: u32) -> Vec<Command> {
        // ESC键在任何状态下都可以退出：走 core action -> effect -> host command 链路。
        if key == KeyCode::ESCAPE.0 {
            return vec![Command::Core(sc_app::Action::Cancel)];
        }

        let phase = self.core.selection().phase().clone();
        match phase {
            core_selection::Phase::Idle => self.system.handle_key_input(key),

            core_selection::Phase::Selecting { .. } => vec![],

            core_selection::Phase::Editing { .. } => {
                // 编辑状态下传递给各个管理器处理
                let mut commands = self.system.handle_key_input(key);
                if commands.is_empty() {
                    commands = self.drawing.handle_key_input(key);
                }
                if commands.is_empty() {
                    commands = self.ui.handle_key_input(key);
                }
                commands
            }
        }
    }

    /// 选择绘图工具
    pub fn select_drawing_tool(&mut self, tool: DrawingTool) -> Vec<Command> {
        let message = DrawingMessage::SelectTool(tool);
        self.drawing.handle_message(message)
    }

    /// 捕获屏幕并创建 D2D 位图
    pub fn capture_screen_to_d2d_bitmap(&mut self) -> AppResult<()> {
        self.screenshot
            .capture_screen_to_d2d_bitmap(&mut self.platform)
            .map_err(|e| AppError::Render(format!("Failed to create D2D bitmap: {e:?}")))
    }

    /// 处理鼠标移动
    pub fn handle_mouse_move(&mut self, x: i32, y: i32) -> Vec<Command> {
        let phase = self.core.selection().phase().clone();
        let commands = match phase {
            core_selection::Phase::Editing { .. } => {
                let mut commands = Vec::new();

                // UI -> Drawing -> Screenshot 的处理顺序
                let (ui_commands, ui_consumed) = self.ui.handle_mouse_move(x, y);
                commands.extend(ui_commands);

                if !ui_consumed {
                    let selection_rect = self.screenshot.get_selection().map(Into::into);
                    let (drawing_commands, drawing_consumed) =
                        self.drawing.handle_mouse_move(x, y, selection_rect);
                    commands.extend(drawing_commands);

                    if !drawing_consumed && !self.drawing.is_dragging() {
                        let (screenshot_commands, _screenshot_consumed) =
                            self.screenshot.handle_mouse_move(x, y);
                        commands.extend(screenshot_commands);
                    }
                }

                commands
            }

            core_selection::Phase::Idle | core_selection::Phase::Selecting { .. } => {
                let (cmds, _consumed) = self.screenshot.handle_mouse_move(x, y);
                cmds
            }
        };

        // 统一设置鼠标指针（保持原逻辑）
        let cursor = {
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

            let ctx = CursorContext {
                mouse_x: x,
                mouse_y: y,
                hovered_button,
                is_button_disabled,
                is_text_editing,
                editing_element_info,
                current_tool,
                selection_rect,
                selected_element_info,
                selection_handle_mode,
            };

            CursorManager::determine_cursor(&ctx, &self.drawing)
        };

        self.host_platform().set_cursor(cursor);
        commands
    }

    /// 处理鼠标按下
    pub fn handle_mouse_down(&mut self, x: i32, y: i32) -> Vec<Command> {
        let phase = self.core.selection().phase().clone();
        match phase {
            core_selection::Phase::Idle => {
                // 从空闲进入框选状态
                let (cmds, consumed) = self.screenshot.handle_mouse_down(x, y);
                if consumed || self.screenshot.has_screenshot() {
                    // Route core actions through the command pipeline.
                    let mut commands = vec![Command::Core(CoreAction::Selection(
                        core_selection::Action::MouseDown { x, y },
                    ))];
                    commands.extend(cmds);
                    commands
                } else {
                    cmds
                }
            }

            core_selection::Phase::Selecting { .. } => {
                let (cmds, _consumed) = self.screenshot.handle_mouse_down(x, y);
                cmds
            }

            core_selection::Phase::Editing { .. } => {
                let mut commands = Vec::new();

                let (ui_commands, ui_consumed) = self.ui.handle_mouse_down(x, y);
                commands.extend(ui_commands);

                if !ui_consumed {
                    let selection_rect = self.screenshot.get_selection().map(Into::into);
                    let (drawing_commands, drawing_consumed) =
                        self.drawing.handle_mouse_down(x, y, selection_rect);
                    commands.extend(drawing_commands);

                    if !drawing_consumed {
                        let (screenshot_commands, screenshot_consumed) =
                            self.screenshot.handle_mouse_down(x, y);
                        commands.extend(screenshot_commands);

                        if !screenshot_consumed {
                            commands.extend(
                                self.drawing
                                    .handle_message(DrawingMessage::SelectElement(None)),
                            );
                        }
                    }
                }

                commands
            }
        }
    }

    /// 处理鼠标释放
    pub fn handle_mouse_up(&mut self, x: i32, y: i32) -> Vec<Command> {
        let phase = self.core.selection().phase().clone();
        match phase {
            core_selection::Phase::Idle => vec![],

            core_selection::Phase::Selecting { .. } => {
                // Let the host update selection geometry / auto-highlight state.
                let (mut commands, _consumed) = self.screenshot.handle_mouse_up(x, y);

                commands.push(Command::Core(CoreAction::Selection(
                    core_selection::Action::MouseUp { x, y },
                )));

                commands
            }

            core_selection::Phase::Editing { .. } => {
                let mut commands = Vec::new();

                let (ui_commands, ui_consumed) = self.ui.handle_mouse_up(x, y);
                commands.extend(ui_commands);

                if !ui_consumed {
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
        }
    }

    /// 处理双击事件
    pub fn handle_double_click(&mut self, x: i32, y: i32) -> Vec<Command> {
        let phase = self.core.selection().phase().clone();
        match phase {
            core_selection::Phase::Editing { .. } => {
                let mut commands = Vec::new();

                // UI层优先处理
                commands.extend(self.ui.handle_double_click(x, y));

                if commands.is_empty() {
                    let selection_rect = self.screenshot.get_selection().map(Into::into);
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

            core_selection::Phase::Idle | core_selection::Phase::Selecting { .. } => vec![],
        }
    }

    /// 处理文本输入
    pub fn handle_text_input(&mut self, character: char) -> Vec<Command> {
        let phase = self.core.selection().phase().clone();
        match phase {
            core_selection::Phase::Editing { .. } => self.drawing.handle_text_input(character),
            core_selection::Phase::Idle | core_selection::Phase::Selecting { .. } => vec![],
        }
    }

    /// 执行截图
    pub fn take_screenshot(&mut self, window: WindowId) -> AppResult<()> {
        // 重置状态并开始截图
        self.platform.clear_background_bitmap();

        let screen_size = self.update_screen_size_cache();
        self.screenshot.reset_state(screen_size);

        // 设置当前窗口并捕获屏幕
        self.screenshot.set_current_window(window);
        self.screenshot.capture_screen(screen_size)?;

        // 显示窗口进入选择模式
        let _ = self.host_platform.show_window(window);

        Ok(())
    }

    /// 直接捕获屏幕（用于热键处理）
    pub fn capture_screen_direct(&mut self) -> AppResult<()> {
        let screen_size = self.update_screen_size_cache();

        self.screenshot
            .capture_screen(screen_size)
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
        if self.core.ocr().is_running() {
            disabled.insert(ToolbarButton::ExtractText);
        }
        self.ui.set_toolbar_disabled(disabled);
    }

    /// 合成选择区域图像和绘图元素，返回BMP数据
    fn compose_selection_with_drawings(
        &mut self,
        selection_rect: core_selection::RectI32,
    ) -> AppResult<Vec<u8>> {
        let sel_rect_drawing: Rect = selection_rect.into();

        // 使用闭包来渲染元素
        let drawing_ref = &self.drawing;

        let bmp_data = self
            .platform
            .render_background_selection_to_bmp(&sel_rect_drawing, |render_target, renderer| {
                drawing_ref
                    .render_elements_to_target(render_target, renderer, &sel_rect_drawing)
                    .map_err(|e| PlatformError::RenderError(format!("{e}")))
            })
            .map_err(|e| AppError::Render(format!("Failed to compose image: {e:?}")))?;

        Ok(bmp_data)
    }

    /// 保存选择区域到剪贴板（包含绘图元素）
    pub fn save_selection_to_clipboard(&mut self, _window: WindowId) -> AppResult<()> {
        // 获取选择区域
        let Some(selection_rect) = self.confirmed_selection_rect() else {
            return Ok(());
        };

        let width = selection_rect.right - selection_rect.left;
        let height = selection_rect.bottom - selection_rect.top;
        if width <= 0 || height <= 0 {
            return Ok(());
        }

        // 合成图像（截图 + 绘图元素）
        let bmp_data = self.compose_selection_with_drawings(selection_rect)?;

        // 将 BMP 数据复制到剪贴板
        self.host_platform
            .copy_bmp_data_to_clipboard(&bmp_data)
            .map_err(|e| AppError::Screenshot(format!("Failed to copy to clipboard: {e}")))
    }

    /// 保存选择区域到文件（包含绘图元素）
    /// 返回 Ok(true) 表示保存成功，Ok(false) 表示用户取消，Err 表示错误
    pub fn save_selection_to_file(&mut self, window: WindowId) -> Result<bool, AppError> {
        // 没有有效选择则直接返回
        let Some(selection_rect) = self.confirmed_selection_rect() else {
            return Ok(false);
        };

        let width = selection_rect.right - selection_rect.left;
        let height = selection_rect.bottom - selection_rect.top;
        if width <= 0 || height <= 0 {
            return Ok(false);
        }

        // 显示文件保存对话框
        let Some(file_path) = self
            .host_platform
            .show_image_save_dialog(window, "screenshot.png")
            .map_err(|e| AppError::Platform(e.to_string()))?
        else {
            return Ok(false); // 用户取消了对话框
        };

        // 合成图像（截图 + 绘图元素）
        let bmp_data = self.compose_selection_with_drawings(selection_rect)?;

        // 将 BMP 数据写入文件
        let mut file = File::create(&file_path)
            .map_err(|e| AppError::File(format!("Failed to create file: {e}")))?;
        file.write_all(&bmp_data)
            .map_err(|e| AppError::File(format!("Failed to write file: {e}")))?;

        Ok(true)
    }

    /// 从选择区域提取文本（简化版本 - 委托给OcrManager）
    pub fn extract_text_from_selection(&mut self, window: WindowId) -> AppResult<()> {
        // 检查是否有选择区域
        let Some(selection_rect) = self.confirmed_selection_rect() else {
            return Ok(());
        };

        // 委托给SystemManager处理整个OCR流程
        let host_platform = self.host_platform.as_ref();

        self.system
            .recognize_text_from_selection(
                selection_rect,
                window,
                &mut self.screenshot,
                host_platform,
            )
            .map_err(|e| AppError::System(format!("OCR识别失败: {e}")))
    }

    /// 固定选择区域（包含绘图元素）
    pub fn pin_selection(&mut self, window: WindowId) -> AppResult<Vec<Command>> {
        // 检查是否有选择区域
        let Some(selection_rect) = self.confirmed_selection_rect() else {
            return Ok(vec![]);
        };

        let width = selection_rect.right - selection_rect.left;
        let height = selection_rect.bottom - selection_rect.top;

        if width <= 0 || height <= 0 {
            return Ok(vec![]);
        }

        // 合成图像（截图 + 绘图元素）
        let bmp_data = self.compose_selection_with_drawings(selection_rect)?;

        // OCR source: use the raw screenshot crop (no drawings) for better recognition.
        let crop_rect: Rect = selection_rect.into();
        let ocr_source_bmp_data = self
            .screenshot
            .get_current_image_data()
            .and_then(|data| crop_bmp(data, &crop_rect).ok());

        // 创建固钉窗口
        if let Err(e) = PreviewWindow::show(
            bmp_data,
            vec![],
            selection_rect,
            true,
            self.current_drawing_config(),
            ocr_source_bmp_data,
        ) {
            return Err(AppError::WinApi(format!(
                "Failed to show pin window: {e:?}"
            )));
        }

        // 隐藏原始截屏窗口
        let _ = self.host_platform.hide_window(window);

        // 重置原始窗口状态，准备下次截屏
        Ok(self.reset_to_initial_state())
    }

    /// 检查是否可以撤销
    pub fn can_undo(&self) -> bool {
        self.drawing.can_undo()
    }

    /// 检查OCR引擎是否可用（非阻塞，基于缓存状态）
    pub fn is_ocr_engine_available(&self) -> bool {
        self.ocr_available
    }

    /// 处理平台无关的输入事件
    ///
    /// 这是新的抽象层 API，接受平台无关的 InputEvent
    pub fn handle_input_event(&mut self, event: InputEvent) -> Vec<Command> {
        match event {
            InputEvent::MouseMove { x, y } => self.handle_mouse_move(x, y),

            InputEvent::MouseDown {
                x,
                y,
                button: MouseButton::Left,
            } => self.handle_mouse_down(x, y),

            InputEvent::MouseUp {
                x,
                y,
                button: MouseButton::Left,
            } => self.handle_mouse_up(x, y),

            InputEvent::DoubleClick {
                x,
                y,
                button: MouseButton::Left,
            } => self.handle_double_click(x, y),

            InputEvent::KeyDown { key, .. } => self.handle_key_input(key.0),

            InputEvent::TextInput { character } => self.handle_text_input(character),

            InputEvent::Timer { id } => self.handle_cursor_timer(id),

            InputEvent::Tray(event) => self.system.handle_tray_event(event),

            // Global hotkey is handled at the platform boundary (needs window side effects).
            InputEvent::Hotkey { .. } => vec![],

            // 其他鼠标按键暂不处理
            InputEvent::MouseDown { .. }
            | InputEvent::MouseUp { .. }
            | InputEvent::DoubleClick { .. } => vec![],

            // KeyUp 和 MouseWheel 暂不处理
            InputEvent::KeyUp { .. } | InputEvent::MouseWheel { .. } => vec![],
        }
    }

    /// Hotkey helper: capture screen and show the overlay window.
    fn perform_capture_and_show(&mut self, window: WindowId) {
        self.start_ocr_engine_async();

        let commands = self.reset_to_initial_state();
        self.execute_command_chain(commands, window);

        let (screen_width, screen_height) = self.get_screen_size();

        if self.capture_screen_direct().is_ok() {
            let _ = self.capture_screen_to_d2d_bitmap();
            let _ = self.host_platform.show_window(window);
            let _ =
                self.host_platform
                    .set_window_topmost(window, 0, 0, screen_width, screen_height);
            let _ = self.host_platform.request_redraw(window);
            let _ = self.host_platform.update_window(window);
        }
    }

    /// 统一处理窗口消息
    /// 返回 Some(result) 表示消息已处理，None 表示需要默认处理
    fn handle_raw_window_message(
        &mut self,
        _window: WindowId,
        _msg: u32,
        _wparam: usize,
        _lparam: isize,
    ) -> Option<isize> {
        None
    }

    /// Summarize OCR results for core (no UI side effects).
    pub fn summarize_ocr_results(&self, ocr_results: &[OcrResult]) -> (bool, bool, String) {
        let has_results = !ocr_results.is_empty();
        let is_ocr_failed = ocr_results.len() == 1 && ocr_results[0].text == "OCR识别失败";

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

    pub fn set_ocr_completion(&mut self, data: OcrCompletionData) {
        self.last_ocr_completion = Some(data);
    }

    pub fn take_ocr_completion(&mut self) -> Option<OcrCompletionData> {
        self.last_ocr_completion.take()
    }
}

impl sc_platform::WindowMessageHandler for App {
    type WindowHandle = WindowId;
    type UserEvent = HostEvent;

    fn handle_input_event(&mut self, window: WindowId, event: InputEvent) -> Option<isize> {
        match event {
            InputEvent::Hotkey { id } if id == HOTKEY_SCREENSHOT_ID as u32 => {
                if self.host_platform.is_window_visible(window) {
                    let _ = self.host_platform.hide_window(window);
                    let _ = self.host_platform.start_timer(
                        window,
                        TIMER_CAPTURE_DELAY_ID as u32,
                        TIMER_CAPTURE_DELAY_MS,
                    );
                } else {
                    self.perform_capture_and_show(window);
                }

                Some(0)
            }

            InputEvent::Timer { id } if id == TIMER_CAPTURE_DELAY_ID as u32 => {
                let _ = self
                    .host_platform
                    .stop_timer(window, TIMER_CAPTURE_DELAY_ID as u32);
                self.perform_capture_and_show(window);
                Some(0)
            }

            _ => {
                let commands = self.handle_input_event(event);
                self.execute_command_chain(commands, window);
                Some(0)
            }
        }
    }

    fn handle_user_event(&mut self, window: WindowId, event: HostEvent) -> Option<isize> {
        match event {
            HostEvent::OcrAvailabilityChanged { available } => {
                self.ocr_available = available;
                self.update_toolbar_state();
                let _ = self.host_platform.request_redraw(window);
                Some(0)
            }

            HostEvent::OcrCompleted(data) => {
                let (has_results, is_failed, text) = self.summarize_ocr_results(&data.ocr_results);
                self.set_ocr_completion(data);

                let commands = self.dispatch_core_action(CoreAction::OcrCompleted {
                    has_results,
                    is_failed,
                    text,
                });
                self.execute_command_chain(commands, window);

                Some(0)
            }

            HostEvent::OcrCancelled => {
                let commands = self.dispatch_core_action(CoreAction::OcrCancelled);
                self.execute_command_chain(commands, window);
                Some(0)
            }
        }
    }

    fn handle_window_event(&mut self, window: WindowId, event: WindowEvent) -> Option<isize> {
        match event {
            WindowEvent::Resized { width, height } => {
                if width <= 0 || height <= 0 {
                    return None;
                }

                if let Err(e) = self.platform.initialize(window, width, height) {
                    eprintln!("Failed to resize renderer: {e}");
                }

                if self.screen_size != (width, height) {
                    self.screen_size = (width, height);
                    self.dirty_tracker
                        .set_screen_size(width as f32, height as f32);
                }

                self.dirty_tracker.mark_full_redraw();
                let _ = self.host_platform.request_redraw(window);

                None
            }

            WindowEvent::DpiChanged { .. } => {
                self.dirty_tracker.mark_full_redraw();
                let _ = self.host_platform.request_redraw(window);
                None
            }

            WindowEvent::DisplayChanged { .. } => {
                let _ = self.update_screen_size_cache();
                self.dirty_tracker.mark_full_redraw();
                let _ = self.host_platform.request_redraw(window);
                None
            }
        }
    }

    fn handle_window_message(
        &mut self,
        window: WindowId,
        msg: u32,
        wparam: usize,
        lparam: isize,
    ) -> Option<isize> {
        self.handle_raw_window_message(window, msg, wparam, lparam)
    }

    fn handle_paint(&mut self, _window: WindowId) -> Option<isize> {
        let _ = self.paint();
        Some(0)
    }

    fn handle_close_requested(&mut self, window: WindowId) -> Option<isize> {
        self.execute_command_chain(
            vec![Command::HideWindow, Command::ResetToInitialState],
            window,
        );
        Some(0)
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
