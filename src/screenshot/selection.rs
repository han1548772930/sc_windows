// 选择区域管理
//
// 负责管理用户的选择区域状态

use crate::constants::{HANDLE_DETECTION_RADIUS, MIN_BOX_SIZE};
use crate::types::DragMode;
use windows::Win32::Foundation::{POINT, RECT};

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
    /// 自动高亮矩形（从原始代码迁移）
    auto_highlight_rect: Option<RECT>,

    // 拖拽相关状态（从原始代码迁移）
    /// 拖拽模式
    drag_mode: DragMode,
    /// 鼠标是否按下
    mouse_pressed: bool,
    /// 拖拽开始位置
    drag_start_pos: POINT,
    /// 拖拽开始时的矩形
    drag_start_rect: RECT,
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
            drag_mode: DragMode::None,
            mouse_pressed: false,
            drag_start_pos: POINT { x: 0, y: 0 },
            drag_start_rect: RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
        }
    }

    /// 重置选择状态（从原始reset_to_initial_state迁移）
    pub fn reset(&mut self) {
        self.selecting = false;
        self.start_point = None;
        self.end_point = None;
        self.selection_rect = None;
        self.auto_highlight_rect = None;
        self.drag_mode = DragMode::None;
        self.mouse_pressed = false;
        self.drag_start_pos = POINT { x: 0, y: 0 };
        self.drag_start_rect = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
    }

    /// 开始选择
    pub fn start_selection(&mut self, x: i32, y: i32) {
        self.selecting = true;
        self.mouse_pressed = true; // 设置鼠标按下状态（从原始代码迁移）
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

    /// 结束选择（按照原始逻辑，包含最小尺寸检查）
    pub fn end_selection(&mut self, x: i32, y: i32) {
        if self.selecting {
            self.end_point = Some((x, y));
            self.update_rect();

            // 检查选择框是否满足最小尺寸要求（从原始代码迁移）
            if let Some(rect) = self.selection_rect {
                let width = rect.right - rect.left;
                let height = rect.bottom - rect.top;

                // 如果选择框太小，清除选择
                if width < crate::constants::MIN_BOX_SIZE || height < crate::constants::MIN_BOX_SIZE
                {
                    self.selection_rect = None;
                }
            }

            self.selecting = false;
            self.mouse_pressed = false; // 清除鼠标按下状态（从原始代码迁移）
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

    /// 检查鼠标是否按下（从原始代码迁移）
    pub fn is_mouse_pressed(&self) -> bool {
        self.mouse_pressed
    }

    /// 设置鼠标按下状态（从原始代码迁移）
    pub fn set_mouse_pressed(&mut self, pressed: bool) {
        self.mouse_pressed = pressed;
    }

    /// 设置拖拽起始位置（从原始代码迁移）
    pub fn set_drag_start_pos(&mut self, x: i32, y: i32) {
        self.drag_start_pos = POINT { x, y };
    }

    /// 获取拖拽起始位置（从原始代码迁移）
    pub fn get_drag_start_pos(&self) -> POINT {
        self.drag_start_pos
    }

    /// 直接设置选择矩形（从原始代码迁移，用于窗口自动高亮）
    pub fn set_selection_rect(&mut self, rect: RECT) {
        self.selection_rect = Some(rect);
    }

    /// 清除选择（从原始代码迁移）
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

    /// 设置自动高亮选择（从原始代码迁移）
    pub fn set_auto_highlight_selection(&mut self, rect: RECT) {
        self.auto_highlight_rect = Some(rect);
        // 自动高亮时也设置为选择状态，但不是手动选择
        self.selection_rect = Some(rect);
    }

    /// 是否有自动高亮（从原始代码迁移）
    pub fn has_auto_highlight(&self) -> bool {
        self.auto_highlight_rect.is_some()
    }

    /// 清除自动高亮（从原始代码迁移）
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

    /// 检测鼠标位置是否在选择框手柄上（从原始代码迁移）
    pub fn get_handle_at_position(&self, x: i32, y: i32) -> DragMode {
        // 如果没有选择区域，返回None
        let rect = match self.get_effective_selection() {
            Some(rect) => rect,
            None => return DragMode::None,
        };

        // 8个调整大小手柄的位置
        let center_x = (rect.left + rect.right) / 2;
        let center_y = (rect.top + rect.bottom) / 2;
        let handles = vec![
            (rect.left, rect.top, DragMode::ResizingTopLeft),
            (center_x, rect.top, DragMode::ResizingTopCenter),
            (rect.right, rect.top, DragMode::ResizingTopRight),
            (rect.right, center_y, DragMode::ResizingMiddleRight),
            (rect.right, rect.bottom, DragMode::ResizingBottomRight),
            (center_x, rect.bottom, DragMode::ResizingBottomCenter),
            (rect.left, rect.bottom, DragMode::ResizingBottomLeft),
            (rect.left, center_y, DragMode::ResizingMiddleLeft),
        ];

        let detection_radius = HANDLE_DETECTION_RADIUS as i32;
        for (hx, hy, mode) in handles.iter() {
            let dx = x - hx;
            let dy = y - hy;
            let distance_sq = dx * dx + dy * dy;
            let radius_sq = detection_radius * detection_radius;

            if distance_sq <= radius_sq {
                return *mode;
            }
        }

        // 检查是否在选择框内部（用于移动）
        let border_margin = 5;
        if x >= rect.left + border_margin
            && x <= rect.right - border_margin
            && y >= rect.top + border_margin
            && y <= rect.bottom - border_margin
        {
            return DragMode::Moving;
        }

        DragMode::None
    }

    /// 开始拖拽操作（从原始代码迁移）
    pub fn start_drag(&mut self, x: i32, y: i32, drag_mode: DragMode) {
        self.drag_mode = drag_mode;
        self.mouse_pressed = true;
        self.drag_start_pos = POINT { x, y };
        if let Some(rect) = self.get_effective_selection() {
            self.drag_start_rect = rect;
        }
    }

    /// 处理拖拽移动（从原始代码迁移）
    pub fn handle_drag(&mut self, x: i32, y: i32) -> bool {
        if !self.mouse_pressed || self.drag_mode == DragMode::None {
            return false;
        }

        let dx = x - self.drag_start_pos.x;
        let dy = y - self.drag_start_pos.y;
        let mut new_rect = self.drag_start_rect;

        match self.drag_mode {
            DragMode::Moving => {
                // 移动整个选择框
                new_rect.left += dx;
                new_rect.top += dy;
                new_rect.right += dx;
                new_rect.bottom += dy;
            }
            DragMode::ResizingTopLeft => {
                new_rect.left += dx;
                new_rect.top += dy;
            }
            DragMode::ResizingTopCenter => {
                new_rect.top += dy;
            }
            DragMode::ResizingTopRight => {
                new_rect.right += dx;
                new_rect.top += dy;
            }
            DragMode::ResizingMiddleRight => {
                new_rect.right += dx;
            }
            DragMode::ResizingBottomRight => {
                new_rect.right += dx;
                new_rect.bottom += dy;
            }
            DragMode::ResizingBottomCenter => {
                new_rect.bottom += dy;
            }
            DragMode::ResizingBottomLeft => {
                new_rect.left += dx;
                new_rect.bottom += dy;
            }
            DragMode::ResizingMiddleLeft => {
                new_rect.left += dx;
            }
            DragMode::DrawingShape => {
                // 绘制图形模式：不修改选择框，由 DrawingManager 处理
                return true;
            }
            _ => return false,
        }

        // 检查最小尺寸
        let width = new_rect.right - new_rect.left;
        let height = new_rect.bottom - new_rect.top;
        if width >= MIN_BOX_SIZE && height >= MIN_BOX_SIZE {
            self.selection_rect = Some(new_rect);
            return true;
        }

        false
    }

    /// 结束拖拽操作（从原始代码迁移）
    pub fn end_drag(&mut self) {
        self.drag_mode = DragMode::None;
        self.mouse_pressed = false;
    }

    /// 获取当前拖拽模式
    pub fn get_drag_mode(&self) -> DragMode {
        self.drag_mode
    }

    /// 是否正在拖拽
    pub fn is_dragging(&self) -> bool {
        self.mouse_pressed && self.drag_mode != DragMode::None
    }
}
