use crate::{DragMode, DrawingElement, DrawingTool, Point, Rect, defaults};

pub const DRAG_THRESHOLD: i32 = 3;
/// Explicit alias for drawing drag threshold (in pixels).
pub const DRAWING_DRAG_THRESHOLD: i32 = DRAG_THRESHOLD;

pub const HANDLE_DETECTION_RADIUS: i32 = 8;

#[inline]
pub fn is_drag_threshold_exceeded(
    start_x: i32,
    start_y: i32,
    current_x: i32,
    current_y: i32,
) -> bool {
    let dx = (current_x - start_x).abs();
    let dy = (current_y - start_y).abs();
    dx > DRAG_THRESHOLD || dy > DRAG_THRESHOLD
}

#[inline]
pub fn clamp_to_rect(x: i32, y: i32, rect: &Rect) -> (i32, i32) {
    (
        x.clamp(rect.left, rect.right),
        y.clamp(rect.top, rect.bottom),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandleConfig {
    Full,
    Corners,
    Endpoints,
    None,
}

impl HandleConfig {
    pub fn for_tool(tool: DrawingTool) -> Self {
        match tool {
            DrawingTool::Arrow => HandleConfig::Endpoints,
            DrawingTool::Text => HandleConfig::Corners,
            DrawingTool::Pen => HandleConfig::None,
            _ => HandleConfig::Full,
        }
    }
}

/// Detect which handle (if any) is hit at the given point, using a configurable radius.
/// This is useful when different UI elements want different hit slop values.
pub fn detect_handle_at_position_with_radius(
    x: i32,
    y: i32,
    rect: &Rect,
    config: HandleConfig,
    radius: i32,
) -> DragMode {
    let radius_sq = radius * radius;

    let (left, top, right, bottom) = (rect.left, rect.top, rect.right, rect.bottom);
    let mid_x = (left + right) / 2;
    let mid_y = (top + bottom) / 2;

    // Check whether (x, y) is within `radius` of (px, py).
    let is_near = |px: i32, py: i32| -> bool {
        let dx = x - px;
        let dy = y - py;
        dx * dx + dy * dy <= radius_sq
    };

    match config {
        HandleConfig::Full => {
            if is_near(left, top) {
                DragMode::ResizingTopLeft
            } else if is_near(mid_x, top) {
                DragMode::ResizingTopCenter
            } else if is_near(right, top) {
                DragMode::ResizingTopRight
            } else if is_near(right, mid_y) {
                DragMode::ResizingMiddleRight
            } else if is_near(right, bottom) {
                DragMode::ResizingBottomRight
            } else if is_near(mid_x, bottom) {
                DragMode::ResizingBottomCenter
            } else if is_near(left, bottom) {
                DragMode::ResizingBottomLeft
            } else if is_near(left, mid_y) {
                DragMode::ResizingMiddleLeft
            } else {
                DragMode::None
            }
        }
        HandleConfig::Corners => {
            if is_near(left, top) {
                DragMode::ResizingTopLeft
            } else if is_near(right, top) {
                DragMode::ResizingTopRight
            } else if is_near(right, bottom) {
                DragMode::ResizingBottomRight
            } else if is_near(left, bottom) {
                DragMode::ResizingBottomLeft
            } else {
                DragMode::None
            }
        }
        HandleConfig::Endpoints => {
            // Endpoints (e.g. arrow): default to rect corners.
            if is_near(left, top) {
                DragMode::ResizingTopLeft
            } else if is_near(right, bottom) {
                DragMode::ResizingBottomRight
            } else {
                DragMode::None
            }
        }
        HandleConfig::None => DragMode::None,
    }
}

/// # Arguments
/// # Returns
pub fn detect_handle_at_position(x: i32, y: i32, rect: &Rect, config: HandleConfig) -> DragMode {
    detect_handle_at_position_with_radius(x, y, rect, config, HANDLE_DETECTION_RADIUS)
}

pub fn detect_arrow_handle(x: i32, y: i32, points: &[Point]) -> DragMode {
    if points.len() < 2 {
        return DragMode::None;
    }

    let radius_sq = HANDLE_DETECTION_RADIUS * HANDLE_DETECTION_RADIUS;

    let start = &points[0];
    let end = &points[1];

    let dx1 = x - start.x;
    let dy1 = y - start.y;
    if dx1 * dx1 + dy1 * dy1 <= radius_sq {
        return DragMode::ResizingTopLeft;
    }

    let dx2 = x - end.x;
    let dy2 = y - end.y;
    if dx2 * dx2 + dy2 * dy2 <= radius_sq {
        return DragMode::ResizingBottomRight;
    }

    DragMode::None
}

/// # Arguments
pub fn calculate_resized_rect(start_rect: Rect, mode: DragMode, dx: i32, dy: i32) -> Rect {
    let mut new_rect = start_rect;

    match mode {
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
        _ => {}
    }

    new_rect
}

/// # Arguments
/// # Returns
pub fn calculate_text_proportional_resize(
    start_rect: Rect,
    start_font_size: f32,
    mode: DragMode,
    dx: i32,
    dy: i32,
) -> (Rect, f32) {
    calculate_text_proportional_resize_with_min_font(
        start_rect,
        start_font_size,
        mode,
        dx,
        dy,
        defaults::MIN_FONT_SIZE,
    )
}

/// Calculate proportional resize for text with an explicit minimum font size.
pub fn calculate_text_proportional_resize_with_min_font(
    start_rect: Rect,
    start_font_size: f32,
    mode: DragMode,
    dx: i32,
    dy: i32,
    min_font_size: f32,
) -> (Rect, f32) {
    let original_width = (start_rect.right - start_rect.left).max(1);
    let original_height = (start_rect.bottom - start_rect.top).max(1);

    let (scale_x, scale_y) = match mode {
        DragMode::ResizingTopLeft => (
            (original_width - dx) as f32 / original_width as f32,
            (original_height - dy) as f32 / original_height as f32,
        ),
        DragMode::ResizingTopRight => (
            (original_width + dx) as f32 / original_width as f32,
            (original_height - dy) as f32 / original_height as f32,
        ),
        DragMode::ResizingBottomRight => (
            (original_width + dx) as f32 / original_width as f32,
            (original_height + dy) as f32 / original_height as f32,
        ),
        DragMode::ResizingBottomLeft => (
            (original_width - dx) as f32 / original_width as f32,
            (original_height + dy) as f32 / original_height as f32,
        ),
        _ => return (start_rect, start_font_size),
    };

    let min_scale_for_font = if start_font_size > 0.0 {
        (min_font_size / start_font_size).min(1.0)
    } else {
        1.0
    };
    let scale = ((scale_x + scale_y) / 2.0).max(0.7).max(min_scale_for_font);

    let new_font_size = (start_font_size * scale).max(min_font_size);

    let new_width = (original_width as f32 * scale) as i32;
    let new_height = (original_height as f32 * scale) as i32;

    let new_rect = match mode {
        DragMode::ResizingTopLeft => Rect {
            left: start_rect.right - new_width,
            top: start_rect.bottom - new_height,
            right: start_rect.right,
            bottom: start_rect.bottom,
        },
        DragMode::ResizingTopRight => Rect {
            left: start_rect.left,
            top: start_rect.bottom - new_height,
            right: start_rect.left + new_width,
            bottom: start_rect.bottom,
        },
        DragMode::ResizingBottomRight => Rect {
            left: start_rect.left,
            top: start_rect.top,
            right: start_rect.left + new_width,
            bottom: start_rect.top + new_height,
        },
        DragMode::ResizingBottomLeft => Rect {
            left: start_rect.right - new_width,
            top: start_rect.top,
            right: start_rect.right,
            bottom: start_rect.top + new_height,
        },
        _ => start_rect,
    };

    (new_rect, new_font_size)
}

pub fn point_in_element(x: i32, y: i32, element: &DrawingElement) -> bool {
    match element.tool {
        DrawingTool::Arrow => {
            if element.points.len() >= 2 {
                point_near_line_segment(
                    x,
                    y,
                    element.points[0].x,
                    element.points[0].y,
                    element.points[1].x,
                    element.points[1].y,
                    (element.thickness + 5.0) as i32,
                )
            } else {
                false
            }
        }
        DrawingTool::Pen => {
            for i in 0..element.points.len().saturating_sub(1) {
                if point_near_line_segment(
                    x,
                    y,
                    element.points[i].x,
                    element.points[i].y,
                    element.points[i + 1].x,
                    element.points[i + 1].y,
                    (element.thickness + 5.0) as i32,
                ) {
                    return true;
                }
            }
            false
        }
        _ => {
            x >= element.rect.left
                && x <= element.rect.right
                && y >= element.rect.top
                && y <= element.rect.bottom
        }
    }
}

fn point_near_line_segment(
    px: i32,
    py: i32,
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
    threshold: i32,
) -> bool {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let len_sq = dx * dx + dy * dy;

    if len_sq == 0 {
        let d = (px - x1) * (px - x1) + (py - y1) * (py - y1);
        return d <= threshold * threshold;
    }

    let t = (((px - x1) * dx + (py - y1) * dy) as f32 / len_sq as f32).clamp(0.0, 1.0);

    let nearest_x = x1 as f32 + t * dx as f32;
    let nearest_y = y1 as f32 + t * dy as f32;

    let dist_sq = (px as f32 - nearest_x).powi(2) + (py as f32 - nearest_y).powi(2);
    dist_sq <= (threshold as f32).powi(2)
}

pub fn update_rect_by_drag(drag_mode: DragMode, dx: i32, dy: i32, original_rect: Rect) -> Rect {
    let mut new_rect = original_rect;

    match drag_mode {
        DragMode::Moving => {
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
        _ => {}
    }

    new_rect
}

#[inline]
pub fn is_rect_valid(rect: &Rect, min_size: i32) -> bool {
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    width >= min_size && height >= min_size
}

pub fn get_handle_positions(rect: &Rect) -> [(i32, i32); 8] {
    let center_x = (rect.left + rect.right) / 2;
    let center_y = (rect.top + rect.bottom) / 2;

    [
        (rect.left, rect.top),
        (center_x, rect.top),
        (rect.right, rect.top),
        (rect.right, center_y),
        (rect.right, rect.bottom),
        (center_x, rect.bottom),
        (rect.left, rect.bottom),
        (rect.left, center_y),
    ]
}

pub fn detect_handle_with_moving_with_radius(
    x: i32,
    y: i32,
    rect: &Rect,
    config: HandleConfig,
    radius: i32,
    allow_moving: bool,
) -> DragMode {
    // Try handles first.
    let handle_result = detect_handle_at_position_with_radius(x, y, rect, config, radius);
    if handle_result != DragMode::None {
        return handle_result;
    }

    // Then fall back to inside-click -> Moving (optional).
    if allow_moving {
        let border_margin = 5;
        if x >= rect.left + border_margin
            && x <= rect.right - border_margin
            && y >= rect.top + border_margin
            && y <= rect.bottom - border_margin
        {
            return DragMode::Moving;
        }
    }

    DragMode::None
}

pub fn detect_handle_with_moving(
    x: i32,
    y: i32,
    rect: &Rect,
    config: HandleConfig,
    allow_moving: bool,
) -> DragMode {
    detect_handle_with_moving_with_radius(x, y, rect, config, HANDLE_DETECTION_RADIUS, allow_moving)
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_drag_threshold() {
        assert!(!super::is_drag_threshold_exceeded(0, 0, 2, 2));
        assert!(super::is_drag_threshold_exceeded(0, 0, 4, 0));
        assert!(super::is_drag_threshold_exceeded(0, 0, 0, 4));
    }

    #[test]
    fn test_clamp_to_rect() {
        let rect = super::Rect::new(10, 10, 100, 100);
        assert_eq!(super::clamp_to_rect(50, 50, &rect), (50, 50));
        assert_eq!(super::clamp_to_rect(0, 0, &rect), (10, 10));
        assert_eq!(super::clamp_to_rect(200, 200, &rect), (100, 100));
    }

    #[test]
    fn test_detect_handle_full() {
        let rect = super::Rect::new(0, 0, 100, 100);

        assert_eq!(
            super::detect_handle_at_position(2, 2, &rect, super::HandleConfig::Full),
            super::DragMode::ResizingTopLeft
        );

        assert_eq!(
            super::detect_handle_at_position(98, 98, &rect, super::HandleConfig::Full),
            super::DragMode::ResizingBottomRight
        );

        assert_eq!(
            super::detect_handle_at_position(50, 50, &rect, super::HandleConfig::Full),
            super::DragMode::None
        );
    }

    #[test]
    fn test_update_rect_by_drag() {
        let rect = super::Rect::new(10, 10, 100, 100);

        let moved = super::update_rect_by_drag(super::DragMode::Moving, 5, 5, rect);
        assert_eq!(moved, super::Rect::new(15, 15, 105, 105));

        let resized =
            super::update_rect_by_drag(super::DragMode::ResizingBottomRight, 10, 10, rect);
        assert_eq!(resized, super::Rect::new(10, 10, 110, 110));
    }
}
