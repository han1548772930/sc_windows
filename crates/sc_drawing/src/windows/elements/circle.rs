use windows::Win32::Graphics::Direct2D::D2D1_ELLIPSE;
use windows_numerics::Vector2;

use super::common::{render_handles_8, render_selection_border};
use crate::windows::context::{RenderContext, RenderOptions};
use crate::windows::renderable::{RenderError, RenderResult, Renderable};
use crate::{DrawingElement, Rect};

/// 圆/椭圆渲染器
pub struct CircleRenderer;

impl Renderable for CircleRenderer {
    fn render(&self, element: &DrawingElement, ctx: &mut RenderContext) -> RenderResult {
        if element.points.len() < 2 {
            return Ok(());
        }

        let p0 = &element.points[0];
        let p1 = &element.points[1];

        // 计算中心点和半径
        let center_x = (p0.x + p1.x) as f32 / 2.0;
        let center_y = (p0.y + p1.y) as f32 / 2.0;
        let radius_x = (p1.x - p0.x).abs() as f32 / 2.0;
        let radius_y = (p1.y - p0.y).abs() as f32 / 2.0;

        let ellipse = D2D1_ELLIPSE {
            point: Vector2 {
                X: center_x,
                Y: center_y,
            },
            radiusX: radius_x,
            radiusY: radius_y,
        };

        let brush = ctx
            .get_brush(element.color)
            .ok_or_else(|| RenderError::ResourceCreation("Failed to create brush".into()))?
            .clone();

        unsafe {
            ctx.render_target
                .DrawEllipse(&ellipse, &brush, element.thickness, None);
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
