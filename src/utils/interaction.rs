// 交互工具函数
//
// 提供选择框和绘图元素的通用交互功能

use crate::constants::HANDLE_DETECTION_RADIUS;
use crate::types::DragMode;
use windows::Win32::Foundation::RECT;

/// 通用的手柄命中检测
///
/// # 参数
/// - `x, y`: 鼠标位置
/// - `rect`: 目标矩形
/// - `allow_moving`: 是否允许通过点击内部移动
///
/// # 返回
/// 返回检测到的拖拽模式
pub fn detect_handle_at_position(x: i32, y: i32, rect: &RECT, allow_moving: bool) -> DragMode {
    let center_x = (rect.left + rect.right) / 2;
    let center_y = (rect.top + rect.bottom) / 2;

    // 8个手柄的位置和对应的拖拽模式
    let handles = [
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

    // 检查是否点击在内部（如果允许移动）
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

/// 通用的拖拽更新逻辑
///
/// # 参数
/// - `drag_mode`: 拖拽模式
/// - `dx, dy`: 鼠标移动距离
/// - `original_rect`: 原始矩形
///
/// # 返回
/// 返回更新后的矩形
pub fn update_rect_by_drag(drag_mode: DragMode, dx: i32, dy: i32, original_rect: RECT) -> RECT {
    let mut new_rect = original_rect;

    match drag_mode {
        DragMode::Moving => {
            // 移动整个矩形
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

/// 检查矩形是否有效（最小尺寸检查）
pub fn is_rect_valid(rect: &RECT, min_size: i32) -> bool {
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    width >= min_size && height >= min_size
}

/// 计算8个手柄的位置
pub fn get_handle_positions(rect: &RECT) -> [(i32, i32); 8] {
    let center_x = (rect.left + rect.right) / 2;
    let center_y = (rect.top + rect.bottom) / 2;

    [
        (rect.left, rect.top),     // 左上
        (center_x, rect.top),      // 上中
        (rect.right, rect.top),    // 右上
        (rect.right, center_y),    // 右中
        (rect.right, rect.bottom), // 右下
        (center_x, rect.bottom),   // 下中
        (rect.left, rect.bottom),  // 左下
        (rect.left, center_y),     // 左中
    ]
}

/// 绘图元素的手柄检测（支持不同元素类型的特殊处理）
///
/// # 参数
/// - `x, y`: 鼠标位置
/// - `rect`: 元素矩形
/// - `tool`: 绘图工具类型
/// - `element_points`: 元素的点集合（用于箭头等特殊元素）
///
/// # 返回
/// 返回检测到的拖拽模式
pub fn detect_element_handle_at_position(
    x: i32,
    y: i32,
    rect: &RECT,
    tool: crate::types::DrawingTool,
    element_points: Option<&[windows::Win32::Foundation::POINT]>,
) -> DragMode {
    let detection_radius = HANDLE_DETECTION_RADIUS as i32;

    // 箭头元素特殊处理：只检查起点和终点
    if tool == crate::types::DrawingTool::Arrow {
        if let Some(points) = element_points {
            if points.len() >= 2 {
                let start = points[0];
                let end = points[1];
                let dx = x - start.x;
                let dy = y - start.y;
                if dx * dx + dy * dy <= detection_radius * detection_radius {
                    return DragMode::ResizingTopLeft;
                }
                let dx2 = x - end.x;
                let dy2 = y - end.y;
                if dx2 * dx2 + dy2 * dy2 <= detection_radius * detection_radius {
                    return DragMode::ResizingBottomRight;
                }
            }
        }
        return DragMode::None;
    }

    let center_x = (rect.left + rect.right) / 2;
    let center_y = (rect.top + rect.bottom) / 2;

    // 文本元素只有4个角的手柄
    let handles = if tool == crate::types::DrawingTool::Text {
        vec![
            (rect.left, rect.top, DragMode::ResizingTopLeft),
            (rect.right, rect.top, DragMode::ResizingTopRight),
            (rect.right, rect.bottom, DragMode::ResizingBottomRight),
            (rect.left, rect.bottom, DragMode::ResizingBottomLeft),
        ]
    } else {
        // 其他元素有8个手柄
        vec![
            (rect.left, rect.top, DragMode::ResizingTopLeft),
            (center_x, rect.top, DragMode::ResizingTopCenter),
            (rect.right, rect.top, DragMode::ResizingTopRight),
            (rect.right, center_y, DragMode::ResizingMiddleRight),
            (rect.right, rect.bottom, DragMode::ResizingBottomRight),
            (center_x, rect.bottom, DragMode::ResizingBottomCenter),
            (rect.left, rect.bottom, DragMode::ResizingBottomLeft),
            (rect.left, center_y, DragMode::ResizingMiddleLeft),
        ]
    };

    for (hx, hy, mode) in handles.into_iter() {
        let dx = x - hx;
        let dy = y - hy;
        if dx * dx + dy * dy <= detection_radius * detection_radius {
            return mode;
        }
    }

    DragMode::None
}
