use windows::Win32::Foundation::{POINT, RECT};

use crate::constants::MIN_BOX_SIZE;
use crate::drawing::DragMode;
use crate::interaction::InteractionTarget;
use crate::utils::{detect_handle_at_position_unified, is_rect_valid, update_rect_by_drag, HandleConfig};

/// 选择框交互模式（仅限选择框操作）
#[derive(Debug, Clone, PartialEq)]
pub enum SelectionInteractionMode {
    /// 无交互
    None,
    /// 正在创建选择框
    Creating,
    /// 移动选择框
    Moving,
    /// 调整选择框大小（包含具体的调整方向）
    Resizing(DragMode),
}

impl SelectionInteractionMode {
    /// 从旧的DragMode转换为新的SelectionInteractionMode
    pub fn from_drag_mode(drag_mode: DragMode) -> Self {
        match drag_mode {
            DragMode::None => SelectionInteractionMode::None,
            DragMode::Drawing => SelectionInteractionMode::Creating,
            DragMode::Moving => SelectionInteractionMode::Moving,
            DragMode::ResizingTopLeft
            | DragMode::ResizingTopCenter
            | DragMode::ResizingTopRight
            | DragMode::ResizingMiddleRight
            | DragMode::ResizingBottomRight
            | DragMode::ResizingBottomCenter
            | DragMode::ResizingBottomLeft
            | DragMode::ResizingMiddleLeft => SelectionInteractionMode::Resizing(drag_mode),
            // 绘图元素相关的拖拽模式不应该在SelectionState中处理
            _ => SelectionInteractionMode::None,
        }
    }

    /// 获取调整大小的具体方向（如果是调整大小模式）
    pub fn get_resize_mode(&self) -> Option<DragMode> {
        match self {
            SelectionInteractionMode::Resizing(mode) => Some(*mode),
            _ => None,
        }
    }
}

/// 选择状态
pub struct SelectionState {
    /// 是否正在选择
    selecting: bool,
    /// 选择起点
    start_point: Option<(i32, i32)>,
    /// 选择终点
    end_point: Option<(i32, i32)>,
    /// 当前选择矩形
    selection_rect: Option<RECT>,
    /// 自动高亮矩形
    auto_highlight_rect: Option<RECT>,

