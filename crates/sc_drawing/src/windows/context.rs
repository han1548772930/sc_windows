use std::collections::HashMap;
use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;
use windows::Win32::Graphics::Direct2D::{
    D2D1_CAP_STYLE_FLAT, D2D1_DASH_STYLE_CUSTOM, D2D1_DASH_STYLE_SOLID, D2D1_LINE_JOIN_MITER,
    D2D1_STROKE_STYLE_PROPERTIES, ID2D1Factory, ID2D1RenderTarget, ID2D1SolidColorBrush,
    ID2D1StrokeStyle,
};
use windows::Win32::Graphics::DirectWrite::IDWriteFactory;

use crate::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BorderStyle {
    #[default]
    Solid,
    Dashed,
}

#[derive(Debug, Clone)]
pub struct RenderOptions {
    pub show_handles: bool,
    pub show_selection_border: bool,
    pub border_style: BorderStyle,
    pub handle_size: f32,
    pub selection_color: Color,
    pub handle_fill_color: Color,
    pub handle_border_color: Color,
    pub dash_pattern: [f32; 2],
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            show_handles: true,
            show_selection_border: true,
            border_style: BorderStyle::Dashed,
            handle_size: 3.0,
            selection_color: Color {
                r: 0.0,
                g: 0.5,
                b: 1.0,
                a: 1.0,
            },
            handle_fill_color: Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
            handle_border_color: Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            dash_pattern: [4.0, 2.0],
        }
    }
}

impl RenderOptions {
    pub fn no_selection_ui() -> Self {
        Self {
            show_handles: false,
            show_selection_border: false,
            ..Default::default()
        }
    }

    pub fn border_only() -> Self {
        Self {
            show_handles: false,
            show_selection_border: true,
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ColorKey {
    r: u32,
    g: u32,
    b: u32,
    a: u32,
}

impl From<Color> for ColorKey {
    fn from(c: Color) -> Self {
        Self {
            r: (c.r * 1000.0) as u32,
            g: (c.g * 1000.0) as u32,
            b: (c.b * 1000.0) as u32,
            a: (c.a * 1000.0) as u32,
        }
    }
}

pub struct RenderContext<'a> {
    pub factory: &'a ID2D1Factory,
    pub render_target: &'a ID2D1RenderTarget,
    pub dwrite_factory: Option<&'a IDWriteFactory>,
    brush_cache: HashMap<ColorKey, ID2D1SolidColorBrush>,
    dashed_style: Option<ID2D1StrokeStyle>,
    solid_style: Option<ID2D1StrokeStyle>,
}

impl<'a> RenderContext<'a> {
    pub fn new(
        factory: &'a ID2D1Factory,
        render_target: &'a ID2D1RenderTarget,
        dwrite_factory: Option<&'a IDWriteFactory>,
    ) -> Self {
        Self {
            factory,
            render_target,
            dwrite_factory,
            brush_cache: HashMap::new(),
            dashed_style: None,
            solid_style: None,
        }
    }

    pub fn get_brush(&mut self, color: Color) -> Option<&ID2D1SolidColorBrush> {
        let key = ColorKey::from(color);
        if !self.brush_cache.contains_key(&key) {
            let d2d_color = D2D1_COLOR_F {
                r: color.r,
                g: color.g,
                b: color.b,
                a: color.a,
            };
            let brush = unsafe {
                self.render_target
                    .CreateSolidColorBrush(&d2d_color, None)
                    .ok()?
            };
            self.brush_cache.insert(key, brush);
        }
        self.brush_cache.get(&key)
    }

    pub fn get_dashed_style(&mut self, dash_pattern: &[f32; 2]) -> Option<&ID2D1StrokeStyle> {
        if self.dashed_style.is_none() {
            let props = D2D1_STROKE_STYLE_PROPERTIES {
                startCap: D2D1_CAP_STYLE_FLAT,
                endCap: D2D1_CAP_STYLE_FLAT,
                dashCap: D2D1_CAP_STYLE_FLAT,
                lineJoin: D2D1_LINE_JOIN_MITER,
                miterLimit: 10.0,
                dashStyle: D2D1_DASH_STYLE_CUSTOM,
                dashOffset: 0.0,
            };
            let style = unsafe {
                self.factory
                    .CreateStrokeStyle(&props, Some(dash_pattern))
                    .ok()?
            };
            self.dashed_style = Some(style);
        }
        self.dashed_style.as_ref()
    }

    pub fn get_solid_style(&mut self) -> Option<&ID2D1StrokeStyle> {
        if self.solid_style.is_none() {
            let props = D2D1_STROKE_STYLE_PROPERTIES {
                startCap: D2D1_CAP_STYLE_FLAT,
                endCap: D2D1_CAP_STYLE_FLAT,
                dashCap: D2D1_CAP_STYLE_FLAT,
                lineJoin: D2D1_LINE_JOIN_MITER,
                miterLimit: 10.0,
                dashStyle: D2D1_DASH_STYLE_SOLID,
                dashOffset: 0.0,
            };
            let style = unsafe { self.factory.CreateStrokeStyle(&props, None).ok()? };
            self.solid_style = Some(style);
        }
        self.solid_style.as_ref()
    }

    pub fn clear_brush_cache(&mut self) {
        self.brush_cache.clear();
    }
}
