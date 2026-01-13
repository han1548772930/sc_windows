use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_ITALIC,
    DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_BOLD, DWRITE_FONT_WEIGHT_NORMAL,
    DWRITE_PARAGRAPH_ALIGNMENT_NEAR, DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_TEXT_METRICS,
    DWriteCreateFactory, IDWriteFactory,
};
use windows::core::{PCWSTR, w};

use crate::{DrawingElement, defaults::MIN_FONT_SIZE};

/// Best-effort: update a text element's size based on its content using DirectWrite metrics.
///
/// The sizing policy (min width/height, padding, and line-height scale) is kept in the caller.
/// This function focuses on platform-specific measurement.
pub fn update_text_element_size_dwrite(
    element: &mut DrawingElement,
    min_width: i32,
    min_height: i32,
    padding: f32,
    line_height_scale: f32,
) {
    let font_size = element.get_effective_font_size();
    let dynamic_line_height = (font_size * line_height_scale).ceil() as i32;

    let text_content = element.text.clone();
    let lines: Vec<&str> = if text_content.is_empty() {
        vec![""]
    } else {
        text_content.lines().collect()
    };

    let line_count = if text_content.is_empty() {
        1
    } else if text_content.ends_with('\n') {
        lines.len() as i32 + 1
    } else {
        lines.len() as i32
    };

    let measured_max_width = measure_max_line_width_dwrite(element, &lines, font_size);

    let mut max_width_f = measured_max_width.unwrap_or(0.0);
    if max_width_f == 0.0 {
        max_width_f = min_width as f32;
    } else {
        // Add a small buffer to avoid characters feeling cramped.
        max_width_f += (font_size * 0.2).max(4.0);
    }

    let new_width = ((max_width_f + padding * 2.0).ceil() as i32).max(min_width);
    let new_height = (line_count * dynamic_line_height + (padding * 2.0) as i32).max(min_height);

    element.rect.right = element.rect.left + new_width;
    element.rect.bottom = element.rect.top + new_height;

    // Keep points synced with rect so selection + rendering behave consistently.
    if !element.points.is_empty() {
        element.set_end_point(element.rect.right, element.rect.bottom);
    }
}

fn to_wide_chars(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn measure_max_line_width_dwrite(
    element: &DrawingElement,
    lines: &[&str],
    font_size: f32,
) -> Option<f32> {
    let factory =
        unsafe { DWriteCreateFactory::<IDWriteFactory>(DWRITE_FACTORY_TYPE_SHARED).ok()? };

    let font_name_wide = to_wide_chars(&element.font_name);
    let font_size = font_size.max(MIN_FONT_SIZE);

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

    let text_format = unsafe {
        factory
            .CreateTextFormat(
                PCWSTR(font_name_wide.as_ptr()),
                None,
                weight,
                style,
                DWRITE_FONT_STRETCH_NORMAL,
                font_size,
                w!(""),
            )
            .ok()?
    };

    unsafe {
        let _ = text_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING);
        let _ = text_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_NEAR);
    }

    let mut max_width_f = 0.0f32;

    for line in lines {
        let wide: Vec<u16> = line.encode_utf16().collect();
        let layout = unsafe {
            factory
                .CreateTextLayout(&wide, &text_format, f32::MAX, f32::MAX)
                .ok()
        };
        let Some(layout) = layout else {
            continue;
        };

        let mut metrics = DWRITE_TEXT_METRICS::default();
        unsafe {
            let _ = layout.GetMetrics(&mut metrics);
        }

        if metrics.width > max_width_f {
            max_width_f = metrics.width;
        }
    }

    Some(max_width_f)
}
