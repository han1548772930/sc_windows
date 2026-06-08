use super::context::{RenderContext, RenderOptions};
use super::elements::common::render_endpoint_handles;
use crate::{DrawingElement, DrawingTool, Rect};

pub type RenderResult<T = ()> = Result<T, RenderError>;

#[derive(Debug)]
pub enum RenderError {
    ResourceCreation(String),
    RenderFailed(String),
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

pub trait Renderable {
    /// # Arguments
    fn render(&self, element: &DrawingElement, ctx: &mut RenderContext) -> RenderResult;

    /// # Arguments
    fn render_selection(
        &self,
        bounds: Rect,
        ctx: &mut RenderContext,
        options: &RenderOptions,
    ) -> RenderResult;

    /// # Arguments
    fn render_handles(
        &self,
        bounds: Rect,
        ctx: &mut RenderContext,
        options: &RenderOptions,
    ) -> RenderResult;
}

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
    pub fn new() -> Self {
        Self {
            rectangle: super::elements::RectangleRenderer,
            circle: super::elements::CircleRenderer,
            arrow: super::elements::ArrowRenderer,
            pen: super::elements::PenRenderer::new(),
            text: super::elements::TextRenderer,
        }
    }

    pub fn get_renderer(&self, element: &DrawingElement) -> &dyn Renderable {
        match element.tool {
            DrawingTool::Rectangle => &self.rectangle,
            DrawingTool::Circle => &self.circle,
            DrawingTool::Arrow => &self.arrow,
            DrawingTool::Pen => &self.pen,
            DrawingTool::Text => &self.text,
            _ => &self.rectangle,
        }
    }

    pub fn render_element(
        &self,
        element: &DrawingElement,
        ctx: &mut RenderContext,
    ) -> RenderResult {
        self.get_renderer(element).render(element, ctx)
    }

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
