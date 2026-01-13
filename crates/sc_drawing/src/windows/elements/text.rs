use windows::Win32::Graphics::Direct2D::D2D1_DRAW_TEXT_OPTIONS_NONE;
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_ITALIC, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT_BOLD, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_PARAGRAPH_ALIGNMENT_NEAR,
    DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_TEXT_RANGE,
};
use windows::core::{PCWSTR, w};
use windows_numerics::Vector2;

use super::common::{render_handles_corners, render_selection_border};
use crate::windows::context::{RenderContext, RenderOptions};
use crate::windows::renderable::{RenderError, RenderResult, Renderable};
use crate::{DrawingElement, Rect};

/// 文本内边距
const TEXT_PADDING: f32 = 4.0;

/// 文本渲染器
pub struct TextRenderer;

impl TextRenderer {
    /// 将字符串转换为宽字符
    fn to_wide_chars(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }
}

impl Renderable for TextRenderer {
    fn render(&self, element: &DrawingElement, ctx: &mut RenderContext) -> RenderResult {
        if element.points.is_empty() || element.text.is_empty() {
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
            // 创建文本格式
            let font_name_wide = Self::to_wide_chars(&element.font_name);
            let font_size = element.get_effective_font_size();

            let weight = if element.font_weight > 400 {
                DWRITE_FONT_WEIGHT_BOLD
            } else {
                DWRITE_FONT_WEIGHT_NORMAL
            };

            let style = if element.font_italic {
                DWRITE_FONT_STYLE_ITALIC
            } else {
                DWRITE_FONT_STYLE_NORMAL
            };

            let text_format = dwrite_factory
                .CreateTextFormat(
                    PCWSTR(font_name_wide.as_ptr()),
                    None,
                    weight,
                    style,
                    DWRITE_FONT_STRETCH_NORMAL,
                    font_size,
                    w!(""),
                )
                .map_err(|e| RenderError::ResourceCreation(format!("TextFormat: {e:?}")))?;

            text_format
                .SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING)
                .map_err(|e| RenderError::ResourceCreation(format!("SetTextAlignment: {e:?}")))?;
            text_format
                .SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_NEAR)
                .map_err(|e| {
                    RenderError::ResourceCreation(format!("SetParagraphAlignment: {e:?}"))
                })?;

            // 计算文本区域（带内边距）
            let width =
                ((element.rect.right - element.rect.left) as f32 - TEXT_PADDING * 2.0).max(0.0);
            let height =
                ((element.rect.bottom - element.rect.top) as f32 - TEXT_PADDING * 2.0).max(0.0);

            // 创建文本布局
            let wide_text: Vec<u16> = element.text.encode_utf16().collect();
            let layout = dwrite_factory
                .CreateTextLayout(&wide_text, &text_format, width, height)
                .map_err(|e| RenderError::ResourceCreation(format!("TextLayout: {e:?}")))?;

            // 应用下划线和删除线
            if !wide_text.is_empty() {
                let range = DWRITE_TEXT_RANGE {
                    startPosition: 0,
                    length: wide_text.len() as u32,
                };

                if element.font_underline {
                    let _ = layout.SetUnderline(true, range);
                }
                if element.font_strikeout {
                    let _ = layout.SetStrikethrough(true, range);
                }
            }

            // 绘制文本
            let origin = Vector2 {
                X: element.rect.left as f32 + TEXT_PADDING,
                Y: element.rect.top as f32 + TEXT_PADDING,
            };

            ctx.render_target
                .DrawTextLayout(origin, &layout, &brush, D2D1_DRAW_TEXT_OPTIONS_NONE);
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
        // 文本只显示4个角手柄
        render_handles_corners(bounds, ctx, options)
    }
}
