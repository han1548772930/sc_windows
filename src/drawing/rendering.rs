use crate::platform::{PlatformError, PlatformRenderer};
use crate::types::{DrawingElement, DrawingTool};
use crate::utils::d2d_helpers::ellipse;
use windows::Win32::Graphics::Direct2D::Common::*;

use super::{DrawingError, DrawingManager, ElementInteractionMode};

impl DrawingManager {
    /// 批量绘制元素数量阈值，超过此数量时启用批量绘制优化
    const BATCH_RENDER_THRESHOLD: usize = 50;

    /// 批量渲染元素（按类型分组优化）
    ///
    /// 将元素按类型分组后批量渲染，减少状态切换次数。
    /// 当元素数量超过 BATCH_RENDER_THRESHOLD 时自动启用。
    pub fn render_elements_batched(
        &self,
        render_target: &windows::Win32::Graphics::Direct2D::ID2D1RenderTarget,
        d2d_renderer: &mut crate::platform::windows::d2d::Direct2DRenderer,
        skip_indices: &std::collections::HashSet<usize>,
    ) -> Result<(), DrawingError> {
        use crate::types::DrawingTool;

        // 按类型分组元素索引
        let mut rectangles = Vec::new();
        let mut circles = Vec::new();
        let mut arrows = Vec::new();
        let mut pens = Vec::new();
        let mut texts = Vec::new();

        for (i, element) in self.elements.get_elements().iter().enumerate() {
            if skip_indices.contains(&i) {
                continue;
            }
            match element.tool {
                DrawingTool::Rectangle => rectangles.push(i),
                DrawingTool::Circle => circles.push(i),
                DrawingTool::Arrow => arrows.push(i),
                DrawingTool::Pen => pens.push(i),
                DrawingTool::Text => texts.push(i),
                DrawingTool::None => {}
            }
        }

        // 按类型批量绘制，减少工具类型判断开销
        // 注意：保持绘制顺序对于重叠元素很重要，这里按原始顺序绘制同类型元素
        let elements = self.elements.get_elements();

        // 批量绘制矩形
        for &i in &rectangles {
            let _ = self.draw_element_d2d(&elements[i], render_target, d2d_renderer);
        }

        // 批量绘制圆形
        for &i in &circles {
            let _ = self.draw_element_d2d(&elements[i], render_target, d2d_renderer);
        }

        // 批量绘制箭头
        for &i in &arrows {
            let _ = self.draw_element_d2d(&elements[i], render_target, d2d_renderer);
        }

        // 批量绘制画笔路径
        for &i in &pens {
            let _ = self.draw_element_d2d(&elements[i], render_target, d2d_renderer);
        }

        // 批量绘制文本（最后绘制，确保文本在最上层）
        for &i in &texts {
            let _ = self.draw_element_d2d(&elements[i], render_target, d2d_renderer);
        }

        Ok(())
    }

