use sc_app::selection as core_selection;
use sc_drawing::{DragMode, HandleConfig, Point, Rect, detect_handle_with_moving_with_radius};

/// Selection state mirrored in the host.
///
/// The selection rectangle is derived from core (`Phase::Editing`) and mirrored here so the host
/// can perform hit-testing and platform-only work (e.g., Win32 API boundaries).
pub struct SelectionState {
    /// 当前已确认选择矩形
    selection_rect: Option<core_selection::RectI32>,

    /// 鼠标是否按下
    mouse_pressed: bool,
    /// 交互开始位置
    interaction_start_pos: Point,

    /// Whether the user is currently interacting with the confirmed selection (move/resize).
    interaction_drag_mode: Option<DragMode>,
}

impl Default for SelectionState {
    fn default() -> Self {
        Self::new()
    }
}

impl SelectionState {
    /// 创建新的选择状态
    pub fn new() -> Self {
        Self {
            selection_rect: None,
            mouse_pressed: false,
            interaction_start_pos: Point::new(0, 0),
            interaction_drag_mode: None,
        }
    }

    /// 重置选择状态
    pub fn reset(&mut self) {
        self.selection_rect = None;
        self.mouse_pressed = false;
        self.interaction_start_pos = Point::new(0, 0);
        self.interaction_drag_mode = None;
    }

    /// 获取当前选择矩形
    pub fn get_selection(&self) -> Option<core_selection::RectI32> {
        self.selection_rect
    }

    /// 是否有选择区域
    pub fn has_selection(&self) -> bool {
        self.selection_rect.is_some()
    }

    /// 检查鼠标是否按下
    pub fn is_mouse_pressed(&self) -> bool {
        self.mouse_pressed
    }

    /// 设置鼠标按下状态
    pub fn set_mouse_pressed(&mut self, pressed: bool) {
        self.mouse_pressed = pressed;
    }

    /// 设置交互起始位置
    pub fn set_interaction_start_pos(&mut self, x: i32, y: i32) {
        self.interaction_start_pos = Point::new(x, y);
    }

    /// 获取交互起始位置
    pub fn get_interaction_start_pos(&self) -> Point {
        self.interaction_start_pos
    }

    /// 清除选择
    pub fn clear_selection(&mut self) {
        self.selection_rect = None;
        self.interaction_drag_mode = None;
        self.mouse_pressed = false; // 清除鼠标按下状态
    }

    /// 直接设置当前选择矩形（作为“已确认选区”）。
    ///
    /// 用于将 auto-highlight 的候选框在“单击确认”时固化为真实选区。
    /// 注意：这里不会做最小尺寸校验（保持与 hover/命中检测一致）。
    pub fn set_confirmed_selection_rect(&mut self, rect: core_selection::RectI32) {
        self.selection_rect = Some(rect);
    }

    /// 清除“已确认选区”（不影响 mouse_pressed / interaction_start_pos）。
    ///
    /// 这是为了让 host 能在不打断 mouse state 的情况下，把 confirmed selection 同步到 core 模型。
    pub fn clear_confirmed_selection_rect(&mut self) {
        self.selection_rect = None;
        self.interaction_drag_mode = None;
    }

    /// 检测鼠标位置是否在选择框手柄上
    pub fn get_handle_at_position(&self, x: i32, y: i32) -> DragMode {
        // 如果没有选择区域，返回None
        let rect = match self.get_selection() {
            Some(rect) => rect,
            None => return DragMode::None,
        };

        // Use drawing-core hit testing with the host-configured hit radius.
        let rect: Rect = rect.into();
        let radius = crate::constants::HANDLE_DETECTION_RADIUS as i32;
        detect_handle_with_moving_with_radius(x, y, &rect, HandleConfig::Full, radius, true)
    }

    /// 开始选择框交互操作
    pub fn start_interaction(&mut self, x: i32, y: i32, drag_mode: DragMode) {
        self.mouse_pressed = true;
        self.interaction_start_pos = Point::new(x, y);
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

// ==================== 单元测试 ====================

#[cfg(test)]
mod tests {
    #[test]
    fn test_selection_state_new() {
        let state = super::SelectionState::new();
        assert!(state.get_selection().is_none());
        assert!(!state.has_selection());
        assert!(!state.is_mouse_pressed());
    }

    #[test]
    fn test_selection_state_reset() {
        let mut state = super::SelectionState::new();

        let rect = super::core_selection::RectI32 {
            left: 10,
            top: 10,
            right: 200,
            bottom: 200,
        };
        state.set_confirmed_selection_rect(rect);
        state.set_mouse_pressed(true);
        assert!(state.has_selection());
        assert!(state.is_mouse_pressed());

        // 重置
        state.reset();
        assert!(state.get_selection().is_none());
        assert!(!state.has_selection());
        assert!(!state.is_mouse_pressed());
    }

    #[test]
    fn test_selection_state_clear() {
        let mut state = super::SelectionState::new();

        let rect = super::core_selection::RectI32 {
            left: 10,
            top: 10,
            right: 200,
            bottom: 200,
        };
        state.set_confirmed_selection_rect(rect);
        state.set_mouse_pressed(true);

        state.clear_selection();
        assert!(state.get_selection().is_none());
        assert!(!state.has_selection());
        assert!(!state.is_mouse_pressed());
    }

    #[test]
    fn test_selection_state_set_confirmed_selection_rect() {
        let mut state = super::SelectionState::new();

        let rect = super::core_selection::RectI32 {
            left: 100,
            top: 100,
            right: 300,
            bottom: 300,
        };

        state.set_confirmed_selection_rect(rect);
        assert!(state.has_selection());

        let current = state.get_selection().unwrap();
        assert_eq!(current.left, 100);
        assert_eq!(current.right, 300);

        state.clear_selection();
        assert!(!state.has_selection());
    }
}
