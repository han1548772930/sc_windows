use windows::Win32::Foundation::{POINT, RECT};
use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;

use crate::{Color, Point, Rect};

/// POINT 扩展 trait
pub trait PointExt {
    fn new(x: i32, y: i32) -> Self;
}

impl PointExt for POINT {
    #[inline]
    fn new(x: i32, y: i32) -> Self {
        POINT { x, y }
    }
}

/// RECT 扩展 trait
pub trait RectExt {
    fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self;
    fn width(&self) -> i32;
    fn height(&self) -> i32;
    fn contains(&self, x: i32, y: i32) -> bool;
}

impl RectExt for RECT {
    #[inline]
    fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        RECT {
            left,
            top,
            right,
            bottom,
        }
    }

    #[inline]
    fn width(&self) -> i32 {
        self.right - self.left
    }

    #[inline]
    fn height(&self) -> i32 {
        self.bottom - self.top
    }

    #[inline]
    fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.left && x <= self.right && y >= self.top && y <= self.bottom
    }
}

// ==================== core ↔ Win32 / D2D conversions ====================

impl From<Point> for POINT {
    #[inline]
    fn from(p: Point) -> Self {
        POINT { x: p.x, y: p.y }
    }
}

impl From<POINT> for Point {
    #[inline]
    fn from(p: POINT) -> Self {
        Point { x: p.x, y: p.y }
    }
}

impl From<Rect> for RECT {
    #[inline]
    fn from(r: Rect) -> Self {
        RECT {
            left: r.left,
            top: r.top,
            right: r.right,
            bottom: r.bottom,
        }
    }
}

impl From<RECT> for Rect {
    #[inline]
    fn from(r: RECT) -> Self {
        Rect {
            left: r.left,
            top: r.top,
            right: r.right,
            bottom: r.bottom,
        }
    }
}

impl From<Color> for D2D1_COLOR_F {
    #[inline]
    fn from(c: Color) -> Self {
        D2D1_COLOR_F {
            r: c.r,
            g: c.g,
            b: c.b,
            a: c.a,
        }
    }
}

impl From<D2D1_COLOR_F> for Color {
    #[inline]
    fn from(c: D2D1_COLOR_F) -> Self {
        Color {
            r: c.r,
            g: c.g,
            b: c.b,
            a: c.a,
        }
    }
}
