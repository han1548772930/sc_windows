use super::DrawingError;
use crate::platform::{PlatformError, PlatformRenderer};
use crate::types::DrawingElement;

/// 元素管理器
pub struct ElementManager {
    /// 所有绘图元素
    elements: Vec<DrawingElement>,
}

impl Default for ElementManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ElementManager {
    /// 创建新的元素管理器
    pub fn new() -> Self {
        Self {
            elements: Vec::new(),
        }
    }

    /// 添加元素
    pub fn add_element(&mut self, element: DrawingElement) {
        self.elements.push(element);
    }

    /// 移除元素
    pub fn remove_element(&mut self, index: usize) -> bool {
        if index < self.elements.len() {
            self.elements.remove(index);
            true
        } else {
            false
        }
    }

    /// 获取指定位置的元素索引（考虑选择框约束）
    pub fn get_element_at_position(&self, x: i32, y: i32) -> Option<usize> {
        // 从后往前查找（最后绘制的元素在最上层）
        for (index, element) in self.elements.iter().enumerate().rev() {
            if element.contains_point(x, y) {
                return Some(index);
            }
        }
        None
    }

    /// 获取指定位置的元素索引（带选择框可见性检查）
    pub fn get_element_at_position_with_selection(
        &self,
        x: i32,
        y: i32,
        selection_rect: Option<windows::Win32::Foundation::RECT>,
    ) -> Option<usize> {
        // 从后往前查找（最后绘制的元素在最上层）
        for (index, element) in self.elements.iter().enumerate().rev() {
            if element.tool == crate::types::DrawingTool::Pen {
                continue;
            }

            if element.contains_point(x, y) {
                // 如果有选择框，检查元素是否在选择框内可见
                if let Some(sel_rect) = selection_rect {
                    if self.is_element_visible_in_selection(element, &sel_rect) {
                        return Some(index);
                    }
                } else {
                    return Some(index);
                }
            }
        }
        None
    }

    /// 检查元素是否在选择框内可见（从原始代码迁移）
    pub fn is_element_visible_in_selection(
        &self,
        element: &crate::types::DrawingElement,
        selection_rect: &windows::Win32::Foundation::RECT,
    ) -> bool {
        let element_rect = element.get_bounding_rect();

        // 检查元素是否与选择框有交集
        !(element_rect.right < selection_rect.left
            || element_rect.left > selection_rect.right
            || element_rect.bottom < selection_rect.top
            || element_rect.top > selection_rect.bottom)
    }

    /// 设置选中状态
    pub fn set_selected(&mut self, index: Option<usize>) {
        // 清除所有选中状态
        for element in &mut self.elements {
            element.selected = false;
        }

        // 设置新的选中状态
        if let Some(idx) = index {
            if idx < self.elements.len() {
                self.elements[idx].selected = true;
            }
        }
    }

