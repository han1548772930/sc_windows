// Direct2D 绘图辅助函数
//
// 统一的Direct2D操作封装，减少重复代码

use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::core::*;
use windows_numerics::*;

// ==================== Direct2D基础辅助 ====================

/// 创建Direct2D点
#[inline]
pub fn d2d_point(x: i32, y: i32) -> Vector2 {
    Vector2 {
        X: x as f32,
        Y: y as f32,
    }
}

/// 创建Direct2D点（从f32坐标）
#[inline]
pub fn d2d_point_f(x: f32, y: f32) -> Vector2 {
    Vector2 {
        X: x,
        Y: y,
    }
}

/// 创建Direct2D矩形
#[inline]
pub fn d2d_rect(left: i32, top: i32, right: i32, bottom: i32) -> D2D_RECT_F {
    D2D_RECT_F {
        left: left as f32,
        top: top as f32,
        right: right as f32,
        bottom: bottom as f32,
    }
}

/// 创建标准化的Direct2D矩形（确保left < right, top < bottom）
#[inline]
pub fn d2d_rect_normalized(x1: i32, y1: i32, x2: i32, y2: i32) -> D2D_RECT_F {
    D2D_RECT_F {
        left: x1.min(x2) as f32,
        top: y1.min(y2) as f32,
        right: x1.max(x2) as f32,
        bottom: y1.max(y2) as f32,
    }
}

// ==================== 画刷创建辅助 ====================

/// 创建纯色画刷的辅助函数
#[inline]
pub unsafe fn create_solid_brush(
    render_target: &ID2D1RenderTarget,
    color: &D2D1_COLOR_F,
) -> Result<ID2D1SolidColorBrush> {
    unsafe { render_target.CreateSolidColorBrush(color, None) }
}

/// 从RGB值创建颜色
#[inline]
pub const fn rgb_color(r: u8, g: u8, b: u8) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: 1.0,
    }
}

/// 从RGBA值创建颜色
#[inline]
pub const fn rgba_color(r: u8, g: u8, b: u8, a: u8) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: a as f32 / 255.0,
    }
}

// ==================== 几何图形辅助 ====================

/// 创建圆角矩形
#[inline]
pub const fn rounded_rect(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    radius: f32,
) -> D2D1_ROUNDED_RECT {
    D2D1_ROUNDED_RECT {
        rect: D2D_RECT_F {
            left: x,
            top: y,
            right: x + width,
            bottom: y + height,
        },
        radiusX: radius,
        radiusY: radius,
    }
}

/// 创建椭圆
#[inline]
pub const fn ellipse(cx: f32, cy: f32, rx: f32, ry: f32) -> D2D1_ELLIPSE {
    D2D1_ELLIPSE {
        point: windows_numerics::Vector2 { X: cx, Y: cy },
        radiusX: rx,
        radiusY: ry,
    }
}

/// 创建圆形（特殊的椭圆）
#[inline]
pub const fn circle(cx: f32, cy: f32, radius: f32) -> D2D1_ELLIPSE {
    ellipse(cx, cy, radius, radius)
}

// ==================== 文本渲染辅助 ====================

/// 文本渲染配置
pub struct TextRenderConfig<'a> {
    pub text: &'a str,
    pub font_name: &'a str,
    pub font_size: f32,
    pub font_weight: DWRITE_FONT_WEIGHT,
    pub font_style: DWRITE_FONT_STYLE,
    pub color: D2D1_COLOR_F,
    pub alignment: DWRITE_TEXT_ALIGNMENT,
    pub paragraph_alignment: DWRITE_PARAGRAPH_ALIGNMENT,
}

