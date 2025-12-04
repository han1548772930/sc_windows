//! 绘图相关类型定义
//!
//! 包含绘图工具、绘图元素和拖拽模式等核心类型。

use std::cell::RefCell;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::ID2D1PathGeometry;
use windows::Win32::Graphics::DirectWrite::IDWriteTextLayout;

use crate::utils::*;

/// 绘图工具类型
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DrawingTool {
    None,
    Rectangle,
    Circle,
    Arrow,
    Pen,
    Text,
}

/// 拖拽模式枚举
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DragMode {
    None,
    Drawing,              // 绘制选择框
    DrawingShape,         // 绘制图形元素
    Moving,               // 移动选择框
    MovingElement,        // 移动绘图元素
    ResizingTopLeft,      // 调整大小 - 左上角
    ResizingTopCenter,    // 调整大小 - 上边中心
    ResizingTopRight,     // 调整大小 - 右上角
    ResizingMiddleRight,  // 调整大小 - 右边中心
    ResizingBottomRight,  // 调整大小 - 右下角
    ResizingBottomCenter, // 调整大小 - 下边中心
    ResizingBottomLeft,   // 调整大小 - 左下角
    ResizingMiddleLeft,   // 调整大小 - 左边中心
}

/// 元素交互模式（用于绘图模块内部）
#[derive(Debug, Clone, PartialEq)]
pub enum ElementInteractionMode {
    None,
    Drawing,
    MovingElement,
    ResizingElement(DragMode),
}

impl ElementInteractionMode {
    pub fn from_drag_mode(drag_mode: DragMode) -> Self {
        match drag_mode {
            DragMode::None => ElementInteractionMode::None,
            DragMode::DrawingShape => ElementInteractionMode::Drawing,
            DragMode::MovingElement => ElementInteractionMode::MovingElement,
            DragMode::ResizingTopLeft
            | DragMode::ResizingTopCenter
            | DragMode::ResizingTopRight
            | DragMode::ResizingMiddleRight
            | DragMode::ResizingBottomRight
            | DragMode::ResizingBottomCenter
            | DragMode::ResizingBottomLeft
            | DragMode::ResizingMiddleLeft => ElementInteractionMode::ResizingElement(drag_mode),
            _ => ElementInteractionMode::None,
        }
    }
}

/// 绘图元素结构体
#[derive(Debug, Clone, PartialEq)]
pub struct DrawingElement {
    pub tool: DrawingTool,
    pub points: Vec<POINT>,
    pub rect: RECT,
    pub color: D2D1_COLOR_F,
    pub thickness: f32, // Stroke width for non-text elements
    // Text-specific properties
    pub text: String,
    pub font_size: f32,
    pub font_name: String,
    pub font_weight: i32,
    pub font_italic: bool,
    pub font_underline: bool,
    pub font_strikeout: bool,
    pub selected: bool,
    // Cache for Pen tool geometry
    pub path_geometry: RefCell<Option<ID2D1PathGeometry>>,
    // Cache for Text tool layout
    pub text_layout: RefCell<Option<IDWriteTextLayout>>,
}

impl DrawingElement {
    pub fn new(tool: DrawingTool) -> Self {
        Self {
            tool,
            points: Vec::new(),
            rect: RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            color: crate::constants::DEFAULT_DRAWING_COLOR,
            thickness: crate::constants::DEFAULT_LINE_THICKNESS,
            text: String::new(),
            font_size: crate::constants::DEFAULT_FONT_SIZE,
            font_name: crate::constants::DEFAULT_FONT_NAME.to_string(),
            font_weight: crate::constants::DEFAULT_FONT_WEIGHT,
            font_italic: false,
            font_underline: false,
            font_strikeout: false,
            selected: false,
            path_geometry: RefCell::new(None),
            text_layout: RefCell::new(None),
        }
    }

