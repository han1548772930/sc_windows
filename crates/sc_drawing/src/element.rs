use std::sync::atomic::{AtomicU64, Ordering};

use crate::types::DrawingTool;

/// 全局元素 ID 生成器
static NEXT_ELEMENT_ID: AtomicU64 = AtomicU64::new(1);

// ==================== 平台无关类型定义（core） ====================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    #[inline]
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    #[inline]
    pub const fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    #[inline]
    pub fn width(&self) -> i32 {
        self.right - self.left
    }

    #[inline]
    pub fn height(&self) -> i32 {
        self.bottom - self.top
    }

    #[inline]
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.left && x <= self.right && y >= self.top && y <= self.bottom
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    #[inline]
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    #[inline]
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }
}

/// 绘画元素默认值
pub mod defaults {
    pub const LINE_THICKNESS: f32 = 3.0;
    pub const FONT_SIZE: f32 = 20.0;
    pub const MIN_FONT_SIZE: f32 = 8.0;
    pub const MAX_FONT_SIZE: f32 = 200.0;
    pub const FONT_NAME: &str = "Microsoft YaHei";
    pub const FONT_WEIGHT: i32 = 400;
    pub const TEXT_WIDTH: i32 = 120;
    pub const TEXT_HEIGHT: i32 = 32;
    pub const ELEMENT_WIDTH: i32 = 50;
    pub const ELEMENT_HEIGHT: i32 = 30;
    pub const CLICK_TOLERANCE: f32 = 5.0;
    pub const ARROW_HEAD_LENGTH: f64 = 15.0;
    pub const ARROW_HEAD_ANGLE: f64 = 0.5;
    pub const ARROW_HEAD_MARGIN: i32 = 20;
    pub const ARROW_MIN_LENGTH: f64 = 20.0;
}

/// 绘图元素结构体
#[derive(Debug, Clone, PartialEq)]
pub struct DrawingElement {
    /// 元素唯一标识符（用于缓存查找）
    pub id: u64,
    /// 绘图工具类型
    pub tool: DrawingTool,
    /// 点集合（用于定义形状）
    pub points: Vec<Point>,
    /// 边界矩形
    pub rect: Rect,
    /// 颜色
    pub color: Color,
    /// 线条粗细
    pub thickness: f32,
    /// 文本内容
    pub text: String,
    /// 字体大小
    pub font_size: f32,
    /// 字体名称
    pub font_name: String,
    /// 字体粗细
    pub font_weight: i32,
    /// 是否斜体
    pub font_italic: bool,
    /// 是否下划线
    pub font_underline: bool,
    /// 是否删除线
    pub font_strikeout: bool,
    /// 是否被选中
    pub selected: bool,
}

impl Default for DrawingElement {
    fn default() -> Self {
        Self::new(DrawingTool::None)
    }
}

/// 默认颜色（红色）
#[inline]
pub fn default_color() -> Color {
    Color::new(1.0, 0.0, 0.0, 1.0)
}

impl DrawingElement {
    /// 创建新的绘图元素
    pub fn new(tool: DrawingTool) -> Self {
        Self {
            id: NEXT_ELEMENT_ID.fetch_add(1, Ordering::Relaxed),
            tool,
            points: Vec::new(),
            rect: Rect::default(),
            color: default_color(),
            thickness: defaults::LINE_THICKNESS,
            text: String::new(),
            font_size: defaults::FONT_SIZE,
            font_name: defaults::FONT_NAME.to_string(),
            font_weight: defaults::FONT_WEIGHT,
            font_italic: false,
            font_underline: false,
            font_strikeout: false,
            selected: false,
        }
    }

    /// 使用指定颜色创建新元素
    pub fn with_color(tool: DrawingTool, color: Color) -> Self {
        let mut elem = Self::new(tool);
        elem.color = color;
        elem
    }

    /// 获取有效的字体大小
    pub fn get_effective_font_size(&self) -> f32 {
        if self.tool == DrawingTool::Text {
            self.font_size.max(defaults::MIN_FONT_SIZE)
        } else {
            defaults::FONT_SIZE
        }
    }

    /// 设置字体大小（限制在有效范围内）
    pub fn set_font_size(&mut self, size: f32) {
        if self.tool == DrawingTool::Text && (self.font_size - size).abs() > 0.001 {
            self.font_size = size.clamp(defaults::MIN_FONT_SIZE, defaults::MAX_FONT_SIZE);
        }
    }

    /// 更新边界矩形
    pub fn update_bounding_rect(&mut self) {
        if self.points.is_empty() {
            return;
        }

        match self.tool {
            DrawingTool::Text => self.update_text_bounds(),
            DrawingTool::Pen => self.update_pen_bounds(),
            DrawingTool::Rectangle | DrawingTool::Circle => self.update_shape_bounds(),
            DrawingTool::Arrow => self.update_arrow_bounds(),
            _ => self.update_default_bounds(),
        }
    }

