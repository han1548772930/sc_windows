use std::cell::RefCell;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::ID2D1PathGeometry;
use windows::Win32::Graphics::DirectWrite::IDWriteTextLayout;

// use crate::svg_icons::SvgIconManager; // 临时注释，待迁移
use crate::utils::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolbarButton {
    Save,
    Rectangle,
    Circle,
    Arrow,
    Pen,
    Text,
    Undo,
    ExtractText, // 新增：文本提取按钮
    Languages,   // 新增：语言按钮
    Confirm,
    Cancel,
    None,
    Pin,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DrawingTool {
    None,
    Rectangle,
    Circle,
    Arrow,
    Pen,
    Text,
}

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

/// 拖拽模式枚举（从原始代码迁移）
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

// IconData 结构体已移除，现在只使用 SVG 图标
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
            color: D2D1_COLOR_F {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            thickness: 3.0,
            text: String::new(),
            font_size: 20.0,
            font_name: "Microsoft YaHei".to_string(),
            font_weight: 400,
            font_italic: false,
            font_underline: false,
            font_strikeout: false,
            selected: false,
            path_geometry: RefCell::new(None),
            text_layout: RefCell::new(None),
        }
    }

    // 注意：from_legacy_data 方法已被移除
    // 该方法用于兼容旧数据格式，现在所有新的绘图元素都通过 new() 方法创建

    /// 获取用于渲染的字体大小（兼容性方法）
    /// 确保文本元素使用font_size字段，其他元素可能仍使用thickness
    pub fn get_effective_font_size(&self) -> f32 {
        if self.tool == DrawingTool::Text {
            self.font_size.max(8.0)
        } else {
            // 非文本元素不应该有字体大小，但为了兼容性返回默认值
            20.0
        }
    }

    /// 设置字体大小（兼容性方法）
    /// 确保正确更新font_size字段
    pub fn set_font_size(&mut self, size: f32) {
        if self.tool == DrawingTool::Text {
            if (self.font_size - size).abs() > 0.001 {
                self.font_size = size.max(8.0);
                self.text_layout.replace(None);
            }
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
                    let margin = 20; // 箭头头部可能超出的范围

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
                        right: self.points[0].x + 50,
                        bottom: self.points[0].y + 30,
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
                    if distance <= (self.thickness + 5.0) as f64 {
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
                    if distance <= (self.thickness + 5.0) as f64 {
                        return true;
                    }

                    let dx = end.x - start.x;
                    let dy = end.y - start.y;
                    let length = ((dx * dx + dy * dy) as f64).sqrt();

                    if length > 20.0 {
                        let arrow_length = 15.0_f64;
                        let arrow_angle = 0.5_f64;
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

                        if distance1 <= (self.thickness + 5.0) as f64
                            || distance2 <= (self.thickness + 5.0) as f64
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
        // Invalidate geometry cache when resizing
        self.path_geometry.replace(None);
        self.text_layout.replace(None);
        match self.tool {
            DrawingTool::Rectangle | DrawingTool::Circle => {
                if self.points.len() >= 2 {
                    self.points[0] = POINT {
                        x: new_rect.left,
                        y: new_rect.top,
                    };
                    self.points[1] = POINT {
                        x: new_rect.right,
                        y: new_rect.bottom,
                    };
                }
            }
            DrawingTool::Arrow => {
                if self.points.len() >= 2 {
                    let old_width = self.rect.right - self.rect.left;
                    let old_height = self.rect.bottom - self.rect.top;
                    let new_width = new_rect.right - new_rect.left;
                    let new_height = new_rect.bottom - new_rect.top;

                    if old_width == 0 || old_height == 0 {
                        self.points[0] = POINT {
                            x: new_rect.left,
                            y: new_rect.top,
                        };
                        self.points[1] = POINT {
                            x: new_rect.right,
                            y: new_rect.bottom,
                        };
                    } else {
                        let old_start = &self.points[0];
                        let old_end = &self.points[1];

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
                }
            }
            DrawingTool::Pen => {
                let old_rect = self.rect;
                let scale_x = (new_rect.right - new_rect.left) as f64
                    / (old_rect.right - old_rect.left) as f64;
                let scale_y = (new_rect.bottom - new_rect.top) as f64
                    / (old_rect.bottom - old_rect.top) as f64;

                for point in &mut self.points {
                    let rel_x = (point.x - old_rect.left) as f64;
                    let rel_y = (point.y - old_rect.top) as f64;
                    point.x = new_rect.left + (rel_x * scale_x) as i32;
                    point.y = new_rect.top + (rel_y * scale_y) as i32;
                }
            }
            DrawingTool::Text => {
                if !self.points.is_empty() {
                    self.points[0] = POINT {
                        x: new_rect.left,
                        y: new_rect.top,
                    };
                    // 确保有第二个点来定义文本框的右下角
                    if self.points.len() >= 2 {
                        self.points[1] = POINT {
                            x: new_rect.right,
                            y: new_rect.bottom,
                        };
                    } else {
                        self.points.push(POINT {
                            x: new_rect.right,
                            y: new_rect.bottom,
                        });
                    }
                }
            }
            _ => {}
        }
        self.rect = new_rect;
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

// IconData 实现已移除