    pub fn get_effective_font_size(&self) -> f32 {
        if self.tool == DrawingTool::Text {
            self.font_size.max(crate::constants::MIN_FONT_SIZE)
        } else {
            // 非文本元素不应该有字体大小，但为了兼容性返回默认值
            crate::constants::DEFAULT_FONT_SIZE
        }
    }

    /// 设置字体大小（兼容性方法）
    /// 确保正确更新font_size字段
    pub fn set_font_size(&mut self, size: f32) {
        if self.tool == DrawingTool::Text
            && (self.font_size - size).abs() > 0.001 {
                // 使用 clamp 确保字体大小在有效范围内
                self.font_size = size.clamp(
                    crate::constants::MIN_FONT_SIZE,
                    crate::constants::MAX_FONT_SIZE,
                );
                self.text_layout.replace(None);
            }
    }

    pub fn update_bounding_rect(&mut self) {
        if self.points.is_empty() {
            return;
        }

        match self.tool {
            DrawingTool::Text => {
                // 文字：动态计算基于内容的边界
                if !self.points.is_empty() {
                    let start = &self.points[0];

                    // 如果有第二个点，使用它定义矩形（已经通过动态调整设置）
                    if self.points.len() >= 2 {
                        let end = &self.points[1];
                        self.rect = RECT {
                            left: start.x.min(end.x),
                            top: start.y.min(end.y),
                            right: start.x.max(end.x),
                            bottom: start.y.max(end.y),
                        };
                    } else {
                        // 使用默认大小（初始状态）
                        self.rect = RECT {
                            left: start.x,
                            top: start.y,
                            right: start.x + crate::constants::DEFAULT_TEXT_WIDTH,
                            bottom: start.y + crate::constants::DEFAULT_TEXT_HEIGHT,
                        };
                    }
                }
            }
            DrawingTool::Pen => {
                // 画笔工具：计算所有点的边界
                if !self.points.is_empty() {
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

                    // 加上线条粗细的边距
                    let margin = (self.thickness / 2.0) as i32 + 1;
                    self.rect = RECT {
                        left: min_x - margin,
                        top: min_y - margin,
                        right: max_x + margin,
                        bottom: max_y + margin,
                    };
                }
            }
            DrawingTool::Rectangle | DrawingTool::Circle => {
                // 矩形和圆形：使用两个点定义边界
                if self.points.len() >= 2 {
                    let start = &self.points[0];
                    let end = &self.points[1];

                    self.rect = RECT {
                        left: start.x.min(end.x),
                        top: start.y.min(end.y),
                        right: start.x.max(end.x),
                        bottom: start.y.max(end.y),
                    };
                }
            }
            DrawingTool::Arrow => {
                // 箭头：使用起点和终点定义边界
                if self.points.len() >= 2 {
                    let start = &self.points[0];
                    let end = &self.points[1];

                    // 考虑箭头头部的额外尺寸
                    let margin = crate::constants::ARROW_HEAD_MARGIN;

                    self.rect = RECT {
                        left: (start.x.min(end.x) - margin),
                        top: (start.y.min(end.y) - margin),
                        right: (start.x.max(end.x) + margin),
                        bottom: (start.y.max(end.y) + margin),
                    };
                }
            }

            _ => {
                // 其他工具使用第一个点作为基准
                if !self.points.is_empty() {
                    self.rect = RECT {
                        left: self.points[0].x,
                        top: self.points[0].y,
                        right: self.points[0].x + crate::constants::DEFAULT_ELEMENT_WIDTH,
                        bottom: self.points[0].y + crate::constants::DEFAULT_ELEMENT_HEIGHT,
                    };
                }
            }
        }
    }

