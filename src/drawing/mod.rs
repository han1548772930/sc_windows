// 绘图管理器模块
//
// 负责绘图工具管理、绘图元素管理、撤销/重做系统

use crate::message::{Command, DrawingMessage};
use crate::platform::{PlatformError, PlatformRenderer};
use crate::types::{DrawingElement, DrawingTool};
use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;

pub mod elements;
pub mod history;
pub mod tools;

use elements::ElementManager;
use history::HistoryManager;
use tools::ToolManager;

/// 绘图管理器
pub struct DrawingManager {
    /// 工具管理器
    tools: ToolManager,
    /// 元素管理器
    elements: ElementManager,
    /// 历史管理器
    history: HistoryManager,
    /// 当前工具
    current_tool: DrawingTool,
    /// 当前绘制的元素
    current_element: Option<DrawingElement>,
    /// 选中的元素索引
    selected_element: Option<usize>,

    // 从WindowState迁移的字段
    /// 绘图颜色（暂时保留，后续可能使用）
    #[allow(dead_code)]
    drawing_color: D2D1_COLOR_F,
    /// 绘图粗细（暂时保留，后续可能使用）
    #[allow(dead_code)]
    drawing_thickness: f32,
    /// 历史记录栈（原始格式，用于兼容）
    history_stack: Vec<crate::drawing::history::HistoryState>,
    /// 拖拽模式（从原始代码迁移）
    drag_mode: crate::types::DragMode,
    /// 鼠标按下状态（从原始代码迁移）
    mouse_pressed: bool,
    /// 拖拽开始位置（从原始代码迁移）
    drag_start_pos: windows::Win32::Foundation::POINT,
    /// 拖拽开始时的选中元素矩形
    drag_start_rect: windows::Win32::Foundation::RECT,
    /// 拖拽开始时的字体大小（用于文字缩放）
    drag_start_font_size: f32,

    /// 文本编辑状态（从原始代码迁移）
    text_editing: bool,
    editing_element_index: Option<usize>,
    text_cursor_pos: usize,
    text_cursor_visible: bool,
    cursor_timer_id: usize,
    just_saved_text: bool, // 防止连续创建文本元素
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

            // 初始化从WindowState迁移的字段
            drawing_color: D2D1_COLOR_F {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            }, // 默认红色
            drawing_thickness: 3.0,
            history_stack: Vec::new(),
            // 初始化拖拽相关字段（从原始代码迁移）
            drag_mode: crate::types::DragMode::None,
            mouse_pressed: false,
            drag_start_pos: windows::Win32::Foundation::POINT { x: 0, y: 0 },
            drag_start_rect: windows::Win32::Foundation::RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            drag_start_font_size: 0.0,

