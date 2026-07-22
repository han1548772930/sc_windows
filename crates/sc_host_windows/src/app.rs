use std::collections::{HashSet, VecDeque};
use std::fs::File;
use std::io::Write;

use sc_platform::WindowId;

use crate::command_executor::CommandExecutor;
use crate::constants::*;
use crate::core_bridge;
use crate::error::{AppError, AppResult};
use crate::screenshot::{ScreenshotError, ScreenshotManager};
use crate::scroll_capture::ScrollCaptureSession;
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
use sc_ui_windows::{
    CursorManager, PreviewWindow, ScrollPreviewWindow, ToolbarButton, UIError, UIManager,
};

use crate::HostEvent;

pub struct App {
    /// Core state/actions/effects (platform-neutral).
    core: AppModel,

    config: ConfigManager,
    screenshot: ScreenshotManager,
    drawing: DrawingManager,
    ui: UIManager,
    system: SystemManager,

    /// Host-facing platform side effects (window ops, timers, clipboard, dialogs, etc.).
    host_platform: Box<dyn HostPlatform<WindowHandle = WindowId>>,

    platform: Direct2DRenderer,
    screen_size: (i32, i32),
    dirty_tracker: DirtyRectTracker,

    /// Cached OCR availability (updated via `HostEvent::OcrAvailabilityChanged`).
    ocr_available: bool,

    /// Cached OCR completion payload (set by `HostEvent::OcrCompleted`, consumed by `Command::ShowOcrPreview`).
    last_ocr_completion: Option<OcrCompletionData>,
    scroll_capture: Option<ScrollCaptureSession>,
    scroll_overlay_window: Option<WindowId>,
    scroll_wheel_sequence: u64,
    scroll_quiet_ticks: u8,
    scroll_wheel_delta: i64,
    scroll_stitch_throttle_ticks: u8,
    scroll_pending_direction: i8,
    scroll_preview_dirty: bool,
    scroll_frame_queue: VecDeque<(Vec<u8>, i8)>,
}

