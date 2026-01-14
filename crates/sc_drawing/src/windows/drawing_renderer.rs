use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D1_COLOR_F, D2D1_FIGURE_BEGIN_HOLLOW, D2D1_FIGURE_END_OPEN,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
    D2D1_COMPATIBLE_RENDER_TARGET_OPTIONS_NONE, ID2D1Bitmap, ID2D1BitmapRenderTarget, ID2D1Factory,
    ID2D1PathGeometry, ID2D1RenderTarget,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_ITALIC, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT_BOLD, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_HIT_TEST_METRICS,
    DWRITE_LINE_SPACING_METHOD, DWRITE_LINE_SPACING_METHOD_UNIFORM,
    DWRITE_PARAGRAPH_ALIGNMENT_NEAR, DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_TEXT_RANGE,
    IDWriteFactory, IDWriteTextFormat, IDWriteTextLayout,
};
use windows::core::{PCWSTR, w};
use windows_numerics::{Matrix3x2, Vector2};

use crate::{Color, DrawingElement, DrawingTool, Point, defaults::MIN_FONT_SIZE};

use super::cache::{ElementId, GeometryCache};
use super::context::{BorderStyle, RenderContext, RenderOptions};
use super::renderable::{RenderError, RenderResult, RendererRegistry};

/// Text rendering constants (kept aligned with the app defaults)
const TEXT_CURSOR_WIDTH: f32 = 3.0;
const TEXT_LINE_HEIGHT_SCALE: f32 = 1.35;
const CURSOR_COLOR: Color = Color {
    r: 0.0,
    g: 1.0,
    b: 0.0,
    a: 1.0,
};

/// Cursor rendering state for the active text editing element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextCursorState {
    pub element_id: ElementId,
    /// Cursor position as a Rust `char` index.
    pub cursor_pos: usize,
    pub visible: bool,
}

/// Scene-level renderer for drawing elements on Windows.
#[derive(Default)]
pub struct DrawingRenderer {
    registry: RendererRegistry,
    geometry_cache: GeometryCache,

    static_layer: Option<ID2D1BitmapRenderTarget>,
    static_layer_size: (u32, u32),

    pen_stroke_cache: Option<ID2D1BitmapRenderTarget>,
    pen_stroke_size: (u32, u32),
    last_drawn_point_index: usize,
}

impl DrawingRenderer {
    pub fn new() -> Self {
        Self {
            registry: RendererRegistry::new(),
            geometry_cache: GeometryCache::new(),
            static_layer: None,
            static_layer_size: (0, 0),
            pen_stroke_cache: None,
            pen_stroke_size: (0, 0),
            last_drawn_point_index: 0,
        }
    }

    /// Clears all cached resources (geometries, static layer, pen incremental cache).
    pub fn invalidate_all(&mut self) {
        self.geometry_cache.invalidate_all();
        self.static_layer = None;
        self.static_layer_size = (0, 0);
        self.clear_pen_stroke_cache();
    }

    /// Clears the incremental pen stroke cache.
    pub fn clear_pen_stroke_cache(&mut self) {
        self.pen_stroke_cache = None;
        self.pen_stroke_size = (0, 0);
        self.last_drawn_point_index = 0;
    }

    /// Removes cached resources for a specific element.
    pub fn remove_element_cache(&mut self, element_id: ElementId) {
        self.geometry_cache.remove(element_id);
    }