            // 初始化文本编辑相关字段（从原始代码迁移）
            text_editing: false,
            editing_element_index: None,
            text_cursor_pos: 0,
            text_cursor_visible: false,
            cursor_timer_id: 1001, // 使用固定的定时器ID
            just_saved_text: false,
        })
    }

    /// 重置状态（从原始reset_to_initial_state迁移）
    pub fn reset_state(&mut self) {
        // 清除所有绘制元素（统一由 ElementManager 管理）
        self.current_element = None;
        self.selected_element = None;

        // 重置工具状态
        self.current_tool = DrawingTool::None;

        // 清除历史记录
        self.history_stack.clear();
        self.history.clear();

        // 重置元素管理器
        self.elements.clear();
    }

    /// 处理绘图消息
    pub fn handle_message(&mut self, message: DrawingMessage) -> Vec<Command> {
        match message {
            DrawingMessage::SelectTool(tool) => {
                // 设置当前工具（从原始代码迁移）
                self.current_tool = tool;
                self.tools.set_current_tool(tool);

                // 清除选中的元素（从原始代码迁移）
                self.selected_element = None;

                // 将所有绘图元素的selected状态设为false（统一通过ElementManager管理）
                self.elements.set_selected(None);

                vec![Command::UpdateToolbar, Command::RequestRedraw]
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
                    // 保存历史状态（包含选择状态）
                    self.history
                        .save_state(&self.elements, self.selected_element);
                    // 添加元素
                    self.elements.add_element(element);
                    vec![Command::RequestRedraw]
                } else {
                    vec![]
                }
            }
            DrawingMessage::Undo => {
                if let Some((elements, sel)) = self.history.undo() {
                    self.elements.restore_state(elements);
                    self.selected_element = sel;
                    self.elements.set_selected(self.selected_element);
                    vec![Command::RequestRedraw]
                } else {
                    vec![]
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
                // 保存历史状态（包含选择状态）
                self.history
                    .save_state(&self.elements, self.selected_element);
                // 删除元素
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
                vec![Command::RequestRedraw]
            }
            DrawingMessage::AddElement(element) => {
                // 保存历史状态（包含选择状态）
                self.history
                    .save_state(&self.elements, self.selected_element);
                // 添加元素
                self.elements.add_element(element);
                // 统一由 ElementManager 管理元素列表
                vec![Command::RequestRedraw]
            }
            DrawingMessage::CheckElementClick(x, y) => {
                // 检查是否点击了元素（从原始代码迁移）
                if let Some(element_index) = self.elements.get_element_at_position(x, y) {
                    // 点击了元素，选择该元素
                    self.selected_element = Some(element_index);

                    // 统一通过 ElementManager 设置选中状态
                    self.elements.set_selected(self.selected_element);

                    vec![Command::RequestRedraw]
                } else {
                    // 没有点击元素，清除选择
                    self.selected_element = None;
                    self.elements.set_selected(None);
                    vec![]
                }
            }
        }
    }

    /// 渲染绘图元素（按照原始代码逻辑，添加裁剪支持）
    pub fn render(
        &self,
        renderer: &mut dyn PlatformRenderer<Error = PlatformError>,
        selection_rect: Option<&windows::Win32::Foundation::RECT>,
        _screen_width: i32,
        _screen_height: i32,
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

            // 渲染元素选择（从原始代码迁移）
            self.draw_element_selection_d2d(d2d_renderer, selection_rect)?;
        } else {
            // 回退到抽象接口
            for element in self.elements.get_elements() {
                self.render_element(element, renderer)?;
            }

            // 渲染当前正在绘制的元素
            if let Some(ref element) = self.current_element {
                self.render_element(element, renderer)?;
            }
        }

        // 恢复裁剪区域（从原始代码迁移）
        if selection_rect.is_some() {
            renderer.pop_clip_rect().map_err(|e| {
                DrawingError::RenderError(format!("Failed to pop clip rect: {:?}", e))
            })?;
        }

        Ok(())
    }

    /// 渲染单个绘图元素（从原始代码迁移）
    fn render_element(
        &self,
        element: &DrawingElement,
        renderer: &mut dyn PlatformRenderer<Error = PlatformError>,
    ) -> Result<(), DrawingError> {
        use crate::platform::traits::{Color, DrawStyle, Point, TextStyle};

        // 转换颜色
        let color = Color {
            r: element.color.r,
            g: element.color.g,
            b: element.color.b,
            a: element.color.a,
        };

        match element.tool {
            crate::types::DrawingTool::Rectangle => {
                if element.points.len() >= 2 {
                    let rect = crate::platform::traits::Rectangle {
                        x: element.points[0].x.min(element.points[1].x) as f32,
                        y: element.points[0].y.min(element.points[1].y) as f32,
                        width: (element.points[1].x - element.points[0].x).abs() as f32,
                        height: (element.points[1].y - element.points[0].y).abs() as f32,
                    };

                    let style = DrawStyle {
                        stroke_color: color,
                        fill_color: None,
                        stroke_width: element.thickness,
                    };

                    renderer.draw_rectangle(rect, &style).map_err(|e| {
                        DrawingError::RenderError(format!("Failed to draw rectangle: {:?}", e))
                    })?;
                }
            }
            crate::types::DrawingTool::Circle => {
                if element.points.len() >= 2 {
                    let center_x = (element.points[0].x + element.points[1].x) as f32 / 2.0;
                    let center_y = (element.points[0].y + element.points[1].y) as f32 / 2.0;
                    let radius_x = (element.points[1].x - element.points[0].x).abs() as f32 / 2.0;
                    let radius_y = (element.points[1].y - element.points[0].y).abs() as f32 / 2.0;

                    // 修复：对于抽象渲染器，使用椭圆而不是圆形
                    // 如果平台支持椭圆，使用椭圆；否则使用较大半径的圆形作为回退
                    let radius = radius_x.max(radius_y); // 回退到圆形

                    let center = Point {
                        x: center_x,
                        y: center_y,
                    };

                    let style = DrawStyle {
                        stroke_color: color,
                        fill_color: None,
                        stroke_width: element.thickness,
                    };

                    // 注意：抽象渲染器目前只支持圆形，但Direct2D支持椭圆
                    renderer.draw_circle(center, radius, &style).map_err(|e| {
                        DrawingError::RenderError(format!("Failed to draw circle: {:?}", e))
                    })?;
                }
            }
            crate::types::DrawingTool::Arrow => {
                if element.points.len() >= 2 {
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

                    // 绘制主线
                    renderer.draw_line(start, end, &style).map_err(|e| {
                        DrawingError::RenderError(format!("Failed to draw arrow line: {:?}", e))
                    })?;

                    // 绘制箭头头部（简化版）
                    let dx = element.points[1].x - element.points[0].x;
                    let dy = element.points[1].y - element.points[0].y;
                    let length = ((dx * dx + dy * dy) as f64).sqrt();

                    if length > 20.0 {
                        let arrow_length = 15.0f64;
                        let arrow_angle = 0.5f64;
                        let unit_x = dx as f64 / length;
                        let unit_y = dy as f64 / length;

                        let wing1 = Point {
                            x: element.points[1].x as f32
                                - (arrow_length
                                    * (unit_x * arrow_angle.cos() + unit_y * arrow_angle.sin()))
                                    as f32,
                            y: element.points[1].y as f32
                                - (arrow_length
                                    * (unit_y * arrow_angle.cos() - unit_x * arrow_angle.sin()))
                                    as f32,
                        };

                        let wing2 = Point {
                            x: element.points[1].x as f32
                                - (arrow_length
                                    * (unit_x * arrow_angle.cos() - unit_y * arrow_angle.sin()))
                                    as f32,
                            y: element.points[1].y as f32
                                - (arrow_length
                                    * (unit_y * arrow_angle.cos() + unit_x * arrow_angle.sin()))
                                    as f32,
                        };

                        renderer.draw_line(end, wing1, &style).map_err(|e| {
                            DrawingError::RenderError(format!(
                                "Failed to draw arrow wing1: {:?}",
                                e
                            ))
                        })?;
                        renderer.draw_line(end, wing2, &style).map_err(|e| {
                            DrawingError::RenderError(format!(
                                "Failed to draw arrow wing2: {:?}",
                                e
                            ))
                        })?;
                    }
                }
            }
            crate::types::DrawingTool::Pen => {
                if element.points.len() > 1 {
                    let style = DrawStyle {
                        stroke_color: color,
                        fill_color: None,
                        stroke_width: element.thickness,
                    };

                    for i in 0..element.points.len() - 1 {
                        let start = Point {
                            x: element.points[i].x as f32,
                            y: element.points[i].y as f32,
                        };
                        let end = Point {
                            x: element.points[i + 1].x as f32,
                            y: element.points[i + 1].y as f32,
                        };

                        renderer.draw_line(start, end, &style).map_err(|e| {
                            DrawingError::RenderError(format!("Failed to draw pen line: {:?}", e))
                        })?;
                    }
                }
            }
            crate::types::DrawingTool::Text => {
                if !element.points.is_empty() && !element.text.is_empty() {
                    let position = Point {
                        x: element.points[0].x as f32,
                        y: element.points[0].y as f32,
                    };

                    let text_style = TextStyle {
                        font_size: element.thickness, // 使用thickness作为字体大小
                        color,
                        font_family: "Microsoft YaHei".to_string(),
                    };

                    renderer
                        .draw_text(&element.text, position, &text_style)
                        .map_err(|e| {
                            DrawingError::RenderError(format!("Failed to draw text: {:?}", e))
                        })?;
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// 渲染元素选择（从原始代码迁移）
    fn draw_element_selection_d2d(
        &self,
        d2d_renderer: &crate::platform::windows::d2d::Direct2DRenderer,
        selection_rect: Option<&windows::Win32::Foundation::RECT>,
    ) -> Result<(), DrawingError> {
        // 只有当有选中元素时才绘制选择指示器
        if let Some(element_index) = self.selected_element {
            if element_index < self.elements.get_elements().len() {
                let element = &self.elements.get_elements()[element_index];

                // 检查元素是否被选中且可见
                if element.selected {
                    self.draw_selected_element_indicators_d2d(
                        element,
                        d2d_renderer,
                        selection_rect,
                    )?;
                }
            }
        }
        Ok(())
    }

    /// 绘制选中元素的指示器（虚线边框和手柄）
    fn draw_selected_element_indicators_d2d(
        &self,
        element: &DrawingElement,
        d2d_renderer: &crate::platform::windows::d2d::Direct2DRenderer,
        selection_rect: Option<&windows::Win32::Foundation::RECT>,
    ) -> Result<(), DrawingError> {
        use windows::Win32::Graphics::Direct2D::Common::*;
        use windows::Win32::Graphics::Direct2D::*;

        if let Some(ref render_target) = d2d_renderer.render_target {
            unsafe {
                // 创建虚线样式（从原始代码迁移）
                if let Some(ref d2d_factory) = d2d_renderer.d2d_factory {
                    let stroke_style_properties = D2D1_STROKE_STYLE_PROPERTIES {
                        startCap: D2D1_CAP_STYLE_FLAT,
                        endCap: D2D1_CAP_STYLE_FLAT,
                        dashCap: D2D1_CAP_STYLE_FLAT,
                        lineJoin: D2D1_LINE_JOIN_MITER,
                        miterLimit: 10.0,
                        dashStyle: D2D1_DASH_STYLE_DASH,
                        dashOffset: 0.0,
                    };

                    if let Ok(stroke_style) =
                        d2d_factory.CreateStrokeStyle(&stroke_style_properties, None)
                    {
                        // 创建虚线边框画刷（从原始代码迁移）
                        let dashed_color = D2D1_COLOR_F {
                            r: 0.0,
                            g: 0.5,
                            b: 1.0,
                            a: 1.0,
                        }; // 蓝色虚线

                        if let Ok(dashed_brush) =
                            render_target.CreateSolidColorBrush(&dashed_color, None)
                        {
                            // 绘制虚线边框
                            self.draw_dashed_border_d2d(
                                element,
                                render_target,
                                &dashed_brush,
                                &stroke_style,
                            )?;

                            // 绘制手柄
                            self.draw_element_handles_d2d(element, render_target, selection_rect)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// 绘制虚线边框（从原始代码迁移）
    fn draw_dashed_border_d2d(
        &self,
        element: &DrawingElement,
        render_target: &windows::Win32::Graphics::Direct2D::ID2D1HwndRenderTarget,
        brush: &windows::Win32::Graphics::Direct2D::ID2D1SolidColorBrush,
        stroke_style: &windows::Win32::Graphics::Direct2D::ID2D1StrokeStyle,
    ) -> Result<(), DrawingError> {
        use windows::Win32::Graphics::Direct2D::Common::*;

        unsafe {
            match element.tool {
                crate::types::DrawingTool::Rectangle | crate::types::DrawingTool::Circle => {
                    if element.points.len() >= 2 {
                        let rect = D2D_RECT_F {
                            left: element.points[0].x.min(element.points[1].x) as f32,
                            top: element.points[0].y.min(element.points[1].y) as f32,
                            right: element.points[0].x.max(element.points[1].x) as f32,
                            bottom: element.points[0].y.max(element.points[1].y) as f32,
                        };

                        if element.tool == crate::types::DrawingTool::Rectangle {
                            render_target.DrawRectangle(&rect, brush, 1.0, Some(stroke_style));
                        } else {
                            // Circle - draw ellipse
                            let ellipse = windows::Win32::Graphics::Direct2D::D2D1_ELLIPSE {
                                point: windows_numerics::Vector2 {
                                    X: (rect.left + rect.right) / 2.0,
                                    Y: (rect.top + rect.bottom) / 2.0,
                                },
                                radiusX: (rect.right - rect.left) / 2.0,
                                radiusY: (rect.bottom - rect.top) / 2.0,
                            };
                            render_target.DrawEllipse(&ellipse, brush, 1.0, Some(stroke_style));
                        }
                    }
                }
                crate::types::DrawingTool::Text => {
                    if !element.points.is_empty() {
                        // For text, draw dashed border around text bounds
                        let text_rect = D2D_RECT_F {
                            left: element.rect.left as f32,
                            top: element.rect.top as f32,
                            right: element.rect.right as f32,
                            bottom: element.rect.bottom as f32,
                        };
                        render_target.DrawRectangle(&text_rect, brush, 1.0, Some(stroke_style));
                    }
                }
                _ => {
                    // For other tools like Arrow, Pen - draw around bounding rect
                    let bounding_rect = D2D_RECT_F {
                        left: element.rect.left as f32,
                        top: element.rect.top as f32,
                        right: element.rect.right as f32,
                        bottom: element.rect.bottom as f32,
                    };
                    render_target.DrawRectangle(&bounding_rect, brush, 1.0, Some(stroke_style));
                }
            }
        }
        Ok(())
    }

    /// 绘制元素手柄（从原始代码迁移）
    fn draw_element_handles_d2d(
        &self,
        element: &DrawingElement,
        render_target: &windows::Win32::Graphics::Direct2D::ID2D1HwndRenderTarget,
        _selection_rect: Option<&windows::Win32::Foundation::RECT>,
    ) -> Result<(), DrawingError> {
        use windows::Win32::Graphics::Direct2D::Common::*;
        use windows::Win32::Graphics::Direct2D::*;

        unsafe {
            // 创建手柄画刷
            let handle_fill_color = D2D1_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            }; // 白色填充
            let handle_border_color = D2D1_COLOR_F {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            }; // 黑色边框

            if let (Ok(fill_brush), Ok(border_brush)) = (
                render_target.CreateSolidColorBrush(&handle_fill_color, None),
                render_target.CreateSolidColorBrush(&handle_border_color, None),
            ) {
                let handle_size = 6.0; // 手柄大小
                let half_handle = handle_size / 2.0;

                match element.tool {
                    crate::types::DrawingTool::Arrow => {
                        // 箭头只显示起点和终点手柄
                        if element.points.len() >= 2 {
                            for point in &element.points[..2] {
                                let handle_ellipse = D2D1_ELLIPSE {
                                    point: windows_numerics::Vector2 {
                                        X: point.x as f32,
                                        Y: point.y as f32,
                                    },
                                    radiusX: half_handle,
                                    radiusY: half_handle,
                                };

                                render_target.FillEllipse(&handle_ellipse, &fill_brush);
                                render_target.DrawEllipse(
                                    &handle_ellipse,
                                    &border_brush,
                                    1.0,
                                    None,
                                );
                            }
                        }
                    }
                    crate::types::DrawingTool::Text => {
                        // 文本元素显示4个角手柄
                        let handles = vec![
                            (element.rect.left, element.rect.top),
                            (element.rect.right, element.rect.top),
                            (element.rect.right, element.rect.bottom),
                            (element.rect.left, element.rect.bottom),
                        ];

                        for (hx, hy) in handles {
                            let handle_ellipse = D2D1_ELLIPSE {
                                point: windows_numerics::Vector2 {
                                    X: hx as f32,
                                    Y: hy as f32,
                                },
                                radiusX: half_handle,
                                radiusY: half_handle,
                            };

                            render_target.FillEllipse(&handle_ellipse, &fill_brush);
                            render_target.DrawEllipse(&handle_ellipse, &border_brush, 1.0, None);
                        }
                    }
                    _ => {
                        // 其他元素显示8个手柄（4个角 + 4个边中点）
                        if element.points.len() >= 2 {
                            let left = element.points[0].x.min(element.points[1].x);
                            let top = element.points[0].y.min(element.points[1].y);
                            let right = element.points[0].x.max(element.points[1].x);
                            let bottom = element.points[0].y.max(element.points[1].y);
                            let center_x = (left + right) / 2;
                            let center_y = (top + bottom) / 2;

                            let handles = vec![
                                (left, top),
                                (center_x, top),
                                (right, top),
                                (right, center_y),
                                (right, bottom),
                                (center_x, bottom),
                                (left, bottom),
                                (left, center_y),
                            ];

                            for (hx, hy) in handles {
                                let handle_ellipse = D2D1_ELLIPSE {
                                    point: windows_numerics::Vector2 {
                                        X: hx as f32,
                                        Y: hy as f32,
                                    },
                                    radiusX: half_handle,
                                    radiusY: half_handle,
                                };

                                render_target.FillEllipse(&handle_ellipse, &fill_brush);
                                render_target.DrawEllipse(
                                    &handle_ellipse,
                                    &border_brush,
                                    1.0,
                                    None,
                                );
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

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

                // 创建文本画刷
                let text_brush = render_target.CreateSolidColorBrush(&element.color, None);

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

                    let font_size = element.thickness.max(12.0); // 最小字体大小12
                    let line_height = font_size * 1.2;

                    // 创建文本格式
                    if let Ok(text_format) = dwrite_factory.CreateTextFormat(
                        windows::core::w!("Microsoft YaHei"),
                        None,
                        windows::Win32::Graphics::DirectWrite::DWRITE_FONT_WEIGHT_NORMAL,
                        windows::Win32::Graphics::DirectWrite::DWRITE_FONT_STYLE_NORMAL,
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

                            // 绘制文本行
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
                }
            }
        }
        Ok(())
    }

    /// 保存历史状态（legacy 兼容，改为从 ElementManager 快照）
    pub fn save_history(&mut self) {
        let state = crate::drawing::history::HistoryState {
            elements: self.elements.get_elements().clone(),
            selected_element: self.selected_element,
        };
        self.history_stack.push(state);
        const MAX_HISTORY: usize = 20;
        if self.history_stack.len() > MAX_HISTORY {
            self.history_stack.remove(0);
        }
    }

    /// 检查是否可以撤销（从WindowState迁移）
    pub fn can_undo(&self) -> bool {
        // 使用新的历史管理器而不是legacy history_stack
        self.history.can_undo()
    }

    // 移除 legacy 接口，统一使用 ElementManager/HistoryManager

    /// 处理鼠标移动（从原始代码迁移，支持拖拽模式）
    pub fn handle_mouse_move(
        &mut self,
        x: i32,
        y: i32,
        selection_rect: Option<windows::Win32::Foundation::RECT>,
    ) -> Vec<Command> {
        if self.mouse_pressed {
            self.update_drag(x, y, selection_rect);
            vec![Command::RequestRedraw]
        } else {
            // 检查是否悬停在元素上（用于改变光标/预览）
            if let Some(_index) = self.elements.get_element_at_position(x, y) {
                // 可在后续添加悬停反馈
                vec![]
            } else {
                vec![]
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
        match self.drag_mode {
            crate::types::DragMode::DrawingShape => {
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
                                element.points.push(self.drag_start_pos);
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
            crate::types::DragMode::MovingElement => {
                if let Some(index) = self.selected_element {
                    if let Some(element) = self.elements.get_elements().get(index) {
                        if element.tool != DrawingTool::Pen {
                            let dx = x - self.drag_start_pos.x;
                            let dy = y - self.drag_start_pos.y;
                            if let Some(el) = self.elements.get_element_mut(index) {
                                let current_dx = el.rect.left - self.drag_start_rect.left;
                                let current_dy = el.rect.top - self.drag_start_rect.top;
                                el.move_by(-current_dx, -current_dy);
                                el.move_by(dx, dy);
                            }
                        }
                    }
                }
            }
            crate::types::DragMode::ResizingTopLeft
            | crate::types::DragMode::ResizingTopCenter
            | crate::types::DragMode::ResizingTopRight
            | crate::types::DragMode::ResizingMiddleRight
            | crate::types::DragMode::ResizingBottomRight
            | crate::types::DragMode::ResizingBottomCenter
            | crate::types::DragMode::ResizingBottomLeft
            | crate::types::DragMode::ResizingMiddleLeft => {
                if let Some(index) = self.selected_element {
                    if let Some(el) = self.elements.get_element_mut(index) {
                        if el.tool == DrawingTool::Pen {
                            return;
                        }
                        let mut new_rect = self.drag_start_rect;
                        let dx = x - self.drag_start_pos.x;
                        let dy = y - self.drag_start_pos.y;
                        match self.drag_mode {
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
                                    match self.drag_mode {
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
                                let original_width =
                                    (self.drag_start_rect.right - self.drag_start_rect.left).max(1);
                                let original_height =
                                    (self.drag_start_rect.bottom - self.drag_start_rect.top).max(1);

                                // 计算比例（不同手柄采用对应方向）
                                let (scale_x, scale_y) = match self.drag_mode {
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
                                el.thickness = (self.drag_start_font_size * scale as f32).max(8.0);
                                // 应用新的矩形
                                el.resize(new_rect);
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
    fn get_element_handle_at_position(
        &self,
        x: i32,
        y: i32,
        rect: &windows::Win32::Foundation::RECT,
        tool: DrawingTool,
        element_index: usize,
    ) -> crate::types::DragMode {
        use crate::types::DragMode;
        let detection_radius = crate::constants::HANDLE_DETECTION_RADIUS as i32;

        // 箭头：起点/终点作为手柄
        if tool == DrawingTool::Arrow {
            if let Some(element) = self.elements.get_elements().get(element_index) {
                if element.points.len() >= 2 {
                    let start = element.points[0];
                    let end = element.points[1];
                    let dx = x - start.x;
                    let dy = y - start.y;
                    if dx * dx + dy * dy <= detection_radius * detection_radius {
                        return DragMode::ResizingTopLeft;
                    }
                    let dx2 = x - end.x;
                    let dy2 = y - end.y;
                    if dx2 * dx2 + dy2 * dy2 <= detection_radius * detection_radius {
                        return DragMode::ResizingBottomRight;
                    }
                }
            }
        }

        let center_x = (rect.left + rect.right) / 2;
        let center_y = (rect.top + rect.bottom) / 2;
        let handles = if tool == DrawingTool::Text {
            vec![
                (rect.left, rect.top, DragMode::ResizingTopLeft),
                (rect.right, rect.top, DragMode::ResizingTopRight),
                (rect.right, rect.bottom, DragMode::ResizingBottomRight),
                (rect.left, rect.bottom, DragMode::ResizingBottomLeft),
            ]
        } else {
            vec![
                (rect.left, rect.top, DragMode::ResizingTopLeft),
                (center_x, rect.top, DragMode::ResizingTopCenter),
                (rect.right, rect.top, DragMode::ResizingTopRight),
                (rect.right, center_y, DragMode::ResizingMiddleRight),
                (rect.right, rect.bottom, DragMode::ResizingBottomRight),
                (center_x, rect.bottom, DragMode::ResizingBottomCenter),
                (rect.left, rect.bottom, DragMode::ResizingBottomLeft),
                (rect.left, center_y, DragMode::ResizingMiddleLeft),
            ]
        };

        for (hx, hy, mode) in handles.into_iter() {
            let dx = x - hx;
            let dy = y - hy;
            if dx * dx + dy * dy <= detection_radius * detection_radius {
                return mode;
            }
        }
        DragMode::None
    }

    /// 处理鼠标按下（从原始代码迁移，支持拖拽模式）
    pub fn handle_mouse_down(
        &mut self,
        x: i32,
        y: i32,
        selection_rect: Option<windows::Win32::Foundation::RECT>,
    ) -> Vec<Command> {
        // 约束：除UI外，绘图交互仅在选择框内生效（保持与原始逻辑一致）
        let inside_selection = match selection_rect {
            Some(r) => x >= r.left && x <= r.right && y >= r.top && y <= r.bottom,
            None => true,
        };

        // 文本工具特殊处理（从原始代码迁移）
        if inside_selection
            && self.current_tool == DrawingTool::Text
            && !self.text_editing
            && !self.just_saved_text
        {
            // 优先检查是否点击现有文本元素的手柄或内容
            if let Some(idx) = self.get_text_element_at_position(x, y) {
                // 进入编辑模式（双击在更上层处理，这里单击直接编辑保持旧逻辑简化）
                return self.start_text_editing(idx);
            }
        }

        // 重置文本保存标志
        self.just_saved_text = false;

        // 选择框外：不消费，让Screenshot处理（例如拖拽选择框）
        if !inside_selection {
            // 但如果当前没有选择框（None），仍应允许绘图交互
            if selection_rect.is_some() {
                return vec![];
            }
        }

        // 先尝试与现有元素交互（无论当前工具是什么）
        // 1) 已选元素的手柄优先
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
                        self.drag_mode = handle_mode;
                        self.mouse_pressed = true;
                        self.drag_start_pos = windows::Win32::Foundation::POINT { x, y };
                        self.drag_start_rect = element.rect;
                        self.drag_start_font_size = element.thickness;
                        return vec![Command::RequestRedraw];
                    }
                    // 2) 已选元素内部（移动）
                    if element.contains_point(x, y) {
                        self.drag_mode = crate::types::DragMode::MovingElement;
                        self.mouse_pressed = true;
                        self.drag_start_pos = windows::Win32::Foundation::POINT { x, y };
                        self.drag_start_rect = element.rect;
                        return vec![Command::RequestRedraw];
                    }
                }
            }
        }

        // 3) 检查是否点击其他元素
        if let Some(idx) = self.elements.get_element_at_position(x, y) {
            // 选择该元素
            self.handle_message(DrawingMessage::SelectElement(Some(idx)));
            if let Some(element) = self.elements.get_elements().get(idx) {
                if element.tool != DrawingTool::Pen {
                    let handle_mode =
                        self.get_element_handle_at_position(x, y, &element.rect, element.tool, idx);
                    self.drag_start_rect = element.rect;
                    self.drag_start_pos = windows::Win32::Foundation::POINT { x, y };
                    if handle_mode != crate::types::DragMode::None {
                        self.drag_mode = handle_mode;
                        self.mouse_pressed = true;
                        return vec![Command::RequestRedraw];
                    } else if element.contains_point(x, y) {
                        self.drag_mode = crate::types::DragMode::MovingElement;
                        self.mouse_pressed = true;
                        return vec![Command::RequestRedraw];
                    }
                }
            }
            // 仅仅选中元素，不开始拖拽
            return vec![Command::RequestRedraw];
        }

        // 4) 若没有元素命中，且选择了绘图工具，则尝试开始绘制
        if self.current_tool != DrawingTool::None {
            if inside_selection {
                self.drag_start_pos = windows::Win32::Foundation::POINT { x, y };
                self.start_drawing_shape(x, y);
                self.mouse_pressed = true;
                return vec![Command::RequestRedraw];
            }
            // 不在选择框内，不消费事件
            return vec![];
        }

        // 5) 工具为None且未命中元素：清除选中，不消费事件
        self.handle_message(DrawingMessage::SelectElement(None));
        vec![]
    }

    /// 开始绘制图形（从原始代码迁移）
    fn start_drawing_shape(&mut self, x: i32, y: i32) {
        // 在开始新的绘制前清除元素选择（保持原始行为）
        self.selected_element = None;
        self.elements.set_selected(None);

        // 保存历史状态
        self.save_history();

        // 设置拖拽模式为绘制图形
        self.drag_mode = crate::types::DragMode::DrawingShape;

        // 创建新元素
        let mut new_element = DrawingElement::new(self.current_tool);
        new_element.color = self.drawing_color;
        new_element.thickness = if self.current_tool == DrawingTool::Text {
            20.0 // 默认字体大小
        } else {
            self.drawing_thickness
        };

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
    pub fn handle_mouse_up(&mut self, _x: i32, _y: i32) -> Vec<Command> {
        if self.mouse_pressed {
            self.end_drag();
            self.mouse_pressed = false;
            self.drag_mode = crate::types::DragMode::None;
            vec![Command::RequestRedraw]
        } else {
            vec![]
        }
    }

    /// 结束拖拽操作（从原始代码迁移）
    fn end_drag(&mut self) {
        if self.drag_mode == crate::types::DragMode::DrawingShape {
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
                        // 文本工具：有位置点就保存
                        !element.points.is_empty()
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

    /// 是否正在进行任何绘图交互（绘制/移动/调整）
    pub fn is_dragging(&self) -> bool {
        self.mouse_pressed && self.drag_mode != crate::types::DragMode::None
    }

    /// 获取当前工具
    pub fn get_current_tool(&self) -> DrawingTool {
        self.current_tool
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
    fn create_and_edit_text_element(&mut self, x: i32, y: i32) -> Vec<Command> {
        // 清除所有元素的选择状态
        self.elements.set_selected(None);
        self.selected_element = None;

        // 保存历史状态
        self.save_history();

        // 创建新的文字元素
        let mut text_element = DrawingElement::new(DrawingTool::Text);
        text_element
            .points
            .push(windows::Win32::Foundation::POINT { x, y });

        // 使用文字颜色
        let (_, text_color, _, _) = crate::constants::get_colors_from_settings();
        text_element.color = text_color;

        // 使用设置中的字体大小
        let settings = crate::simple_settings::SimpleSettings::load();
        text_element.thickness = settings.font_size;
        text_element.text = String::new(); // 空文本，等待用户输入
        text_element.selected = true;

        // 设置文本框大小
        const TEXT_BOX_WIDTH: i32 = 200;
        const TEXT_BOX_HEIGHT: i32 = 30;
        text_element.rect = windows::Win32::Foundation::RECT {
            left: x,
            top: y,
            right: x + TEXT_BOX_WIDTH,
            bottom: y + TEXT_BOX_HEIGHT,
        };

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

        vec![
            Command::StartTimer(self.cursor_timer_id as u32, 500), // 启动光标闪烁定时器
            Command::RequestRedraw,
        ]
    }

    /// 停止文本编辑模式
    fn stop_text_editing(&mut self) -> Vec<Command> {
        if !self.text_editing {
            return vec![];
        }

        self.text_editing = false;
        self.text_cursor_visible = false;
        self.just_saved_text = true; // 防止立即创建新文本

        // 如果文本为空，删除该元素
        if let Some(element_index) = self.editing_element_index {
            if let Some(el) = self.elements.get_elements().get(element_index) {
                if el.text.trim().is_empty() {
                    // 从元素管理器中删除
                    let _ = self.elements.remove_element(element_index);
                }
            }
        }

        self.editing_element_index = None;

        // 清除选中状态
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

    /// 动态调整文字框大小（多行，带内边距，近似宽度测量）
    fn update_text_element_size(&mut self, element_index: usize) {
        use crate::constants::{MIN_TEXT_HEIGHT, MIN_TEXT_WIDTH, TEXT_PADDING};
        if let Some(element) = self.elements.get_element_mut(element_index) {
            let font_size = element.thickness.max(8.0);
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

            // 近似测量每行宽度：字符数 * 字号 * 比例
            let mut max_width_f = 0.0f32;
            for line in &lines {
                let chars = line.chars().count() as f32;
                let width = chars * font_size * 0.6; // 经验比例
                if width > max_width_f {
                    max_width_f = width;
                }
            }
            if max_width_f == 0.0 {
                max_width_f = MIN_TEXT_WIDTH as f32;
            } else {
                // 小幅增加缓冲
                max_width_f += (font_size * 0.2).max(4.0);
            }

            let new_width = ((max_width_f + TEXT_PADDING * 2.0).ceil() as i32).max(MIN_TEXT_WIDTH);
            let new_height = (line_count * dynamic_line_height + (TEXT_PADDING * 2.0) as i32)
                .max(MIN_TEXT_HEIGHT);

            element.rect.right = element.rect.left + new_width;
            element.rect.bottom = element.rect.top + new_height;
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
