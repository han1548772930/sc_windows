use windows::Win32::Foundation::{POINT, RECT};
use windows::Win32::Graphics::Direct2D::{
    D2D1_DRAW_TEXT_OPTIONS_NONE, ID2D1Factory, ID2D1PathGeometry, ID2D1RenderTarget,
};

use crate::constants::TEXT_PADDING;
use crate::platform::traits::{Color, DrawStyle, Point, Rectangle};
use crate::platform::windows::d2d::Direct2DRenderer;
use crate::rendering::LayerType;
use crate::utils::{
    d2d_helpers::{create_text_format_from_element, create_text_layout_with_style, ellipse},
    d2d_point, d2d_rect_normalized,
};

use super::cache::GeometryCache;
use super::types::{DrawingElement, DrawingTool};
use super::{DrawingError, DrawingManager, ElementInteractionMode};

impl DrawingManager {
    /// 渲染绘图元素到指定的渲染目标（用于离屏合成）
    ///
    /// 该方法将所有元素渲染到指定的渲染目标，并应用坐标偏移
    /// 用于将绘图元素合成到截图上导出
    pub fn render_elements_to_target(
        &self,
        render_target: &ID2D1RenderTarget,
        d2d_renderer: &mut Direct2DRenderer,
        selection_rect: &RECT,
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
    ///
    /// 使用三明治绘制法：
    /// 1. 检查并更新静态层缓存（如果需要）
    /// 2. 绘制缓存的静态层
    /// 3. 绘制动态层（选中元素 + 当前绘制元素 + 选中指示器）
    pub fn render(
        &mut self,
        d2d_renderer: &mut Direct2DRenderer,
        selection_rect: Option<&RECT>,
    ) -> Result<(), DrawingError> {
        // 如果有选择区域，设置裁剪
        if let Some(rect) = selection_rect {
            let clip_rect = Rectangle {
                x: rect.left as f32,
                y: rect.top as f32,
                width: (rect.right - rect.left) as f32,
                height: (rect.bottom - rect.top) as f32,
            };
            d2d_renderer.push_clip_rect(clip_rect).map_err(|e| {
                DrawingError::RenderError(format!("Failed to set clip rect: {e:?}"))
            })?;
        }

        {
            // 检查是否需要重建静态层缓存
            let needs_rebuild = self
                .layer_cache
                .needs_rebuild(LayerType::StaticElements);

            if needs_rebuild {
                // 重建静态层缓存
                let selected_idx = self.selected_element;
                let elements_snapshot: Vec<_> = self.elements.get_elements().to_vec();
                let d2d_factory = d2d_renderer.d2d_factory.clone();

                // 【优化】预先为所有 Pen 元素填充 GeometryCache
                for element in &elements_snapshot {
                    if element.tool == DrawingTool::Pen && element.points.len() > 1 {
                        let element_id = element.id as usize;
                        let points = &element.points;
                        let factory_ref = d2d_factory.clone();
                        // 确保缓存中存在该元素的 geometry
                        let _ = self.geometry_cache.get_or_create_path(element_id, || {
                            Self::create_pen_path_geometry_from_factory(
                                factory_ref.as_ref()?,
                                points,
                            )
                        });
                    }
                }

                // 【优化】直接传递 GeometryCache 引用给闭包，避免 clone HashMap
                let geometry_cache = &self.geometry_cache;
                let rebuild_result = d2d_renderer.update_static_layer_with_context(
                    geometry_cache,
                    |layer_target, renderer, cache| {
                        // 绘制所有非选中的静态元素到 layer_target
                        for (i, element) in elements_snapshot.iter().enumerate() {
                            // 跳过选中元素（它在动态层绘制）
                            if Some(i) == selected_idx {
                                continue;
                            }
                            // 【优化】直接从 GeometryCache 查找 geometry
                            let _ = Self::draw_element_d2d_with_geometry_cache(
                                element,
                                layer_target,
                                renderer,
                                cache,
                            );
                        }
                        Ok(())
                    },
                );

                if rebuild_result.is_ok() {
                    // 标记缓存已重建
                    self.layer_cache.mark_rebuilt(LayerType::StaticElements, 0);
                }
            }

            // 绘制静态层缓存到屏幕
            if d2d_renderer.is_layer_target_valid() {
                let _ = d2d_renderer.draw_static_layer();
            } else {
                // 回退：如果没有缓存，直接绘制所有静态元素
                if let Some(render_target) = &d2d_renderer.render_target {
                    let target_clone = render_target.clone();
                    let target_interface: &ID2D1RenderTarget = &target_clone;
                    for (i, element) in self.elements.get_elements().iter().enumerate() {
                        if Some(i) != self.selected_element {
                            let _ = self.draw_element_d2d(element, target_interface, d2d_renderer);
                        }
                    }
                }
            }

            // 绘制动态层：选中元素（使用 GeometryCache）
            if let Some(index) = self.selected_element
                && let Some(element) = self.elements.get_elements().get(index).cloned()
                && let Some(render_target) = d2d_renderer.render_target.clone()
            {
                let target_interface: &ID2D1RenderTarget = &render_target;
                let _ = self.draw_element_d2d_cached(&element, target_interface, d2d_renderer);
            }

            // 绘制动态层：当前正在绘制的元素
            // 注意：当前绘制元素不使用缓存（因为点集合在不断变化）
            if let Some(ref element) = self.current_element
                && let Some(render_target) = d2d_renderer.render_target.clone()
            {
                let target_interface: &ID2D1RenderTarget = &render_target;
                let _ = Self::draw_element_d2d_static(element, target_interface, d2d_renderer);
            }

            // 绘制动态层：元素选中指示器
            self.draw_element_selection(d2d_renderer, selection_rect)?;
        }

        // 恢复裁剪区域
        if selection_rect.is_some() {
            d2d_renderer.pop_clip_rect().map_err(|e| {
                DrawingError::RenderError(format!("Failed to pop clip rect: {e:?}"))
            })?;
        }

        Ok(())
    }

    /// 静态方法：绘制元素到指定渲染目标（直接从 GeometryCache 查找）
    ///
    /// 此方法用于静态层重建，对于 Pen 元素直接从 GeometryCache 查找已缓存的 PathGeometry，
    /// 避免每次重建时重复创建 geometry，也无需 clone 整个 HashMap。
    fn draw_element_d2d_with_geometry_cache(
        element: &DrawingElement,
        render_target: &ID2D1RenderTarget,
        d2d_renderer: &mut Direct2DRenderer,
        geometry_cache: &GeometryCache,
    ) -> Result<(), DrawingError> {
        let color = Color {
            r: element.color.r,
            g: element.color.g,
            b: element.color.b,
            a: element.color.a,
        };

        let brush = d2d_renderer
            .get_or_create_brush(color)
            .map_err(|e| DrawingError::RenderError(format!("Failed to get brush: {e:?}")))?;

        unsafe {
            match element.tool {
                DrawingTool::Pen => {
                    if element.points.len() > 1 {
                        // 【优化核心】直接从 GeometryCache 查找已缓存的 geometry
                        if let Some(path_geometry) =
                            geometry_cache.get_path(element.id as usize)
                        {
                            render_target.DrawGeometry(
                                path_geometry,
                                &brush,
                                element.thickness,
                                None,
                            );
                        } else {
                            // 回退：如果缓存中没有，则临时创建（不应该发生）
                            if let Some(path_geometry) =
                                Self::create_pen_path_geometry(d2d_renderer, &element.points)
                            {
                                render_target.DrawGeometry(
                                    &path_geometry,
                                    &brush,
                                    element.thickness,
                                    None,
                                );
                            }
                        }
                    }
                }
                // 其他元素类型委托给原有静态方法
                _ => {
                    return Self::draw_element_d2d_static(element, render_target, d2d_renderer);
                }
            }
        }
        Ok(())
    }

    /// 静态方法：绘制元素到指定渲染目标（用于缓存层绘制）
    ///
    /// 这是 draw_element_d2d 的静态版本，不需要 &self 引用
    fn draw_element_d2d_static(
        element: &DrawingElement,
        render_target: &ID2D1RenderTarget,
        d2d_renderer: &mut Direct2DRenderer,
    ) -> Result<(), DrawingError> {
        let color = Color {
            r: element.color.r,
            g: element.color.g,
            b: element.color.b,
            a: element.color.a,
        };

        let brush = d2d_renderer
            .get_or_create_brush(color)
            .map_err(|e| DrawingError::RenderError(format!("Failed to get brush: {e:?}")))?;

        unsafe {
            match element.tool {
                DrawingTool::Text => {
                    if !element.points.is_empty() {
                        if let Some(ref dwrite_factory) = d2d_renderer.dwrite_factory {
                            // 1. 使用辅助函数创建 TextFormat (恢复 Bold/Italic 支持)
                            // 之前这里写死了 DWRITE_FONT_WEIGHT_NORMAL，导致加粗丢失
                            if let Ok(text_format) = create_text_format_from_element(
                                dwrite_factory,
                                &element.font_name,
                                element.font_size,
                                element.font_weight,
                                element.font_italic,
                            )
                            {
                                // 2. 计算带 Padding 的文本区域 (保持位置一致)
                                let padding = TEXT_PADDING;
                                let width = (element.rect.right as f32
                                    - element.rect.left as f32
                                    - padding * 2.0)
                                    .max(0.0);
                                let height = (element.rect.bottom as f32
                                    - element.rect.top as f32
                                    - padding * 2.0)
                                    .max(0.0);

                                // 3. 使用辅助函数创建 TextLayout (恢复 Underline/Strikeout 支持)

                                if let Ok(layout) = create_text_layout_with_style(
                                    dwrite_factory,
                                    &text_format,
                                    &element.text,
                                    width,
                                    height,
                                    element.font_underline,
                                    element.font_strikeout,
                                )
                                {
                                    // 4. 绘制 TextLayout
                                    let origin = d2d_point(
                                        element.rect.left as i32 + padding as i32,
                                        element.rect.top as i32 + padding as i32,
                                    );

                                    render_target.DrawTextLayout(
                                        origin,
                                        &layout,
                                        &brush,
                                        D2D1_DRAW_TEXT_OPTIONS_NONE,
                                    );
                                }
                            }
                        }
                    }
                }
                DrawingTool::Rectangle => {
                    if element.points.len() >= 2 {
                        let rect = d2d_rect_normalized(
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
                        let ellipse = ellipse(center_x, center_y, radius_x, radius_y);
                        render_target.DrawEllipse(&ellipse, &brush, element.thickness, None);
                    }
                }
                DrawingTool::Arrow => {
                    if element.points.len() >= 2 {
                        let start = d2d_point(element.points[0].x, element.points[0].y);
                        let end = d2d_point(element.points[1].x, element.points[1].y);
                        render_target.DrawLine(start, end, &brush, element.thickness, None);

                        // 箭头头部
                        let dx = element.points[1].x - element.points[0].x;
                        let dy = element.points[1].y - element.points[0].y;
                        let length = ((dx * dx + dy * dy) as f64).sqrt();

                        if length > 20.0 {
                            let arrow_length = 15.0f64;
                            let arrow_angle = 0.5f64;
                            let unit_x = dx as f64 / length;
                            let unit_y = dy as f64 / length;

                            let wing1 = d2d_point(
                                element.points[1].x
                                    - (arrow_length
                                        * (unit_x * arrow_angle.cos() + unit_y * arrow_angle.sin()))
                                        as i32,
                                element.points[1].y
                                    - (arrow_length
                                        * (unit_y * arrow_angle.cos() - unit_x * arrow_angle.sin()))
                                        as i32,
                            );
                            let wing2 = d2d_point(
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
                        // 使用 PathGeometry 绘制铅笔路径（比 DrawLine 循环更高效）
                        if let Some(path_geometry) =
                            Self::create_pen_path_geometry(d2d_renderer, &element.points)
                        {
                            render_target.DrawGeometry(
                                &path_geometry,
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
        Ok(())
    }

    /// 从点集合创建 Pen 工具的 PathGeometry
    ///
    /// 使用 PathGeometry 代替多次 DrawLine 调用，提高渲染效率
    fn create_pen_path_geometry(
        d2d_renderer: &Direct2DRenderer,
        points: &[POINT],
    ) -> Option<ID2D1PathGeometry> {
        use windows::Win32::Graphics::Direct2D::Common::{
            D2D1_FIGURE_BEGIN_HOLLOW, D2D1_FIGURE_END_OPEN,
        };

        if points.len() < 2 {
            return None;
        }

        let factory = d2d_renderer.d2d_factory.as_ref()?;

        unsafe {
            // 创建 PathGeometry
            let path_geometry = factory.CreatePathGeometry().ok()?;

            // 打开 GeometrySink
            let sink = path_geometry.Open().ok()?;

            // 移动到第一个点
            let first_point = windows_numerics::Vector2 {
                X: points[0].x as f32,
                Y: points[0].y as f32,
            };
            sink.BeginFigure(first_point, D2D1_FIGURE_BEGIN_HOLLOW);

            // 添加所有后续点作为线段
            for point in points.iter().skip(1) {
                let d2d_point = windows_numerics::Vector2 {
                    X: point.x as f32,
                    Y: point.y as f32,
                };
                sink.AddLine(d2d_point);
            }

            // 结束图形（不闭合）
            sink.EndFigure(D2D1_FIGURE_END_OPEN);
            sink.Close().ok()?;

            Some(path_geometry)
        }
    }

    /// 渲染元素选择指示（虚线边框 + 手柄）
    pub(super) fn draw_element_selection(
        &self,
        renderer: &mut Direct2DRenderer,
        selection_rect: Option<&RECT>,
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

    /// 选中指示器绘制
    fn draw_selected_element_indicators(
        &self,
        renderer: &mut Direct2DRenderer,
        element: &DrawingElement,
        _selection_rect: Option<&RECT>,
    ) -> Result<(), DrawingError> {
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
        renderer: &mut Direct2DRenderer,
        rect: Rectangle,
        handle_radius: f32,
        fill_color: Color,
        border_color: Color,
    ) -> Result<(), DrawingError> {
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

    /// 使用Direct2D渲染单个元素（动态层，使用 GeometryCache）
    ///
    /// 此方法用于渲染动态层元素（选中元素、当前绘制元素）。
    /// 对于 Pen 元素，使用 GeometryCache 缓存 PathGeometry；
    /// 对于文本元素，使用支持光标渲染的 `draw_text_element_d2d`。
    pub(super) fn draw_element_d2d_cached(
        &mut self,
        element: &DrawingElement,
        render_target: &ID2D1RenderTarget,
        d2d_renderer: &mut Direct2DRenderer,
    ) -> Result<(), DrawingError> {
        // 文本元素需要特殊处理（支持光标渲染）
        if element.tool == DrawingTool::Text {
            if !element.points.is_empty() {
                self.draw_text_element_d2d(element, render_target, d2d_renderer)?;
            }
            return Ok(());
        }

        // Pen 元素使用 GeometryCache 缓存 PathGeometry
        if element.tool == DrawingTool::Pen && element.points.len() > 1 {
            let color = Color {
                r: element.color.r,
                g: element.color.g,
                b: element.color.b,
                a: element.color.a,
            };
            let brush = d2d_renderer
                .get_or_create_brush(color)
                .map_err(|e| DrawingError::RenderError(format!("Failed to get brush: {e:?}")))?;

            let element_id = element.id as usize;
            let points = &element.points;
            let d2d_factory = d2d_renderer.d2d_factory.clone();

            // 使用 GeometryCache 获取或创建 PathGeometry
            if let Some(geometry) = self.geometry_cache.get_or_create_path(element_id, || {
                Self::create_pen_path_geometry_from_factory(d2d_factory.as_ref()?, points)
            }) {
                unsafe {
                    render_target.DrawGeometry(geometry, &brush, element.thickness, None);
                }
            }
            return Ok(());
        }

        // 其他元素委托给静态方法
        Self::draw_element_d2d_static(element, render_target, d2d_renderer)
    }

    /// 使用Direct2D渲染单个元素（无缓存版本，用于导出）
    pub(super) fn draw_element_d2d(
        &self,
        element: &DrawingElement,
        render_target: &ID2D1RenderTarget,
        d2d_renderer: &mut Direct2DRenderer,
    ) -> Result<(), DrawingError> {
        // 文本元素需要特殊处理（支持光标渲染）
        if element.tool == DrawingTool::Text {
            if !element.points.is_empty() {
                self.draw_text_element_d2d(element, render_target, d2d_renderer)?;
            }
            return Ok(());
        }

        // 非文本元素委托给静态方法
        Self::draw_element_d2d_static(element, render_target, d2d_renderer)
    }

    /// 从 D2D Factory 创建 Pen 工具的 PathGeometry
    fn create_pen_path_geometry_from_factory(
        factory: &ID2D1Factory,
        points: &[POINT],
    ) -> Option<ID2D1PathGeometry> {
        use windows::Win32::Graphics::Direct2D::Common::{
            D2D1_FIGURE_BEGIN_HOLLOW, D2D1_FIGURE_END_OPEN,
        };

        if points.len() < 2 {
            return None;
        }

        unsafe {
            let path_geometry = factory.CreatePathGeometry().ok()?;
            let sink = path_geometry.Open().ok()?;

            let first_point = windows_numerics::Vector2 {
                X: points[0].x as f32,
                Y: points[0].y as f32,
            };
            sink.BeginFigure(first_point, D2D1_FIGURE_BEGIN_HOLLOW);

            for point in points.iter().skip(1) {
                let d2d_point = windows_numerics::Vector2 {
                    X: point.x as f32,
                    Y: point.y as f32,
                };
                sink.AddLine(d2d_point);
            }

            sink.EndFigure(D2D1_FIGURE_END_OPEN);
            sink.Close().ok()?;

            Some(path_geometry)
        }
    }
}