    fn update_text_bounds(&mut self) {
        let start = &self.points[0];
        if self.points.len() >= 2 {
            let end = &self.points[1];
            self.rect = Rect {
                left: start.x.min(end.x),
                top: start.y.min(end.y),
                right: start.x.max(end.x),
                bottom: start.y.max(end.y),
            };
        } else {
            self.rect = Rect {
                left: start.x,
                top: start.y,
                right: start.x + defaults::TEXT_WIDTH,
                bottom: start.y + defaults::TEXT_HEIGHT,
            };
        }
    }

    fn update_pen_bounds(&mut self) {
        let mut min_x = self.points[0].x;
        let mut max_x = self.points[0].x;
        let mut min_y = self.points[0].y;
        let mut max_y = self.points[0].y;

        for point in &self.points {
            min_x = min_x.min(point.x);
            max_x = max_x.max(point.x);
            min_y = min_y.min(point.y);
            max_y = max_y.max(point.y);
        }

        let margin = (self.thickness / 2.0) as i32 + 1;
        self.rect = Rect {
            left: min_x - margin,
            top: min_y - margin,
            right: max_x + margin,
            bottom: max_y + margin,
        };
    }

    fn update_shape_bounds(&mut self) {
        if self.points.len() >= 2 {
            let start = &self.points[0];
            let end = &self.points[1];
            self.rect = Rect {
                left: start.x.min(end.x),
                top: start.y.min(end.y),
                right: start.x.max(end.x),
                bottom: start.y.max(end.y),
            };
        }
    }

    fn update_arrow_bounds(&mut self) {
        if self.points.len() >= 2 {
            let start = &self.points[0];
            let end = &self.points[1];
            let margin = defaults::ARROW_HEAD_MARGIN;
            self.rect = Rect {
                left: start.x.min(end.x) - margin,
                top: start.y.min(end.y) - margin,
                right: start.x.max(end.x) + margin,
                bottom: start.y.max(end.y) + margin,
            };
        }
    }

    fn update_default_bounds(&mut self) {
        if !self.points.is_empty() {
            self.rect = Rect {
                left: self.points[0].x,
                top: self.points[0].y,
                right: self.points[0].x + defaults::ELEMENT_WIDTH,
                bottom: self.points[0].y + defaults::ELEMENT_HEIGHT,
            };
        }
    }

    /// 检查点是否在元素内
    pub fn contains_point(&self, x: i32, y: i32) -> bool {
        match self.tool {
            DrawingTool::Pen => self.contains_point_pen(x, y),
            DrawingTool::Rectangle | DrawingTool::Circle => self.contains_point_shape(x, y),
            DrawingTool::Arrow => self.contains_point_arrow(x, y),
            DrawingTool::Text => self.rect.contains(x, y),
            _ => false,
        }
    }

    fn contains_point_pen(&self, x: i32, y: i32) -> bool {
        if self.points.len() < 2 {
            return false;
        }

        for i in 0..self.points.len() - 1 {
            let p1 = &self.points[i];
            let p2 = &self.points[i + 1];

            let distance = point_to_line_distance(x, y, p1.x, p1.y, p2.x, p2.y);
            if distance <= (self.thickness + defaults::CLICK_TOLERANCE) as f64 {
                return true;
            }
        }
        false
    }

    fn contains_point_shape(&self, x: i32, y: i32) -> bool {
        if self.points.len() >= 2 {
            let start = &self.points[0];
            let end = &self.points[1];
            let left = start.x.min(end.x);
            let right = start.x.max(end.x);
            let top = start.y.min(end.y);
            let bottom = start.y.max(end.y);

            x >= left && x <= right && y >= top && y <= bottom
        } else {
            false
        }
    }