impl<'a> Default for TextRenderConfig<'a> {
    fn default() -> Self {
        Self {
            text: "",
            font_name: "Segoe UI",
            font_size: 14.0,
            font_weight: DWRITE_FONT_WEIGHT_NORMAL,
            font_style: DWRITE_FONT_STYLE_NORMAL,
            color: D2D1_COLOR_F {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            alignment: DWRITE_TEXT_ALIGNMENT_LEADING,
            paragraph_alignment: DWRITE_PARAGRAPH_ALIGNMENT_NEAR,
        }
    }
}

/// 渲染文本的统一函数
pub unsafe fn render_text(
    render_target: &ID2D1RenderTarget,
    dwrite_factory: &IDWriteFactory,
    rect: &D2D_RECT_F,
    config: &TextRenderConfig,
) -> Result<()> {
    unsafe {
        // 创建文本格式
        let font_name_wide = crate::utils::to_wide_chars(config.font_name);
        let text_format = dwrite_factory.CreateTextFormat(
            PCWSTR(font_name_wide.as_ptr()),
            None,
            config.font_weight,
            config.font_style,
            DWRITE_FONT_STRETCH_NORMAL,
            config.font_size,
            w!(""),
        )?;

        // 设置对齐
        text_format.SetTextAlignment(config.alignment)?;
        text_format.SetParagraphAlignment(config.paragraph_alignment)?;

        // 创建画刷
        let brush = create_solid_brush(render_target, &config.color)?;

        // 转换文本
        let text_wide: Vec<u16> = config.text.encode_utf16().collect();

        // 绘制文本
        render_target.DrawText(
            &text_wide,
            &text_format,
            rect,
            &brush,
            D2D1_DRAW_TEXT_OPTIONS_NONE,
            DWRITE_MEASURING_MODE_NATURAL,
        );

        Ok(())
    }
}

// ==================== 图形绘制辅助 ====================

/// 绘制虚线矩形
pub unsafe fn draw_dashed_rectangle(
    render_target: &ID2D1RenderTarget,
    rect: &D2D_RECT_F,
    color: &D2D1_COLOR_F,
    stroke_width: f32,
    dash_pattern: &[f32],
) -> Result<()> {
    unsafe {
        let brush = create_solid_brush(render_target, color)?;

        // 创建虚线样式
        let factory = render_target.GetFactory()?;
        let stroke_style = factory.CreateStrokeStyle(
            &D2D1_STROKE_STYLE_PROPERTIES {
                startCap: D2D1_CAP_STYLE_FLAT,
                endCap: D2D1_CAP_STYLE_FLAT,
                dashCap: D2D1_CAP_STYLE_FLAT,
                lineJoin: D2D1_LINE_JOIN_MITER,
                miterLimit: 10.0,
                dashStyle: D2D1_DASH_STYLE_CUSTOM,
                dashOffset: 0.0,
            },
            Some(dash_pattern),
        )?;

        render_target.DrawRectangle(rect, &brush, stroke_width, Some(&stroke_style));
        Ok(())
    }
}

/// 绘制填充的圆角矩形
pub unsafe fn fill_rounded_rectangle(
    render_target: &ID2D1RenderTarget,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    radius: f32,
    color: &D2D1_COLOR_F,
) -> Result<()> {
    unsafe {
        let brush = create_solid_brush(render_target, color)?;
        let rect = rounded_rect(x, y, width, height, radius);
        render_target.FillRoundedRectangle(&rect, &brush);
        Ok(())
    }
}

/// 绘制带边框的圆角矩形
pub unsafe fn draw_rounded_rectangle_with_border(
    render_target: &ID2D1RenderTarget,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    radius: f32,
    fill_color: Option<&D2D1_COLOR_F>,
    border_color: &D2D1_COLOR_F,
    border_width: f32,
) -> Result<()> {
    unsafe {
        let rect = rounded_rect(x, y, width, height, radius);

        // 填充
        if let Some(fill) = fill_color {
            let fill_brush = create_solid_brush(render_target, fill)?;
            render_target.FillRoundedRectangle(&rect, &fill_brush);
        }

        // 边框
        let border_brush = create_solid_brush(render_target, border_color)?;
        render_target.DrawRoundedRectangle(&rect, &border_brush, border_width, None);

        Ok(())
    }
}

// ==================== 变换辅助 ====================

/// 保存和恢复变换的RAII守卫
pub struct TransformGuard<'a> {
    render_target: &'a ID2D1RenderTarget,
    original_transform: Matrix3x2,
}

