use crate::message::{Command, DrawingMessage};
use crate::platform::{PlatformError, PlatformRenderer};
use crate::types::{DrawingElement, DrawingTool};

pub mod elements;
pub mod history;
pub mod tools;

use elements::ElementManager;
use history::HistoryManager;
use tools::ToolManager;

#[derive(Debug, Clone, PartialEq)]
pub enum ElementInteractionMode {
    None,
    Drawing,
    MovingElement,
    ResizingElement(crate::types::DragMode),
}

impl ElementInteractionMode {
    fn from_drag_mode(drag_mode: crate::types::DragMode) -> Self {
        match drag_mode {
            crate::types::DragMode::None => ElementInteractionMode::None,
            crate::types::DragMode::DrawingShape => ElementInteractionMode::Drawing,
            crate::types::DragMode::MovingElement => ElementInteractionMode::MovingElement,
            crate::types::DragMode::ResizingTopLeft
            | crate::types::DragMode::ResizingTopCenter
            | crate::types::DragMode::ResizingTopRight
            | crate::types::DragMode::ResizingMiddleRight
            | crate::types::DragMode::ResizingBottomRight
            | crate::types::DragMode::ResizingBottomCenter
            | crate::types::DragMode::ResizingBottomLeft
            | crate::types::DragMode::ResizingMiddleLeft => {
                ElementInteractionMode::ResizingElement(drag_mode)
            }
            _ => ElementInteractionMode::None,
        }
    }
}

pub struct DrawingManager {
    tools: ToolManager,
    elements: ElementManager,
    history: HistoryManager,
    current_tool: DrawingTool,
    current_element: Option<DrawingElement>,
    selected_element: Option<usize>,
    interaction_mode: ElementInteractionMode,
    mouse_pressed: bool,
    interaction_start_pos: windows::Win32::Foundation::POINT,
    interaction_start_rect: windows::Win32::Foundation::RECT,
    interaction_start_font_size: f32,
    text_editing: bool,
    editing_element_index: Option<usize>,
    text_cursor_pos: usize,
    text_cursor_visible: bool,
    cursor_timer_id: usize,
    just_saved_text: bool,
}

impl DrawingManager {
    /// 创建新的绘图管理器
    pub fn new() -> Result<Self, DrawingError> {
        Ok(Self {
            tools: ToolManager::new(),
            elements: ElementManager::new(),
            history: HistoryManager::new(),
            current_tool: DrawingTool::None,
            current_element: None,
            selected_element: None,

            interaction_mode: ElementInteractionMode::None,
            mouse_pressed: false,
            interaction_start_pos: windows::Win32::Foundation::POINT { x: 0, y: 0 },
            interaction_start_rect: windows::Win32::Foundation::RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            interaction_start_font_size: 0.0,

            text_editing: false,
            editing_element_index: None,
            text_cursor_pos: 0,
            text_cursor_visible: false,
            cursor_timer_id: 1001,
            just_saved_text: false,
        })
    }

    /// 重置状态
    pub fn reset_state(&mut self) {
        self.current_element = None;
        self.selected_element = None;
        self.current_tool = DrawingTool::None;
        self.history.clear();
        self.elements.clear();
        self.interaction_mode = ElementInteractionMode::None;
        self.mouse_pressed = false;
        self.text_editing = false;
        self.editing_element_index = None;
        self.text_cursor_pos = 0;
        self.text_cursor_visible = false;
        self.just_saved_text = false;
    }

    /// 处理绘图消息
    pub fn handle_message(&mut self, message: DrawingMessage) -> Vec<Command> {
        match message {
            DrawingMessage::SelectTool(tool) => {
                let mut commands = Vec::new();

                if self.text_editing {
                    commands.extend(self.stop_text_editing());
                }

                self.current_tool = tool;
                self.tools.set_current_tool(tool);
                self.selected_element = None;
                self.elements.set_selected(None);

                commands.extend(vec![Command::UpdateToolbar, Command::RequestRedraw]);
                commands
            }
            DrawingMessage::StartDrawing(x, y) => {
                if self.current_tool != DrawingTool::None {
                    let mut element = DrawingElement::new(self.current_tool);
                    element
                        .points
                        .push(windows::Win32::Foundation::POINT { x, y });
                    self.current_element = Some(element);
                    vec![Command::RequestRedraw]
                } else {
                    vec![]
                }
            }
            DrawingMessage::UpdateDrawing(x, y) => {
                if let Some(ref mut element) = self.current_element {
                    match self.current_tool {
                        DrawingTool::Pen => {
                            element
                                .points
                                .push(windows::Win32::Foundation::POINT { x, y });
                        }
                        DrawingTool::Rectangle | DrawingTool::Circle | DrawingTool::Arrow => {
                            if element.points.len() >= 2 {
                                element.points[1] = windows::Win32::Foundation::POINT { x, y };
                            } else {
                                element
                                    .points
                                    .push(windows::Win32::Foundation::POINT { x, y });
                            }
                        }
                        _ => {}
                    }
                    element.update_bounding_rect();
                    vec![Command::RequestRedraw]
                } else {
                    vec![]
                }
            }
            DrawingMessage::FinishDrawing => {
                if let Some(element) = self.current_element.take() {
                    self.elements.add_element(element);
                    vec![Command::UpdateToolbar, Command::RequestRedraw]
                } else {
                    vec![]
                }
            }
            DrawingMessage::Undo => {
                if let Some((elements, sel)) = self.history.undo() {
                    self.elements.restore_state(elements);
                    self.selected_element = sel;
                    self.elements.set_selected(self.selected_element);
                    vec![Command::UpdateToolbar, Command::RequestRedraw]
                } else {
                    vec![Command::UpdateToolbar]
                }
            }
            DrawingMessage::Redo => {
                if let Some((elements, sel)) = self.history.redo() {
                    self.elements.restore_state(elements);
                    self.selected_element = sel;
                    self.elements.set_selected(self.selected_element);
                    vec![Command::RequestRedraw]
                } else {
                    vec![]
                }
            }
            DrawingMessage::DeleteElement(index) => {
                self.history
                    .save_state(&self.elements, self.selected_element);
                if self.elements.remove_element(index) {
                    self.selected_element = None;
                    vec![Command::RequestRedraw]
                } else {
                    vec![]
                }
            }
            DrawingMessage::SelectElement(index) => {
                self.selected_element = index;
                self.elements.set_selected(index);

                if let Some(idx) = index {
                    if let Some(element) = self.elements.get_elements().get(idx) {
                        self.current_tool = element.tool;
                        self.tools.set_current_tool(element.tool);
                    }
                }

                vec![Command::UpdateToolbar, Command::RequestRedraw]
            }
            DrawingMessage::AddElement(element) => {
                self.history
                    .save_state(&self.elements, self.selected_element);
                self.elements.add_element(element);
                vec![Command::RequestRedraw]
            }
            DrawingMessage::CheckElementClick(x, y) => {
                if let Some(element_index) = self.elements.get_element_at_position(x, y) {
                    self.selected_element = Some(element_index);
                    self.elements.set_selected(self.selected_element);
                    vec![Command::UpdateToolbar, Command::RequestRedraw]
                } else {
                    self.selected_element = None;
                    self.elements.set_selected(None);
                    vec![Command::UpdateToolbar, Command::RequestRedraw]
                }
            }
        }
    }

