use std::collections::HashSet;

use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;

use super::svg_icons::SvgIconManager;
use super::{ToolbarButton, UIError};
use sc_drawing::DrawingTool;
use sc_host_protocol::Command;
use sc_platform_windows::windows::d2d::Direct2DRenderer;

/// Toolbar state and rendering coordinator.
pub struct ToolbarManager {
    /// Whether the toolbar is visible.
    pub visible: bool,

    /// Current platform-neutral layout computed by `sc_ui`.
    layout: Option<sc_ui::toolbar::ToolbarLayout>,

    /// Currently hovered button.
    pub hovered_button: ToolbarButton,
    /// Current selected button.
    pub clicked_button: ToolbarButton,
    /// Button that received the last mouse down.
    pub pressed_button: ToolbarButton,
    /// Disabled buttons.
    pub disabled_buttons: HashSet<ToolbarButton>,
}

impl ToolbarManager {
    /// Create a toolbar manager.
    pub fn new() -> Result<Self, UIError> {
        Ok(Self {
            visible: false,
            layout: None,
            hovered_button: ToolbarButton::None,
            clicked_button: ToolbarButton::None,
            pressed_button: ToolbarButton::None,
            disabled_buttons: HashSet::new(),
        })
    }

    /// Update toolbar position for the current selection.
    pub fn update_position(
        &mut self,
        selection_rect: sc_app::selection::RectI32,
        screen_width: i32,
        screen_height: i32,
    ) {
        let style = sc_ui::toolbar::ToolbarStyle::default();

        let Some(layout) = sc_ui::toolbar::layout_toolbar(
            (screen_width, screen_height),
            Some(selection_rect),
            &style,
        ) else {
            return;
        };

        self.layout = Some(layout);
        self.visible = true;
    }

    /// Return the button at the given point.
    pub fn get_button_at_position(&self, x: i32, y: i32) -> ToolbarButton {
        self.layout
            .as_ref()
            .map(|layout| layout.hit_test(x, y))
            .unwrap_or(ToolbarButton::None)
    }

    /// Set the hovered button.
    pub fn set_hovered_button(&mut self, button: ToolbarButton) {
        self.hovered_button = button;
    }

    /// Set the selected button.
    pub fn set_clicked_button(&mut self, button: ToolbarButton) {
        self.clicked_button = button;
    }
    /// Replace the disabled button set.
    pub fn set_disabled(&mut self, buttons: HashSet<ToolbarButton>) {
        self.disabled_buttons = buttons;
    }

    /// Clear selected button state.
    pub fn clear_clicked_button(&mut self) {
        self.clicked_button = ToolbarButton::None;
    }

    /// Show the toolbar.
    pub fn show(&mut self) {
        self.visible = true;
    }

    /// Hide the toolbar.
    pub fn hide(&mut self) {
        self.visible = false;
        self.layout = None;
        self.hovered_button = ToolbarButton::None;
        self.pressed_button = ToolbarButton::None;
    }

    /// Handle a toolbar button click.
    pub fn handle_button_click(&mut self, button: ToolbarButton) -> Vec<Command> {
        match button {
            ToolbarButton::Rectangle
            | ToolbarButton::Circle
            | ToolbarButton::Arrow
            | ToolbarButton::Pen
            | ToolbarButton::Text => {
                self.clicked_button = button;
            }
            _ => {}
        }

        match button {
            ToolbarButton::Save => {
                vec![Command::Core(sc_app::Action::SaveSelectionToFile)]
            }
            ToolbarButton::Rectangle => vec![Command::Core(sc_app::Action::SelectDrawingTool(
                DrawingTool::Rectangle,
            ))],
            ToolbarButton::Circle => vec![Command::Core(sc_app::Action::SelectDrawingTool(
                DrawingTool::Circle,
            ))],
            ToolbarButton::Arrow => vec![Command::Core(sc_app::Action::SelectDrawingTool(
                DrawingTool::Arrow,
            ))],
            ToolbarButton::Pen => vec![Command::Core(sc_app::Action::SelectDrawingTool(
                DrawingTool::Pen,
            ))],
            ToolbarButton::Text => vec![Command::Core(sc_app::Action::SelectDrawingTool(
                DrawingTool::Text,
            ))],
            ToolbarButton::Undo => vec![Command::Core(sc_app::Action::Undo)],
            ToolbarButton::ExtractText => vec![Command::Core(sc_app::Action::ExtractText)],
            ToolbarButton::Languages => {
                // Reserved for future language selection UI.
                vec![]
            }
            ToolbarButton::Pin => {
                vec![Command::Core(sc_app::Action::PinSelection)]
            }
            ToolbarButton::Confirm => {
                vec![Command::Core(sc_app::Action::SaveSelectionToClipboard)]
            }
            ToolbarButton::Cancel => {
                vec![Command::Core(sc_app::Action::Cancel)]
            }
            _ => vec![Command::None],
        }
    }