    /// Renders a full frame.
    ///
    /// Returns `true` if the static layer was rebuilt.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        factory: &ID2D1Factory,
        render_target: &ID2D1RenderTarget,
        dwrite_factory: Option<&IDWriteFactory>,
        screen_size: (u32, u32),
        elements: &[DrawingElement],
        current_element: Option<&DrawingElement>,
        selected_index: Option<usize>,
        cursor: Option<TextCursorState>,
        clip_rect: Option<&RECT>,
        static_layer_needs_rebuild: bool,
    ) -> RenderResult<bool> {
        // Optional clip
        if let Some(r) = clip_rect {
            unsafe {
                let clip = D2D_RECT_F {
                    left: r.left as f32,
                    top: r.top as f32,
                    right: r.right as f32,
                    bottom: r.bottom as f32,
                };
                render_target.PushAxisAlignedClip(&clip, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
            }
        }

        let mut rebuilt = false;

        if static_layer_needs_rebuild {
            self.rebuild_static_layer(
                factory,
                render_target,
                dwrite_factory,
                screen_size,
                elements,
                selected_index,
            )?;
            rebuilt = true;
        }

        // Draw static layer if available, otherwise draw the static content directly.
        if self.static_layer.is_some() {
            self.draw_static_layer(render_target)?;
        } else {
            let mut ctx = RenderContext::new(factory, render_target, dwrite_factory);
            for (i, element) in elements.iter().enumerate() {
                if Some(i) == selected_index {
                    continue;
                }
                self.draw_element(element, &mut ctx, None)?;
            }
        }

        // Dynamic: selected element
        if let Some(idx) = selected_index
            && let Some(element) = elements.get(idx)
        {
            let mut ctx = RenderContext::new(factory, render_target, dwrite_factory);
            self.draw_element(element, &mut ctx, cursor)?;
        }

        // Dynamic: current element
        if let Some(element) = current_element {
            if element.tool == DrawingTool::Pen {
                self.draw_incremental_pen_stroke(factory, render_target, screen_size, element)?;
                self.draw_pen_stroke_from_cache(render_target)?;
            } else {
                let mut ctx = RenderContext::new(factory, render_target, dwrite_factory);
                self.draw_element(element, &mut ctx, None)?;
            }
        }

        // Selection UI
        // Text selection box/handles are only shown while that text element is in edit mode.
        let editing_text_id = cursor.map(|c| c.element_id);
        self.draw_selection_ui(
            factory,
            render_target,
            dwrite_factory,
            elements,
            editing_text_id,
        )?;

        // Pop clip
        if clip_rect.is_some() {
            unsafe {
                render_target.PopAxisAlignedClip();
            }
        }

        Ok(rebuilt)
    }

    /// Renders all elements to an arbitrary render target with an offset transform.
    ///
    /// Used for export/compositing.
    #[allow(clippy::too_many_arguments)]
    pub fn render_elements_to_target_with_offset(
        &self,
        factory: &ID2D1Factory,
        render_target: &ID2D1RenderTarget,
        dwrite_factory: Option<&IDWriteFactory>,
        offset_x: f32,
        offset_y: f32,
        elements: &[DrawingElement],
        current_element: Option<&DrawingElement>,
    ) -> RenderResult<()> {
        unsafe {
            let transform = Matrix3x2::translation(offset_x, offset_y);
            render_target.SetTransform(&transform);
        }

        {
            let mut ctx = RenderContext::new(factory, render_target, dwrite_factory);
            for element in elements {
                self.draw_element_export(element, &mut ctx)?;
            }
            if let Some(element) = current_element {
                self.draw_element_export(element, &mut ctx)?;
            }
        }

        unsafe {
            render_target.SetTransform(&Matrix3x2::identity());
        }

        Ok(())
    }

    fn rebuild_static_layer(
        &mut self,
        factory: &ID2D1Factory,
        render_target: &ID2D1RenderTarget,
        dwrite_factory: Option<&IDWriteFactory>,
        screen_size: (u32, u32),
        elements: &[DrawingElement],
        selected_index: Option<usize>,
    ) -> RenderResult<()> {
        self.ensure_static_layer(render_target, screen_size)?;

        let layer = self
            .static_layer
            .as_ref()
            .ok_or_else(|| RenderError::InvalidState("Static layer target not available".into()))?
            .clone();

        unsafe {
            layer.BeginDraw();
            let clear = D2D1_COLOR_F {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            };
            layer.Clear(Some(&clear));
        }

        let layer_rt: &ID2D1RenderTarget = &layer;
        let mut ctx = RenderContext::new(factory, layer_rt, dwrite_factory);

        for (i, element) in elements.iter().enumerate() {
            if Some(i) == selected_index {
                continue;
            }
            self.draw_element(element, &mut ctx, None)?;
        }

        unsafe {
            layer.EndDraw(None, None).map_err(|e| {
                RenderError::RenderFailed(format!("Static layer EndDraw failed: {e:?}"))
            })?;
        }

        Ok(())
    }

    fn draw_static_layer(&self, render_target: &ID2D1RenderTarget) -> RenderResult<()> {
        let layer = self
            .static_layer
            .as_ref()
            .ok_or_else(|| RenderError::InvalidState("Static layer target not available".into()))?;

        let bitmap: ID2D1Bitmap = unsafe {
            layer
                .GetBitmap()
                .map_err(|e| RenderError::RenderFailed(format!("GetBitmap failed: {e:?}")))?
        };

        unsafe {
            render_target.DrawBitmap(
                &bitmap,
                None,
                1.0,
                D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                None,
            );
        }

        Ok(())
    }

    fn draw_selection_ui(
        &mut self,
        factory: &ID2D1Factory,
        render_target: &ID2D1RenderTarget,
        dwrite_factory: Option<&IDWriteFactory>,
        elements: &[DrawingElement],
        editing_text_id: Option<ElementId>,
    ) -> RenderResult<()> {
        let mut ctx = RenderContext::new(factory, render_target, dwrite_factory);

        // Keep options aligned with the previous behavior.
        let options = RenderOptions {
            show_handles: true,
            show_selection_border: true,
            border_style: BorderStyle::Dashed,
            ..RenderOptions::default()
        };

        for element in elements.iter().filter(|e| e.selected) {
            if element.tool == DrawingTool::Text {
                // Only show the text box (selection border + handles) while editing this text element.
                if editing_text_id != Some(element.id) {
                    continue;
                }
            }

            self.registry
                .render_element_selection(element, &mut ctx, &options)?;
        }

        Ok(())
    }

    fn draw_element(
        &mut self,
        element: &DrawingElement,
        ctx: &mut RenderContext,
        cursor: Option<TextCursorState>,
    ) -> RenderResult<()> {
        match element.tool {
            DrawingTool::Pen => self.draw_pen_element(ctx, element),
            DrawingTool::Text => self.draw_text_element(ctx, element, cursor),
            _ => self.registry.render_element(element, ctx),
        }
    }

    /// Draws an element without mutating internal caches.
    ///
    /// This is used for export/compositing paths where `DrawingManager` may only have `&self`.
    fn draw_element_export(
        &self,
        element: &DrawingElement,
        ctx: &mut RenderContext,
    ) -> RenderResult<()> {
        match element.tool {
            DrawingTool::Pen => {
                if element.points.len() < 2 {
                    return Ok(());
                }

                let brush = ctx
                    .get_brush(element.color)
                    .ok_or_else(|| RenderError::ResourceCreation("Failed to create brush".into()))?
                    .clone();

                if let Some(path) = Self::create_pen_path_geometry(ctx.factory, &element.points) {
                    unsafe {
                        ctx.render_target
                            .DrawGeometry(&path, &brush, element.thickness, None);
                    }
                }

                Ok(())
            }
            DrawingTool::Text => {
                if element.points.is_empty() {
                    return Ok(());
                }

                let Some(dwrite_factory) = ctx.dwrite_factory else {
                    return Ok(());
                };

                let brush = ctx
                    .get_brush(element.color)
                    .ok_or_else(|| RenderError::ResourceCreation("Failed to create brush".into()))?
                    .clone();

                unsafe {
                    let text_format = create_text_format_from_element(
                        dwrite_factory,
                        &element.font_name,
                        element.get_effective_font_size(),
                        element.font_weight,
                        element.font_italic,
                    )?;

                    let padding =
                        super::text::text_padding_for_font_size(element.get_effective_font_size());

                    let width =
                        ((element.rect.right - element.rect.left) as f32 - padding * 2.0).max(0.0);
                    let height =
                        ((element.rect.bottom - element.rect.top) as f32 - padding * 2.0).max(0.0);

                    let layout = create_text_layout_with_style(
                        dwrite_factory,
                        &text_format,
                        &element.text,
                        width,
                        height,
                        element.font_underline,
                        element.font_strikeout,
                    )?;

                    let origin = Vector2 {
                        X: element.rect.left as f32 + padding,
                        Y: element.rect.top as f32 + padding,
                    };

                    // Clip to the text element bounds. Without this, when the box becomes smaller
                    // than the laid-out text (e.g. due to rounding during resize), text/caret can
                    // be drawn outside the selection box.
                    let clip_rect = D2D_RECT_F {
                        left: element.rect.left as f32,
                        top: element.rect.top as f32,
                        right: element.rect.right as f32,
                        bottom: element.rect.bottom as f32,
                    };

                    ctx.render_target
                        .PushAxisAlignedClip(&clip_rect, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);

                    ctx.render_target.DrawTextLayout(
                        origin,
                        &layout,
                        &brush,
                        windows::Win32::Graphics::Direct2D::D2D1_DRAW_TEXT_OPTIONS_NONE,
                    );

                    ctx.render_target.PopAxisAlignedClip();
                }

                Ok(())
            }
            _ => self.registry.render_element(element, ctx),
        }
    }

    fn draw_pen_element(
        &mut self,
        ctx: &mut RenderContext,
        element: &DrawingElement,
    ) -> RenderResult<()> {
        if element.points.len() < 2 {
            return Ok(());
        }

        let brush = ctx
            .get_brush(element.color)
            .ok_or_else(|| RenderError::ResourceCreation("Failed to create brush".into()))?
            .clone();

        let id = element.id;
        let factory = ctx.factory;
        let points = &element.points;

        let geometry = self
            .geometry_cache
            .get_or_create_path(id, || Self::create_pen_path_geometry(factory, points))
            .ok_or_else(|| {
                RenderError::ResourceCreation("Failed to create pen path geometry".into())
            })?;

        unsafe {
            ctx.render_target
                .DrawGeometry(geometry, &brush, element.thickness, None);
        }

        Ok(())
    }

    fn create_pen_path_geometry(
        factory: &ID2D1Factory,
        points: &[Point],
    ) -> Option<ID2D1PathGeometry> {
        if points.len() < 2 {
            return None;
        }

        unsafe {
            let path = factory.CreatePathGeometry().ok()?;
            let sink = path.Open().ok()?;

            let first = Vector2 {
                X: points[0].x as f32,
                Y: points[0].y as f32,
            };
            sink.BeginFigure(first, D2D1_FIGURE_BEGIN_HOLLOW);

            for p in points.iter().skip(1) {
                let v = Vector2 {
                    X: p.x as f32,
                    Y: p.y as f32,
                };
                sink.AddLine(v);
            }

            sink.EndFigure(D2D1_FIGURE_END_OPEN);
            sink.Close().ok()?;

            Some(path)
        }
    }

    fn draw_text_element(
        &mut self,
        ctx: &mut RenderContext,
        element: &DrawingElement,
        cursor: Option<TextCursorState>,
    ) -> RenderResult<()> {
        if element.points.is_empty() {
            return Ok(());
        }

        let dwrite_factory = ctx
            .dwrite_factory
            .ok_or_else(|| RenderError::InvalidState("DirectWrite factory not available".into()))?;

        let brush = ctx
            .get_brush(element.color)
            .ok_or_else(|| RenderError::ResourceCreation("Failed to create brush".into()))?
            .clone();

        unsafe {
            let text_format = create_text_format_from_element(
                dwrite_factory,
                &element.font_name,
                element.get_effective_font_size(),
                element.font_weight,
                element.font_italic,
            )?;

            let padding =
                super::text::text_padding_for_font_size(element.get_effective_font_size());

            let width = ((element.rect.right - element.rect.left) as f32 - padding * 2.0).max(0.0);
            let height = ((element.rect.bottom - element.rect.top) as f32 - padding * 2.0).max(0.0);

            let layout = create_text_layout_with_style(
                dwrite_factory,
                &text_format,
                &element.text,
                width,
                height,
                element.font_underline,
                element.font_strikeout,
            )?;

            let origin = Vector2 {
                X: element.rect.left as f32 + padding,
                Y: element.rect.top as f32 + padding,
            };

            // Clip to the text element bounds so reflow/rounding during resize can't draw outside.
            let clip_rect = D2D_RECT_F {
                left: element.rect.left as f32,
                top: element.rect.top as f32,
                right: element.rect.right as f32,
                bottom: element.rect.bottom as f32,
            };

            ctx.render_target
                .PushAxisAlignedClip(&clip_rect, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);

            ctx.render_target.DrawTextLayout(
                origin,
                &layout,
                &brush,
                windows::Win32::Graphics::Direct2D::D2D1_DRAW_TEXT_OPTIONS_NONE,
            );

            let cursor_res = if let Some(c) = cursor
                && c.visible
                && c.element_id == element.id
            {
                let cursor_brush = ctx
                    .get_brush(CURSOR_COLOR)
                    .ok_or_else(|| {
                        RenderError::ResourceCreation("Failed to create cursor brush".into())
                    })?
                    .clone();

                let content_rect = D2D_RECT_F {
                    left: element.rect.left as f32 + padding,
                    top: element.rect.top as f32 + padding,
                    right: element.rect.right as f32 - padding,
                    bottom: element.rect.bottom as f32 - padding,
                };

                draw_text_cursor(
                    ctx.render_target,
                    element,
                    &layout,
                    &content_rect,
                    &cursor_brush,
                    c.cursor_pos,
                )
            } else {
                Ok(())
            };

            ctx.render_target.PopAxisAlignedClip();

            cursor_res?;
        }

        Ok(())
    }

    fn ensure_static_layer(
        &mut self,
        render_target: &ID2D1RenderTarget,
        screen_size: (u32, u32),
    ) -> RenderResult<()> {
        if self.static_layer.is_some() && self.static_layer_size == screen_size {
            return Ok(());
        }

        self.static_layer = None;
        self.static_layer_size = screen_size;

        unsafe {
            let target = render_target
                .CreateCompatibleRenderTarget(
                    None,
                    None,
                    None,
                    D2D1_COMPATIBLE_RENDER_TARGET_OPTIONS_NONE,
                )
                .map_err(|e| {
                    RenderError::ResourceCreation(format!(
                        "CreateCompatibleRenderTarget (static layer) failed: {e:?}"
                    ))
                })?;
            self.static_layer = Some(target);
        }

        Ok(())
    }

    fn ensure_pen_cache(
        &mut self,
        render_target: &ID2D1RenderTarget,
        screen_size: (u32, u32),
    ) -> RenderResult<()> {
        if self.pen_stroke_cache.is_some() && self.pen_stroke_size == screen_size {
            return Ok(());
        }

        self.pen_stroke_cache = None;
        self.pen_stroke_size = screen_size;
        self.last_drawn_point_index = 0;

        unsafe {
            let cache = render_target
                .CreateCompatibleRenderTarget(
                    None,
                    None,
                    None,
                    D2D1_COMPATIBLE_RENDER_TARGET_OPTIONS_NONE,
                )
                .map_err(|e| {
                    RenderError::ResourceCreation(format!(
                        "CreateCompatibleRenderTarget (pen cache) failed: {e:?}"
                    ))
                })?;

            cache.BeginDraw();
            let clear = D2D1_COLOR_F {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            };
            cache.Clear(Some(&clear));
            cache.EndDraw(None, None).map_err(|e| {
                RenderError::RenderFailed(format!("Pen cache EndDraw failed: {e:?}"))
            })?;

            self.pen_stroke_cache = Some(cache);
        }

        Ok(())
    }

    fn draw_incremental_pen_stroke(
        &mut self,
        factory: &ID2D1Factory,
        render_target: &ID2D1RenderTarget,
        screen_size: (u32, u32),
        element: &DrawingElement,
    ) -> RenderResult<()> {
        if element.tool != DrawingTool::Pen {
            return Ok(());
        }

        let points = &element.points;
        if points.len() <= 1 {
            return Ok(());
        }

        self.ensure_pen_cache(render_target, screen_size)?;

        let cache = self
            .pen_stroke_cache
            .as_ref()
            .ok_or_else(|| RenderError::InvalidState("Pen cache not available".into()))?
            .clone();

        let last_idx = self.last_drawn_point_index;
        let current_len = points.len();

        if last_idx >= current_len.saturating_sub(1) {
            return Ok(());
        }

        let start_idx = if last_idx == 0 { 0 } else { last_idx };

        unsafe {
            cache.BeginDraw();

            // Create a tiny context for this cache target.
            let cache_rt: &ID2D1RenderTarget = &cache;
            let mut ctx = RenderContext::new(factory, cache_rt, None);

            let brush = ctx
                .get_brush(element.color)
                .ok_or_else(|| RenderError::ResourceCreation("Failed to create brush".into()))?
                .clone();

            for i in start_idx..current_len.saturating_sub(1) {
                let p1 = &points[i];
                let p2 = &points[i + 1];

                let start = Vector2 {
                    X: p1.x as f32,
                    Y: p1.y as f32,
                };
                let end = Vector2 {
                    X: p2.x as f32,
                    Y: p2.y as f32,
                };

                cache_rt.DrawLine(start, end, &brush, element.thickness, None);
            }

            cache.EndDraw(None, None).map_err(|e| {
                RenderError::RenderFailed(format!("Pen cache EndDraw failed: {e:?}"))
            })?;
        }

        self.last_drawn_point_index = current_len.saturating_sub(1);

        Ok(())
    }

    fn draw_pen_stroke_from_cache(&self, render_target: &ID2D1RenderTarget) -> RenderResult<()> {
        let cache = match &self.pen_stroke_cache {
            Some(c) => c,
            None => return Ok(()),
        };

        let bitmap: ID2D1Bitmap = unsafe {
            cache
                .GetBitmap()
                .map_err(|e| RenderError::RenderFailed(format!("GetBitmap failed: {e:?}")))?
        };

        unsafe {
            render_target.DrawBitmap(
                &bitmap,
                None,
                1.0,
                D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                None,
            );
        }

        Ok(())
    }
}

