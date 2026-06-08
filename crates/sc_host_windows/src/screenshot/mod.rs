use sc_platform::WindowId;

use crate::system::SystemError;
use sc_app::selection as core_selection;
use sc_drawing::{DragMode, Rect};
use sc_highlight::{AutoHighlightMoveAction, AutoHighlightMoveArgs, AutoHighlighter};
use sc_host_protocol::Command;
use sc_platform_windows::windows::Direct2DRenderer;

pub mod selection;

use selection::SelectionState;

pub struct ScreenshotManager {
    selection: SelectionState,
    current_screenshot: Option<ScreenshotData>,

    screen_width: i32,
    screen_height: i32,

    hide_ui_for_capture: bool,

    show_selection_handles: bool,

    auto_highlight: AutoHighlighter,

    current_window: WindowId,
}

pub struct ScreenshotData {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

impl ScreenshotManager {
    pub fn has_screenshot(&self) -> bool {
        self.current_screenshot.is_some()
    }

    pub fn new(screen_size: (i32, i32)) -> Result<Self, ScreenshotError> {
        let (screen_width, screen_height) = screen_size;

        Ok(Self {
            selection: SelectionState::new(),
            current_screenshot: None,

            screen_width,
            screen_height,
            hide_ui_for_capture: false,
            show_selection_handles: true,
            auto_highlight: {
                let mut highlighter = AutoHighlighter::new();
                highlighter.start_detection()?;
                highlighter
            },
            current_window: WindowId::INVALID,
        })
    }

    pub fn set_show_selection_handles(&mut self, show: bool) {
        self.show_selection_handles = show;
    }

    pub fn get_screen_width(&self) -> i32 {
        self.screen_width
    }

    pub fn get_screen_height(&self) -> i32 {
        self.screen_height
    }

    pub fn should_show_selection_handles(&self) -> bool {
        self.show_selection_handles
    }

    pub fn is_hiding_ui_for_capture(&self) -> bool {
        self.hide_ui_for_capture
    }

    pub fn render(&mut self, d2d_renderer: &mut Direct2DRenderer) -> Result<(), ScreenshotError> {
        d2d_renderer.draw_background_bitmap_fullscreen();
        Ok(())
    }

    pub fn reset_state(&mut self, screen_size: (i32, i32)) {
        self.current_screenshot = None;

        self.selection.reset();

        let (w, h) = screen_size;
        self.screen_width = w;
        self.screen_height = h;

        self.hide_ui_for_capture = false;

        self.show_selection_handles = true;

        self.auto_highlight.reset();
    }

    pub fn set_current_window(&mut self, window: WindowId) {
        self.current_window = window;
    }

    pub fn capture_screen(
        &mut self,
        screen_size: (i32, i32),
    ) -> std::result::Result<(), ScreenshotError> {
        self.capture_screen_with_exclude_window(screen_size, self.current_window)
    }

    pub fn capture_screen_with_exclude_window(
        &mut self,
        screen_size: (i32, i32),
        _exclude_window: WindowId,
    ) -> std::result::Result<(), ScreenshotError> {
        let (current_screen_width, current_screen_height) = screen_size;

        self.screen_width = current_screen_width;
        self.screen_height = current_screen_height;

        self.current_screenshot = Some(ScreenshotData {
            width: self.screen_width as u32,
            height: self.screen_height as u32,
            data: vec![],
        });

        if let Err(e) = self.auto_highlight.refresh_windows() {
            eprintln!("Warning: Failed to refresh windows: {e:?}");
        }

        Ok(())
    }

    pub fn capture_screen_to_d2d_bitmap(
        &mut self,
        d2d_renderer: &mut Direct2DRenderer,
    ) -> std::result::Result<(), ScreenshotError> {
        let screen_rect = Rect {
            left: 0,
            top: 0,
            right: self.screen_width,
            bottom: self.screen_height,
        };

        let (d2d_bitmap, bmp_data) = d2d_renderer
            .capture_screen_region_to_d2d_bitmap_and_bmp_data(screen_rect)
            .map_err(|e| {
                ScreenshotError::RenderError(format!("Failed to create D2D bitmap: {e:?}"))
            })?;

        if let Some(bmp_data) = bmp_data
            && let Some(ref mut screenshot) = self.current_screenshot
        {
            screenshot.data = bmp_data;
        }

        d2d_renderer.set_background_bitmap(d2d_bitmap);
        Ok(())
    }

    pub fn get_current_image_data(&self) -> Option<&[u8]> {
        self.current_screenshot.as_ref().map(|s| s.data.as_slice())
    }