    /// Render the toolbar.
    pub fn render(
        &self,
        d2d_renderer: &mut Direct2DRenderer,
        svg_icons: &SvgIconManager,
    ) -> Result<(), UIError> {
        if !self.visible {
            return Ok(());
        }

        // Background + hover highlight are expressed as a platform-neutral RenderList.
        let style = sc_ui::toolbar::ToolbarStyle::default();

        let Some(layout) = self.layout.as_ref() else {
            return Ok(());
        };

        let mut list = sc_ui::toolbar::build_toolbar_background_render_list(
            layout,
            self.hovered_button,
            &style,
        );

        list.execute(d2d_renderer)
            .map_err(|e| UIError::RenderError(format!("render list execute failed: {e:?}")))?;

        // Icons remain host-specific.
        unsafe {
            if let Some(render_target) = &d2d_renderer.render_target {
                for b in &layout.buttons {
                    let button_rect = b.rect;
                    let button_type = b.button;

                    // Determine icon color.
                    let is_disabled = self.disabled_buttons.contains(&button_type);
                    let icon_color = if is_disabled {
                        Some((170, 170, 170))
                    } else if self.clicked_button == button_type {
                        Some((33, 196, 94))
                    } else {
                        Some((16, 16, 16))
                    };

                    if let Ok(Some(icon_bitmap)) =
                        svg_icons.render_icon_to_bitmap(button_type, render_target, 24, icon_color)
                    {
                        let icon_size = 20.0;
                        let icon_x = button_rect.x + (button_rect.width - icon_size) / 2.0;
                        let icon_y = button_rect.y + (button_rect.height - icon_size) / 2.0;

                        let icon_rect = D2D_RECT_F {
                            left: icon_x,
                            top: icon_y,
                            right: icon_x + icon_size,
                            bottom: icon_y + icon_size,
                        };

                        render_target.DrawBitmap(
                            &icon_bitmap,
                            Some(&icon_rect),
                            1.0,
                            windows::Win32::Graphics::Direct2D::D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                            None,
                        );
                    }
                }
            }
        }

        Ok(())
    }

    /// Handle mouse move.
    pub fn handle_mouse_move(&mut self, x: i32, y: i32) -> Vec<Command> {
        if !self.visible {
            return vec![];
        }

        let hovered_button = self.get_button_at_position(x, y);

        if self.hovered_button != hovered_button {
            self.hovered_button = hovered_button;
            vec![Command::RequestRedraw]
        } else {
            vec![]
        }
    }

    /// Handle mouse down.
    pub fn handle_mouse_down(&mut self, x: i32, y: i32) -> Vec<Command> {
        if !self.visible {
            return vec![];
        }

        let button_type = self.get_button_at_position(x, y);
        if button_type != ToolbarButton::None {
            if self.disabled_buttons.contains(&button_type) {
                return vec![];
            }

            self.pressed_button = button_type;
            return self.handle_button_click(button_type);
        }

        vec![]
    }

    /// Handle mouse up.
    pub fn handle_mouse_up(&mut self, x: i32, y: i32) -> Vec<Command> {
        if !self.visible {
            return vec![];
        }

        let toolbar_button = self.get_button_at_position(x, y);
        if toolbar_button != ToolbarButton::None && toolbar_button == self.pressed_button {
            self.pressed_button = ToolbarButton::None;
            vec![]
        } else {
            self.pressed_button = ToolbarButton::None;
            vec![]
        }
    }

    /// Whether the toolbar is visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Handle double click.
    pub fn handle_double_click(&mut self, x: i32, y: i32) -> Vec<Command> {
        self.handle_mouse_down(x, y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sc_app::Action;

    #[test]
    fn drawing_tool_click_selects_button_and_emits_core_action() {
        let mut manager = ToolbarManager::new().unwrap();

        let commands = manager.handle_button_click(ToolbarButton::Rectangle);

        assert_eq!(manager.clicked_button, ToolbarButton::Rectangle);
        assert_eq!(
            commands,
            vec![Command::Core(Action::SelectDrawingTool(
                DrawingTool::Rectangle
            ))]
        );
    }

    #[test]
    fn action_button_click_does_not_replace_selected_tool() {
        let mut manager = ToolbarManager::new().unwrap();
        manager.clicked_button = ToolbarButton::Pen;

        let commands = manager.handle_button_click(ToolbarButton::Save);

        assert_eq!(manager.clicked_button, ToolbarButton::Pen);
        assert_eq!(commands, vec![Command::Core(Action::SaveSelectionToFile)]);
    }

    #[test]
    fn hide_clears_transient_pointer_state_but_keeps_selection() {
        let mut manager = ToolbarManager::new().unwrap();
        manager.visible = true;
        manager.hovered_button = ToolbarButton::Save;
        manager.pressed_button = ToolbarButton::Save;
        manager.clicked_button = ToolbarButton::Text;

        manager.hide();

        assert!(!manager.visible);
        assert_eq!(manager.hovered_button, ToolbarButton::None);
        assert_eq!(manager.pressed_button, ToolbarButton::None);
        assert_eq!(manager.clicked_button, ToolbarButton::Text);
    }
}
