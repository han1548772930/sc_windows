// 绘图渲染模块 - 处理所有绘图元素的渲染逻辑

use crate::platform::{PlatformError, PlatformRenderer};
use crate::types::{DrawingElement, DrawingTool};
use crate::utils::d2d_helpers::{create_solid_brush, ellipse};

use super::{DrawingError, DrawingManager, ElementInteractionMode};

impl DrawingManager {
    /// 渲染绘图元素（支持裁剪区域）
    pub fn render(
        &self,
        renderer: &mut dyn PlatformRenderer<Error = PlatformError>,
        selection_rect: Option<&windows::Win32::Foundation::RECT>,
    ) -> Result<(), DrawingError> {
        // 如果有选择区域，设置裁剪
        if let Some(rect) = selection_rect {
            let clip_rect = crate::platform::traits::Rectangle {
                x: rect.left as f32,
                y: rect.top as f32,
                width: (rect.right - rect.left) as f32,
                height: (rect.bottom - rect.top) as f32,
            };
            renderer.push_clip_rect(clip_rect).map_err(|e| {
                DrawingError::RenderError(format!("Failed to set clip rect: {e:?}"))
            })?;
        }

        // 尝试使用Direct2D直接渲染（更高效）
        if let Some(d2d_renderer) = renderer
            .as_any()
            .downcast_ref::<crate::platform::windows::d2d::Direct2DRenderer>()
        {
            // 渲染所有元素（现在被裁剪限制）
            for element in self.elements.get_elements() {
                self.draw_element_d2d(element, d2d_renderer)?;
            }

            // 渲染当前正在绘制的元素
            if let Some(ref element) = self.current_element {
                self.draw_element_d2d(element, d2d_renderer)?;
            }

            // 渲染元素选择（平台无关路径）
            self.draw_element_selection(renderer, selection_rect)?;
        } else {
            // 抽象接口已移除，现在只支持 Direct2D
            return Err(DrawingError::RenderError(
                "Only Direct2D rendering is supported".to_string(),
            ));
        }

        // 恢复裁剪区域
        if selection_rect.is_some() {
            renderer.pop_clip_rect().map_err(|e| {
                DrawingError::RenderError(format!("Failed to pop clip rect: {e:?}"))
            })?;
        }

        Ok(())
    }

    /// 使用平台无关接口渲染元素选择指示（虚线边框 + 手柄）
    pub(super) fn draw_element_selection(
        &self,
        renderer: &mut dyn PlatformRenderer<Error = PlatformError>,
        selection_rect: Option<&windows::Win32::Foundation::RECT>,
    ) -> Result<(), DrawingError> {
        // 只有当有选中元素时才绘制选择指示器
        if self.selected_element.is_none() {
            return Ok(());
        }

        for element in self.elements.get_elements() {
            if element.selected {
                self.draw_selected_element_indicators(renderer, element, selection_rect)?;
            }
        }
        Ok(())
    }

