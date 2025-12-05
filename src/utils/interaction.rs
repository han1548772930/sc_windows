use crate::constants::HANDLE_DETECTION_RADIUS;
use crate::drawing::DragMode;
use windows::Win32::Foundation::RECT;

/// 手柄检测配置
#[derive(Debug, Clone)]
pub enum HandleConfig {
    /// 8个手柄（常规矩形）
    Full,
    /// 4个角手柄（文本框）
    Corners,
    /// 2个端点（箭头）
    Endpoints(Option<&'static [windows::Win32::Foundation::POINT]>),
}

/// 通用的手柄命中检测（合并版本）
///
/// # 参数
/// - `x, y`: 鼠标位置
/// - `rect`: 目标矩形
/// - `config`: 手柄配置
/// - `allow_moving`: 是否允许通过点击内部移动
///
/// # 返回
/// 返回检测到的拖拽模式
pub fn detect_handle_at_position_unified(
    x: i32,
    y: i32,
    rect: &RECT,
    config: HandleConfig,
    allow_moving: bool,
) -> DragMode {
    let detection_radius = HANDLE_DETECTION_RADIUS as i32;

    match config {
        HandleConfig::Endpoints(points_opt) => {
            // 箭头元素特殊处理：只检查起点和终点
            if let Some(points) = points_opt
                && points.len() >= 2 {
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
            return DragMode::None;
        }
        HandleConfig::Corners => {
            // 文本元素只有4个角的手柄
            let handles = [
                (rect.left, rect.top, DragMode::ResizingTopLeft),
                (rect.right, rect.top, DragMode::ResizingTopRight),
                (rect.right, rect.bottom, DragMode::ResizingBottomRight),
                (rect.left, rect.bottom, DragMode::ResizingBottomLeft),
            ];

            for (hx, hy, mode) in handles.iter() {
                let dx = x - hx;
                let dy = y - hy;
                if dx * dx + dy * dy <= detection_radius * detection_radius {
                    return *mode;
                }
            }
        }
        HandleConfig::Full => {
            // 8个手柄的完整检测
            let center_x = (rect.left + rect.right) / 2;
            let center_y = (rect.top + rect.bottom) / 2;

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

            for (hx, hy, mode) in handles.iter() {
                let dx = x - hx;
                let dy = y - hy;
                if dx * dx + dy * dy <= detection_radius * detection_radius {
                    return *mode;
                }
            }
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