    pub fn contains_point(&self, x: i32, y: i32) -> bool {
        match self.tool {
            DrawingTool::Pen => {
                if self.points.len() < 2 {
                    return false;
                }

                for i in 0..self.points.len() - 1 {
                    let p1 = &self.points[i];
                    let p2 = &self.points[i + 1];

                    let distance = point_to_line_distance(x, y, p1.x, p1.y, p2.x, p2.y);
                    if distance <= (self.thickness + crate::constants::ELEMENT_CLICK_TOLERANCE) as f64 {
                        return true;
                    }
                }
                false
            }
            DrawingTool::Rectangle | DrawingTool::Circle => {
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
            DrawingTool::Arrow => {
                if self.points.len() >= 2 {
                    let start = &self.points[0];
                    let end = &self.points[1];

                    let distance = point_to_line_distance(x, y, start.x, start.y, end.x, end.y);
                    if distance <= (self.thickness + crate::constants::ELEMENT_CLICK_TOLERANCE) as f64 {
                        return true;
                    }

                    let dx = end.x - start.x;
                    let dy = end.y - start.y;
                    let length = ((dx * dx + dy * dy) as f64).sqrt();

                    if length > crate::constants::ARROW_MIN_LENGTH {
                        let arrow_length = crate::constants::ARROW_HEAD_LENGTH;
                        let arrow_angle = crate::constants::ARROW_HEAD_ANGLE;
                        let unit_x = dx as f64 / length;
                        let unit_y = dy as f64 / length;

                        let wing1_x = end.x
                            - (arrow_length
                                * (unit_x * arrow_angle.cos() + unit_y * arrow_angle.sin()))
                                as i32;
                        let wing1_y = end.y
                            - (arrow_length
                                * (unit_y * arrow_angle.cos() - unit_x * arrow_angle.sin()))
                                as i32;

                        let wing2_x = end.x
                            - (arrow_length
                                * (unit_x * arrow_angle.cos() - unit_y * arrow_angle.sin()))
                                as i32;
                        let wing2_y = end.y
                            - (arrow_length
                                * (unit_y * arrow_angle.cos() + unit_x * arrow_angle.sin()))
                                as i32;

                        let distance1 =
                            point_to_line_distance(x, y, end.x, end.y, wing1_x, wing1_y);
                        let distance2 =
                            point_to_line_distance(x, y, end.x, end.y, wing2_x, wing2_y);

                        if distance1 <= (self.thickness + crate::constants::ELEMENT_CLICK_TOLERANCE) as f64
                            || distance2 <= (self.thickness + crate::constants::ELEMENT_CLICK_TOLERANCE) as f64
                        {
                            return true;
                        }
                    }

                    false
                } else {
                    false
                }
            }
            DrawingTool::Text => {
                // 🔧 直接使用 rect 字段，确保和显示、选择框完全一致
                x >= self.rect.left
                    && x <= self.rect.right
                    && y >= self.rect.top
                    && y <= self.rect.bottom
            }
            _ => false,
        }
    }

    pub fn resize(&mut self, new_rect: RECT) {
        self.invalidate_geometry_cache();
        match self.tool {
            DrawingTool::Rectangle | DrawingTool::Circle => self.resize_two_point_shape(new_rect),
            DrawingTool::Arrow => self.resize_arrow(new_rect),
            DrawingTool::Pen => self.resize_freeform(new_rect),
            DrawingTool::Text => self.resize_text(new_rect),
            _ => {}
        }
        self.rect = new_rect;
    }

    fn invalidate_geometry_cache(&mut self) {
        self.path_geometry.replace(None);
        self.text_layout.replace(None);
    }

    fn resize_two_point_shape(&mut self, new_rect: RECT) {
        if self.points.len() >= 2 {
            self.points[0] = POINT { x: new_rect.left, y: new_rect.top };
            self.points[1] = POINT { x: new_rect.right, y: new_rect.bottom };
        }
    }

    fn resize_arrow(&mut self, new_rect: RECT) {
        if self.points.len() < 2 { return; }
        let old_width = (self.rect.right - self.rect.left).max(1);
        let old_height = (self.rect.bottom - self.rect.top).max(1);
        let new_width = new_rect.right - new_rect.left;
        let new_height = new_rect.bottom - new_rect.top;

        let old_start = self.points[0];
        let old_end = self.points[1];

        let start_rel_x = (old_start.x - self.rect.left) as f64 / old_width as f64;
        let start_rel_y = (old_start.y - self.rect.top) as f64 / old_height as f64;
        let end_rel_x = (old_end.x - self.rect.left) as f64 / old_width as f64;
        let end_rel_y = (old_end.y - self.rect.top) as f64 / old_height as f64;

        self.points[0] = POINT {
            x: new_rect.left + (start_rel_x * new_width as f64) as i32,
            y: new_rect.top + (start_rel_y * new_height as f64) as i32,
        };
        self.points[1] = POINT {
            x: new_rect.left + (end_rel_x * new_width as f64) as i32,
            y: new_rect.top + (end_rel_y * new_height as f64) as i32,
        };
    }

    fn resize_freeform(&mut self, new_rect: RECT) {
        let old_rect = self.rect;
        let old_w = (old_rect.right - old_rect.left).max(1) as f64;
        let old_h = (old_rect.bottom - old_rect.top).max(1) as f64;
        let scale_x = (new_rect.right - new_rect.left) as f64 / old_w;
        let scale_y = (new_rect.bottom - new_rect.top) as f64 / old_h;
        for point in &mut self.points {
            let rel_x = (point.x - old_rect.left) as f64;
            let rel_y = (point.y - old_rect.top) as f64;
            point.x = new_rect.left + (rel_x * scale_x) as i32;
            point.y = new_rect.top + (rel_y * scale_y) as i32;
        }
    }

    fn resize_text(&mut self, new_rect: RECT) {
        if self.points.is_empty() { return; }
        self.points[0] = POINT { x: new_rect.left, y: new_rect.top };
        if self.points.len() >= 2 {
            self.points[1] = POINT { x: new_rect.right, y: new_rect.bottom };
        } else {
            self.points.push(POINT { x: new_rect.right, y: new_rect.bottom });
        }
    }

    pub fn move_by(&mut self, dx: i32, dy: i32) {
        // Invalidate geometry cache when moving
        self.path_geometry.replace(None);
        for point in &mut self.points {
            point.x += dx;
            point.y += dy;
        }
        self.rect.left += dx;
        self.rect.right += dx;
        self.rect.top += dy;
        self.rect.bottom += dy;
    }

    pub fn get_bounding_rect(&self) -> RECT {
        match self.tool {
            DrawingTool::Rectangle
            | DrawingTool::Circle
            | DrawingTool::Arrow
            | DrawingTool::Text => self.rect,
            DrawingTool::Pen => {
                if self.points.is_empty() {
                    return RECT {
                        left: 0,
                        top: 0,
                        right: 0,
                        bottom: 0,
                    };
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

                RECT {
                    left: min_x,
                    top: min_y,
                    right: max_x,
                    bottom: max_y,
                }
            }
            _ => RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
        }
    }
}

// ==================== 单元测试 ====================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_drawing_element_new() {
        let element = DrawingElement::new(DrawingTool::Rectangle);
        assert_eq!(element.tool, DrawingTool::Rectangle);
        assert!(element.points.is_empty());
        assert_eq!(element.thickness, 3.0);
        assert!(!element.selected);
    }

