use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;

use super::common::{render_handles_8, render_selection_border};
use crate::windows::context::{RenderContext, RenderOptions};
use crate::windows::renderable::{RenderError, RenderResult, Renderable};
use crate::{DrawingElement, Rect};

/// 矩形渲染器
pub struct RectangleRenderer;

impl Renderable for RectangleRenderer {
    fn render(&self, element: &DrawingElement, ctx: &mut RenderContext) -> RenderResult {
        if element.points.len() < 2 {
            return Ok(());
        }

        let p0 = &element.points[0];
        let p1 = &element.points[1];

        // 标准化矩形（确保 left < right, top < bottom）
        let rect = D2D_RECT_F {
            left: p0.x.min(p1.x) as f32,
            top: p0.y.min(p1.y) as f32,
            right: p0.x.max(p1.x) as f32,
            bottom: p0.y.max(p1.y) as f32,
        };

        let brush = ctx
            .get_brush(element.color)
            .ok_or_else(|| RenderError::ResourceCreation("Failed to create brush".into()))?
            .clone();

        unsafe {
            ctx.render_target
                .DrawRectangle(&rect, &brush, element.thickness, None);
        }

        Ok(())
    }

    fn render_selection(
        &self,
        bounds: Rect,
        ctx: &mut RenderContext,
        options: &RenderOptions,
    ) -> RenderResult {
        render_selection_border(bounds, ctx, options)
    }

    fn render_handles(
        &self,
        bounds: Rect,
        ctx: &mut RenderContext,
        options: &RenderOptions,
    ) -> RenderResult {
        render_handles_8(bounds, ctx, options)
    }
}