    fn contains_point_arrow(&self, x: i32, y: i32) -> bool {
        if self.points.len() < 2 {
            return false;
        }

        let start = &self.points[0];
        let end = &self.points[1];

        // 检查主线段
        let distance = point_to_line_distance(x, y, start.x, start.y, end.x, end.y);
        if distance <= (self.thickness + defaults::CLICK_TOLERANCE) as f64 {
            return true;
        }

        // 检查箭头头部
        let dx = end.x - start.x;
        let dy = end.y - start.y;
        let length = ((dx * dx + dy * dy) as f64).sqrt();

        if length > defaults::ARROW_MIN_LENGTH {
            let arrow_length = defaults::ARROW_HEAD_LENGTH;
            let arrow_angle = defaults::ARROW_HEAD_ANGLE;
            let unit_x = dx as f64 / length;
            let unit_y = dy as f64 / length;

            let wing1_x = end.x
                - (arrow_length * (unit_x * arrow_angle.cos() + unit_y * arrow_angle.sin())) as i32;
            let wing1_y = end.y
                - (arrow_length * (unit_y * arrow_angle.cos() - unit_x * arrow_angle.sin())) as i32;

            let wing2_x = end.x
                - (arrow_length * (unit_x * arrow_angle.cos() - unit_y * arrow_angle.sin())) as i32;
            let wing2_y = end.y
                - (arrow_length * (unit_y * arrow_angle.cos() + unit_x * arrow_angle.sin())) as i32;

            let d1 = point_to_line_distance(x, y, end.x, end.y, wing1_x, wing1_y);
            let d2 = point_to_line_distance(x, y, end.x, end.y, wing2_x, wing2_y);

            if d1 <= (self.thickness + defaults::CLICK_TOLERANCE) as f64
                || d2 <= (self.thickness + defaults::CLICK_TOLERANCE) as f64
            {
                return true;
            }
        }

        false
    }

    /// 调整元素大小
    pub fn resize(&mut self, new_rect: Rect) {
        match self.tool {
            DrawingTool::Rectangle | DrawingTool::Circle => self.resize_two_point_shape(new_rect),
            DrawingTool::Arrow => self.resize_arrow(new_rect),
            DrawingTool::Pen => self.resize_freeform(new_rect),
            DrawingTool::Text => self.resize_text(new_rect),
            _ => {}
        }
        self.rect = new_rect;
    }

    fn resize_two_point_shape(&mut self, new_rect: Rect) {
        if self.points.len() >= 2 {
            self.points[0] = Point::new(new_rect.left, new_rect.top);
            self.points[1] = Point::new(new_rect.right, new_rect.bottom);
        }
    }

    fn resize_arrow(&mut self, new_rect: Rect) {
        if self.points.len() < 2 {
            return;
        }

        let old_width = self.rect.width().max(1);
        let old_height = self.rect.height().max(1);
        let new_width = new_rect.width();
        let new_height = new_rect.height();

        let old_start = self.points[0];
        let old_end = self.points[1];

        let start_rel_x = (old_start.x - self.rect.left) as f64 / old_width as f64;
        let start_rel_y = (old_start.y - self.rect.top) as f64 / old_height as f64;
        let end_rel_x = (old_end.x - self.rect.left) as f64 / old_width as f64;
        let end_rel_y = (old_end.y - self.rect.top) as f64 / old_height as f64;

        self.points[0] = Point::new(
            new_rect.left + (start_rel_x * new_width as f64) as i32,
            new_rect.top + (start_rel_y * new_height as f64) as i32,
        );
        self.points[1] = Point::new(
            new_rect.left + (end_rel_x * new_width as f64) as i32,
            new_rect.top + (end_rel_y * new_height as f64) as i32,
        );
    }

    fn resize_freeform(&mut self, new_rect: Rect) {
        let old_w = self.rect.width().max(1) as f64;
        let old_h = self.rect.height().max(1) as f64;
        let scale_x = new_rect.width() as f64 / old_w;
        let scale_y = new_rect.height() as f64 / old_h;

        for point in &mut self.points {
            let rel_x = (point.x - self.rect.left) as f64;
            let rel_y = (point.y - self.rect.top) as f64;
            point.x = new_rect.left + (rel_x * scale_x) as i32;
            point.y = new_rect.top + (rel_y * scale_y) as i32;
        }
    }

    fn resize_text(&mut self, new_rect: Rect) {
        if self.points.is_empty() {
            return;
        }
        self.points[0] = Point::new(new_rect.left, new_rect.top);
        if self.points.len() >= 2 {
            self.points[1] = Point::new(new_rect.right, new_rect.bottom);
        } else {
            self.points
                .push(Point::new(new_rect.right, new_rect.bottom));
        }
    }

    /// 移动元素
    pub fn move_by(&mut self, dx: i32, dy: i32) {
        for point in &mut self.points {
            point.x += dx;
            point.y += dy;
        }
        self.rect.left += dx;
        self.rect.right += dx;
        self.rect.top += dy;
        self.rect.bottom += dy;
    }

    /// 获取边界矩形
    pub fn get_bounding_rect(&self) -> Rect {
        match self.tool {
            DrawingTool::Rectangle
            | DrawingTool::Circle
            | DrawingTool::Arrow
            | DrawingTool::Text => self.rect,
            DrawingTool::Pen => {
                if self.points.is_empty() {
                    return Rect::default();
                }
                let mut min_x = self.points[0].x;
                let mut max_x = self.points[0].x;
                let mut min_y = self.points[0].y;
                let mut max_y = self.points[0].y;

                for point in &self.points {
                    min_x = min_x.min(point.x);
                    max_x = max_x.max(point.x);
                    min_y = min_y.min(point.y);
                    max_y = max_y.max(point.y);
                }

                Rect::new(min_x, min_y, max_x, max_y)
            }
            _ => Rect::default(),
        }
    }