impl<'a> TransformGuard<'a> {
    /// 创建新的变换守卫并应用变换
    pub unsafe fn new(render_target: &'a ID2D1RenderTarget, transform: &Matrix3x2) -> Self {
        unsafe {
            let mut original_transform = Matrix3x2::identity();
            render_target.GetTransform(&mut original_transform);
            render_target.SetTransform(transform);

            Self {
                render_target,
                original_transform,
            }
        }
    }
}

impl<'a> Drop for TransformGuard<'a> {
    fn drop(&mut self) {
        unsafe {
            self.render_target.SetTransform(&self.original_transform);
        }
    }
}

/// 创建平移变换
#[inline]
pub fn translation_transform(x: f32, y: f32) -> Matrix3x2 {
    Matrix3x2::translation(x, y)
}

/// 创建缩放变换
#[inline]
pub fn scale_transform(scale_x: f32, scale_y: f32, center_x: f32, center_y: f32) -> Matrix3x2 {
    // 先平移到原点，缩放，再平移回去
    let to_origin = Matrix3x2::translation(-center_x, -center_y);
    let scale = Matrix3x2::scale(scale_x, scale_y);
    let from_origin = Matrix3x2::translation(center_x, center_y);
    to_origin * scale * from_origin
}

/// 创建旋转变换
#[inline]
pub fn rotation_transform(angle: f32, center_x: f32, center_y: f32) -> Matrix3x2 {
    Matrix3x2::rotation_around(
        angle,
        windows_numerics::Vector2 {
            X: center_x,
            Y: center_y,
        },
    )
}

// ==================== 裁剪区域辅助 ====================

/// 裁剪区域RAII守卫
pub struct ClipGuard<'a> {
    render_target: &'a ID2D1RenderTarget,
}

impl<'a> ClipGuard<'a> {
    /// 创建矩形裁剪区域
    pub unsafe fn new_rect(
        render_target: &'a ID2D1RenderTarget,
        rect: &D2D_RECT_F,
    ) -> Result<Self> {
        unsafe {
            render_target.PushAxisAlignedClip(rect, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
            Ok(Self { render_target })
        }
    }
}

impl<'a> Drop for ClipGuard<'a> {
    fn drop(&mut self) {
        unsafe {
            self.render_target.PopAxisAlignedClip();
        }
    }
}

// ==================== 批处理辅助 ====================

/// 批量绘制操作的辅助结构
pub struct DrawBatch<'a> {
    render_target: &'a ID2D1RenderTarget,
    started: bool,
}

impl<'a> DrawBatch<'a> {
    /// 创建新的批处理
    pub unsafe fn new(render_target: &'a ID2D1RenderTarget) -> Result<Self> {
        unsafe {
            render_target.BeginDraw();
            Ok(Self {
                render_target,
                started: true,
            })
        }
    }

    /// 清除背景
    pub unsafe fn clear(&self, color: &D2D1_COLOR_F) {
        unsafe {
            self.render_target.Clear(Some(color));
        }
    }

    /// 结束批处理并提交
    pub unsafe fn commit(mut self) -> Result<()> {
        unsafe {
            self.started = false;
            self.render_target.EndDraw(None, None)?;
            Ok(())
        }
    }
}

impl<'a> Drop for DrawBatch<'a> {
    fn drop(&mut self) {
        if self.started {
            unsafe {
                let _ = self.render_target.EndDraw(None, None);
            }
        }
    }
}

use windows_numerics::Matrix3x2;
