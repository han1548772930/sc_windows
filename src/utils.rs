use std::{ffi::OsStr, iter::once, os::windows::ffi::OsStrExt};

use windows::Win32::Graphics::Direct2D::Common::*;
use windows_numerics::*;

// 辅助函数
pub fn to_wide_chars(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(once(0)).collect()
}

pub fn d2d_point(x: i32, y: i32) -> Vector2 {
    Vector2 {
        X: x as f32,
        Y: y as f32,
    }
}

pub fn d2d_rect(left: i32, top: i32, right: i32, bottom: i32) -> D2D_RECT_F {
    D2D_RECT_F {
        left: left as f32,
        top: top as f32,
        right: right as f32,
        bottom: bottom as f32,
    }
}

// 计算点到线段的距离
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
