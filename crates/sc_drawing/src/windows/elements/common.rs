use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;
use windows::Win32::Graphics::Direct2D::D2D1_ELLIPSE;
use windows_numerics::Vector2;

use crate::Rect;
use crate::windows::context::{BorderStyle, RenderContext, RenderOptions};
use crate::windows::renderable::{RenderError, RenderResult};

pub fn render_selection_border(
    bounds: Rect,
    ctx: &mut RenderContext,
    options: &RenderOptions,
) -> RenderResult {
    let rect = D2D_RECT_F {
        left: bounds.left as f32,
        top: bounds.top as f32,
        right: bounds.right as f32,
        bottom: bounds.bottom as f32,
    };

    let brush = ctx
        .get_brush(options.selection_color)
        .ok_or_else(|| RenderError::ResourceCreation("Failed to create selection brush".into()))?
        .clone();

    unsafe {
        match options.border_style {
            BorderStyle::Dashed => {
                let style = ctx.get_dashed_style(&options.dash_pattern).cloned();
                if let Some(ref s) = style {
                    ctx.render_target.DrawRectangle(&rect, &brush, 1.0, s);
                }
            }
            BorderStyle::Solid => {
                ctx.render_target.DrawRectangle(&rect, &brush, 1.0, None);
            }
        }
    }

    Ok(())
}

pub fn render_handles_8(
    bounds: Rect,
    ctx: &mut RenderContext,
    options: &RenderOptions,
) -> RenderResult {
    let (left, top, right, bottom) = (
        bounds.left as f32,
        bounds.top as f32,
        bounds.right as f32,
        bounds.bottom as f32,
    );
    let mid_x = (left + right) / 2.0;
    let mid_y = (top + bottom) / 2.0;

    let positions = [
        (left, top),
        (mid_x, top),
        (right, top),
        (right, mid_y),
        (right, bottom),
        (mid_x, bottom),
        (left, bottom),
        (left, mid_y),
    ];

    render_handle_circles(ctx, &positions, options)
}

pub fn render_handles_corners(
    bounds: Rect,
    ctx: &mut RenderContext,
    options: &RenderOptions,
) -> RenderResult {
    let positions = [
        (bounds.left as f32, bounds.top as f32),
        (bounds.right as f32, bounds.top as f32),
        (bounds.right as f32, bounds.bottom as f32),
        (bounds.left as f32, bounds.bottom as f32),
    ];

    render_handle_circles(ctx, &positions, options)
}

fn render_handle_circles(
    ctx: &mut RenderContext,
    positions: &[(f32, f32)],
    options: &RenderOptions,
) -> RenderResult {
    let fill_brush = ctx
        .get_brush(options.handle_fill_color)
        .ok_or_else(|| RenderError::ResourceCreation("Failed to create fill brush".into()))?
        .clone();
    let border_brush = ctx
        .get_brush(options.handle_border_color)
        .ok_or_else(|| RenderError::ResourceCreation("Failed to create border brush".into()))?
        .clone();

    let radius = options.handle_size;

    unsafe {
        for &(x, y) in positions {
            let ellipse = D2D1_ELLIPSE {
                point: Vector2 { X: x, Y: y },
                radiusX: radius,
                radiusY: radius,
            };
            ctx.render_target.FillEllipse(&ellipse, &fill_brush);
            ctx.render_target
                .DrawEllipse(&ellipse, &border_brush, 1.0, None);
        }
    }

    Ok(())
}

pub fn render_endpoint_handles(
    start: (f32, f32),
    end: (f32, f32),
    ctx: &mut RenderContext,
    options: &RenderOptions,
) -> RenderResult {
    render_handle_circles(ctx, &[start, end], options)
}
