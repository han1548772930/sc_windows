// 工具函数模块

use std::{ffi::OsStr, iter::once, os::windows::ffi::OsStrExt};

pub mod command_helpers;
pub mod d2d_helpers;
pub mod interaction;
pub mod win_api;

// 重新导出常用函数
pub use command_helpers::{execute_and_hide, execute_save_operation, execute_with_error_handling};
pub use d2d_helpers::*;
pub use interaction::*;

// ==================== 字符串转换 ====================

/// 将字符串转换为Windows API所需的宽字符格式
pub fn to_wide_chars(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(once(0)).collect()
}

// ==================== Windows消息处理 ====================

/// 从LPARAM中提取鼠标坐标
#[inline]
pub fn extract_mouse_coords(lparam: windows::Win32::Foundation::LPARAM) -> (i32, i32) {
    let x = (lparam.0 & 0xFFFF) as i16 as i32;
    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
    (x, y)
}

/// 检查鼠标移动是否超过拖拽阈值
#[inline]
pub fn is_drag_threshold_exceeded(
    start_x: i32,
    start_y: i32,
    current_x: i32,
    current_y: i32,
) -> bool {
    let dx = (current_x - start_x).abs();
    let dy = (current_y - start_y).abs();
    dx > crate::constants::DRAG_THRESHOLD || dy > crate::constants::DRAG_THRESHOLD
}


// ==================== 坐标和边界处理 ====================

/// 将坐标限制在矩形范围内
#[inline]
pub fn clamp_to_rect(x: i32, y: i32, rect: &windows::Win32::Foundation::RECT) -> (i32, i32) {
    (
        x.max(rect.left).min(rect.right),
        y.max(rect.top).min(rect.bottom),
    )
}

/// 将矩形限制在屏幕范围内
#[inline]
pub fn clamp_rect_to_screen(
    rect: windows::Win32::Foundation::RECT,
    screen_width: i32,
    screen_height: i32,
) -> windows::Win32::Foundation::RECT {
    windows::Win32::Foundation::RECT {
        left: rect.left.max(0),
        top: rect.top.max(0),
        right: rect.right.min(screen_width),
        bottom: rect.bottom.min(screen_height),
    }
}

/// 检查点是否在矩形内
#[inline]
pub fn point_in_rect(x: i32, y: i32, rect: &windows::Win32::Foundation::RECT) -> bool {
    x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom
}

// ==================== 几何计算 ====================

/// 计算点到线段的距离
pub fn point_to_line_distance(px: i32, py: i32, x1: i32, y1: i32, x2: i32, y2: i32) -> f64 {
    let px = px as f64;
    let py = py as f64;
    let x1 = x1 as f64;
    let y1 = y1 as f64;
    let x2 = x2 as f64;
    let y2 = y2 as f64;

    let a = px - x1;
    let b = py - y1;
    let c = x2 - x1;
    let d = y2 - y1;

    let dot = a * c + b * d;
    let len_sq = c * c + d * d;

    if len_sq == 0.0 {
        return ((px - x1).powi(2) + (py - y1).powi(2)).sqrt();
    }

    let param = dot / len_sq;

    let (xx, yy) = if param < 0.0 {
        (x1, y1)
    } else if param > 1.0 {
        (x2, y2)
    } else {
        (x1 + param * c, y1 + param * d)
    };

    ((px - xx).powi(2) + (py - yy).powi(2)).sqrt()
}