impl App {
    pub fn new(
        platform: Direct2DRenderer,
        events: UserEventSender<HostEvent>,
        host_platform: Box<dyn HostPlatform<WindowHandle = WindowId>>,
    ) -> AppResult<Self> {
        let config = ConfigManager::new();

        let shared_settings = config.get_shared();

        let screen_size = host_platform.screen_size();

        let screenshot = ScreenshotManager::new(screen_size)?;

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
            scroll_capture: None,
            scroll_overlay_window: None,
            scroll_wheel_sequence: 0,
            scroll_quiet_ticks: 3,
            scroll_wheel_delta: 0,
            scroll_stitch_throttle_ticks: 0,
            scroll_pending_direction: 0,
            scroll_preview_dirty: false,
            scroll_frame_queue: VecDeque::new(),
        })
    }

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
        if let Some(window) = self.scroll_overlay_window.take() {
            let _ = sc_platform_windows::windows::system::set_window_region_hole(
                window,
                self.screen_size,
                None,
            );
        }
        sc_platform_windows::windows::system::stop_scroll_wheel_hook();
        self.scroll_capture = None;
        self.scroll_preview_dirty = false;
        self.scroll_frame_queue.clear();
        self.ui.set_scrolling_mode(false);
        ScrollPreviewWindow::close();

        vec![]
    }

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

    fn validated_selection_rect(&self) -> Option<core_selection::RectI32> {
        let rect = self.confirmed_selection_rect()?;
        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;
        if width <= 0 || height <= 0 {
            None
        } else {
            Some(rect)
        }
    }

    pub fn has_valid_selection(&self) -> bool {
        self.confirmed_selection_rect().is_some() && self.screenshot.has_screenshot()
    }

    pub fn mark_dirty_rect(&mut self, rect: core_selection::RectI32) {
        self.dirty_tracker.mark_dirty(Rectangle {
            x: rect.left as f32,
            y: rect.top as f32,
            width: (rect.right - rect.left) as f32,
            height: (rect.bottom - rect.top) as f32,
        });
    }

    pub fn mark_full_redraw(&mut self) {
        self.dirty_tracker.mark_full_redraw();
    }

    pub fn paint(&mut self) -> AppResult<()> {
        // The WM_PAINT cycle is managed by the platform runner.
        self.render()
    }

    pub fn render(&mut self) -> AppResult<()> {
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

        self.dirty_tracker.clear();

        Ok(())
    }

    pub(crate) fn handle_ui_message(&mut self, message: UIMessage) -> Vec<Command> {
        let (screen_width, screen_height) = self.screen_size;
        self.ui.handle_message(message, screen_width, screen_height)
    }

    pub(crate) fn handle_drawing_message(&mut self, message: DrawingMessage) -> Vec<Command> {
        self.drawing.handle_message(message)
    }

    pub fn init_system_tray(&mut self, window: WindowId) -> AppResult<()> {
        let host_platform = self.host_platform.as_ref();
        self.system
            .initialize(window, host_platform)
            .map_err(|e| AppError::Init(format!("Failed to initialize system tray: {e}")))
    }

    pub fn start_async_ocr_check(&mut self) {
        self.system.start_async_ocr_check();
    }

    pub fn handle_cursor_timer(&mut self, timer_id: u32) -> Vec<Command> {
        self.drawing.handle_cursor_timer(timer_id)
    }

    pub fn stop_ocr_engine_async(&self) {
        self.system.stop_ocr_engine_async();
    }

    pub fn start_ocr_engine_async(&self) {
        self.system.start_ocr_engine_async();
    }

    pub fn reload_settings(&mut self) -> Vec<Command> {
        self.config.reload();

        self.system.reload_settings();

        // Inject updated drawing config (no Settings dependency inside sc_drawing_host).
        let drawing_config = Self::drawing_config_from_settings(&self.config.get());
        self.drawing.update_config(drawing_config);

        vec![Command::UpdateToolbar, Command::RequestRedraw]
    }

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

    pub fn reregister_hotkey(&mut self, window: WindowId) -> Result<(), PlatformServicesError> {
        let host_platform = self.host_platform.as_ref();
        self.system.reregister_hotkey(window, host_platform)
    }

    pub(crate) fn cleanup_before_quit(&mut self) {
        let host_platform = self.host_platform.as_ref();
        self.system.cleanup_platform(host_platform);
    }

    pub(crate) fn dispatch_core_action(&mut self, action: sc_app::Action) -> Vec<Command> {
        let is_selection_mouse_up = matches!(
            action,
            sc_app::Action::Selection(core_selection::Action::MouseUp { .. })
        );

        let had_active_highlight = self.core.selection().has_auto_highlight()
            && self.core.selection().hover_selection().is_some();

        let mut commands = core_bridge::dispatch(&mut self.core, action);

        // Auto-highlight should only be updated after core has confirmed/rejected selection.
        if is_selection_mouse_up {
            let is_click = self
                .core
                .take_selection_mouse_up_is_click()
                .unwrap_or(false);
            let selection_has_selection = self.confirmed_selection_rect().is_some();
            if self.screenshot.handle_auto_highlight_mouse_up(
                is_click,
                selection_has_selection,
                had_active_highlight,
            ) {
                commands.push(Command::RequestRedraw);
            }
        }

        commands
    }

    pub fn handle_key_input(&mut self, key: u32) -> Vec<Command> {
        if key == KeyCode::ESCAPE.0 {
            return vec![Command::Core(sc_app::Action::Cancel)];
        }

        let phase = self.core.selection().phase().clone();
        match phase {
            core_selection::Phase::Idle => self.system.handle_key_input(key),

            core_selection::Phase::Selecting { .. } => vec![],

            core_selection::Phase::Editing { .. } => {
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

    pub fn select_drawing_tool(&mut self, tool: DrawingTool) -> Vec<Command> {
        let message = DrawingMessage::SelectTool(tool);
        self.drawing.handle_message(message)
    }

    pub fn capture_screen_to_d2d_bitmap(&mut self) -> AppResult<()> {
        self.screenshot
            .capture_screen_to_d2d_bitmap(&mut self.platform)
            .map_err(|e| AppError::Render(format!("Failed to create D2D bitmap: {e:?}")))
    }

    pub fn handle_mouse_move(&mut self, x: i32, y: i32) -> Vec<Command> {
        let phase = self.core.selection().phase().clone();
        let hover_selection = self.core.selection().hover_selection();
        let commands = match phase {
            core_selection::Phase::Editing { .. } => {
                self.handle_mouse_move_editing(x, y, hover_selection)
            }

            core_selection::Phase::Idle | core_selection::Phase::Selecting { .. } => {
                self.handle_mouse_move_non_editing(x, y, hover_selection)
            }
        };

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

            let selection_rect = self.confirmed_selection_rect();
            let current_tool = self.drawing.get_current_tool();
            let selection_handle_mode =
                self.screenshot
                    .get_handle_at_position(self.confirmed_selection_rect(), x, y);

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

    pub fn handle_mouse_down(&mut self, x: i32, y: i32) -> Vec<Command> {
        let phase = self.core.selection().phase().clone();
        let has_auto_highlight = self.core.selection().has_auto_highlight()
            && self.core.selection().hover_selection().is_some();
        match phase {
            core_selection::Phase::Idle => {
                let (cmds, consumed) = self.screenshot.handle_mouse_down(
                    x,
                    y,
                    self.confirmed_selection_rect(),
                    has_auto_highlight,
                );
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
                let (cmds, _consumed) = self.screenshot.handle_mouse_down(
                    x,
                    y,
                    self.confirmed_selection_rect(),
                    has_auto_highlight,
                );
                cmds
            }

            core_selection::Phase::Editing { .. } => {
                self.handle_mouse_down_editing(x, y, has_auto_highlight)
            }
        }
    }

    pub fn handle_mouse_up(&mut self, x: i32, y: i32) -> Vec<Command> {
        let phase = self.core.selection().phase().clone();
        match phase {
            core_selection::Phase::Idle => vec![],

            core_selection::Phase::Selecting { .. } => {
                // Let the host update selection geometry / auto-highlight state.
                let (mut commands, _consumed) =
                    self.screenshot
                        .handle_mouse_up(x, y, self.confirmed_selection_rect());

                commands.push(Command::Core(CoreAction::Selection(
                    core_selection::Action::MouseUp { x, y },
                )));

                commands
            }

            core_selection::Phase::Editing { .. } => self.handle_mouse_up_editing(x, y),
        }
    }

    fn handle_mouse_move_editing(
        &mut self,
        x: i32,
        y: i32,
        hover_selection: Option<core_selection::RectI32>,
    ) -> Vec<Command> {
        let mut commands = Vec::new();

        let (ui_commands, ui_consumed) = self.ui.handle_mouse_move(x, y);
        commands.extend(ui_commands);

        if self.has_scrolling_capture() {
            return commands;
        }

        if !ui_consumed {
            let selection_rect = self.confirmed_selection_rect().map(Into::into);
            let (drawing_commands, drawing_consumed) =
                self.drawing.handle_mouse_move(x, y, selection_rect);
            commands.extend(drawing_commands);

            if !drawing_consumed && !self.drawing.is_dragging() {
                let (screenshot_commands, _screenshot_consumed) = self
                    .screenshot
                    .handle_mouse_move(x, y, self.confirmed_selection_rect(), hover_selection);
                commands.extend(screenshot_commands);
            }
        }

        commands
    }

    fn handle_mouse_move_non_editing(
        &mut self,
        x: i32,
        y: i32,
        hover_selection: Option<core_selection::RectI32>,
    ) -> Vec<Command> {
        let (cmds, _consumed) = self.screenshot.handle_mouse_move(
            x,
            y,
            self.confirmed_selection_rect(),
            hover_selection,
        );
        cmds
    }

    fn handle_mouse_down_editing(
        &mut self,
        x: i32,
        y: i32,
        has_auto_highlight: bool,
    ) -> Vec<Command> {
        let mut commands = Vec::new();

        let (ui_commands, ui_consumed) = self.ui.handle_mouse_down(x, y);
        commands.extend(ui_commands);

        if self.has_scrolling_capture() {
            return commands;
        }

        if !ui_consumed {
            let selection_rect = self.confirmed_selection_rect().map(Into::into);
            let (drawing_commands, drawing_consumed) =
                self.drawing.handle_mouse_down(x, y, selection_rect);
            commands.extend(drawing_commands);

            if !drawing_consumed {
                let (screenshot_commands, screenshot_consumed) = self.screenshot.handle_mouse_down(
                    x,
                    y,
                    self.confirmed_selection_rect(),
                    has_auto_highlight,
                );
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

    fn handle_mouse_up_editing(&mut self, x: i32, y: i32) -> Vec<Command> {
        let mut commands = Vec::new();

        let (ui_commands, ui_consumed) = self.ui.handle_mouse_up(x, y);
        commands.extend(ui_commands);

        if self.has_scrolling_capture() {
            return commands;
        }

        if !ui_consumed {
            let (drawing_commands, drawing_consumed) = self.drawing.handle_mouse_up(x, y);
            commands.extend(drawing_commands);

            if !drawing_consumed {
                let (screenshot_commands, _screenshot_consumed) =
                    self.screenshot
                        .handle_mouse_up(x, y, self.confirmed_selection_rect());
                commands.extend(screenshot_commands);
            }
        }

        commands
    }

    pub fn handle_double_click(&mut self, x: i32, y: i32) -> Vec<Command> {
        let phase = self.core.selection().phase().clone();
        match phase {
            core_selection::Phase::Editing { .. } => {
                let mut commands = Vec::new();

                commands.extend(self.ui.handle_double_click(x, y));

                if commands.is_empty() {
                    let selection_rect = self.confirmed_selection_rect().map(Into::into);
                    let dcmds = self
                        .drawing
                        .handle_double_click(x, y, selection_rect.as_ref());
                    if dcmds.is_empty() {
                        commands.extend(self.screenshot.handle_double_click(
                            x,
                            y,
                            self.confirmed_selection_rect(),
                        ));
                    } else {
                        commands.extend(dcmds);
                    }
                }

                commands
            }

            core_selection::Phase::Idle | core_selection::Phase::Selecting { .. } => vec![],
        }
    }

    pub fn handle_text_input(&mut self, character: char) -> Vec<Command> {
        let phase = self.core.selection().phase().clone();
        match phase {
            core_selection::Phase::Editing { .. } => self.drawing.handle_text_input(character),
            core_selection::Phase::Idle | core_selection::Phase::Selecting { .. } => vec![],
        }
    }

    pub fn take_screenshot(&mut self, window: WindowId) -> AppResult<()> {
        self.platform.clear_background_bitmap();

        let screen_size = self.update_screen_size_cache();
        self.screenshot.reset_state(screen_size);

        self.screenshot.set_current_window(window);
        self.screenshot.capture_screen(screen_size)?;

        let _ = self.host_platform.show_window(window);

        Ok(())
    }

    pub fn capture_screen_direct(&mut self) -> AppResult<()> {
        let screen_size = self.update_screen_size_cache();

        self.screenshot
            .capture_screen(screen_size)
            .map_err(|e| AppError::Screenshot(format!("Failed to capture screen: {e:?}")))
    }

    pub fn update_toolbar_state(&mut self) {
        let current_tool = self.drawing.get_current_tool();

        self.ui.update_toolbar_selected_tool(current_tool);

        let show_handles = matches!(current_tool, DrawingTool::None);
        self.screenshot.set_show_selection_handles(show_handles);

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

    fn compose_selection_with_drawings(
        &mut self,
        selection_rect: core_selection::RectI32,
    ) -> AppResult<Vec<u8>> {
        let sel_rect_drawing: Rect = selection_rect.into();

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

    pub fn save_selection_to_clipboard(&mut self, _window: WindowId) -> AppResult<()> {
        let Some(selection_rect) = self.validated_selection_rect() else {
            return Ok(());
        };

        let bmp_data = self.compose_selection_with_drawings(selection_rect)?;

        self.host_platform
            .copy_bmp_data_to_clipboard(&bmp_data)
            .map_err(|e| AppError::Screenshot(format!("Failed to copy to clipboard: {e}")))
    }

    pub(crate) fn has_scrolling_capture(&self) -> bool {
        self.scroll_capture.is_some()
    }

    pub(crate) fn start_scrolling_capture(&mut self, window: WindowId) -> AppResult<()> {
        let Some(selection) = self.validated_selection_rect() else {
            return Err(AppError::Screenshot("请先选择滚动截图区域".to_string()));
        };
        self.ui.set_scrolling_mode(true);
        self.screenshot.set_show_selection_handles(false);
        let _ = self.handle_ui_message(UIMessage::ShowToolbar(selection));
        let center_x = selection.left + (selection.right - selection.left) / 2;
        let center_y = selection.top + (selection.bottom - selection.top) / 2;
        sc_platform_windows::windows::system::window_below_at_screen_point(
            window, center_x, center_y,
        )
        .ok_or_else(|| AppError::Screenshot("未找到选区下方的可滚动窗口".to_string()))?;
        self.scroll_overlay_window = Some(window);
        sc_platform_windows::windows::system::set_window_region_hole(
            window,
            self.screen_size,
            Some(selection.into()),
        )
        .map_err(AppError::Screenshot)?;
        let first =
            match sc_platform_windows::windows::gdi::capture_screen_region_to_bmp(selection.into())
            {
                Ok(first) => first,
                Err(error) => {
                    let _ = sc_platform_windows::windows::system::set_window_region_hole(
                        window,
                        self.screen_size,
                        None,
                    );
                    self.ui.set_scrolling_mode(false);
                    self.screenshot.set_show_selection_handles(true);
                    return Err(AppError::Screenshot(format!("滚动首帧捕获失败: {error}")));
                }
            };
        self.scroll_capture =
            Some(ScrollCaptureSession::new(selection, &first).map_err(AppError::Screenshot)?);
        let sequence =
            sc_platform_windows::windows::system::start_scroll_wheel_hook(selection.into())
                .map_err(AppError::Screenshot)?;
        self.scroll_wheel_sequence = sequence;
        self.scroll_wheel_delta = sc_platform_windows::windows::system::scroll_wheel_delta_total();
        self.scroll_stitch_throttle_ticks = 0;
        self.scroll_quiet_ticks = 3;
        self.scroll_pending_direction = 0;
        self.scroll_preview_dirty = false;
        self.scroll_frame_queue.clear();
        ScrollPreviewWindow::show_or_update(selection, self.scrolling_preview_bmp()?)
            .map_err(AppError::Screenshot)?;
        self.host_platform
            .start_timer(
                window,
                TIMER_SCROLL_CAPTURE_ID as u32,
                TIMER_SCROLL_CAPTURE_MS,
            )
            .map_err(|e| AppError::Platform(e.to_string()))?;
        Ok(())
    }

    pub(crate) fn advance_scrolling_capture(&mut self, window: WindowId) -> AppResult<()> {
        let Some(selection) = self.scroll_capture.as_ref().map(|s| s.selection()) else {
            return Ok(());
        };
        let sequence = sc_platform_windows::windows::system::scroll_wheel_sequence();
        if sequence != self.scroll_wheel_sequence {
            if self.scroll_quiet_ticks >= 3 && self.scroll_frame_queue.is_empty() {
                self.scroll_capture
                    .as_mut()
                    .expect("scroll capture checked above")
                    .begin_gesture();
            }
            self.scroll_wheel_sequence = sequence;
            self.scroll_quiet_ticks = 0;
        } else if self.scroll_quiet_ticks < 3 {
            self.scroll_quiet_ticks += 1;
        } else {
            return Ok(());
        }
        let bmp = sc_platform_windows::windows::gdi::capture_screen_region_to_bmp(selection.into())
            .map_err(|e| AppError::Screenshot(format!("滚动帧捕获失败: {e}")))?;
        let wheel_delta = sc_platform_windows::windows::system::scroll_wheel_delta_total();
        if wheel_delta != self.scroll_wheel_delta {
            self.scroll_pending_direction = match wheel_delta.cmp(&self.scroll_wheel_delta) {
                std::cmp::Ordering::Less => 1,
                std::cmp::Ordering::Greater => -1,
                std::cmp::Ordering::Equal => 0,
            };
            self.scroll_wheel_delta = wheel_delta;
        }
        // Smooth scrolling keeps repainting after the wheel message. Those settling frames still
        // belong to the same directed gesture; marking them as unknown lets repeated chat rows
        // match in the opposite direction.
        let direction = self.scroll_pending_direction;
        self.scroll_frame_queue.push_back((bmp, direction));

        let settled = self.scroll_quiet_ticks >= 3;
        self.scroll_stitch_throttle_ticks = self.scroll_stitch_throttle_ticks.saturating_add(1);
        if !settled && self.scroll_stitch_throttle_ticks < SCROLL_STITCH_THROTTLE_TICKS {
            return Ok(());
        }
        let mut changed = false;
        let mut finished = false;
        let frames_to_process = if settled {
            self.scroll_frame_queue.len()
        } else {
            (SCROLL_STITCH_THROTTLE_TICKS as usize).min(self.scroll_frame_queue.len())
        };
        for _ in 0..frames_to_process {
            let Some((frame, frame_direction)) = self.scroll_frame_queue.pop_front() else {
                break;
            };
            let outcome = self
                .scroll_capture
                .as_mut()
                .expect("scroll capture checked above")
                .push_frame(&frame, frame_direction)
                .map_err(AppError::Screenshot)?;
            changed |= outcome.changed;
            finished |= outcome.finished;
            if finished {
                break;
            }
        }
        if changed {
            self.scroll_preview_dirty = true;
        }
        if settled
            && self
                .scroll_capture
                .as_mut()
                .expect("scroll capture checked above")
                .finish_gesture()
        {
            self.scroll_preview_dirty = true;
        }

        if finished {
            let _ = self
                .host_platform
                .stop_timer(window, TIMER_SCROLL_CAPTURE_ID as u32);
            sc_platform_windows::windows::system::stop_scroll_wheel_hook();
            let _ = self.host_platform.request_redraw(window);
        }
        // The preview must represent every frame already consumed from the FIFO. Delaying this
        // until the gesture settles makes it visibly lag behind the captured window.
        let refresh_preview = self.scroll_preview_dirty;
        if refresh_preview {
            ScrollPreviewWindow::show_or_update(selection, self.scrolling_preview_bmp()?)
                .map_err(AppError::Screenshot)?;
            self.scroll_preview_dirty = false;
        }
        self.scroll_stitch_throttle_ticks = 0;
        if settled || finished {
            self.scroll_pending_direction = 0;
        }
        Ok(())
    }

    fn scrolling_bmp(&self) -> AppResult<Vec<u8>> {
        self.scroll_capture
            .as_ref()
            .ok_or_else(|| AppError::Screenshot("没有可用的滚动截图".to_string()))?
            .bmp_data()
            .map_err(AppError::Screenshot)
    }

    fn scrolling_preview_bmp(&self) -> AppResult<Vec<u8>> {
        let session = self
            .scroll_capture
            .as_ref()
            .ok_or_else(|| AppError::Screenshot("没有可用的滚动截图".to_string()))?;
        let max_window_height = (self.screen_size.1 * 9 / 10)
            .min(self.screen_size.1 - 24)
            .max(180);
        session
            .preview_bmp_data(264, (max_window_height - 16).max(1) as u32)
            .map_err(AppError::Screenshot)
    }

    pub(crate) fn save_scrolling_to_clipboard(&mut self) -> AppResult<()> {
        let bmp = self.scrolling_bmp()?;
        self.host_platform
            .copy_bmp_data_to_clipboard(&bmp)
            .map_err(|e| AppError::Screenshot(format!("复制滚动截图失败: {e}")))
    }

    pub(crate) fn save_scrolling_to_file(&mut self, window: WindowId) -> AppResult<bool> {
        let Some(path) = self
            .host_platform
            .show_image_save_dialog(window, "scroll-capture.bmp")
            .map_err(|e| AppError::Platform(e.to_string()))?
        else {
            return Ok(false);
        };
        let bmp = self.scrolling_bmp()?;
        let mut file = File::create(path).map_err(|e| AppError::File(e.to_string()))?;
        file.write_all(&bmp)
            .map_err(|e| AppError::File(e.to_string()))?;
        Ok(true)
    }

    pub(crate) fn edit_scrolling_capture(&mut self, window: WindowId) -> AppResult<()> {
        let selection = self
            .scroll_capture
            .as_ref()
            .ok_or_else(|| AppError::Screenshot("没有可用的滚动截图".to_string()))?
            .selection();
        ScrollPreviewWindow::close();
        PreviewWindow::show(
            self.scrolling_bmp()?,
            vec![],
            selection,
            true,
            self.current_drawing_config(),
            None,
        )
        .map_err(|e| AppError::WinApi(format!("打开滚动截图预览失败: {e:?}")))?;
        let _ = self.host_platform.hide_window(window);
        self.reset_to_initial_state();
        Ok(())
    }

    pub fn save_selection_to_file(&mut self, window: WindowId) -> Result<bool, AppError> {
        let Some(selection_rect) = self.validated_selection_rect() else {
            return Ok(false);
        };

        let Some(file_path) = self
            .host_platform
            .show_image_save_dialog(window, "screenshot.png")
            .map_err(|e| AppError::Platform(e.to_string()))?
        else {
            return Ok(false);
        };

        let bmp_data = self.compose_selection_with_drawings(selection_rect)?;

        let mut file = File::create(&file_path)
            .map_err(|e| AppError::File(format!("Failed to create file: {e}")))?;
        file.write_all(&bmp_data)
            .map_err(|e| AppError::File(format!("Failed to write file: {e}")))?;

        Ok(true)
    }

    pub fn extract_text_from_selection(&mut self, window: WindowId) -> AppResult<()> {
        let Some(selection_rect) = self.confirmed_selection_rect() else {
            return Ok(());
        };

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

    pub fn pin_selection(&mut self, window: WindowId) -> AppResult<Vec<Command>> {
        let Some(selection_rect) = self.validated_selection_rect() else {
            return Ok(vec![]);
        };

        let bmp_data = self.compose_selection_with_drawings(selection_rect)?;

        // OCR source: use the raw screenshot crop (no drawings) for better recognition.
        let crop_rect: Rect = selection_rect.into();
        let ocr_source_bmp_data = self
            .screenshot
            .get_current_image_data()
            .and_then(|data| crop_bmp(data, &crop_rect).ok());

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

        let _ = self.host_platform.hide_window(window);

        Ok(self.reset_to_initial_state())
    }

    pub fn can_undo(&self) -> bool {
        self.drawing.can_undo()
    }

    pub fn is_ocr_engine_available(&self) -> bool {
        self.ocr_available
    }

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

            InputEvent::MouseDown { .. }
            | InputEvent::MouseUp { .. }
            | InputEvent::DoubleClick { .. } => vec![],

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

    fn handle_raw_window_message(
        &mut self,
        _window: WindowId,
        msg: u32,
        _wparam: usize,
        lparam: isize,
    ) -> Option<isize> {
        let _ = (msg, lparam);
        None
    }

    /// Summarize OCR results for core (no UI side effects).
    pub fn summarize_ocr_results(&self, ocr_results: &[OcrResult]) -> (bool, bool, String) {
        sc_ocr::summarize_outcome(ocr_results).into_summary()
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

            InputEvent::Timer { id } if id == TIMER_SCROLL_CAPTURE_ID as u32 => {
                if let Err(e) = self.advance_scrolling_capture(window) {
                    eprintln!("Scrolling capture failed: {e}");
                    let _ = self
                        .host_platform
                        .stop_timer(window, TIMER_SCROLL_CAPTURE_ID as u32);
                    let _ = self.host_platform.show_window(window);
                }
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
            HostEvent::OcrAvailabilityChanged {
                generation,
                available,
            } => {
                if generation != self.system.ocr_generation() {
                    return Some(0);
                }
                self.ocr_available = available;
                self.update_toolbar_state();
                let _ = self.host_platform.request_redraw(window);
                Some(0)
            }

            HostEvent::OcrCompleted { generation, data } => {
                if generation != self.system.ocr_generation() {
                    return Some(0);
                }
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

            HostEvent::OcrCancelled { generation } => {
                if generation != self.system.ocr_generation() {
                    return Some(0);
                }
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
