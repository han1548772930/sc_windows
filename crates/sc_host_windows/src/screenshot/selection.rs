use sc_app::selection as core_selection;
use sc_drawing::{DragMode, HandleConfig, Rect, detect_handle_with_moving_with_radius};

/// Host-side selection interaction state (mouse/drag tracking).
pub struct SelectionState {
    mouse_pressed: bool,

    /// Whether the user is currently interacting with the confirmed selection (move/resize).
    interaction_drag_mode: Option<DragMode>,
}

impl Default for SelectionState {
    fn default() -> Self {
        Self::new()
    }
}

impl SelectionState {
    pub fn new() -> Self {
        Self {
            mouse_pressed: false,
            interaction_drag_mode: None,
        }
    }

    pub fn reset(&mut self) {
        self.mouse_pressed = false;
        self.interaction_drag_mode = None;
    }

    pub fn is_mouse_pressed(&self) -> bool {
        self.mouse_pressed
    }

    pub fn set_mouse_pressed(&mut self, pressed: bool) {
        self.mouse_pressed = pressed;
    }

    pub fn clear_selection(&mut self) {
        self.interaction_drag_mode = None;
        self.mouse_pressed = false;
    }

    pub fn get_handle_at_position(
        &self,
        selection_rect: Option<core_selection::RectI32>,
        x: i32,
        y: i32,
    ) -> DragMode {
        let rect = match selection_rect {
            Some(rect) => rect,
            None => return DragMode::None,
        };

        // Use drawing-core hit testing with the host-configured hit radius.
        let rect: Rect = rect.into();
        let radius = crate::constants::HANDLE_DETECTION_RADIUS as i32;
        detect_handle_with_moving_with_radius(x, y, &rect, HandleConfig::Full, radius, true)
    }

    pub fn start_interaction(&mut self, _x: i32, _y: i32, drag_mode: DragMode) {
        self.mouse_pressed = true;
        self.interaction_drag_mode = Some(drag_mode);
    }

    pub fn end_interaction(&mut self) {
        self.interaction_drag_mode = None;
        self.mouse_pressed = false;
    }

    pub fn is_interacting(&self) -> bool {
        self.interaction_drag_mode.is_some()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_selection_state_new() {
        let state = super::SelectionState::new();
        assert!(!state.is_mouse_pressed());
        assert!(!state.is_interacting());
    }

    #[test]
    fn test_selection_state_reset() {
        let mut state = super::SelectionState::new();
        state.set_mouse_pressed(true);
        state.start_interaction(0, 0, super::DragMode::Moving);
        assert!(state.is_interacting());
        assert!(state.is_mouse_pressed());

        state.reset();
        assert!(!state.is_mouse_pressed());
        assert!(!state.is_interacting());
    }

    #[test]
    fn test_selection_state_clear() {
        let mut state = super::SelectionState::new();
        state.set_mouse_pressed(true);
        state.start_interaction(0, 0, super::DragMode::Moving);
        assert!(state.is_interacting());

        state.clear_selection();
        assert!(!state.is_mouse_pressed());
        assert!(!state.is_interacting());
    }

    #[test]
    fn test_selection_state_interaction_lifecycle() {
        let mut state = super::SelectionState::new();
        assert!(!state.is_interacting());
        state.start_interaction(0, 0, super::DragMode::Moving);
        assert!(state.is_interacting());
        state.end_interaction();
        assert!(!state.is_interacting());
        assert!(!state.is_mouse_pressed());
    }
}