fn to_wide_chars(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn create_text_format_from_element(
    dwrite_factory: &IDWriteFactory,
    font_name: &str,
    font_size: f32,
    font_weight: i32,
    font_italic: bool,
) -> RenderResult<IDWriteTextFormat> {
    let font_size = font_size.max(MIN_FONT_SIZE);
    let font_name_wide = to_wide_chars(font_name);

    let weight = if font_weight > 400 {
        DWRITE_FONT_WEIGHT_BOLD
    } else {
        DWRITE_FONT_WEIGHT_NORMAL
    };

    let style = if font_italic {
        DWRITE_FONT_STYLE_ITALIC
    } else {
        DWRITE_FONT_STYLE_NORMAL
    };

    let text_format = unsafe {
        dwrite_factory
            .CreateTextFormat(
                PCWSTR(font_name_wide.as_ptr()),
                None,
                weight,
                style,
                DWRITE_FONT_STRETCH_NORMAL,
                font_size,
                w!(""),
            )
            .map_err(|e| RenderError::ResourceCreation(format!("TextFormat: {e:?}")))?
    };

    unsafe {
        text_format
            .SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING)
            .map_err(|e| RenderError::ResourceCreation(format!("SetTextAlignment: {e:?}")))?;
        text_format
            .SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_NEAR)
            .map_err(|e| RenderError::ResourceCreation(format!("SetParagraphAlignment: {e:?}")))?;

        // Keep DirectWrite line spacing aligned with our sizing/caret logic.
        //
        // Without this, DWrite hit-test metrics (and thus caret height) can diverge from the
        // host-side `TEXT_LINE_HEIGHT_SCALE` policy, causing visible caret height changes after
        // inserting newlines.
        let desired_line_spacing = (font_size * TEXT_LINE_HEIGHT_SCALE).ceil();
        if desired_line_spacing > 0.0 {
            // Choose a baseline that keeps text visually centered within our line box.
            //
            // We start from DirectWrite's default line spacing/baseline for the current font size,
            // then apply our custom `desired_line_spacing` by distributing the extra leading evenly
            // above and below (CSS-like half-leading).
            let mut method = DWRITE_LINE_SPACING_METHOD(0);
            let mut default_spacing = 0.0f32;
            let mut default_baseline = 0.0f32;

            let baseline = if text_format
                .GetLineSpacing(&mut method, &mut default_spacing, &mut default_baseline)
                .is_ok()
                && default_spacing > 0.0
            {
                let extra = (desired_line_spacing - default_spacing).max(0.0);
                (default_baseline + extra / 2.0).clamp(0.0, desired_line_spacing)
            } else {
                // Fallback heuristic.
                (desired_line_spacing * 0.72).clamp(0.0, desired_line_spacing)
            };

            let _ = text_format.SetLineSpacing(
                DWRITE_LINE_SPACING_METHOD_UNIFORM,
                desired_line_spacing,
                baseline,
            );
        }
    }

    Ok(text_format)
}