    #[test]
    fn test_drawing_element_contains_point_rectangle() {
        let mut element = DrawingElement::new(DrawingTool::Rectangle);
        element.points = vec![
            POINT { x: 10, y: 10 },
            POINT { x: 100, y: 100 },
        ];
        element.update_bounding_rect();

        // 点在矩形内
        assert!(element.contains_point(50, 50));
        assert!(element.contains_point(10, 10));
        assert!(element.contains_point(100, 100));
        
        // 点在矩形外
        assert!(!element.contains_point(5, 5));
        assert!(!element.contains_point(150, 150));
    }

    #[test]
    fn test_drawing_element_contains_point_circle() {
        let mut element = DrawingElement::new(DrawingTool::Circle);
        element.points = vec![
            POINT { x: 0, y: 0 },
            POINT { x: 100, y: 100 },
        ];
        element.update_bounding_rect();

        // 圆形使用边界矩形检测
        assert!(element.contains_point(50, 50));
        assert!(!element.contains_point(150, 150));
    }

    #[test]
    fn test_drawing_element_contains_point_text() {
        let mut element = DrawingElement::new(DrawingTool::Text);
        element.points = vec![POINT { x: 20, y: 20 }];
        element.text = "Hello".to_string();
        element.update_bounding_rect();

        // 文本元素使用rect字段检测
        assert!(element.contains_point(25, 25));
    }

