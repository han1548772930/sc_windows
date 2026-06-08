use std::fs;

use anyhow::Result;

pub struct SvgRenderOptions {
    pub size: u32,
    pub scale: f32,
    pub color_override: Option<(u8, u8, u8)>,
    pub output_format: PixelFormat,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    Rgba,
    Bgra,
}

impl Default for SvgRenderOptions {
    fn default() -> Self {
        Self {
            size: 24,
            scale: 1.0,
            color_override: None,
            output_format: PixelFormat::Bgra,
        }
    }
}

pub fn render_svg_to_pixels(
    svg_content: &str,
    options: &SvgRenderOptions,
) -> Result<(Vec<u8>, u32, u32)> {
    let processed_svg = if let Some((r, g, b)) = options.color_override {
        let color_hex = format!("#{r:02x}{g:02x}{b:02x}");
        svg_content
            .replace(
                "stroke=\"currentColor\"",
                &format!("stroke=\"{color_hex}\""),
            )
            .replace("fill=\"currentColor\"", &format!("fill=\"{color_hex}\""))
    } else {
        svg_content.to_string()
    };

    let tree = usvg::Tree::from_str(&processed_svg, &usvg::Options::default())?;

    let render_size = (options.size as f32 * options.scale) as u32;

    let mut pixmap = tiny_skia::Pixmap::new(render_size, render_size)
        .ok_or_else(|| anyhow::anyhow!("Failed to create pixmap"))?;

    pixmap.fill(tiny_skia::Color::TRANSPARENT);

    let svg_size = tree.size();
    let transform = tiny_skia::Transform::from_scale(
        options.size as f32 * options.scale / svg_size.width(),
        options.size as f32 * options.scale / svg_size.height(),
    );

    resvg::render(&tree, transform, &mut pixmap.as_mut());

    let pixels = match options.output_format {
        PixelFormat::Rgba => pixmap.data().to_vec(),
        PixelFormat::Bgra => rgba_to_bgra(pixmap.data()),
    };

    Ok((pixels, render_size, render_size))
}

/// Render SVG with common options (size/format/color override).
pub fn render_svg_pixels(
    svg_content: &str,
    size: u32,
    output_format: PixelFormat,
    color_override: Option<(u8, u8, u8)>,
) -> Result<(Vec<u8>, u32, u32)> {
    let options = SvgRenderOptions {
        size,
        scale: 1.0,
        color_override,
        output_format,
    };
    render_svg_to_pixels(svg_content, &options)
}

pub fn render_svg_from_file(path: &str, options: &SvgRenderOptions) -> Result<(Vec<u8>, u32, u32)> {
    let svg_content = fs::read_to_string(path)?;
    render_svg_to_pixels(&svg_content, options)
}

fn rgba_to_bgra(rgba: &[u8]) -> Vec<u8> {
    let mut bgra = Vec::with_capacity(rgba.len());
    for chunk in rgba.chunks(4) {
        if chunk.len() == 4 {
            bgra.push(chunk[2]); // B
            bgra.push(chunk[1]); // G
            bgra.push(chunk[0]); // R
            bgra.push(chunk[3]); // A
        }
    }
    bgra
}

pub fn apply_color_to_pixels(pixels: &mut [u8], color: (u8, u8, u8), format: PixelFormat) {
    let (r, g, b) = color;
    let (r_idx, g_idx, b_idx) = match format {
        PixelFormat::Rgba => (0, 1, 2),
        PixelFormat::Bgra => (2, 1, 0),
    };

    for chunk in pixels.chunks_mut(4) {
        if chunk.len() != 4 {
            continue;
        }

        let a = chunk[3] as u16;
        if a == 0 {
            continue;
        }

        chunk[r_idx] = ((r as u16 * a + 127) / 255) as u8;
        chunk[g_idx] = ((g as u16 * a + 127) / 255) as u8;
        chunk[b_idx] = ((b as u16 * a + 127) / 255) as u8;
    }
}
