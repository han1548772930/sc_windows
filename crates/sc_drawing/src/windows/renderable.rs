use super::context::{RenderContext, RenderOptions};
use super::elements::common::render_endpoint_handles;
use crate::{DrawingElement, DrawingTool, Rect};

/// 渲染结果
pub type RenderResult<T = ()> = Result<T, RenderError>;

/// 渲染错误
#[derive(Debug)]
pub enum RenderError {
    /// 资源创建失败
    ResourceCreation(String),
    /// 渲染失败
    RenderFailed(String),
    /// 无效状态
    InvalidState(String),
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderError::ResourceCreation(msg) => write!(f, "Resource creation failed: {msg}"),
            RenderError::RenderFailed(msg) => write!(f, "Render failed: {msg}"),
            RenderError::InvalidState(msg) => write!(f, "Invalid state: {msg}"),
        }
    }
}

impl std::error::Error for RenderError {}

/// 可渲染元素 trait
///
/// 每种绘图元素类型（矩形、圆、箭头、画笔、文本）实现此 trait，
/// 将渲染逻辑与业务逻辑分离。
pub trait Renderable {
    /// 渲染元素本身
    ///
    /// # Arguments
    /// * `element` - 要渲染的元素
    /// * `ctx` - 渲染上下文
    fn render(&self, element: &DrawingElement, ctx: &mut RenderContext) -> RenderResult;

    /// 渲染选中边框
    ///
    /// # Arguments
    /// * `bounds` - 元素边界
    /// * `ctx` - 渲染上下文
    /// * `options` - 渲染选项
    fn render_selection(
        &self,
        bounds: Rect,
        ctx: &mut RenderContext,
        options: &RenderOptions,
    ) -> RenderResult;

    /// 渲染调整手柄
    ///
    /// # Arguments
    /// * `bounds` - 元素边界
    /// * `ctx` - 渲染上下文
    /// * `options` - 渲染选项
    fn render_handles(
        &self,
        bounds: Rect,
        ctx: &mut RenderContext,
        options: &RenderOptions,
    ) -> RenderResult;
}

/// 渲染器注册表
///
/// 根据元素工具类型分发到对应的渲染器。
pub struct RendererRegistry {
    pub rectangle: super::elements::RectangleRenderer,
    pub circle: super::elements::CircleRenderer,
    pub arrow: super::elements::ArrowRenderer,
    pub pen: super::elements::PenRenderer,
    pub text: super::elements::TextRenderer,
}

impl Default for RendererRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl RendererRegistry {
    /// 创建渲染器注册表
    pub fn new() -> Self {
        Self {
            rectangle: super::elements::RectangleRenderer,
            circle: super::elements::CircleRenderer,
            arrow: super::elements::ArrowRenderer,
            pen: super::elements::PenRenderer::new(),
            text: super::elements::TextRenderer,
        }
    }

    /// 根据元素获取对应的渲染器
    pub fn get_renderer(&self, element: &DrawingElement) -> &dyn Renderable {
        match element.tool {
            DrawingTool::Rectangle => &self.rectangle,
            DrawingTool::Circle => &self.circle,
            DrawingTool::Arrow => &self.arrow,
            DrawingTool::Pen => &self.pen,
            DrawingTool::Text => &self.text,
            _ => &self.rectangle, // 默认
        }
    }

    /// 渲染元素
    pub fn render_element(
        &self,
        element: &DrawingElement,
        ctx: &mut RenderContext,
    ) -> RenderResult {
        self.get_renderer(element).render(element, ctx)
    }

    /// 渲染元素的选中状态
    pub fn render_element_selection(
        &self,
        element: &DrawingElement,
        ctx: &mut RenderContext,
        options: &RenderOptions,
    ) -> RenderResult {
        let bounds = element.rect;
        let renderer = self.get_renderer(element);

        if options.show_selection_border {
            renderer.render_selection(bounds, ctx, options)?;
        }

        if options.show_handles {
            // Arrow handles must be rendered at the actual endpoints, not at the bounding-rect corners.
            if element.tool == DrawingTool::Arrow {
                if element.points.len() >= 2 {
                    let start = (element.points[0].x as f32, element.points[0].y as f32);
                    let end = (element.points[1].x as f32, element.points[1].y as f32);
                    render_endpoint_handles(start, end, ctx, options)?;
                }
            } else {
                renderer.render_handles(bounds, ctx, options)?;
            }
        }

        Ok(())
    }
}