    /// 渲染所有元素
    pub fn render(
        &self,
        renderer: &mut dyn PlatformRenderer<Error = PlatformError>,
    ) -> Result<(), DrawingError> {
        use crate::platform::traits::{Color, DrawStyle, Point, TextStyle};
        use crate::types::DrawingTool;

        for element in &self.elements {
            // 转换颜色格式
            let color = Color {
                r: element.color.r,
                g: element.color.g,
                b: element.color.b,
                a: element.color.a,
            };

            match element.tool {
                DrawingTool::Rectangle => {
                    if element.points.len() >= 2 {
                        let rect = crate::platform::traits::Rectangle {
                            x: element.points[0].x as f32,
                            y: element.points[0].y as f32,
                            width: (element.points[1].x - element.points[0].x) as f32,
                            height: (element.points[1].y - element.points[0].y) as f32,
                        };

                        let style = DrawStyle {
                            stroke_color: color,
                            fill_color: None,
                            stroke_width: element.thickness,
                        };

                        renderer
                            .draw_rectangle(rect, &style)
                            .map_err(|e| DrawingError::RenderError(e.to_string()))?;
                    }
                }
                DrawingTool::Circle => {
                    if element.points.len() >= 2 {
                        let center_x = (element.points[0].x + element.points[1].x) as f32 / 2.0;
                        let center_y = (element.points[0].y + element.points[1].y) as f32 / 2.0;
                        let radius_x =
                            (element.points[1].x - element.points[0].x).abs() as f32 / 2.0;
                        let radius_y =
                            (element.points[1].y - element.points[0].y).abs() as f32 / 2.0;

                        // 使用较大的半径作为圆形半径（简化版本）
                        let radius = radius_x.max(radius_y);

                        let center = Point {
                            x: center_x,
                            y: center_y,
                        };

                        let style = DrawStyle {
                            stroke_color: color,
                            fill_color: None,
                            stroke_width: element.thickness,
                        };

                        renderer
                            .draw_circle(center, radius, &style)
                            .map_err(|e| DrawingError::RenderError(e.to_string()))?;
                    }
                }
                DrawingTool::Arrow => {
                    if element.points.len() >= 2 {
                        // 绘制箭头主线
                        let start = Point {
                            x: element.points[0].x as f32,
                            y: element.points[0].y as f32,
                        };
                        let end = Point {
                            x: element.points[1].x as f32,
                            y: element.points[1].y as f32,
                        };

                        let style = DrawStyle {
                            stroke_color: color,
                            fill_color: None,
                            stroke_width: element.thickness,
                        };

                        renderer
                            .draw_line(start, end, &style)
                            .map_err(|e| DrawingError::RenderError(e.to_string()))?;

                        // 绘制箭头头部（简化版本）
                        let dx = end.x - start.x;
                        let dy = end.y - start.y;
                        let length = (dx * dx + dy * dy).sqrt();

                        if length > 0.0 {
                            let arrow_length = 15.0;
                            let arrow_angle: f32 = 0.5; // 约30度

                            let unit_x = dx / length;
                            let unit_y = dy / length;

                            // 箭头翼1
                            let wing1_x = end.x
                                - arrow_length
                                    * (unit_x * arrow_angle.cos() - unit_y * arrow_angle.sin());
                            let wing1_y = end.y
                                - arrow_length
                                    * (unit_x * arrow_angle.sin() + unit_y * arrow_angle.cos());

                            // 箭头翼2
                            let wing2_x = end.x
                                - arrow_length
                                    * (unit_x * arrow_angle.cos() + unit_y * arrow_angle.sin());
                            let wing2_y = end.y
                                - arrow_length
                                    * (-unit_x * arrow_angle.sin() + unit_y * arrow_angle.cos());

                            let wing1 = Point {
                                x: wing1_x,
                                y: wing1_y,
                            };
                            let wing2 = Point {
                                x: wing2_x,
                                y: wing2_y,
                            };

                            renderer
                                .draw_line(end, wing1, &style)
                                .map_err(|e| DrawingError::RenderError(e.to_string()))?;
                            renderer
                                .draw_line(end, wing2, &style)
                                .map_err(|e| DrawingError::RenderError(e.to_string()))?;
                        }
                    }
                }
                DrawingTool::Pen => {
                    if element.points.len() > 1 {
                        let style = DrawStyle {
                            stroke_color: color,
                            fill_color: None,
                            stroke_width: element.thickness,
                        };

                        // 连接所有点绘制自由线条
                        for i in 0..element.points.len() - 1 {
                            let start = Point {
                                x: element.points[i].x as f32,
                                y: element.points[i].y as f32,
                            };
                            let end = Point {
                                x: element.points[i + 1].x as f32,
                                y: element.points[i + 1].y as f32,
                            };

                            renderer
                                .draw_line(start, end, &style)
                                .map_err(|e| DrawingError::RenderError(e.to_string()))?;
                        }
                    }
                }
                DrawingTool::Text => {
                    if !element.points.is_empty() && !element.text.is_empty() {
                        let position = Point {
                            x: element.points[0].x as f32,
                            y: element.points[0].y as f32,
                        };

                        let text_style = TextStyle {
                            font_size: element.font_size,
                            color,
                            font_family: element.font_name.clone(), // use element's own font
                        };

                        renderer
                            .draw_text(&element.text, position, &text_style)
                            .map_err(|e| DrawingError::RenderError(e.to_string()))?;
                    }
                }
                DrawingTool::None => {
                    // 不绘制任何内容
                }
            }
        }
        Ok(())
    }

    /// 获取所有元素的引用
    pub fn get_elements(&self) -> &Vec<DrawingElement> {
        &self.elements
    }

    /// 获取可变元素引用
    pub fn get_element_mut(&mut self, index: usize) -> Option<&mut DrawingElement> {
        self.elements.get_mut(index)
    }

    /// 设置指定索引的元素
    pub fn set_element(&mut self, index: usize, element: DrawingElement) -> bool {
        if index < self.elements.len() {
            self.elements[index] = element;
            true
        } else {
            false
        }
    }

    /// 恢复状态（用于撤销/重做）
    pub fn restore_state(&mut self, elements: Vec<DrawingElement>) {
        self.elements = elements;
    }

    /// 清空所有元素
    pub fn clear(&mut self) {
        self.elements.clear();
    }

    /// 获取元素数量
    pub fn count(&self) -> usize {
        self.elements.len()
    }
}