    // 选择框交互状态（仅限选择框操作）
    /// 选择框交互模式
    selection_interaction_mode: SelectionInteractionMode,
    /// 鼠标是否按下
    mouse_pressed: bool,
    /// 交互开始位置
    interaction_start_pos: POINT,
    /// 交互开始时的选择框矩形
    interaction_start_rect: RECT,
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
            selecting: false,
            start_point: None,
            end_point: None,
            selection_rect: None,
            auto_highlight_rect: None,
            selection_interaction_mode: SelectionInteractionMode::None,
            mouse_pressed: false,
            interaction_start_pos: POINT { x: 0, y: 0 },
            interaction_start_rect: RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
        }
    }

    /// 重置选择状态
    pub fn reset(&mut self) {
        self.selecting = false;
        self.start_point = None;
        self.end_point = None;
        self.selection_rect = None;
        self.auto_highlight_rect = None;
        self.selection_interaction_mode = SelectionInteractionMode::None;
        self.mouse_pressed = false;
        self.interaction_start_pos = POINT { x: 0, y: 0 };
        self.interaction_start_rect = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
    }

    /// 开始选择
    pub fn start_selection(&mut self, x: i32, y: i32) {
        self.selecting = true;
        self.mouse_pressed = true; // 设置鼠标按下状态
        self.start_point = Some((x, y));
        self.end_point = Some((x, y));
        self.update_rect();
    }

    /// 更新终点
    pub fn update_end_point(&mut self, x: i32, y: i32) {
        if self.selecting {
            self.end_point = Some((x, y));
            self.update_rect();
        }
    }

    /// 结束选择
    pub fn end_selection(&mut self, x: i32, y: i32) {
        if self.selecting {
            self.end_point = Some((x, y));
            self.update_rect();

            // 检查选择框是否满足最小尺寸要求
            if let Some(rect) = self.selection_rect {
                let width = rect.right - rect.left;
                let height = rect.bottom - rect.top;

                // 如果选择框太小，清除选择
            if width < MIN_BOX_SIZE || height < MIN_BOX_SIZE {
                    self.selection_rect = None;
                }
            }

            self.selecting = false;
            self.mouse_pressed = false; // 清除鼠标按下状态
        }
    }

    /// 更新选择矩形（从外部设置）
    pub fn update(&mut self, rect: RECT) {
        self.selection_rect = Some(rect);
        self.selecting = false;
    }

    /// 清除选择
    pub fn clear(&mut self) {
        self.selecting = false;
        self.start_point = None;
        self.end_point = None;
        self.selection_rect = None;
    }

    /// 是否正在选择
    pub fn is_selecting(&self) -> bool {
        self.selecting
    }

    /// 获取当前选择矩形
    pub fn get_selection(&self) -> Option<RECT> {
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
        self.interaction_start_pos = POINT { x, y };
    }

    /// 获取交互起始位置
    pub fn get_interaction_start_pos(&self) -> POINT {
        self.interaction_start_pos
    }

    /// 直接设置选择矩形（用于窗口自动高亮）
    pub fn set_selection_rect(&mut self, rect: RECT) {
        self.selection_rect = Some(rect);
    }

    /// 清除选择
    pub fn clear_selection(&mut self) {
        self.selection_rect = None;
        self.selecting = false;
        self.mouse_pressed = false; // 清除鼠标按下状态
        self.start_point = None;
        self.end_point = None;
    }

    /// 更新内部矩形
    fn update_rect(&mut self) {
        if let (Some((x1, y1)), Some((x2, y2))) = (self.start_point, self.end_point) {
            self.selection_rect = Some(RECT {
                left: x1.min(x2),
                top: y1.min(y2),
                right: x1.max(x2),
                bottom: y1.max(y2),
            });
        }
    }

    /// 设置自动高亮选择
    pub fn set_auto_highlight_selection(&mut self, rect: RECT) {
        self.auto_highlight_rect = Some(rect);
        // 自动高亮时也设置为选择状态，但不是手动选择
        self.selection_rect = Some(rect);
    }

    /// 是否有自动高亮
    pub fn has_auto_highlight(&self) -> bool {
        self.auto_highlight_rect.is_some()
    }

    /// 清除自动高亮
    pub fn clear_auto_highlight(&mut self) {
        self.auto_highlight_rect = None;
        // 如果当前选择是自动高亮产生的，也清除选择
        if !self.selecting {
            self.selection_rect = None;
        }
    }

    /// 获取当前有效的选择矩形（优先返回手动选择，其次是自动高亮）
    pub fn get_effective_selection(&self) -> Option<RECT> {
        if self.selecting || (self.selection_rect.is_some() && self.auto_highlight_rect.is_none()) {
            // 手动选择优先
            self.selection_rect
        } else {
            // 自动高亮
            self.auto_highlight_rect
        }
    }

    /// 检测鼠标位置是否在选择框手柄上
    pub fn get_handle_at_position(&self, x: i32, y: i32) -> DragMode {
        // 如果没有选择区域，返回None
        let rect = match self.get_effective_selection() {
            Some(rect) => rect,
            None => return DragMode::None,
        };

        // 使用统一的手柄检测函数
        detect_handle_at_position_unified(x, y, &rect, HandleConfig::Full, true)
    }

    /// 开始选择框交互操作
    pub fn start_interaction(&mut self, x: i32, y: i32, drag_mode: DragMode) {
        self.selection_interaction_mode = SelectionInteractionMode::from_drag_mode(drag_mode);
        self.mouse_pressed = true;
        self.interaction_start_pos = POINT { x, y };
        if let Some(rect) = self.get_effective_selection() {
            self.interaction_start_rect = rect;
        }
    }

    /// 处理选择框交互移动
    pub fn handle_interaction(&mut self, x: i32, y: i32) -> bool {
        if !self.mouse_pressed || self.selection_interaction_mode == SelectionInteractionMode::None
        {
            return false;
        }

        let dx = x - self.interaction_start_pos.x;
        let dy = y - self.interaction_start_pos.y;

        // 根据交互模式确定拖拽模式
        let drag_mode = match &self.selection_interaction_mode {
            SelectionInteractionMode::Moving => DragMode::Moving,
            SelectionInteractionMode::Resizing(resize_mode) => *resize_mode,
            SelectionInteractionMode::Creating => return false,
            SelectionInteractionMode::None => return false,
        };

        // 使用共用的拖拽更新函数
        let new_rect = update_rect_by_drag(drag_mode, dx, dy, self.interaction_start_rect);

        // 检查最小尺寸
        if is_rect_valid(&new_rect, MIN_BOX_SIZE) {
            self.selection_rect = Some(new_rect);
            return true;
        }

        false
    }

    /// 结束选择框交互操作
    pub fn end_interaction(&mut self) {
        self.selection_interaction_mode = SelectionInteractionMode::None;
        self.mouse_pressed = false;
    }

    /// 获取当前选择框交互模式
    pub fn get_interaction_mode(&self) -> &SelectionInteractionMode {
        &self.selection_interaction_mode
    }

    /// 是否正在进行选择框交互
    pub fn is_interacting(&self) -> bool {
        self.mouse_pressed && self.selection_interaction_mode != SelectionInteractionMode::None
    }
}

