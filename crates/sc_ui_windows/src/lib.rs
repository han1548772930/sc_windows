pub mod constants;
pub mod cursor;
pub mod icon_assets;
pub mod preview;
pub mod settings;
pub mod svg;
pub mod svg_icons;
pub mod toolbar;

use std::collections::HashSet;

use sc_app::selection::RectI32;
use sc_drawing::DrawingTool;
use sc_host_protocol::{Command, UIMessage};
use sc_platform_windows::windows::Direct2DRenderer;
use sc_ui::selection_overlay::build_selection_overlay_render_list;

pub use cursor::CursorManager;
pub use preview::PreviewWindow;
pub use sc_ui::toolbar::ToolbarButton;

pub use settings::SettingsWindow;

use svg_icons::SvgIconManager;
use toolbar::ToolbarManager;

/// Coordinates host-specific UI elements.
pub struct UIManager {
    toolbar: ToolbarManager,
    svg_icons: SvgIconManager,
}

impl UIManager {
    /// Create a UI manager.
    pub fn new() -> Result<Self, UIError> {
        let mut svg_icons = SvgIconManager::new();
        if let Err(e) = svg_icons.load_icons() {
            eprintln!("Failed to load SVG icons: {e}");
        }

        Ok(Self {
            toolbar: ToolbarManager::new()?,
            svg_icons,
        })
    }

    /// Reset transient UI state.
    pub fn reset_state(&mut self) {
        self.toolbar.hide();
        self.toolbar.clicked_button = ToolbarButton::None;
    }

    /// Handle a platform-neutral UI message.
    pub fn handle_message(
        &mut self,
        message: UIMessage,
        screen_width: i32,
        screen_height: i32,
    ) -> Vec<Command> {
        match message {
            UIMessage::ShowToolbar(rect) => {
                self.toolbar
                    .update_position(rect, screen_width, screen_height);
                self.toolbar.show();
                vec![Command::UpdateToolbar, Command::RequestRedraw]
            }
            UIMessage::HideToolbar => {
                self.toolbar.hide();
                vec![Command::RequestRedraw]
            }
            UIMessage::UpdateToolbarPosition(rect) => {
                if self.toolbar.is_visible() {
                    self.toolbar
                        .update_position(rect, screen_width, screen_height);
                    vec![Command::RequestRedraw]
                } else {
                    vec![]
                }
            }
        }
    }

    /// Render the toolbar UI.
    pub fn render(&self, d2d_renderer: &mut Direct2DRenderer) -> Result<(), UIError> {
        self.toolbar.render(d2d_renderer, &self.svg_icons)?;
        Ok(())
    }

    /// Render selection overlay UI.
    pub fn render_selection_ui(
        &self,
        d2d_renderer: &mut Direct2DRenderer,
        screen_size: (i32, i32),
        selection_rect: Option<RectI32>,
        show_handles: bool,
        hide_ui_for_capture: bool,
        has_auto_highlight: bool,
    ) -> Result<(), UIError> {
        if let Some(mut render_list) = build_selection_overlay_render_list(
            screen_size,
            selection_rect,
            show_handles,
            hide_ui_for_capture,
            has_auto_highlight,
        ) {
            render_list
                .execute(d2d_renderer)
                .map_err(|e| UIError::RenderError(format!("render list execute failed: {e}")))?;
        }
        Ok(())
    }

    /// Handle toolbar mouse move.
    pub fn handle_mouse_move(&mut self, x: i32, y: i32) -> (Vec<Command>, bool) {
        let commands = self.toolbar.handle_mouse_move(x, y);
        let consumed = !commands.is_empty();
        (commands, consumed)
    }

    /// Handle toolbar mouse down.
    pub fn handle_mouse_down(&mut self, x: i32, y: i32) -> (Vec<Command>, bool) {
        let commands = self.toolbar.handle_mouse_down(x, y);
        let consumed = !commands.is_empty();
        (commands, consumed)
    }

    /// Handle toolbar mouse up.
    pub fn handle_mouse_up(&mut self, x: i32, y: i32) -> (Vec<Command>, bool) {
        let commands = self.toolbar.handle_mouse_up(x, y);
        let consumed = !commands.is_empty();
        (commands, consumed)
    }

    /// Handle toolbar double click.
    pub fn handle_double_click(&mut self, x: i32, y: i32) -> Vec<Command> {
        self.toolbar.handle_double_click(x, y)
    }

    /// Handle UI-level keyboard input.
    pub fn handle_key_input(&mut self, _key: u32) -> Vec<Command> {
        vec![]
    }

    /// Return whether the toolbar is visible.
    pub fn is_toolbar_visible(&self) -> bool {
        self.toolbar.is_visible()
    }

    /// Return the currently hovered toolbar button.
    pub fn get_hovered_button(&self) -> ToolbarButton {
        self.toolbar.hovered_button
    }

    /// Return whether a toolbar button is disabled.
    pub fn is_button_disabled(&self, button: ToolbarButton) -> bool {
        self.toolbar.disabled_buttons.contains(&button)
    }

    /// Update selected drawing tool state in the toolbar.
    pub fn update_toolbar_selected_tool(&mut self, tool: DrawingTool) {
        let button = match tool {
            DrawingTool::Rectangle => ToolbarButton::Rectangle,
            DrawingTool::Circle => ToolbarButton::Circle,
            DrawingTool::Arrow => ToolbarButton::Arrow,
            DrawingTool::Pen => ToolbarButton::Pen,
            DrawingTool::Text => ToolbarButton::Text,
            DrawingTool::None => ToolbarButton::None,
        };

        self.toolbar.clicked_button = button;
    }

    /// Replace disabled toolbar button state.
    pub fn set_toolbar_disabled(&mut self, buttons: HashSet<ToolbarButton>) {
        self.toolbar.set_disabled(buttons);
    }
}

/// UI errors.
#[derive(Debug, thiserror::Error)]
pub enum UIError {
    #[error("UI render error: {0}")]
    RenderError(String),
    #[error("UI init error: {0}")]
    InitError(String),
}
