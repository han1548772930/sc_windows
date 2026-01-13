/// 颜色定义
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    /// 创建不透明颜色
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    /// 创建带透明度的颜色
    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// 黑色
    pub const BLACK: Color = Color::rgb(0.0, 0.0, 0.0);
    /// 白色
    pub const WHITE: Color = Color::rgb(1.0, 1.0, 1.0);
    /// 透明
    pub const TRANSPARENT: Color = Color::rgba(0.0, 0.0, 0.0, 0.0);
}

impl Default for Color {
    fn default() -> Self {
        Self::BLACK
    }
}

/// 点定义
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub const ZERO: Point = Point::new(0.0, 0.0);
}

/// 矩形定义
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rectangle {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rectangle {
    /// 创建新的矩形
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// 从左上角和右下角坐标创建矩形
    pub fn from_bounds(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            x: left,
            y: top,
            width: right - left,
            height: bottom - top,
        }
    }

    /// 获取右边界
    #[inline]
    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    /// 获取下边界
    #[inline]
    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }

    /// 检查点是否在矩形内
    #[inline]
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x <= self.right() && y >= self.y && y <= self.bottom()
    }

    /// 检查两个矩形是否相交
    pub fn intersects(&self, other: &Rectangle) -> bool {
        self.x < other.right()
            && self.right() > other.x
            && self.y < other.bottom()
            && self.bottom() > other.y
    }

    /// 合并两个矩形
    pub fn union(&self, other: &Rectangle) -> Rectangle {
        let left = self.x.min(other.x);
        let top = self.y.min(other.y);
        let right = self.right().max(other.right());
        let bottom = self.bottom().max(other.bottom());

        Rectangle::from_bounds(left, top, right, bottom)
    }

    /// 扩展矩形
    pub fn expand(&self, margin: f32) -> Rectangle {
        Rectangle {
            x: self.x - margin,
            y: self.y - margin,
            width: self.width + margin * 2.0,
            height: self.height + margin * 2.0,
        }
    }

    /// 零矩形
    pub const ZERO: Rectangle = Rectangle::new(0.0, 0.0, 0.0, 0.0);
}

/// 文本样式
#[derive(Debug, Clone)]
pub struct TextStyle {
    pub font_size: f32,
    pub color: Color,
    pub font_family: String,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font_size: 14.0,
            color: Color::BLACK,
            font_family: "Microsoft YaHei".to_string(),
        }
    }
}

/// 绘制样式
#[derive(Debug, Clone)]
pub struct DrawStyle {
    pub stroke_color: Color,
    pub fill_color: Option<Color>,
    pub stroke_width: f32,
}

impl Default for DrawStyle {
    fn default() -> Self {
        Self {
            stroke_color: Color::BLACK,
            fill_color: None,
            stroke_width: 1.0,
        }
    }
}

/// 平台位图ID（由渲染后端管理的位图句柄）
pub type BitmapId = u64;

#[cfg(test)]
mod tests {
    #[test]
    fn test_color_creation() {
        let c = super::Color::rgb(1.0, 0.5, 0.0);
        assert_eq!(c.r, 1.0);
        assert_eq!(c.g, 0.5);
        assert_eq!(c.b, 0.0);
        assert_eq!(c.a, 1.0);
    }

    #[test]
    fn test_rectangle_bounds() {
        let r = super::Rectangle::new(10.0, 20.0, 100.0, 50.0);
        assert_eq!(r.right(), 110.0);
        assert_eq!(r.bottom(), 70.0);
    }

    #[test]
    fn test_rectangle_contains() {
        let r = super::Rectangle::new(0.0, 0.0, 100.0, 100.0);
        assert!(r.contains(50.0, 50.0));
        assert!(!r.contains(150.0, 50.0));
    }

    #[test]
    fn test_rectangle_intersects() {
        let r1 = super::Rectangle::new(0.0, 0.0, 100.0, 100.0);
        let r2 = super::Rectangle::new(50.0, 50.0, 100.0, 100.0);
        let r3 = super::Rectangle::new(200.0, 200.0, 100.0, 100.0);

        assert!(r1.intersects(&r2));
        assert!(!r1.intersects(&r3));
    }
}