    pub fn handle_mouse_move(
        &mut self,
        x: i32,
        y: i32,
        selection_rect: Option<core_selection::RectI32>,
        hover_selection: Option<core_selection::RectI32>,
    ) -> (Vec<Command>, bool) {
        if self.selection.is_interacting() {
            return (
                vec![
                    Command::Core(sc_app::Action::Selection(
                        core_selection::Action::EditDragMove { x, y },
                    )),
                    Command::RequestRedraw,
                ],
                true,
            );
        }

        if self.selection.is_mouse_pressed() && selection_rect.is_none() {
            return (
                vec![
                    Command::Core(sc_app::Action::Selection(
                        core_selection::Action::MouseMove { x, y },
                    )),
                    Command::RequestRedraw,
                ],
                true,
            );
        }

        let action = self
            .auto_highlight
            .handle_mouse_move(AutoHighlightMoveArgs {
                x,
                y,
                screen_width: self.screen_width,
                screen_height: self.screen_height,
                current_highlight: hover_selection,
                selecting: false,
                interacting: false,
            });

        match action {
            AutoHighlightMoveAction::SetHighlight(target) => (
                vec![
                    Command::Core(sc_app::Action::Selection(
                        core_selection::Action::SetHoverSelection {
                            selection: Some(target.rect),
                        },
                    )),
                    Command::Core(sc_app::Action::Selection(
                        core_selection::Action::SetAutoHighlightActive { active: true },
                    )),
                    Command::RequestRedraw,
                ],
                true,
            ),
            AutoHighlightMoveAction::ClearHighlight => (
                vec![
                    Command::Core(sc_app::Action::Selection(
                        core_selection::Action::SetHoverSelection { selection: None },
                    )),
                    Command::Core(sc_app::Action::Selection(
                        core_selection::Action::SetAutoHighlightActive { active: false },
                    )),
                    Command::RequestRedraw,
                ],
                true,
            ),

            AutoHighlightMoveAction::None => (vec![], false),
        }
    }

    pub fn handle_mouse_down(
        &mut self,
        x: i32,
        y: i32,
        selection_rect: Option<core_selection::RectI32>,
        has_auto_highlight: bool,
    ) -> (Vec<Command>, bool) {
        if self.current_screenshot.is_none() {
            return (vec![], false);
        }

        self.selection.set_mouse_pressed(true);

        if has_auto_highlight {
            return (vec![], true);
        }

        if selection_rect.is_some() {}

        if selection_rect.is_some() {
            let drag_mode = self.selection.get_handle_at_position(selection_rect, x, y);
            if drag_mode != DragMode::None {
                self.selection.start_interaction(x, y, drag_mode);
                return (
                    vec![
                        Command::Core(sc_app::Action::Selection(
                            core_selection::Action::BeginEditDrag { drag_mode, x, y },
                        )),
                        Command::RequestRedraw,
                    ],
                    true,
                );
            }

            self.selection.set_mouse_pressed(false);
            return (vec![], false);
        }

        if !self.auto_highlight.enabled() {
            (
                vec![
                    Command::Core(sc_app::Action::Selection(
                        core_selection::Action::SetHoverSelection { selection: None },
                    )),
                    Command::Core(sc_app::Action::Selection(
                        core_selection::Action::SetAutoHighlightActive { active: false },
                    )),
                    // Seed initial selection so UI can immediately derive from core state.
                    Command::Core(sc_app::Action::Selection(
                        core_selection::Action::MouseMove { x, y },
                    )),
                    Command::RequestRedraw,
                ],
                true,
            )
        } else {
            (vec![], false)
        }
    }

    pub fn handle_mouse_up(
        &mut self,
        _x: i32,
        _y: i32,
        selection_rect: Option<core_selection::RectI32>,
    ) -> (Vec<Command>, bool) {
        let mut commands = Vec::new();

        let is_manual_selecting = !self.auto_highlight.enabled()
            && self.selection.is_mouse_pressed()
            && selection_rect.is_none();

        if is_manual_selecting {
            commands.push(Command::RequestRedraw);
        } else if self.selection.is_interacting() {
            self.selection.end_interaction();
            commands.push(Command::RequestRedraw);

            commands.push(Command::Core(sc_app::Action::Selection(
                core_selection::Action::EndEditDrag,
            )));
        }

        self.selection.set_mouse_pressed(false);

        let consumed = !commands.is_empty();
        (commands, consumed)
    }

    pub fn handle_auto_highlight_mouse_up(
        &mut self,
        is_click: bool,
        selection_has_selection: bool,
        had_active_highlight: bool,
    ) -> bool {
        self.auto_highlight
            .handle_mouse_up(is_click, selection_has_selection, had_active_highlight)
    }

    pub fn handle_double_click(
        &mut self,
        _x: i32,
        _y: i32,
        selection_rect: Option<core_selection::RectI32>,
    ) -> Vec<Command> {
        if selection_rect.is_some() {
            vec![Command::Core(sc_app::Action::SaveSelectionToClipboard)]
        } else {
            vec![]
        }
    }
}

#[derive(Debug)]
pub enum ScreenshotError {
    CaptureError(String),
    SaveError(String),
    InitError(String),
    RenderError(String),
    SystemError(SystemError),
}

impl From<SystemError> for ScreenshotError {
    fn from(error: SystemError) -> Self {
        ScreenshotError::SystemError(error)
    }
}

impl From<sc_highlight::WindowDetectionError> for ScreenshotError {
    fn from(error: sc_highlight::WindowDetectionError) -> Self {
        ScreenshotError::SystemError(SystemError::WindowDetectionError(error.to_string()))
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
    pub fn get_handle_at_position(
        &self,
        selection_rect: Option<core_selection::RectI32>,
        x: i32,
        y: i32,
    ) -> DragMode {
        self.selection.get_handle_at_position(selection_rect, x, y)
    }
}

impl ScreenshotManager {
    pub fn get_selection_image(
        &self,
        selection_rect: Option<core_selection::RectI32>,
    ) -> Option<Vec<u8>> {
        if selection_rect.is_some() && self.has_screenshot() {
            self.current_screenshot.as_ref().map(|s| s.data.clone())
        } else {
            None
        }
    }
}
