pub mod element;
pub mod history;
pub mod interaction;
pub mod manager;
pub mod types;

#[cfg(feature = "windows")]
pub mod windows;

pub use sc_rendering;

// 重新导出常用类型
pub use element::{Color, DrawingElement, Point, Rect, defaults};
pub use history::ActionHistory;
pub use history::DrawingAction;
pub use interaction::{
    DRAG_THRESHOLD, HANDLE_DETECTION_RADIUS, HandleConfig, calculate_resized_rect,
    calculate_text_proportional_resize, clamp_to_rect, detect_arrow_handle,
    detect_handle_at_position, detect_handle_at_position_with_radius, detect_handle_with_moving,
    detect_handle_with_moving_with_radius, get_handle_positions, is_drag_threshold_exceeded,
    is_rect_valid, point_in_element, update_rect_by_drag,
};
pub use manager::ElementManager;
pub use types::{DragMode, DrawingTool, ElementInteractionMode};