    /// 平台无关的选中指示器绘制
    fn draw_selected_element_indicators(
        &self,
        renderer: &mut dyn PlatformRenderer<Error = PlatformError>,
        element: &DrawingElement,
        _selection_rect: Option<&windows::Win32::Foundation::RECT>,
    ) -> Result<(), DrawingError> {
        use crate::platform::traits::{Color, DrawStyle, Point, Rectangle};

        // 箭头特殊：只绘制端点手柄（圆形），不绘制虚线边框（保持原行为）
        if element.tool == DrawingTool::Arrow {
            let radius = 3.0_f32;
            let fill = Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            };
            let border = Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            };
            let style = DrawStyle {
                stroke_color: border,
                fill_color: Some(fill),
                stroke_width: 1.0,
            };
            if element.points.len() >= 2 {
                let pts = [
                    Point {
                        x: element.points[0].x as f32,
                        y: element.points[0].y as f32,
                    },
                    Point {
                        x: element.points[1].x as f32,
                        y: element.points[1].y as f32,
                    },
                ];
                for p in pts.iter() {
                    renderer.draw_circle(*p, radius, &style).map_err(|e| {
                        DrawingError::RenderError(format!("draw circle handle failed: {e}"))
                    })?;
                }
            }
            return Ok(());
        }

        // 其他元素的处理
        let rect = Rectangle {
            x: element.rect.left as f32,
            y: element.rect.top as f32,
            width: (element.rect.right - element.rect.left) as f32,
            height: (element.rect.bottom - element.rect.top) as f32,
        };

        // 检查是否是文本元素且正在拖动
        let is_text_dragging = element.tool == DrawingTool::Text
            && self.mouse_pressed
            && self.interaction_mode == ElementInteractionMode::MovingElement;

        // 检查是否是文本元素且正在编辑
        let is_text_editing = element.tool == DrawingTool::Text && self.text_editing;

        // 文本元素的特殊处理：只有在编辑模式时才显示边框和手柄
        if element.tool == DrawingTool::Text {
            // 文本元素只有在编辑模式时才显示UI
            if is_text_editing && !is_text_dragging {
                // 虚线边框（使用高层接口）
                let dashed_color = Color {
                    r: 0.0,
                    g: 0.5,
                    b: 1.0,
                    a: 1.0,
                };
                let dash_pattern: [f32; 2] = [4.0, 2.0];
                renderer
                    .draw_selection_border(rect, dashed_color, 1.0, Some(&dash_pattern))
                    .map_err(|e| {
                        DrawingError::RenderError(format!("draw dashed border failed: {e}"))
                    })?;

                // 手柄绘制 - 文本编辑时只显示4个角的手柄
                let handle_radius = 3.0_f32;
                let fill_color = Color {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                };
                let border_color = Color {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                };

                self.draw_corner_handles_only(
                    renderer,
                    rect,
                    handle_radius,
                    fill_color,
                    border_color,
                )?;
            }
            // 文本选中但未编辑时，或者拖动时，不显示任何UI
        } else {
            // 非文本元素：正常显示边框和8个手柄
            // 虚线边框（使用高层接口）
            let dashed_color = Color {
                r: 0.0,
                g: 0.5,
                b: 1.0,
                a: 1.0,
            };
            let dash_pattern: [f32; 2] = [4.0, 2.0];
            renderer
                .draw_selection_border(rect, dashed_color, 1.0, Some(&dash_pattern))
                .map_err(|e| {
                    DrawingError::RenderError(format!("draw dashed border failed: {e}"))
                })?;

            // 手柄绘制 - 非文本元素显示8个手柄
            let handle_radius = 3.0_f32;
            let fill_color = Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            };
            let border_color = Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            };

            renderer
                .draw_element_handles(rect, handle_radius, fill_color, border_color, 1.0)
                .map_err(|e| {
                    DrawingError::RenderError(format!("draw element handles failed: {e}"))
                })?;
        }

        Ok(())
    }

    /// 只绘制4个角的手柄（用于文本编辑时）
    fn draw_corner_handles_only(
        &self,
        renderer: &mut dyn PlatformRenderer<Error = PlatformError>,
        rect: crate::platform::traits::Rectangle,
        handle_radius: f32,
        fill_color: crate::platform::traits::Color,
        border_color: crate::platform::traits::Color,
    ) -> Result<(), DrawingError> {
        use crate::platform::traits::{DrawStyle, Point};

        let style = DrawStyle {
            stroke_color: border_color,
            fill_color: Some(fill_color),
            stroke_width: 1.0,
        };

        // 只绘制4个角的手柄
        let corner_positions = [
            Point {
                x: rect.x,
                y: rect.y,
            }, // 左上
            Point {
                x: rect.x + rect.width,
                y: rect.y,
            }, // 右上
            Point {
                x: rect.x + rect.width,
                y: rect.y + rect.height,
            }, // 右下
            Point {
                x: rect.x,
                y: rect.y + rect.height,
            }, // 左下
        ];

        for pos in corner_positions.iter() {
            renderer
                .draw_circle(*pos, handle_radius, &style)
                .map_err(|e| {
                    DrawingError::RenderError(format!("draw corner handle failed: {e}"))
                })?;
        }

        Ok(())
    }

    /// 使用Direct2D渲染单个元素
    pub(super) fn draw_element_d2d(
        &self,
        element: &DrawingElement,
        d2d_renderer: &crate::platform::windows::d2d::Direct2DRenderer,
    ) -> Result<(), DrawingError> {
        if let Some(ref render_target) = d2d_renderer.render_target {
            unsafe {
                // 使用新的辅助函数创建画刷
                if let Ok(brush) = create_solid_brush(render_target, &element.color) {
                    match element.tool {
                        DrawingTool::Text => {
                            if !element.points.is_empty() {
                                self.draw_text_element_d2d(element, d2d_renderer)?;
                            }
                        }
                        DrawingTool::Rectangle => {
                            if element.points.len() >= 2 {
                                let rect = crate::utils::d2d_rect_normalized(
                                    element.points[0].x,
                                    element.points[0].y,
                                    element.points[1].x,
                                    element.points[1].y,
                                );
                                render_target.DrawRectangle(&rect, &brush, element.thickness, None);
                            }
                        }
                        DrawingTool::Circle => {
                            if element.points.len() >= 2 {
                                let center_x =
                                    (element.points[0].x + element.points[1].x) as f32 / 2.0;
                                let center_y =
                                    (element.points[0].y + element.points[1].y) as f32 / 2.0;
                                let radius_x =
                                    (element.points[1].x - element.points[0].x).abs() as f32 / 2.0;
                                let radius_y =
                                    (element.points[1].y - element.points[0].y).abs() as f32 / 2.0;

                                // 使用新的辅助函数创建椭圆
                                let ellipse = ellipse(center_x, center_y, radius_x, radius_y);

                                render_target.DrawEllipse(
                                    &ellipse,
                                    &brush,
                                    element.thickness,
                                    None,
                                );
                            }
                        }
                        DrawingTool::Arrow => {
                            if element.points.len() >= 2 {
                                let start = crate::utils::d2d_point(
                                    element.points[0].x,
                                    element.points[0].y,
                                );
                                let end = crate::utils::d2d_point(
                                    element.points[1].x,
                                    element.points[1].y,
                                );

                                // 绘制主线
                                render_target.DrawLine(start, end, &brush, element.thickness, None);

                                // 绘制箭头头部
                                let dx = element.points[1].x - element.points[0].x;
                                let dy = element.points[1].y - element.points[0].y;
                                let length = ((dx * dx + dy * dy) as f64).sqrt();

                                if length > 20.0 {
                                    let arrow_length = 15.0f64;
                                    let arrow_angle = 0.5f64;
                                    let unit_x = dx as f64 / length;
                                    let unit_y = dy as f64 / length;

                                    let wing1 = crate::utils::d2d_point(
                                        element.points[1].x
                                            - (arrow_length
                                                * (unit_x * arrow_angle.cos()
                                                    + unit_y * arrow_angle.sin()))
                                                as i32,
                                        element.points[1].y
                                            - (arrow_length
                                                * (unit_y * arrow_angle.cos()
                                                    - unit_x * arrow_angle.sin()))
                                                as i32,
                                    );

                                    let wing2 = crate::utils::d2d_point(
                                        element.points[1].x
                                            - (arrow_length
                                                * (unit_x * arrow_angle.cos()
                                                    - unit_y * arrow_angle.sin()))
                                                as i32,
                                        element.points[1].y
                                            - (arrow_length
                                                * (unit_y * arrow_angle.cos()
                                                    + unit_x * arrow_angle.sin()))
                                                as i32,
                                    );

                                    render_target.DrawLine(
                                        end,
                                        wing1,
                                        &brush,
                                        element.thickness,
                                        None,
                                    );
                                    render_target.DrawLine(
                                        end,
                                        wing2,
                                        &brush,
                                        element.thickness,
                                        None,
                                    );
                                }
                            }
                        }
                        DrawingTool::Pen => {
                            if element.points.len() > 1 {
                                for i in 0..element.points.len() - 1 {
                                    let start = crate::utils::d2d_point(
                                        element.points[i].x,
                                        element.points[i].y,
                                    );
                                    let end = crate::utils::d2d_point(
                                        element.points[i + 1].x,
                                        element.points[i + 1].y,
                                    );
                                    render_target.DrawLine(
                                        start,
                                        end,
                                        &brush,
                                        element.thickness,
                                        None,
                                    );
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(())
    }
}