    /// 渲染绘图元素到指定的渲染目标（用于离屏合成）
    ///
    /// 该方法将所有元素渲染到指定的渲染目标，并应用坐标偏移
    /// 用于将绘图元素合成到截图上导出
    pub fn render_elements_to_target(
        &self,
        render_target: &windows::Win32::Graphics::Direct2D::ID2D1RenderTarget,
        d2d_renderer: &mut crate::platform::windows::d2d::Direct2DRenderer,
        selection_rect: &windows::Win32::Foundation::RECT,
    ) -> Result<(), DrawingError> {
        // 计算偏移量：元素坐标是屏幕坐标，需要转换为离屏目标坐标
        let offset_x = -(selection_rect.left as f32);
        let offset_y = -(selection_rect.top as f32);

        // 使用 SetTransform 应用平移变换
        let transform = windows_numerics::Matrix3x2::translation(offset_x, offset_y);
        unsafe {
            render_target.SetTransform(&transform);
        }

        // 渲染所有已提交的元素（使用标准方法，变换由 SetTransform 处理）
        for element in self.elements.get_elements() {
            self.draw_element_d2d(element, render_target, d2d_renderer)?;
        }

        // 渲染当前正在绘制的元素
        if let Some(ref element) = self.current_element {
            self.draw_element_d2d(element, render_target, d2d_renderer)?;
        }

        // 恢复变换
        unsafe {
            render_target.SetTransform(&windows_numerics::Matrix3x2::identity());
        }

        Ok(())
    }

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
            .as_any_mut()
            .downcast_mut::<crate::platform::windows::d2d::Direct2DRenderer>(
        ) {
            // 直接渲染所有元素
            if let Some(render_target) = &d2d_renderer.render_target {
                let target_clone = render_target.clone();
                let element_count = self.elements.get_elements().len();

                // 当元素数量超过阈值时，使用批量绘制优化
                if element_count >= Self::BATCH_RENDER_THRESHOLD {
                    let mut skip_indices = std::collections::HashSet::new();
                    if let Some(idx) = self.selected_element {
                        skip_indices.insert(idx);
                    }
                    let _ =
                        self.render_elements_batched(&target_clone, d2d_renderer, &skip_indices);
                } else {
                    // 小量元素时保持原始顺序绘制
                    let target_interface: &windows::Win32::Graphics::Direct2D::ID2D1RenderTarget =
                        &target_clone;
                    for (i, element) in self.elements.get_elements().iter().enumerate() {
                        if Some(i) != self.selected_element {
                            let _ = self.draw_element_d2d(element, target_interface, d2d_renderer);
                        }
                    }
                }
            }

            // 渲染选中元素（动态）置于顶层
            if let Some(index) = self.selected_element
                && let Some(element) = self.elements.get_elements().get(index)
                && let Some(render_target) = d2d_renderer.render_target.clone()
            {
                let target_interface: &windows::Win32::Graphics::Direct2D::ID2D1RenderTarget =
                    &render_target;
                let _ = self.draw_element_d2d(element, target_interface, d2d_renderer);
            }

            // 渲染当前正在绘制的元素
            if let Some(ref element) = self.current_element
                && let Some(render_target) = d2d_renderer.render_target.clone()
            {
                let target_interface: &windows::Win32::Graphics::Direct2D::ID2D1RenderTarget =
                    &render_target;
                let _ = self.draw_element_d2d(element, target_interface, d2d_renderer);
            }

            // 渲染元素选择（平台无关路径）
            self.draw_element_selection(renderer, selection_rect)?;
        } else {
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

        // Common styles
        let handle_radius = 3.0_f32;
        let handle_fill_color = Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };
        let handle_border_color = Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };
        let selection_border_color = Color {
            r: 0.0,
            g: 0.5,
            b: 1.0,
            a: 1.0,
        };
        let dash_pattern: [f32; 2] = [4.0, 2.0];

        // 箭头特殊：只绘制端点手柄（圆形），不绘制虚线边框（保持原行为）
        if element.tool == DrawingTool::Arrow {
            let style = DrawStyle {
                stroke_color: handle_border_color,
                fill_color: Some(handle_fill_color),
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
                    renderer
                        .draw_circle(*p, handle_radius, &style)
                        .map_err(|e| {
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
                renderer
                    .draw_selection_border(rect, selection_border_color, 1.0, Some(&dash_pattern))
                    .map_err(|e| {
                        DrawingError::RenderError(format!("draw dashed border failed: {e}"))
                    })?;

                // 手柄绘制 - 文本编辑时只显示4个角的手柄
                self.draw_corner_handles_only(
                    renderer,
                    rect,
                    handle_radius,
                    handle_fill_color,
                    handle_border_color,
                )?;
            }
            // 文本选中但未编辑时，或者拖动时，不显示任何UI
        } else {
            // 非文本元素：正常显示边框和8个手柄
            // 虚线边框（使用高层接口）
            renderer
                .draw_selection_border(rect, selection_border_color, 1.0, Some(&dash_pattern))
                .map_err(|e| {
                    DrawingError::RenderError(format!("draw dashed border failed: {e}"))
                })?;

            // 手柄绘制 - 非文本元素显示8个手柄
            renderer
                .draw_element_handles(
                    rect,
                    handle_radius,
                    handle_fill_color,
                    handle_border_color,
                    1.0,
                )
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
        render_target: &windows::Win32::Graphics::Direct2D::ID2D1RenderTarget,
        d2d_renderer: &mut crate::platform::windows::d2d::Direct2DRenderer,
    ) -> Result<(), DrawingError> {
        // Create brush using cached helper
        let color = crate::platform::traits::Color {
            r: element.color.r,
            g: element.color.g,
            b: element.color.b,
            a: element.color.a,
        };

        // Use get_or_create_brush from d2d_renderer
        // Note: This uses d2d_renderer.render_target to create brush.
        // If 'render_target' passed to this function is different (e.g. layer target),
        // brushes are usually compatible if created from same factory/device context chain.
        // In Direct2D, resources created by a render target are generally only usable by that render target.
        // However, ID2D1BitmapRenderTarget is created from ID2D1HwndRenderTarget, so they share resources.

        let brush = d2d_renderer
            .get_or_create_brush(color)
            .map_err(|e| DrawingError::RenderError(format!("Failed to get brush: {e:?}")))?;

        unsafe {
            match element.tool {
                DrawingTool::Text => {
                    if !element.points.is_empty() {
                        // Delegate to text rendering module
                        // Passing d2d_renderer mutably
                        self.draw_text_element_d2d(element, render_target, d2d_renderer)?;
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
                        let center_x = (element.points[0].x + element.points[1].x) as f32 / 2.0;
                        let center_y = (element.points[0].y + element.points[1].y) as f32 / 2.0;
                        let radius_x =
                            (element.points[1].x - element.points[0].x).abs() as f32 / 2.0;
                        let radius_y =
                            (element.points[1].y - element.points[0].y).abs() as f32 / 2.0;

                        // 使用新的辅助函数创建椭圆
                        let ellipse = ellipse(center_x, center_y, radius_x, radius_y);

                        render_target.DrawEllipse(&ellipse, &brush, element.thickness, None);
                    }
                }
                DrawingTool::Arrow => {
                    if element.points.len() >= 2 {
                        let start =
                            crate::utils::d2d_point(element.points[0].x, element.points[0].y);
                        let end = crate::utils::d2d_point(element.points[1].x, element.points[1].y);

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
                                        * (unit_x * arrow_angle.cos() + unit_y * arrow_angle.sin()))
                                        as i32,
                                element.points[1].y
                                    - (arrow_length
                                        * (unit_y * arrow_angle.cos() - unit_x * arrow_angle.sin()))
                                        as i32,
                            );

                            let wing2 = crate::utils::d2d_point(
                                element.points[1].x
                                    - (arrow_length
                                        * (unit_x * arrow_angle.cos() - unit_y * arrow_angle.sin()))
                                        as i32,
                                element.points[1].y
                                    - (arrow_length
                                        * (unit_y * arrow_angle.cos() + unit_x * arrow_angle.sin()))
                                        as i32,
                            );

                            render_target.DrawLine(end, wing1, &brush, element.thickness, None);
                            render_target.DrawLine(end, wing2, &brush, element.thickness, None);
                        }
                    }
                }
                DrawingTool::Pen => {
                    if element.points.len() > 1 {
                        // Try to use cached geometry
                        let mut cached_geometry = element.path_geometry.borrow_mut();

                        if cached_geometry.is_none() {
                            // Create new geometry
                            if let Some(factory) = &d2d_renderer.d2d_factory
                                && let Ok(geometry) = factory.CreatePathGeometry()
                                && let Ok(sink) = geometry.Open()
                            {
                                let start = crate::utils::d2d_point(
                                    element.points[0].x,
                                    element.points[0].y,
                                );

                                sink.BeginFigure(start, D2D1_FIGURE_BEGIN_HOLLOW);

                                // Convert points to D2D1_POINT_2F
                                // Skip the first point as it is the start point
                                for i in 1..element.points.len() {
                                    let p = crate::utils::d2d_point(
                                        element.points[i].x,
                                        element.points[i].y,
                                    );
                                    sink.AddLine(p);
                                }

                                sink.EndFigure(D2D1_FIGURE_END_OPEN);

                                if sink.Close().is_ok() {
                                    *cached_geometry = Some(geometry);
                                }
                            }
                        }

                        // Draw geometry if available
                        if let Some(geometry) = cached_geometry.as_ref() {
                            render_target.DrawGeometry(geometry, &brush, element.thickness, None);
                        } else {
                            // Fallback to simple line drawing if geometry creation failed
                            for i in 0..element.points.len() - 1 {
                                let start = crate::utils::d2d_point(
                                    element.points[i].x,
                                    element.points[i].y,
                                );
                                let end = crate::utils::d2d_point(
                                    element.points[i + 1].x,
                                    element.points[i + 1].y,
                                );
                                render_target.DrawLine(start, end, &brush, element.thickness, None);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

}