// ==================== 单元测试 ====================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selection_state_new() {
        let state = SelectionState::new();
        assert!(!state.is_selecting());
        assert!(state.get_selection().is_none());
        assert!(!state.has_selection());
        assert!(!state.is_mouse_pressed());
    }

    #[test]
    fn test_selection_state_start_and_end() {
        let mut state = SelectionState::new();
        
        // 开始选择
        state.start_selection(10, 10);
        assert!(state.is_selecting());
        assert!(state.is_mouse_pressed());
        
        // 更新终点
        state.update_end_point(100, 100);
        
        // 结束选择
        state.end_selection(100, 100);
        assert!(!state.is_selecting());
        assert!(!state.is_mouse_pressed());
        
        // 检查选择矩形
        let rect = state.get_selection().expect("应该有选择矩形");
        assert_eq!(rect.left, 10);
        assert_eq!(rect.top, 10);
        assert_eq!(rect.right, 100);
        assert_eq!(rect.bottom, 100);
    }

    #[test]
    fn test_selection_state_min_size_check() {
        let mut state = SelectionState::new();
        
        // 创建一个太小的选择框
        state.start_selection(10, 10);
        state.end_selection(20, 20); // 10x10，小于 MIN_BOX_SIZE(50)
        
        // 选择框应该被清除
        assert!(state.get_selection().is_none());
    }

    #[test]
    fn test_selection_state_reset() {
        let mut state = SelectionState::new();
        
        // 创建选择
        state.start_selection(10, 10);
        state.end_selection(200, 200);
        assert!(state.has_selection());
        
        // 重置
        state.reset();
        assert!(!state.is_selecting());
        assert!(state.get_selection().is_none());
        assert!(!state.has_selection());
        assert!(!state.is_mouse_pressed());
    }

    #[test]
    fn test_selection_state_clear() {
        let mut state = SelectionState::new();
        
        state.start_selection(10, 10);
        state.end_selection(200, 200);
        
        state.clear();
        assert!(state.get_selection().is_none());
    }

    #[test]
    fn test_selection_state_auto_highlight() {
        let mut state = SelectionState::new();
        
        let rect = RECT {
            left: 100,
            top: 100,
            right: 300,
            bottom: 300,
        };
        
        state.set_auto_highlight_selection(rect);
        assert!(state.has_auto_highlight());
        assert!(state.has_selection());
        
        let effective = state.get_effective_selection().unwrap();
        assert_eq!(effective.left, 100);
        assert_eq!(effective.right, 300);
        
        state.clear_auto_highlight();
        assert!(!state.has_auto_highlight());
    }

    #[test]
    fn test_selection_interaction_mode() {
        // 测试从 DragMode 转换
        assert_eq!(
            SelectionInteractionMode::from_drag_mode(DragMode::None),
            SelectionInteractionMode::None
        );
        assert_eq!(
            SelectionInteractionMode::from_drag_mode(DragMode::Drawing),
            SelectionInteractionMode::Creating
        );
        assert_eq!(
            SelectionInteractionMode::from_drag_mode(DragMode::Moving),
            SelectionInteractionMode::Moving
        );
        
        // 测试调整大小模式
        let resize_mode = SelectionInteractionMode::from_drag_mode(DragMode::ResizingTopLeft);
        assert!(matches!(resize_mode, SelectionInteractionMode::Resizing(_)));
        assert_eq!(resize_mode.get_resize_mode(), Some(DragMode::ResizingTopLeft));
    }
}

// --- InteractionTarget 接口实现 ---
impl InteractionTarget for SelectionState {
    fn hit_test(&self, x: i32, y: i32) -> DragMode {
        self.get_handle_at_position(x, y)
    }
    fn begin_interaction(&mut self, x: i32, y: i32, mode: DragMode) {
        self.start_interaction(x, y, mode)
    }
    fn update_interaction(&mut self, x: i32, y: i32) -> bool {
        self.handle_interaction(x, y)
    }
    fn end_interaction(&mut self) {
        self.end_interaction()
    }
    fn is_interacting(&self) -> bool {
        self.is_interacting()
    }
    fn rect(&self) -> Option<RECT> {
        self.get_selection()
    }
}