fn create_text_layout_with_style(
    dwrite_factory: &IDWriteFactory,
    text_format: &IDWriteTextFormat,
    text: &str,
    width: f32,
    height: f32,
    underline: bool,
    strikeout: bool,
) -> RenderResult<IDWriteTextLayout> {
    let wide_text: Vec<u16> = text.encode_utf16().collect();

    let layout = unsafe {
        dwrite_factory
            .CreateTextLayout(&wide_text, text_format, width, height)
            .map_err(|e| RenderError::ResourceCreation(format!("TextLayout: {e:?}")))?
    };

    if !wide_text.is_empty() {
        let range = DWRITE_TEXT_RANGE {
            startPosition: 0,
            length: wide_text.len() as u32,
        };
        unsafe {
            if underline {
                let _ = layout.SetUnderline(true, range);
            }
            if strikeout {
                let _ = layout.SetStrikethrough(true, range);
            }
        }
    }

    Ok(layout)
}

fn draw_text_cursor(
    render_target: &ID2D1RenderTarget,
    element: &DrawingElement,
    layout: &IDWriteTextLayout,
    text_content_rect: &D2D_RECT_F,
    cursor_brush: &windows::Win32::Graphics::Direct2D::ID2D1SolidColorBrush,
    cursor_pos_chars: usize,
) -> RenderResult<()> {
    let mut point_x = 0.0f32;
    let mut point_y = 0.0f32;
    let mut metrics = DWRITE_HIT_TEST_METRICS::default();

    let font_size = element.get_effective_font_size();

    // Convert cursor char index to UTF-16 offset
    let utf16_len = element.text.encode_utf16().count();
    let utf16_pos = element
        .text
        .chars()
        .take(cursor_pos_chars)
        .map(|c| c.len_utf16())
        .sum::<usize>()
        .min(utf16_len);

    if utf16_len == 0 {
        point_x = 0.0;
        point_y = 0.0;
    } else {
        // Prefer hit testing the exact caret position. This is important when the caret is at the
        // end of the text and the last character is a newline: `textLength` correctly maps to the
        // start of the next line.
        let mut hit_ok = unsafe {
            layout
                .HitTestTextPosition(
                    utf16_pos as u32,
                    false,
                    &mut point_x,
                    &mut point_y,
                    &mut metrics,
                )
                .is_ok()
        };

        if !hit_ok && utf16_pos > 0 {
            hit_ok = unsafe {
                layout
                    .HitTestTextPosition(
                        (utf16_pos - 1) as u32,
                        true,
                        &mut point_x,
                        &mut point_y,
                        &mut metrics,
                    )
                    .is_ok()
            };
        }

        if !hit_ok {
            point_x = 0.0;
            point_y = 0.0;
        }
    }

    let abs_x = text_content_rect.left + point_x;
    let abs_y = text_content_rect.top + point_y;

    // Use DirectWrite's hit-test metrics to position the caret.
    //
    // NOTE: `point_y` returned by HitTestTextPosition is already the Y offset of the line within
    // the layout. `metrics.top` is also layout-relative, so adding both would double-count the
    // line offset (the bug that showed up after inserting newlines).
    //
    // We clamp caret height to font_size for a cleaner visual, and center it within the line box.
    let line_box_height = if metrics.height > 0.0 {
        metrics.height
    } else {
        // Fallback for edge cases (e.g. empty line / unusual hit-test results).
        (font_size * TEXT_LINE_HEIGHT_SCALE).ceil()
    };

    let caret_height = font_size.ceil().min(line_box_height);

    // Center the caret vertically within the line box.
    let caret_top = abs_y + ((line_box_height - caret_height) / 2.0).max(0.0);

    let cursor_rect = D2D_RECT_F {
        left: abs_x,
        top: caret_top,
        right: abs_x + TEXT_CURSOR_WIDTH,
        bottom: caret_top + caret_height,
    };

    unsafe {
        render_target.FillRectangle(&cursor_rect, cursor_brush);
    }

    Ok(())
}
