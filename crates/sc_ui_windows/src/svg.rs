use std::fs;

use anyhow::Result;

/// SVG 渲染选项
pub struct SvgRenderOptions {
    /// 渲染尺寸
    pub size: u32,
    /// 缩放因子（用于高 DPI）
    pub scale: f32,
    /// 颜色覆盖（RGB）
    pub color_override: Option<(u8, u8, u8)>,
    /// 输出格式
    pub output_format: PixelFormat,
}

/// 像素格式
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    /// RGBA 格式（tiny_skia 原生格式）
    Rgba,
    /// BGRA 格式（Direct2D 需要的格式）
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

/// 从 SVG 字符串渲染为像素数据
///
/// # 参数
/// - `svg_content`: SVG 内容字符串
/// - `options`: 渲染选项
///
/// # 返回
/// - 渲染后的像素数据和实际尺寸 (pixels, width, height)
pub fn render_svg_to_pixels(
    svg_content: &str,
    options: &SvgRenderOptions,
) -> Result<(Vec<u8>, u32, u32)> {
    // 处理颜色替换（在 SVG 字符串级别）
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

    // 解析 SVG
    let tree = usvg::Tree::from_str(&processed_svg, &usvg::Options::default())?;

    // 计算渲染尺寸
    let render_size = (options.size as f32 * options.scale) as u32;

    // 创建像素缓冲区
    let mut pixmap = tiny_skia::Pixmap::new(render_size, render_size)
        .ok_or_else(|| anyhow::anyhow!("Failed to create pixmap"))?;

    pixmap.fill(tiny_skia::Color::TRANSPARENT);

    // 计算变换（保持宽高比）
    let svg_size = tree.size();
    let transform = tiny_skia::Transform::from_scale(
        options.size as f32 * options.scale / svg_size.width(),
        options.size as f32 * options.scale / svg_size.height(),
    );

    // 渲染 SVG
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    // 转换像素格式
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

/// 从文件路径加载并渲染 SVG
pub fn render_svg_from_file(path: &str, options: &SvgRenderOptions) -> Result<(Vec<u8>, u32, u32)> {
    let svg_content = fs::read_to_string(path)?;
    render_svg_to_pixels(&svg_content, options)
}

/// 将 RGBA 像素数据转换为 BGRA 格式
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

/// 在像素级别应用颜色覆盖
///
/// 用于需要在渲染后修改颜色的场景（如 PreviewRenderer 的悬停效果）
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

        // Direct2D 位图这里使用的是 D2D1_ALPHA_MODE_PREMULTIPLIED。
        // tiny-skia/resvg 渲染输出也是预乘 alpha。
        // 所以在做颜色覆盖时必须保持预乘，否则在半透明边缘会出现发白/锯齿光晕。
        chunk[r_idx] = ((r as u16 * a + 127) / 255) as u8;
        chunk[g_idx] = ((g as u16 * a + 127) / 255) as u8;
        chunk[b_idx] = ((b as u16 * a + 127) / 255) as u8;
    }
}
