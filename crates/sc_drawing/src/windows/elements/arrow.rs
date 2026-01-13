use windows_numerics::Vector2;

use super::common::render_endpoint_handles;
use crate::windows::context::{RenderContext, RenderOptions};
use crate::windows::renderable::{RenderError, RenderResult, Renderable};
use crate::{DrawingElement, Rect};

/// 箭头渲染器
pub struct ArrowRenderer;

impl ArrowRenderer {
    /// 箭头头部长度
    const ARROW_LENGTH: f64 = 15.0;
    /// 箭头头部角度
    const ARROW_ANGLE: f64 = 0.5;
    /// 最小显示箭头头部的线段长度
    const MIN_LENGTH_FOR_HEAD: f64 = 20.0;
}

impl Renderable for ArrowRenderer {
    fn render(&self, element: &DrawingElement, ctx: &mut RenderContext) -> RenderResult {
        if element.points.len() < 2 {
            return Ok(());
        }

        let p0 = &element.points[0];
        let p1 = &element.points[1];

        let start = Vector2 {
            X: p0.x as f32,
            Y: p0.y as f32,
        };
        let end = Vector2 {
            X: p1.x as f32,
            Y: p1.y as f32,
        };

        let brush = ctx
            .get_brush(element.color)
            .ok_or_else(|| RenderError::ResourceCreation("Failed to create brush".into()))?
            .clone();

        unsafe {
            // 绘制主线段
            ctx.render_target
                .DrawLine(start, end, &brush, element.thickness, None);

            // 绘制箭头头部
            let dx = p1.x - p0.x;
            let dy = p1.y - p0.y;
            let length = ((dx * dx + dy * dy) as f64).sqrt();

            if length > Self::MIN_LENGTH_FOR_HEAD {
                let unit_x = dx as f64 / length;
                let unit_y = dy as f64 / length;

                let cos_angle = Self::ARROW_ANGLE.cos();
                let sin_angle = Self::ARROW_ANGLE.sin();

                // 箭头翅膀1
                let wing1 = Vector2 {
                    X: (p1.x as f64
                        - Self::ARROW_LENGTH * (unit_x * cos_angle + unit_y * sin_angle))
                        as f32,
                    Y: (p1.y as f64
                        - Self::ARROW_LENGTH * (unit_y * cos_angle - unit_x * sin_angle))
                        as f32,
                };

                // 箭头翅膀2
                let wing2 = Vector2 {
                    X: (p1.x as f64
                        - Self::ARROW_LENGTH * (unit_x * cos_angle - unit_y * sin_angle))
                        as f32,
                    Y: (p1.y as f64
                        - Self::ARROW_LENGTH * (unit_y * cos_angle + unit_x * sin_angle))
                        as f32,
                };

                ctx.render_target
                    .DrawLine(end, wing1, &brush, element.thickness, None);
                ctx.render_target
                    .DrawLine(end, wing2, &brush, element.thickness, None);
            }
        }

        Ok(())
    }

    fn render_selection(
        &self,
        _bounds: Rect,
        _ctx: &mut RenderContext,
        _options: &RenderOptions,
    ) -> RenderResult {
        // 箭头不显示选中边框，只显示端点手柄
        Ok(())
    }

    fn render_handles(
        &self,
        bounds: Rect,
        ctx: &mut RenderContext,
        options: &RenderOptions,
    ) -> RenderResult {
        // 箭头只显示两个端点手柄
        let start = (bounds.left as f32, bounds.top as f32);
        let end = (bounds.right as f32, bounds.bottom as f32);
        render_endpoint_handles(start, end, ctx, options)
    }
}