    /// 渲染绘图元素（按照原始代码逻辑，添加裁剪支持）
    pub fn render(
        &self,
        renderer: &mut dyn PlatformRenderer<Error = PlatformError>,
        selection_rect: Option<&windows::Win32::Foundation::RECT>,
    ) -> Result<(), DrawingError> {
        // 如果有选择区域，设置裁剪（从原始代码迁移）
        if let Some(rect) = selection_rect {
            let clip_rect = crate::platform::traits::Rectangle {
                x: rect.left as f32,
                y: rect.top as f32,
                width: (rect.right - rect.left) as f32,
                height: (rect.bottom - rect.top) as f32,
            };
            renderer.push_clip_rect(clip_rect).map_err(|e| {
                DrawingError::RenderError(format!("Failed to set clip rect: {:?}", e))
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

        // 恢复裁剪区域（从原始代码迁移）
        if selection_rect.is_some() {
            renderer.pop_clip_rect().map_err(|e| {
                DrawingError::RenderError(format!("Failed to pop clip rect: {:?}", e))
            })?;
        }

        Ok(())
    }

    /// 使用平台无关接口渲染元素选择指示（虚线边框 + 手柄）
    fn draw_element_selection(
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
                        DrawingError::RenderError(format!("draw circle handle failed: {}", e))
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
                        DrawingError::RenderError(format!("draw dashed border failed: {}", e))
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
                    DrawingError::RenderError(format!("draw dashed border failed: {}", e))
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
                    DrawingError::RenderError(format!("draw element handles failed: {}", e))
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
                    DrawingError::RenderError(format!("draw corner handle failed: {}", e))
                })?;
        }

        Ok(())
    }

    // render_element 函数已移除，使用 draw_element_d2d 代替