    #[test]
    fn test_drawing_element_move_by() {
        let mut element = DrawingElement::new(DrawingTool::Rectangle);
        element.points = vec![
            POINT { x: 10, y: 10 },
            POINT { x: 100, y: 100 },
        ];
        element.rect = RECT {
            left: 10,
            top: 10,
            right: 100,
            bottom: 100,
        };

        element.move_by(50, 30);

        assert_eq!(element.points[0].x, 60);
        assert_eq!(element.points[0].y, 40);
        assert_eq!(element.points[1].x, 150);
        assert_eq!(element.points[1].y, 130);
        assert_eq!(element.rect.left, 60);
        assert_eq!(element.rect.top, 40);
    }

    #[test]
    fn test_drawing_element_resize() {
        let mut element = DrawingElement::new(DrawingTool::Rectangle);
        element.points = vec![
            POINT { x: 0, y: 0 },
            POINT { x: 100, y: 100 },
        ];
        element.rect = RECT {
            left: 0,
            top: 0,
            right: 100,
            bottom: 100,
        };

        let new_rect = RECT {
            left: 50,
            top: 50,
            right: 200,
            bottom: 200,
        };
        element.resize(new_rect);

        assert_eq!(element.rect, new_rect);
        assert_eq!(element.points[0].x, 50);
        assert_eq!(element.points[0].y, 50);
        assert_eq!(element.points[1].x, 200);
        assert_eq!(element.points[1].y, 200);
    }

    #[test]
    fn test_drawing_element_update_bounding_rect_pen() {
        let mut element = DrawingElement::new(DrawingTool::Pen);
        element.points = vec![
            POINT { x: 10, y: 20 },
            POINT { x: 50, y: 10 },
            POINT { x: 30, y: 60 },
        ];
        element.thickness = 2.0;

        element.update_bounding_rect();

        // 边界应该包含所有点，加上线条粗细边距
        let margin = (element.thickness / 2.0) as i32 + 1;
        assert_eq!(element.rect.left, 10 - margin);
        assert_eq!(element.rect.top, 10 - margin);
        assert_eq!(element.rect.right, 50 + margin);
        assert_eq!(element.rect.bottom, 60 + margin);
    }

    #[test]
    fn test_drawing_element_get_effective_font_size() {
        let mut element = DrawingElement::new(DrawingTool::Text);
        
        element.font_size = 24.0;
        assert_eq!(element.get_effective_font_size(), 24.0);

        // 小于最小值时返回最小值
        element.font_size = 4.0;
        assert_eq!(element.get_effective_font_size(), 8.0);
    }

    #[test]
    fn test_drawing_element_set_font_size() {
        let mut element = DrawingElement::new(DrawingTool::Text);
        
        element.set_font_size(32.0);
        assert_eq!(element.font_size, 32.0);

        // 超过最大值时限制为最大值
        element.set_font_size(300.0);
        assert_eq!(element.font_size, crate::constants::MAX_FONT_SIZE);

        // 低于最小值时限制为最小值
        element.set_font_size(2.0);
        assert_eq!(element.font_size, crate::constants::MIN_FONT_SIZE);
    }

    #[test]
    fn test_drag_mode_equality() {
        assert_eq!(DragMode::None, DragMode::None);
        assert_ne!(DragMode::Drawing, DragMode::Moving);
        assert_eq!(DragMode::ResizingTopLeft, DragMode::ResizingTopLeft);
    }
}