    /// 添加点
    pub fn add_point(&mut self, x: i32, y: i32) {
        self.points.push(Point::new(x, y));
    }

    /// 设置第二个点（用于形状绘制）
    pub fn set_end_point(&mut self, x: i32, y: i32) {
        if self.points.len() >= 2 {
            self.points[1] = Point::new(x, y);
        } else {
            self.points.push(Point::new(x, y));
        }
    }
}

/// 计算点到线段的距离
fn point_to_line_distance(px: i32, py: i32, x1: i32, y1: i32, x2: i32, y2: i32) -> f64 {
    let px = px as f64;
    let py = py as f64;
    let x1 = x1 as f64;
    let y1 = y1 as f64;
    let x2 = x2 as f64;
    let y2 = y2 as f64;

    let dx = x2 - x1;
    let dy = y2 - y1;
    let len_sq = dx * dx + dy * dy;

    if len_sq == 0.0 {
        return ((px - x1).powi(2) + (py - y1).powi(2)).sqrt();
    }

    let t = ((px - x1) * dx + (py - y1) * dy) / len_sq;
    let t = t.clamp(0.0, 1.0);

    let closest_x = x1 + t * dx;
    let closest_y = y1 + t * dy;

    ((px - closest_x).powi(2) + (py - closest_y).powi(2)).sqrt()
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_element_new() {
        let element = super::DrawingElement::new(super::DrawingTool::Rectangle);
        assert_eq!(element.tool, super::DrawingTool::Rectangle);
        assert!(element.points.is_empty());
        assert_eq!(element.thickness, super::defaults::LINE_THICKNESS);
        assert!(!element.selected);
    }

    #[test]
    fn test_element_contains_point_rectangle() {
        let mut element = super::DrawingElement::new(super::DrawingTool::Rectangle);
        element.points = vec![super::Point::new(10, 10), super::Point::new(100, 100)];
        element.update_bounding_rect();

        assert!(element.contains_point(50, 50));
        assert!(element.contains_point(10, 10));
        assert!(element.contains_point(100, 100));
        assert!(!element.contains_point(5, 5));
        assert!(!element.contains_point(150, 150));
    }

    #[test]
    fn test_element_move_by() {
        let mut element = super::DrawingElement::new(super::DrawingTool::Rectangle);
        element.points = vec![super::Point::new(10, 10), super::Point::new(100, 100)];
        element.rect = super::Rect::new(10, 10, 100, 100);

        element.move_by(50, 30);

        assert_eq!(element.points[0].x, 60);
        assert_eq!(element.points[0].y, 40);
        assert_eq!(element.points[1].x, 150);
        assert_eq!(element.points[1].y, 130);
        assert_eq!(element.rect.left, 60);
        assert_eq!(element.rect.top, 40);
    }

    #[test]
    fn test_element_resize() {
        let mut element = super::DrawingElement::new(super::DrawingTool::Rectangle);
        element.points = vec![super::Point::new(0, 0), super::Point::new(100, 100)];
        element.rect = super::Rect::new(0, 0, 100, 100);

        let new_rect = super::Rect::new(50, 50, 200, 200);
        element.resize(new_rect);

        assert_eq!(element.rect, new_rect);
        assert_eq!(element.points[0].x, 50);
        assert_eq!(element.points[0].y, 50);
    }

    #[test]
    fn test_font_size() {
        let mut element = super::DrawingElement::new(super::DrawingTool::Text);

        element.set_font_size(32.0);
        assert_eq!(element.font_size, 32.0);

        element.set_font_size(300.0);
        assert_eq!(element.font_size, super::defaults::MAX_FONT_SIZE);

        element.set_font_size(2.0);
        assert_eq!(element.font_size, super::defaults::MIN_FONT_SIZE);
    }

    #[test]
    fn test_pen_bounds() {
        let mut element = super::DrawingElement::new(super::DrawingTool::Pen);
        element.points = vec![
            super::Point::new(10, 20),
            super::Point::new(50, 10),
            super::Point::new(30, 60),
        ];
        element.thickness = 2.0;

        element.update_bounding_rect();

        let margin = (element.thickness / 2.0) as i32 + 1;
        assert_eq!(element.rect.left, 10 - margin);
        assert_eq!(element.rect.top, 10 - margin);
        assert_eq!(element.rect.right, 50 + margin);
        assert_eq!(element.rect.bottom, 60 + margin);
    }
}