    /// 使用Direct2D渲染单个元素（从原始代码迁移）
    fn draw_element_d2d(
        &self,
        element: &DrawingElement,
        d2d_renderer: &crate::platform::windows::d2d::Direct2DRenderer,
    ) -> Result<(), DrawingError> {
        use windows::Win32::Graphics::Direct2D::*;

        if let Some(ref render_target) = d2d_renderer.render_target {
            unsafe {
                // 创建画刷（从原始代码迁移）
                let element_brush = render_target.CreateSolidColorBrush(&element.color, None);
                if let Ok(brush) = element_brush {
                    match element.tool {
                        DrawingTool::Text => {
                            if !element.points.is_empty() {
                                self.draw_text_element_d2d(element, d2d_renderer)?;
                            }
                        }
                        DrawingTool::Rectangle => {
                            if element.points.len() >= 2 {
                                // 使用原始代码的helper函数，确保行为一致
                                let rect = crate::utils::d2d_rect(
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

                                // 修复：保持原始行为，允许椭圆而不是强制圆形
                                let ellipse = D2D1_ELLIPSE {
                                    point: windows_numerics::Vector2 {
                                        X: center_x,
                                        Y: center_y,
                                    },
                                    radiusX: radius_x, // 使用实际的X半径
                                    radiusY: radius_y, // 使用实际的Y半径
                                };

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
                                // 使用原始代码的helper函数，确保行为一致
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

                                // 绘制箭头头部（从原始代码迁移）
                                let dx = element.points[1].x - element.points[0].x;
                                let dy = element.points[1].y - element.points[0].y;
                                let length = ((dx * dx + dy * dy) as f64).sqrt();

                                if length > 20.0 {
                                    let arrow_length = 15.0f64;
                                    let arrow_angle = 0.5f64;
                                    let unit_x = dx as f64 / length;
                                    let unit_y = dy as f64 / length;

                                    // 使用原始代码的helper函数，确保行为一致
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
                                    // 使用原始代码的helper函数，确保行为一致
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

    /// 使用Direct2D渲染文本元素（从原始代码完整迁移，支持多行、内边距、光标）
    fn draw_text_element_d2d(
        &self,
        element: &DrawingElement,
        d2d_renderer: &crate::platform::windows::d2d::Direct2DRenderer,
    ) -> Result<(), DrawingError> {
        if element.points.is_empty() {
            return Ok(());
        }

        if let (Some(render_target), Some(dwrite_factory)) =
            (&d2d_renderer.render_target, &d2d_renderer.dwrite_factory)
        {
            unsafe {
                // 计算文本区域（从原始代码迁移）
                let text_rect = if element.points.len() >= 2 {
                    crate::utils::d2d_rect(
                        element.points[0].x,
                        element.points[0].y,
                        element.points[1].x,
                        element.points[1].y,
                    )
                } else if !element.points.is_empty() {
                    // 如果只有一个点，使用默认大小
                    crate::utils::d2d_rect(
                        element.points[0].x,
                        element.points[0].y,
                        element.points[0].x + crate::constants::DEFAULT_TEXT_WIDTH,
                        element.points[0].y + crate::constants::DEFAULT_TEXT_HEIGHT,
                    )
                } else {
                    return Ok(());
                };

                // 使用元素自身的颜色属性创建文本画刷
                let font_color = element.color;

                // 创建文本画刷（使用元素颜色）
                let text_brush = render_target.CreateSolidColorBrush(&font_color, None);

                if let Ok(brush) = text_brush {
                    // 添加内边距（从原始代码迁移）
                    let text_content_rect =
                        windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F {
                            left: text_rect.left + crate::constants::TEXT_PADDING,
                            top: text_rect.top + crate::constants::TEXT_PADDING,
                            right: text_rect.right - crate::constants::TEXT_PADDING,
                            bottom: text_rect.bottom - crate::constants::TEXT_PADDING,
                        };

                    // 支持多行文字显示（从原始代码迁移）
                    let lines: Vec<&str> = if element.text.is_empty() {
                        vec![""] // 空文本时显示一个空行（用于显示光标）
                    } else {
                        element.text.lines().collect()
                    };

                    let font_size = element.font_size.max(12.0); // 最小字体大小12
                    let line_height = font_size * 1.2;
                    let font_name_wide = crate::utils::to_wide_chars(&element.font_name);
                    let font_weight = if element.font_weight > 400 {
                        windows::Win32::Graphics::DirectWrite::DWRITE_FONT_WEIGHT_BOLD
                    } else {
                        windows::Win32::Graphics::DirectWrite::DWRITE_FONT_WEIGHT_NORMAL
                    };
                    let font_style = if element.font_italic {
                        windows::Win32::Graphics::DirectWrite::DWRITE_FONT_STYLE_ITALIC
                    } else {
                        windows::Win32::Graphics::DirectWrite::DWRITE_FONT_STYLE_NORMAL
                    };

                    // 创建动态字体格式，使用设置中的字体属性（与旧代码保持一致）
                    if let Ok(text_format) = dwrite_factory.CreateTextFormat(
                        windows::core::PCWSTR(font_name_wide.as_ptr()),
                        None,
                        font_weight,
                        font_style,
                        windows::Win32::Graphics::DirectWrite::DWRITE_FONT_STRETCH_NORMAL,
                        font_size,
                        windows::core::w!(""),
                    ) {
                        // 设置文本对齐
                        let _ = text_format.SetTextAlignment(
                            windows::Win32::Graphics::DirectWrite::DWRITE_TEXT_ALIGNMENT_LEADING,
                        );
                        let _ = text_format.SetParagraphAlignment(
                            windows::Win32::Graphics::DirectWrite::DWRITE_PARAGRAPH_ALIGNMENT_NEAR,
                        );

                        // 逐行绘制文本（从原始代码迁移）
                        for (line_index, line) in lines.iter().enumerate() {
                            let line_y = text_content_rect.top + (line_index as f32 * line_height);

                            // 检查是否超出文本区域
                            if line_y + line_height > text_content_rect.bottom {
                                break;
                            }

                            let line_rect =
                                windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F {
                                    left: text_content_rect.left,
                                    top: line_y,
                                    right: text_content_rect.right,
                                    bottom: line_y + line_height,
                                };

                            // 转换文本为宽字符
                            let wide_text: Vec<u16> = line.encode_utf16().collect();

                            // 如果需要下划线或删除线，使用TextLayout；否则使用简单的DrawText
                            if element.font_underline || element.font_strikeout {
                                // 创建文本布局以支持下划线和删除线（使用元素属性）
                                if let Ok(text_layout) = dwrite_factory.CreateTextLayout(
                                    &wide_text,
                                    &text_format,
                                    line_rect.right - line_rect.left,
                                    line_rect.bottom - line_rect.top,
                                ) {
                                    // 应用下划线和删除线（与旧代码保持一致）
                                    if element.font_underline && !wide_text.is_empty() {
                                        let _ = text_layout.SetUnderline(
                                            true,
                                            windows::Win32::Graphics::DirectWrite::DWRITE_TEXT_RANGE {
                                                startPosition: 0,
                                                length: wide_text.len() as u32,
                                            },
                                        );
                                    }
                                    if element.font_strikeout && !wide_text.is_empty() {
                                        let _ = text_layout.SetStrikethrough(
                                            true,
                                            windows::Win32::Graphics::DirectWrite::DWRITE_TEXT_RANGE {
                                                startPosition: 0,
                                                length: wide_text.len() as u32,
                                            },
                                        );
                                    }

                                    // 绘制文本布局（与旧代码保持一致）
                                    render_target.DrawTextLayout(
                                        crate::utils::d2d_point(line_rect.left as i32, line_rect.top as i32),
                                        &text_layout,
                                        &brush,
                                        windows::Win32::Graphics::Direct2D::D2D1_DRAW_TEXT_OPTIONS_NONE,
                                    );
                                }
                            } else {
                                // 简单文本绘制（无特殊样式）
                                render_target.DrawText(
                                    &wide_text,
                                    &text_format,
                                    &line_rect,
                                    &brush,
                                    windows::Win32::Graphics::Direct2D::D2D1_DRAW_TEXT_OPTIONS_NONE,
                                    windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL,
                                );
                            }
                        }

                        // 绘制文本输入光标（caret），仅当该元素正在编辑且光标可见
                        if self.text_editing && self.text_cursor_visible {
                            if let Some(edit_idx) = self.editing_element_index {
                                // 通过指针相等找到当前元素索引
                                if let Some(current_idx) = self
                                    .elements
                                    .get_elements()
                                    .iter()
                                    .position(|e| std::ptr::eq(e, element))
                                {
                                    if current_idx == edit_idx {
                                        // 基于字符计算当前行与列
                                        let before: String = element
                                            .text
                                            .chars()
                                            .take(self.text_cursor_pos)
                                            .collect();
                                        let lines_before: Vec<&str> = before.lines().collect();
                                        let caret_line = if before.ends_with('\n') {
                                            lines_before.len()
                                        } else {
                                            lines_before.len().saturating_sub(1)
                                        };
                                        let current_line_text = if before.ends_with('\n') {
                                            ""
                                        } else {
                                            lines_before.last().copied().unwrap_or("")
                                        };
                                        // 使用 DirectWrite 精确测量光标前文本宽度
                                        let before_wide: Vec<u16> =
                                            current_line_text.encode_utf16().collect();
                                        let mut caret_x = text_content_rect.left;
                                        if let Ok(layout) = dwrite_factory.CreateTextLayout(
                                            &before_wide,
                                            &text_format,
                                            f32::MAX,
                                            f32::MAX,
                                        ) {
                                            let mut metrics = windows::Win32::Graphics::DirectWrite::DWRITE_TEXT_METRICS::default();
                                            let _ = layout.GetMetrics(&mut metrics);
                                            caret_x += metrics.width;
                                        }

                                        let caret_y_top = text_content_rect.top
                                            + (caret_line as f32) * line_height;
                                        let caret_y_bottom = (caret_y_top + line_height)
                                            .min(text_content_rect.bottom);

                                        let caret_rect = windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F {
                                            left: caret_x,
                                            top: caret_y_top,
                                            right: caret_x + 1.0,
                                            bottom: caret_y_bottom,
                                        };

                                        if let Ok(cursor_brush) = render_target
                                            .CreateSolidColorBrush(
                                                &crate::constants::COLOR_TEXT_CURSOR,
                                                None,
                                            )
                                        {
                                            render_target.FillRectangle(&caret_rect, &cursor_brush);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    // save_history 方法已移除，使用 HistoryManager 代替

    /// 检查是否可以撤销（从WindowState迁移）
    pub fn can_undo(&self) -> bool {
        // 使用新的历史管理器而不是legacy history_stack
        self.history.can_undo()
    }

    // 移除 legacy 接口，统一使用 ElementManager/HistoryManager

    /// 处理鼠标移动（从原始代码迁移，支持拖拽模式）
    /// 返回 (命令列表, 是否消费了事件)
    pub fn handle_mouse_move(
        &mut self,
        x: i32,
        y: i32,
        selection_rect: Option<windows::Win32::Foundation::RECT>,
    ) -> (Vec<Command>, bool) {
        if self.mouse_pressed {
            // 添加拖拽距离阈值检查（与旧代码保持一致）
            let dx = (x - self.interaction_start_pos.x).abs();
            let dy = (y - self.interaction_start_pos.y).abs();

            // 只有当移动距离超过阈值时才开始真正的拖拽
            if dx > crate::constants::DRAG_THRESHOLD || dy > crate::constants::DRAG_THRESHOLD {
                self.update_drag(x, y, selection_rect);
                (vec![Command::RequestRedraw], true)
            } else {
                // 移动距离不够，不进行拖拽，但仍然消费事件（因为鼠标已按下）
                (vec![], true)
            }
        } else {
            // 检查是否悬停在元素上（用于改变光标/预览）
            if let Some(_index) = self.elements.get_element_at_position(x, y) {
                // 可在后续添加悬停反馈，但不消费事件
                (vec![], false)
            } else {
                (vec![], false)
            }
        }
    }

    /// 更新拖拽操作（从原始代码迁移）
    fn update_drag(
        &mut self,
        x: i32,
        y: i32,
        selection_rect: Option<windows::Win32::Foundation::RECT>,
    ) {
        match &self.interaction_mode {
            ElementInteractionMode::Drawing => {
                if let Some(ref mut element) = self.current_element {
                    // 如果有选择框，限制绘制在选择框内（从原始代码迁移）
                    let (clamped_x, clamped_y) = if let Some(rect) = selection_rect {
                        (
                            x.max(rect.left).min(rect.right),
                            y.max(rect.top).min(rect.bottom),
                        )
                    } else {
                        (x, y)
                    };

                    match element.tool {
                        DrawingTool::Pen => {
                            element.points.push(windows::Win32::Foundation::POINT {
                                x: clamped_x,
                                y: clamped_y,
                            });
                        }
                        DrawingTool::Rectangle | DrawingTool::Circle | DrawingTool::Arrow => {
                            if element.points.is_empty() {
                                element.points.push(self.interaction_start_pos);
                            }
                            if element.points.len() == 1 {
                                element.points.push(windows::Win32::Foundation::POINT {
                                    x: clamped_x,
                                    y: clamped_y,
                                });
                            } else {
                                element.points[1] = windows::Win32::Foundation::POINT {
                                    x: clamped_x,
                                    y: clamped_y,
                                };
                            }
                            // 更新rect信息（用于边界检查）
                            let start = &element.points[0];
                            let end = &element.points[1];
                            element.rect = windows::Win32::Foundation::RECT {
                                left: start.x.min(end.x),
                                top: start.y.min(end.y),
                                right: start.x.max(end.x),
                                bottom: start.y.max(end.y),
                            };
                        }
                        _ => {}
                    }
                }
            }
            ElementInteractionMode::MovingElement => {
                if let Some(index) = self.selected_element {
                    if let Some(element) = self.elements.get_elements().get(index) {
                        if element.tool != DrawingTool::Pen {
                            let dx = x - self.interaction_start_pos.x;
                            let dy = y - self.interaction_start_pos.y;
                            if let Some(el) = self.elements.get_element_mut(index) {
                                let current_dx = el.rect.left - self.interaction_start_rect.left;
                                let current_dy = el.rect.top - self.interaction_start_rect.top;
                                el.move_by(-current_dx, -current_dy);
                                el.move_by(dx, dy);
                            }
                        }
                    }
                }
            }
            ElementInteractionMode::ResizingElement(resize_mode) => {
                if let Some(index) = self.selected_element {
                    if let Some(el) = self.elements.get_element_mut(index) {
                        if el.tool == DrawingTool::Pen {
                            return;
                        }
                        let mut new_rect = self.interaction_start_rect;
                        let dx = x - self.interaction_start_pos.x;
                        let dy = y - self.interaction_start_pos.y;
                        match resize_mode {
                            crate::types::DragMode::ResizingTopLeft => {
                                new_rect.left += dx;
                                new_rect.top += dy;
                            }
                            crate::types::DragMode::ResizingTopCenter => {
                                new_rect.top += dy;
                            }
                            crate::types::DragMode::ResizingTopRight => {
                                new_rect.right += dx;
                                new_rect.top += dy;
                            }
                            crate::types::DragMode::ResizingMiddleRight => {
                                new_rect.right += dx;
                            }
                            crate::types::DragMode::ResizingBottomRight => {
                                new_rect.right += dx;
                                new_rect.bottom += dy;
                            }
                            crate::types::DragMode::ResizingBottomCenter => {
                                new_rect.bottom += dy;
                            }
                            crate::types::DragMode::ResizingBottomLeft => {
                                new_rect.left += dx;
                                new_rect.bottom += dy;
                            }
                            crate::types::DragMode::ResizingMiddleLeft => {
                                new_rect.left += dx;
                            }
                            _ => {}
                        }

                        match el.tool {
                            DrawingTool::Arrow => {
                                // 仅支持通过左上角/右下角手柄调整起点/终点（与旧逻辑一致）
                                if el.points.len() >= 2 {
                                    match resize_mode {
                                        crate::types::DragMode::ResizingTopLeft => {
                                            el.points[0] =
                                                windows::Win32::Foundation::POINT { x, y };
                                            el.update_bounding_rect();
                                        }
                                        crate::types::DragMode::ResizingBottomRight => {
                                            el.points[1] =
                                                windows::Win32::Foundation::POINT { x, y };
                                            el.update_bounding_rect();
                                        }
                                        _ => {
                                            // 其他手柄对箭头不生效
                                        }
                                    }
                                }
                            }
                            DrawingTool::Text => {
                                // 文本元素：按比例缩放并同步字体大小（基于拖拽起始字号）
                                let original_width = (self.interaction_start_rect.right
                                    - self.interaction_start_rect.left)
                                    .max(1);
                                let original_height = (self.interaction_start_rect.bottom
                                    - self.interaction_start_rect.top)
                                    .max(1);

                                // 计算比例（不同手柄采用对应方向）
                                let (scale_x, scale_y) = match resize_mode {
                                    crate::types::DragMode::ResizingTopLeft => (
                                        (original_width - dx) as f32 / original_width as f32,
                                        (original_height - dy) as f32 / original_height as f32,
                                    ),
                                    crate::types::DragMode::ResizingTopRight => (
                                        (original_width + dx) as f32 / original_width as f32,
                                        (original_height - dy) as f32 / original_height as f32,
                                    ),
                                    crate::types::DragMode::ResizingBottomRight => (
                                        (original_width + dx) as f32 / original_width as f32,
                                        (original_height + dy) as f32 / original_height as f32,
                                    ),
                                    crate::types::DragMode::ResizingBottomLeft => (
                                        (original_width - dx) as f32 / original_width as f32,
                                        (original_height + dy) as f32 / original_height as f32,
                                    ),
                                    crate::types::DragMode::ResizingTopCenter => (
                                        1.0,
                                        (original_height - dy) as f32 / original_height as f32,
                                    ),
                                    crate::types::DragMode::ResizingBottomCenter => (
                                        1.0,
                                        (original_height + dy) as f32 / original_height as f32,
                                    ),
                                    crate::types::DragMode::ResizingMiddleLeft => {
                                        ((original_width - dx) as f32 / original_width as f32, 1.0)
                                    }
                                    crate::types::DragMode::ResizingMiddleRight => {
                                        ((original_width + dx) as f32 / original_width as f32, 1.0)
                                    }
                                    _ => (1.0, 1.0),
                                };

                                let scale = scale_x.min(scale_y).max(0.1);
                                // 使用兼容性方法设置字体大小
                                let new_font_size =
                                    (self.interaction_start_font_size * scale as f32).max(8.0);
                                el.set_font_size(new_font_size);

                                // 重要：根据新的字体大小重新计算文本框尺寸（与旧代码保持一致）
                                // 先应用新的矩形，然后调用update_text_element_size来精确调整
                                el.resize(new_rect);

                                // 获取当前元素索引并调用文本尺寸更新函数
                                if let Some(element_index) = self.selected_element {
                                    self.update_text_element_size(element_index);
                                }
                            }
                            _ => {
                                // 其他元素：按矩形调整
                                el.resize(new_rect);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// 检测指定元素矩形上的手柄命中（参考旧代码逻辑）
    pub fn get_element_handle_at_position(
        &self,
        x: i32,
        y: i32,
        rect: &windows::Win32::Foundation::RECT,
        tool: DrawingTool,
        element_index: usize,
    ) -> crate::types::DragMode {
        let _detection_radius = crate::constants::HANDLE_DETECTION_RADIUS as i32;

        // 获取元素的点集合（用于箭头等特殊元素）
        let element_points = if let Some(element) = self.elements.get_elements().get(element_index)
        {
            Some(element.points.as_slice())
        } else {
            None
        };

        // 使用统一的绘图元素手柄检测函数
        // 根据工具类型选择合适的配置
        let config = match tool {
            crate::types::DrawingTool::Arrow => {
                // 箭头元素的特殊处理
                let detection_radius = crate::constants::HANDLE_DETECTION_RADIUS as i32;
                if let Some(points) = element_points {
                    if points.len() >= 2 {
                        let start = points[0];
                        let end = points[1];
                        let dx = x - start.x;
                        let dy = y - start.y;
                        if dx * dx + dy * dy <= detection_radius * detection_radius {
                            return crate::types::DragMode::ResizingTopLeft;
                        }
                        let dx2 = x - end.x;
                        let dy2 = y - end.y;
                        if dx2 * dx2 + dy2 * dy2 <= detection_radius * detection_radius {
                            return crate::types::DragMode::ResizingBottomRight;
                        }
                    }
                }
                return crate::types::DragMode::None;
            }
            crate::types::DrawingTool::Text => crate::utils::HandleConfig::Corners,
            _ => crate::utils::HandleConfig::Full,
        };

        // 委托给统一的检测函数
        crate::utils::detect_handle_at_position_unified(x, y, rect, config, false)
    }

    /// 处理鼠标按下（从原始代码迁移，支持拖拽模式）
    /// 返回 (命令列表, 是否消费了事件)
    pub fn handle_mouse_down(
        &mut self,
        x: i32,
        y: i32,
        selection_rect: Option<windows::Win32::Foundation::RECT>,
    ) -> (Vec<Command>, bool) {
        // 重置标志，下次点击可以创建新文本（与原代码保持一致）
        // 注意：这必须在函数开始时重置，确保每次新的点击事件都会重置状态
        self.just_saved_text = false;

        // 约束：除UI外，绘图交互仅在选择框内生效（保持与原始逻辑一致）
        let inside_selection = match selection_rect {
            Some(r) => x >= r.left && x <= r.right && y >= r.top && y <= r.bottom,
            None => true,
        };

        // 文本编辑状态下的特殊处理（从原始代码迁移）
        if self.text_editing {
            if let Some(editing_index) = self.editing_element_index {
                // 检查是否点击了正在编辑的文本元素
                if let Some(element) = self.elements.get_elements().get(editing_index) {
                    if element.contains_point(x, y) {
                        // 点击正在编辑的文本元素，检查是否点击了手柄
                        let handle_mode = self.get_element_handle_at_position(
                            x,
                            y,
                            &element.rect,
                            element.tool,
                            editing_index,
                        );
                        if handle_mode != crate::types::DragMode::None {
                            // 点击了手柄，开始拖拽
                            self.interaction_mode =
                                ElementInteractionMode::from_drag_mode(handle_mode);
                            self.mouse_pressed = true;
                            self.interaction_start_pos = windows::Win32::Foundation::POINT { x, y };
                            self.interaction_start_rect = element.rect;
                            self.interaction_start_font_size = element.font_size;
                            return (vec![Command::RequestRedraw], true);
                        }
                        // 点击文本内容区域，继续编辑；返回重绘以消费事件，避免上层退出
                        return (vec![Command::RequestRedraw], true);
                    }
                }
                // 点击了其他地方，停止编辑并立即返回（修复空文本框删除问题）
                // 避免后续逻辑干扰 stop_text_editing 的清理操作
                let stop_commands = self.stop_text_editing();
                return (stop_commands, true);
            }
        }

        // 文本工具特殊处理（从原始代码迁移）
        if inside_selection
            && self.current_tool == DrawingTool::Text
            && !self.text_editing
            && !self.just_saved_text
        {
            // 检查是否点击了任何现有元素（与原始代码保持一致）
            if let Some(idx) = self.elements.get_element_at_position(x, y) {
                // 如果点击的是文本元素，选择它
                if let Some(element) = self.elements.get_elements().get(idx) {
                    if element.tool == DrawingTool::Text {
                        // 先获取元素信息，避免借用冲突
                        let element_rect = element.rect;
                        let element_font_size = element.font_size;

                        // 单击只选择文本元素，不进入编辑模式（双击才进入编辑模式）
                        self.handle_message(DrawingMessage::SelectElement(Some(idx)));

                        // 立即设置拖动状态，就像原代码那样（修复文本无法拖动的问题）
                        self.interaction_mode = ElementInteractionMode::MovingElement;
                        self.mouse_pressed = true;
                        self.interaction_start_pos = windows::Win32::Foundation::POINT { x, y };
                        self.interaction_start_rect = element_rect;
                        self.interaction_start_font_size = element_font_size;

                        return (vec![Command::UpdateToolbar, Command::RequestRedraw], true);
                    }
                }
                // 点击了其他类型元素：不在此处处理，继续后续通用元素命中逻辑（允许选择并拖动该元素）
            } else {
                // 未命中任何元素：在选框内空白处创建新文本并直接进入编辑
                return self.create_and_edit_text_element(x, y);
            }
        }

        // 选择框外：不消费，让Screenshot处理（例如拖拽选择框）
        if !inside_selection {
            // 但如果当前没有选择框（None），仍应允许绘图交互
            if selection_rect.is_some() {
                return (vec![], false);
            }
        }

        // 先尝试与现有元素交互（无论当前工具是什么）
        // 1) 已选元素的手柄优先 - 但必须在选择框内
        if inside_selection {
            if let Some(sel_idx) = self.selected_element {
                if let Some(element) = self.elements.get_elements().get(sel_idx) {
                    if element.tool != DrawingTool::Pen {
                        let handle_mode = self.get_element_handle_at_position(
                            x,
                            y,
                            &element.rect,
                            element.tool,
                            sel_idx,
                        );
                        if handle_mode != crate::types::DragMode::None {
                            self.interaction_mode =
                                ElementInteractionMode::from_drag_mode(handle_mode);
                            self.mouse_pressed = true;
                            self.interaction_start_pos = windows::Win32::Foundation::POINT { x, y };
                            self.interaction_start_rect = element.rect;
                            self.interaction_start_font_size = element.font_size;
                            return (vec![Command::RequestRedraw], true);
                        }
                        // 2) 已选元素内部（移动）- 也必须在选择框内且元素可见
                        if element.contains_point(x, y) {
                            // 检查元素是否在选择框内可见（与原代码逻辑一致）
                            let element_visible = if let Some(sel_rect) = selection_rect {
                                self.elements
                                    .is_element_visible_in_selection(element, &sel_rect)
                            } else {
                                true
                            };

                            if element_visible {
                                self.interaction_mode = ElementInteractionMode::MovingElement;
                                self.mouse_pressed = true;
                                self.interaction_start_pos =
                                    windows::Win32::Foundation::POINT { x, y };
                                self.interaction_start_rect = element.rect;
                                return (vec![Command::RequestRedraw], true);
                            }
                        }
                    }
                }
            }
        }

        // 3) 检查是否点击其他元素 - 但必须在选择框内
        if inside_selection {
            if let Some(idx) =
                self.elements
                    .get_element_at_position_with_selection(x, y, selection_rect)
            {
                // 先获取元素信息，避免借用冲突
                let (element_tool, element_rect, element_font_size) = {
                    if let Some(element) = self.elements.get_elements().get(idx) {
                        if element.tool == DrawingTool::Pen {
                            return (vec![], false); // 返回空命令，不消费此事件
                        }

                        (element.tool, element.rect, element.font_size)
                    } else {
                        return (vec![], false);
                    }
                };

                // 如果是画笔元素，不允许选择（与旧代码保持一致）
                if element_tool == DrawingTool::Pen {
                    // 笔画不能被选择，直接返回空命令
                    return (vec![], false);
                }

                // 选择该元素（仅非笔画元素）
                self.handle_message(DrawingMessage::SelectElement(Some(idx)));

                // 设置交互起始状态
                self.interaction_start_rect = element_rect;
                self.interaction_start_pos = windows::Win32::Foundation::POINT { x, y };
                self.interaction_start_font_size = element_font_size;

                // 检查是否点击了手柄（与原代码逻辑一致）
                let handle_mode =
                    self.get_element_handle_at_position(x, y, &element_rect, element_tool, idx);

                if handle_mode != crate::types::DragMode::None {
                    // 点击了手柄，开始手柄拖拽
                    self.interaction_mode = ElementInteractionMode::from_drag_mode(handle_mode);
                    self.mouse_pressed = true;
                    return (vec![Command::UpdateToolbar, Command::RequestRedraw], true);
                } else {
                    // 没有点击手柄，立即开始移动元素（与原代码逻辑一致）
                    self.interaction_mode = ElementInteractionMode::MovingElement;
                    self.mouse_pressed = true;
                    return (vec![Command::UpdateToolbar, Command::RequestRedraw], true);
                }
            }
        }

        // 4) 若没有元素命中，且选择了绘图工具，则尝试开始绘制
        if self.current_tool != DrawingTool::None {
            if inside_selection {
                self.interaction_start_pos = windows::Win32::Foundation::POINT { x, y };
                self.start_drawing_shape(x, y);
                self.mouse_pressed = true;
                return (vec![Command::RequestRedraw], true);
            }
            // 不在选择框内，不消费事件
            return (vec![], false);
        }

        // 5) 工具为None且未命中元素：清除选中，但不消费事件（让 ScreenshotManager 有机会处理）
        if self.selected_element.is_some() {
            // 只有当确实有选中元素需要清除时才处理
            self.selected_element = None;
            self.elements.set_selected(None);
            (vec![Command::UpdateToolbar, Command::RequestRedraw], true)
        } else {
            // 没有选中元素，不消费事件
            (vec![], false)
        }
    }

    /// 开始绘制图形（从原始代码迁移）
    fn start_drawing_shape(&mut self, x: i32, y: i32) {
        // 在开始新的绘制前清除元素选择（保持原始行为）
        self.selected_element = None;
        self.elements.set_selected(None);

        // 保存历史状态（在操作开始前保存，以便精确撤销）
        self.history
            .save_state(&self.elements, self.selected_element);

        // 设置交互模式为绘制图形
        self.interaction_mode = ElementInteractionMode::Drawing;

        // 创建新元素
        let mut new_element = DrawingElement::new(self.current_tool);
        if self.current_tool == DrawingTool::Text {
            // 文本元素使用字体颜色与字体设置（修复：使用正确的font_color而不是text_color）
            let settings = crate::settings::Settings::load();
            new_element.color = windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F {
                r: settings.font_color.0 as f32 / 255.0,
                g: settings.font_color.1 as f32 / 255.0,
                b: settings.font_color.2 as f32 / 255.0,
                a: 1.0,
            };
            new_element.font_size = settings.font_size;
            new_element.font_name = settings.font_name.clone();
            new_element.font_weight = settings.font_weight;
            new_element.font_italic = settings.font_italic;
            new_element.font_underline = settings.font_underline;
            new_element.font_strikeout = settings.font_strikeout;
        } else {
            // 其他元素使用绘图颜色与线宽（从 ToolManager 获取）
            new_element.color = self.tools.get_brush_color();
            new_element.thickness = self.tools.get_line_thickness();
        }

        match self.current_tool {
            DrawingTool::Pen => {
                new_element
                    .points
                    .push(windows::Win32::Foundation::POINT { x, y });
            }
            DrawingTool::Rectangle | DrawingTool::Circle | DrawingTool::Arrow => {
                new_element
                    .points
                    .push(windows::Win32::Foundation::POINT { x, y });
            }
            DrawingTool::Text => {
                new_element
                    .points
                    .push(windows::Win32::Foundation::POINT { x, y });
            }
            _ => {}
        }

        self.current_element = Some(new_element);
    }

    /// 处理鼠标释放（从原始代码迁移，支持拖拽模式）
    /// 返回 (命令列表, 是否消费了事件)
    pub fn handle_mouse_up(&mut self, _x: i32, _y: i32) -> (Vec<Command>, bool) {
        if self.mouse_pressed {
            self.end_drag();
            self.mouse_pressed = false;
            self.interaction_mode = ElementInteractionMode::None;
            (vec![Command::RequestRedraw], true)
        } else {
            (vec![], false)
        }
    }

    /// 结束拖拽操作（从原始代码迁移）
    fn end_drag(&mut self) {
        if self.interaction_mode == ElementInteractionMode::Drawing {
            if let Some(mut element) = self.current_element.take() {
                // 根据不同工具类型判断是否保存
                let should_save = match element.tool {
                    DrawingTool::Pen => {
                        // 手绘工具：至少要有2个点
                        element.points.len() > 1
                    }
                    DrawingTool::Rectangle | DrawingTool::Circle | DrawingTool::Arrow => {
                        // 形状工具：检查尺寸
                        if element.points.len() >= 2 {
                            let dx = (element.points[1].x - element.points[0].x).abs();
                            let dy = (element.points[1].y - element.points[0].y).abs();
                            dx > 5 || dy > 5 // 至少有一个方向大于5像素
                        } else {
                            false
                        }
                    }
                    DrawingTool::Text => {
                        // 文本工具：必须有位置点且文本内容不为空
                        !element.points.is_empty() && !element.text.trim().is_empty()
                    }
                    _ => false,
                };

                if should_save {
                    // 关键：保存前更新边界矩形
                    element.update_bounding_rect();
                    // 通过 ElementManager 添加元素
                    self.elements.add_element(element);
                }
            }
        }
    }

    /// 处理键盘输入（从原始代码迁移，支持文本编辑）
    pub fn handle_key_input(&mut self, key: u32) -> Vec<Command> {
        // 处理文字编辑相关按键（从原始代码迁移）
        if self.text_editing {
            match key {
                0x1B => {
                    // VK_ESCAPE - 退出文字编辑模式
                    return self.stop_text_editing();
                }
                0x0D => {
                    // VK_RETURN - 插入换行符
                    return self.handle_text_input('\n');
                }
                0x08 => {
                    // VK_BACK - 退格键删除字符
                    return self.handle_backspace();
                }
                0x25 => {
                    // VK_LEFT - 光标向左移动
                    return self.move_cursor_left();
                }
                0x27 => {
                    // VK_RIGHT - 光标向右移动
                    return self.move_cursor_right();
                }
                0x24 => {
                    // VK_HOME - 光标移动到行首
                    return self.move_cursor_to_line_start();
                }
                0x23 => {
                    // VK_END - 光标移动到行尾
                    return self.move_cursor_to_line_end();
                }
                0x26 => {
                    // VK_UP - 光标向上移动一行
                    return self.move_cursor_up();
                }
                0x28 => {
                    // VK_DOWN - 光标向下移动一行
                    return self.move_cursor_down();
                }
                _ => {}
            }
        }

        // 常规键盘快捷键
        match key {
            // Ctrl+Z - 撤销
            26 => self.handle_message(DrawingMessage::Undo),
            // Ctrl+Y - 重做
            25 => self.handle_message(DrawingMessage::Redo),
            // Delete - 删除选中元素
            46 => {
                if let Some(index) = self.selected_element {
                    self.handle_message(DrawingMessage::DeleteElement(index))
                } else {
                    vec![]
                }
            }
            _ => vec![],
        }
    }

    /// 获取当前拖拽模式（用于光标显示）：
    /// - 元素移动 => Moving
    /// - 元素调整大小 => 返回对应的 Resizing* 模式
    /// - 非拖拽 => None
    pub fn get_current_drag_mode(&self) -> Option<crate::types::DragMode> {
        match &self.interaction_mode {
            ElementInteractionMode::MovingElement => Some(crate::types::DragMode::Moving),
            ElementInteractionMode::ResizingElement(mode) => Some(*mode),
            _ => None,
        }
    }

    /// 是否正在进行任何绘图交互（绘制/移动/调整）
    pub fn is_dragging(&self) -> bool {
        self.mouse_pressed && self.interaction_mode != ElementInteractionMode::None
    }

    /// 获取当前工具
    pub fn get_current_tool(&self) -> DrawingTool {
        self.current_tool
    }

    /// 是否正在文本编辑
    pub fn is_text_editing(&self) -> bool {
        self.text_editing
    }

    /// 当前编辑元素索引
    pub fn get_editing_element_index(&self) -> Option<usize> {
        self.editing_element_index
    }

    /// 获取已选中元素索引
    pub fn get_selected_element_index(&self) -> Option<usize> {
        self.selected_element
    }

    /// 只读获取元素引用
    pub fn get_element_ref(&self, index: usize) -> Option<&DrawingElement> {
        self.elements.get_elements().get(index)
    }

    /// 获取选中元素的工具类型（用于同步工具栏状态）
    pub fn get_selected_element_tool(&self) -> Option<DrawingTool> {
        if let Some(index) = self.selected_element {
            if let Some(element) = self.elements.get_elements().get(index) {
                return Some(element.tool);
            }
        }
        None
    }

    /// 重新加载绘图属性（从设置中同步最新的颜色和粗细等属性）
    /// 注意：现在 ToolManager 直接从设置读取配置，无需手动同步
    pub fn reload_drawing_properties(&mut self) {
        // ToolManager 现在直接从 SimpleSettings 读取配置
        // 无需手动同步，配置会在需要时自动从设置中读取
        // 这个方法保留是为了兼容性，但实际上不再需要做任何操作
    }

    /// 处理双击事件（优先用于文本编辑）
    pub fn handle_double_click(
        &mut self,
        x: i32,
        y: i32,
        _selection_rect: Option<&windows::Win32::Foundation::RECT>,
    ) -> Vec<Command> {
        // 如果双击的是文本元素，则进入编辑模式
        if let Some(index) = self.get_text_element_at_position(x, y) {
            return self.start_text_editing(index);
        }
        vec![]
    }

    /// 处理文本输入（从原始代码迁移）
    pub fn handle_text_input(&mut self, character: char) -> Vec<Command> {
        if !self.text_editing {
            return vec![];
        }

        if let Some(element_index) = self.editing_element_index {
            if let Some(element) = self.elements.get_element_mut(element_index) {
                // 在光标位置插入字符
                let char_count = element.text.chars().count();
                if self.text_cursor_pos <= char_count {
                    // 将字符索引转换为字节索引
                    let byte_pos = element
                        .text
                        .char_indices()
                        .nth(self.text_cursor_pos)
                        .map(|(i, _)| i)
                        .unwrap_or(element.text.len());
                    element.text.insert(byte_pos, character);
                    self.text_cursor_pos += 1;

                    // 动态调整文字框大小
                    self.update_text_element_size(element_index);

                    return vec![Command::RequestRedraw];
                }
            }
        }
        vec![]
    }

    /// 处理光标定时器（从原始代码迁移）
    pub fn handle_cursor_timer(&mut self, timer_id: u32) -> Vec<Command> {
        if self.text_editing && timer_id == self.cursor_timer_id as u32 {
            // 切换光标可见性
            self.text_cursor_visible = !self.text_cursor_visible;
            vec![Command::RequestRedraw]
        } else {
            vec![]
        }
    }

    // ===== 文本编辑相关方法（从原始代码迁移） =====

    /// 获取指定位置的文本元素索引
    fn get_text_element_at_position(&self, x: i32, y: i32) -> Option<usize> {
        for (index, element) in self.elements.get_elements().iter().enumerate() {
            if element.tool == DrawingTool::Text {
                if x >= element.rect.left
                    && x <= element.rect.right
                    && y >= element.rect.top
                    && y <= element.rect.bottom
                {
                    return Some(index);
                }
            }
        }
        None
    }

    /// 开始文本编辑模式
    fn start_text_editing(&mut self, element_index: usize) -> Vec<Command> {
        // 清除其他元素的选中状态并选中当前文字元素（统一通过ElementManager）
        self.elements.set_selected(None);
        self.selected_element = Some(element_index);
        self.elements.set_selected(self.selected_element);

        // 开始文字编辑模式
        self.text_editing = true;
        self.editing_element_index = Some(element_index);
        if let Some(el) = self.elements.get_elements().get(element_index) {
            self.text_cursor_pos = el.text.chars().count();
        } else {
            self.text_cursor_pos = 0;
        }
        self.text_cursor_visible = true;

        vec![
            Command::StartTimer(self.cursor_timer_id as u32, 500), // 启动光标闪烁定时器
            Command::RequestRedraw,
        ]
    }

    /// 创建新文本元素并开始编辑
    fn create_and_edit_text_element(&mut self, x: i32, y: i32) -> (Vec<Command>, bool) {
        // 清除所有元素的选择状态
        self.elements.set_selected(None);
        self.selected_element = None;

        // 确保工具栏状态与当前工具保持一致（与原始代码一致）
        self.current_tool = DrawingTool::Text;

        // 保存历史状态（在操作开始前保存，以便精确撤销）
        self.history
            .save_state(&self.elements, self.selected_element);

        // 创建新的文字元素
        let mut text_element = DrawingElement::new(DrawingTool::Text);
        text_element
            .points
            .push(windows::Win32::Foundation::POINT { x, y });

        // 使用设置中的字体大小、颜色和样式（仅在创建时读取一次并保存到元素上）
        let settings = crate::settings::Settings::load();
        text_element.color = windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F {
            r: settings.font_color.0 as f32 / 255.0,
            g: settings.font_color.1 as f32 / 255.0,
            b: settings.font_color.2 as f32 / 255.0,
            a: 1.0,
        };
        text_element.font_size = settings.font_size;
        text_element.font_name = settings.font_name.clone();
        text_element.font_weight = settings.font_weight;
        text_element.font_italic = settings.font_italic;
        text_element.font_underline = settings.font_underline;
        text_element.font_strikeout = settings.font_strikeout;
        text_element.text = String::new(); // 空文本，等待用户输入
        text_element.selected = true;

        // 根据字体大小动态计算初始文本框尺寸（与旧代码保持一致）
        let font_size = text_element.font_size;
        let dynamic_line_height = (font_size * 1.2) as i32;
        let initial_width = (font_size * 6.0) as i32; // 大约6个字符的宽度
        let initial_height = dynamic_line_height + (crate::constants::TEXT_PADDING * 2.0) as i32;

        // 设置第二个点来定义文本框尺寸（与旧代码保持一致）
        text_element.points.push(windows::Win32::Foundation::POINT {
            x: x + initial_width,
            y: y + initial_height,
        });

        // 更新边界矩形
        text_element.update_bounding_rect();

        // 通过 ElementManager 添加，并以其索引作为编辑目标
        self.elements.add_element(text_element);
        let element_index = self.elements.count().saturating_sub(1);

        // 开始编辑模式
        self.text_editing = true;
        self.editing_element_index = Some(element_index);
        self.text_cursor_pos = 0;
        self.text_cursor_visible = true;
        self.selected_element = Some(element_index);
        self.elements.set_selected(self.selected_element);

        (
            vec![
                Command::StartTimer(self.cursor_timer_id as u32, 500), // 启动光标闪烁定时器
                Command::RequestRedraw,
            ],
            true,
        )
    }

    /// 停止文本编辑模式
    fn stop_text_editing(&mut self) -> Vec<Command> {
        if !self.text_editing {
            return vec![];
        }

        // 立即隐藏光标，确保保存时光标不可见
        self.text_cursor_visible = false;

        // 先停止编辑状态，再检查是否需要删除空元素
        self.text_editing = false;
        let editing_index = self.editing_element_index;
        self.editing_element_index = None;
        self.text_cursor_pos = 0;

        // 保存当前工具状态，确保在保存文本后保持文本工具（与原始代码一致）
        self.current_tool = DrawingTool::Text;

        // 检查当前编辑的文本元素是否为空，如果为空则删除
        if let Some(element_index) = editing_index {
            if let Some(element) = self.elements.get_elements().get(element_index) {
                let should_delete = element.text.trim().is_empty();

                if should_delete {
                    // 删除空元素
                    let _ = self.elements.remove_element(element_index);

                    // 更新选中元素索引（与原始代码逻辑一致）
                    if let Some(selected) = self.selected_element {
                        if selected == element_index {
                            self.selected_element = None;
                        } else if selected > element_index {
                            self.selected_element = Some(selected - 1);
                        }
                    }
                }
            }
        }

        // 强制确保工具状态保持为文本工具，防止被其他逻辑重置（与原始代码一致）
        self.current_tool = DrawingTool::Text;

        // 设置标志，防止立即创建新的文本元素（与原代码保持一致）
        self.just_saved_text = true;

        // 清除选中状态，这样保存文本后就不会进入手柄检查逻辑（与原始代码一致）
        self.selected_element = None;
        self.elements.set_selected(None);

        vec![
            Command::StopTimer(self.cursor_timer_id as u32), // 停止光标闪烁定时器
            Command::RequestRedraw,
        ]
    }

    /// 处理退格键
    fn handle_backspace(&mut self) -> Vec<Command> {
        if !self.text_editing {
            return vec![];
        }

        if let Some(element_index) = self.editing_element_index {
            if self.text_cursor_pos > 0 {
                if let Some(element) = self.elements.get_element_mut(element_index) {
                    // 删除光标前的字符
                    let char_count = element.text.chars().count();
                    if self.text_cursor_pos <= char_count {
                        let chars: Vec<char> = element.text.chars().collect();
                        if self.text_cursor_pos > 0 {
                            chars
                                .iter()
                                .take(self.text_cursor_pos - 1)
                                .chain(chars.iter().skip(self.text_cursor_pos))
                                .collect::<String>()
                                .clone_into(&mut element.text);
                            self.text_cursor_pos -= 1;
                        }
                    }

                    // 动态调整文字框大小
                    self.update_text_element_size(element_index);

                    return vec![Command::RequestRedraw];
                }
            }
        }
        vec![]
    }

    /// 光标向左移动
    fn move_cursor_left(&mut self) -> Vec<Command> {
        if self.text_cursor_pos > 0 {
            self.text_cursor_pos -= 1;
            vec![Command::RequestRedraw]
        } else {
            vec![]
        }
    }

    /// 光标向右移动
    fn move_cursor_right(&mut self) -> Vec<Command> {
        if let Some(element_index) = self.editing_element_index {
            if let Some(el) = self.elements.get_elements().get(element_index) {
                let char_count = el.text.chars().count();
                if self.text_cursor_pos < char_count {
                    self.text_cursor_pos += 1;
                    return vec![Command::RequestRedraw];
                }
            }
        }
        vec![]
    }

    /// 光标移动到行首（准确到当前行）
    fn move_cursor_to_line_start(&mut self) -> Vec<Command> {
        if let Some(element_index) = self.editing_element_index {
            if let Some(el) = self.elements.get_elements().get(element_index) {
                let before = el
                    .text
                    .chars()
                    .take(self.text_cursor_pos)
                    .collect::<String>();
                if let Some(last_nl) = before.rfind('\n') {
                    self.text_cursor_pos = last_nl + 1;
                } else {
                    self.text_cursor_pos = 0;
                }
                return vec![Command::RequestRedraw];
            }
        }
        vec![]
    }

    /// 光标移动到行尾（准确到当前行）
    fn move_cursor_to_line_end(&mut self) -> Vec<Command> {
        if let Some(element_index) = self.editing_element_index {
            if let Some(el) = self.elements.get_elements().get(element_index) {
                let after = el
                    .text
                    .chars()
                    .skip(self.text_cursor_pos)
                    .collect::<String>();
                if let Some(next_nl) = after.find('\n') {
                    self.text_cursor_pos += next_nl;
                } else {
                    self.text_cursor_pos = el.text.chars().count();
                }
                return vec![Command::RequestRedraw];
            }
        }
        vec![]
    }

    /// 光标向上移动一行（基于字符计算）
    fn move_cursor_up(&mut self) -> Vec<Command> {
        if let Some(element_index) = self.editing_element_index {
            if let Some(el) = self.elements.get_elements().get(element_index) {
                let before = el
                    .text
                    .chars()
                    .take(self.text_cursor_pos)
                    .collect::<String>();
                let lines_before: Vec<&str> = before.lines().collect();
                if lines_before.len() > 1 {
                    let current_line_text = if before.ends_with('\n') {
                        ""
                    } else {
                        lines_before.last().copied().unwrap_or("")
                    };
                    let current_col = current_line_text.chars().count();
                    let current_line_start = if before.ends_with('\n') {
                        lines_before.len()
                    } else {
                        lines_before.len() - 1
                    };
                    if current_line_start > 0 {
                        let prev_line_text = lines_before[current_line_start - 1];
                        let prev_len = prev_line_text.chars().count();
                        let target_col = current_col.min(prev_len);
                        let chars_before_prev: usize = lines_before[..current_line_start - 1]
                            .iter()
                            .map(|line| line.chars().count() + 1)
                            .sum();
                        self.text_cursor_pos = chars_before_prev + target_col;
                        return vec![Command::RequestRedraw];
                    }
                }
            }
        }
        vec![]
    }

    /// 光标向下移动一行（基于字符计算）
    fn move_cursor_down(&mut self) -> Vec<Command> {
        if let Some(element_index) = self.editing_element_index {
            if let Some(el) = self.elements.get_elements().get(element_index) {
                let before = el
                    .text
                    .chars()
                    .take(self.text_cursor_pos)
                    .collect::<String>();
                let after = el
                    .text
                    .chars()
                    .skip(self.text_cursor_pos)
                    .collect::<String>();
                if let Some(next_nl) = after.find('\n') {
                    let lines_before: Vec<&str> = before.lines().collect();
                    let current_line_text = if before.ends_with('\n') {
                        ""
                    } else {
                        lines_before.last().copied().unwrap_or("")
                    };
                    let current_col = current_line_text.chars().count();
                    let from_next = &after[next_nl + 1..];
                    let next_line_text = if let Some(end_pos) = from_next.find('\n') {
                        &from_next[..end_pos]
                    } else {
                        from_next
                    };
                    let next_len = next_line_text.chars().count();
                    let target_col = current_col.min(next_len);
                    self.text_cursor_pos = self.text_cursor_pos + next_nl + 1 + target_col;
                    return vec![Command::RequestRedraw];
                }
            }
        }
        vec![]
    }

    /// 动态调整文字框大小（使用 DirectWrite 精确测量）
    fn update_text_element_size(&mut self, element_index: usize) {
        use crate::constants::{MIN_TEXT_HEIGHT, MIN_TEXT_WIDTH, TEXT_PADDING};
        use windows::Win32::Graphics::DirectWrite::*;
        use windows::core::w;

        if let Some(element) = self.elements.get_element_mut(element_index) {
            let font_size = element.get_effective_font_size();
            let dynamic_line_height = (font_size * 1.2).ceil() as i32;

            let text_content = element.text.clone();
            let lines: Vec<&str> = if text_content.is_empty() {
                vec![""]
            } else {
                text_content.lines().collect()
            };
            let line_count = if text_content.is_empty() {
                1
            } else if text_content.ends_with('\n') {
                lines.len() + 1
            } else {
                lines.len()
            } as i32;

            // 使用 DirectWrite 精确测量最长行宽度
            let mut max_width_f = 0.0f32;
            unsafe {
                if let Ok(factory) =
                    DWriteCreateFactory::<IDWriteFactory>(DWRITE_FACTORY_TYPE_SHARED)
                {
                    let font_name_wide = crate::utils::to_wide_chars(&element.font_name);
                    let weight = if element.font_weight > 400 {
                        DWRITE_FONT_WEIGHT_BOLD
                    } else {
                        DWRITE_FONT_WEIGHT_NORMAL
                    };
                    let style = if element.font_italic {
                        DWRITE_FONT_STYLE_ITALIC
                    } else {
                        DWRITE_FONT_STYLE_NORMAL
                    };
                    if let Ok(text_format) = factory.CreateTextFormat(
                        windows::core::PCWSTR(font_name_wide.as_ptr()),
                        None,
                        weight,
                        style,
                        DWRITE_FONT_STRETCH_NORMAL,
                        font_size,
                        w!(""),
                    ) {
                        for line in &lines {
                            let wide: Vec<u16> = line.encode_utf16().collect();
                            if let Ok(layout) =
                                factory.CreateTextLayout(&wide, &text_format, f32::MAX, f32::MAX)
                            {
                                let mut metrics = DWRITE_TEXT_METRICS::default();
                                let _ = layout.GetMetrics(&mut metrics);
                                if metrics.width > max_width_f {
                                    max_width_f = metrics.width;
                                }
                            }
                        }
                    }
                }
            }

            if max_width_f == 0.0 {
                max_width_f = MIN_TEXT_WIDTH as f32;
            } else {
                // 增加适当的缓冲，避免字符被挤压
                max_width_f += (font_size * 0.2).max(4.0);
            }

            let new_width = ((max_width_f + TEXT_PADDING * 2.0).ceil() as i32).max(MIN_TEXT_WIDTH);
            let new_height = (line_count * dynamic_line_height + (TEXT_PADDING * 2.0) as i32)
                .max(MIN_TEXT_HEIGHT);

            element.rect.right = element.rect.left + new_width;
            element.rect.bottom = element.rect.top + new_height;

            // 保持 points 与 rect 同步，确保渲染和选择区域一致
            if !element.points.is_empty() {
                if element.points.len() >= 2 {
                    element.points[1].x = element.rect.right;
                    element.points[1].y = element.rect.bottom;
                } else {
                    element.points.push(windows::Win32::Foundation::POINT {
                        x: element.rect.right,
                        y: element.rect.bottom,
                    });
                }
            }
        }
    }
}

/// 绘图错误类型
#[derive(Debug)]
pub enum DrawingError {
    /// 渲染错误
    RenderError(String),
    /// 初始化错误
    InitError(String),
}

impl std::fmt::Display for DrawingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DrawingError::RenderError(msg) => write!(f, "Drawing render error: {}", msg),
            DrawingError::InitError(msg) => write!(f, "Drawing init error: {}", msg),
        }
    }
}

impl std::error::Error for DrawingError {}
