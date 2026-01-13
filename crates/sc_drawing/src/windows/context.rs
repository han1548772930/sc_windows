use std::collections::HashMap;
use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;
use windows::Win32::Graphics::Direct2D::{
    D2D1_CAP_STYLE_FLAT, D2D1_DASH_STYLE_CUSTOM, D2D1_DASH_STYLE_SOLID, D2D1_LINE_JOIN_MITER,
    D2D1_STROKE_STYLE_PROPERTIES, ID2D1Factory, ID2D1RenderTarget, ID2D1SolidColorBrush,
    ID2D1StrokeStyle,
};
use windows::Win32::Graphics::DirectWrite::IDWriteFactory;

use crate::Color;

/// 边框样式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BorderStyle {
    /// 实线
    #[default]
    Solid,
    /// 虚线
    Dashed,
}

/// 渲染选项
///
/// 配置元素渲染时的显示选项。
#[derive(Debug, Clone)]
pub struct RenderOptions {
    /// 是否显示调整手柄
    pub show_handles: bool,
    /// 是否显示选中边框
    pub show_selection_border: bool,
    /// 边框样式
    pub border_style: BorderStyle,
    /// 手柄大小（半径）
    pub handle_size: f32,
    /// 选中边框颜色
    pub selection_color: Color,
    /// 手柄填充颜色
    pub handle_fill_color: Color,
    /// 手柄边框颜色
    pub handle_border_color: Color,
    /// 虚线模式 [dash, gap]
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
    /// 创建不显示任何选中UI的选项
    pub fn no_selection_ui() -> Self {
        Self {
            show_handles: false,
            show_selection_border: false,
            ..Default::default()
        }
    }

    /// 创建只显示边框的选项
    pub fn border_only() -> Self {
        Self {
            show_handles: false,
            show_selection_border: true,
            ..Default::default()
        }
    }
}

/// 颜色键（用于画笔缓存）
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

/// 渲染上下文
///
/// 封装 Direct2D 渲染所需的资源和状态。
pub struct RenderContext<'a> {
    /// D2D 工厂
    pub factory: &'a ID2D1Factory,
    /// 渲染目标
    pub render_target: &'a ID2D1RenderTarget,
    /// DirectWrite 工厂（用于文本渲染）
    pub dwrite_factory: Option<&'a IDWriteFactory>,
    /// 画笔缓存
    brush_cache: HashMap<ColorKey, ID2D1SolidColorBrush>,
    /// 虚线样式缓存
    dashed_style: Option<ID2D1StrokeStyle>,
    /// 实线样式缓存
    solid_style: Option<ID2D1StrokeStyle>,
}

impl<'a> RenderContext<'a> {
    /// 创建新的渲染上下文
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

    /// 获取或创建指定颜色的画笔
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

    /// 获取虚线描边样式
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

    /// 获取实线描边样式
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

    /// 清除画笔缓存
    pub fn clear_brush_cache(&mut self) {
        self.brush_cache.clear();
    }
}
