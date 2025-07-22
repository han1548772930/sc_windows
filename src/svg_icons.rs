use crate::types::ToolbarButton;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;

#[derive(Debug)]
pub struct SvgIconManager {
    icons: HashMap<ToolbarButton, Vec<u8>>,
    rendered_icons: RefCell<HashMap<(ToolbarButton, Option<(u8, u8, u8)>), ID2D1Bitmap>>,
}

impl SvgIconManager {
    pub fn new() -> Self {
        Self {
            icons: HashMap::new(),
            rendered_icons: RefCell::new(HashMap::new()),
        }
    }

    pub fn load_icons(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // 定义图标文件映射
        let icon_files = [
            (ToolbarButton::Arrow, "icons/move-up-right.svg"),
            (ToolbarButton::Rectangle, "icons/square.svg"),
            (ToolbarButton::Circle, "icons/circle.svg"),
            (ToolbarButton::Pen, "icons/pen.svg"),
            (ToolbarButton::Text, "icons/type.svg"),
            (ToolbarButton::Undo, "icons/undo-2.svg"),
            (ToolbarButton::ExtractText, "icons/extracttext.svg"),
            (ToolbarButton::Languages, "icons/languages.svg"),    
            (ToolbarButton::Save, "icons/download.svg"),
            (ToolbarButton::Pin, "icons/pin.svg"),
            (ToolbarButton::Confirm, "icons/check.svg"),
            (ToolbarButton::Cancel, "icons/x.svg"),
        ];

        for (button, file_path) in &icon_files {
            if Path::new(file_path).exists() {
                let svg_data = std::fs::read(file_path)?;
                self.icons.insert(*button, svg_data);
            }
        }

        Ok(())
    }

    pub fn render_icon_to_bitmap(
        &self,
        button: ToolbarButton,
        render_target: &ID2D1RenderTarget,
        size: u32,
        color: Option<(u8, u8, u8)>, // RGB 颜色，None 表示使用默认颜色
    ) -> Result<Option<ID2D1Bitmap>, Box<dyn std::error::Error>> {
        // 如果已经渲染过，直接返回
        let cache_key = (button, color);
        if let Some(bitmap) = self.rendered_icons.borrow().get(&cache_key) {
            return Ok(Some(bitmap.clone()));
        }

        // 获取 SVG 数据
        let svg_data = match self.icons.get(&button) {
            Some(data) => data,
            None => return Ok(None),
        };

        // 解析 SVG，如果需要修改颜色，先处理 SVG 字符串
        let mut svg_str = String::from_utf8_lossy(svg_data).to_string();

        // 如果指定了颜色，修改 SVG 的 stroke 属性
        if let Some((r, g, b)) = color {
            let color_hex = format!("#{:02x}{:02x}{:02x}", r, g, b);
            // 替换 stroke="currentColor" 为指定颜色
            svg_str = svg_str.replace(
                "stroke=\"currentColor\"",
                &format!("stroke=\"{}\"", color_hex),
            );
            // 也替换可能的 fill="currentColor"
            svg_str = svg_str.replace("fill=\"currentColor\"", &format!("fill=\"{}\"", color_hex));
        }

        let opt = usvg::Options::default();
        let tree = usvg::Tree::from_str(&svg_str, &opt)?;

        // 创建 tiny-skia 画布
        let mut pixmap = tiny_skia::Pixmap::new(size, size).unwrap();

        // 渲染 SVG 到画布
        let transform = tiny_skia::Transform::from_scale(
            size as f32 / tree.size().width(),
            size as f32 / tree.size().height(),
        );

        resvg::render(&tree, transform, &mut pixmap.as_mut());

        // 将 pixmap 数据转换为 Direct2D 位图
        let bitmap = self.create_d2d_bitmap_from_pixmap(render_target, &pixmap, size)?;

        // 缓存位图
        self.rendered_icons
            .borrow_mut()
            .insert(cache_key, bitmap.clone());

        Ok(Some(bitmap))
    }

    fn create_d2d_bitmap_from_pixmap(
        &self,
        render_target: &ID2D1RenderTarget,
        pixmap: &tiny_skia::Pixmap,
        size: u32,
    ) -> Result<ID2D1Bitmap, Box<dyn std::error::Error>> {
        unsafe {
            let bitmap_properties = D2D1_BITMAP_PROPERTIES {
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 96.0,
                dpiY: 96.0,
            };

            let size_u = D2D_SIZE_U {
                width: size,
                height: size,
            };

            // 转换 RGBA 到 BGRA
            let mut bgra_data = Vec::with_capacity(pixmap.data().len());
            for chunk in pixmap.data().chunks(4) {
                if chunk.len() == 4 {
                    bgra_data.push(chunk[2]); // B
                    bgra_data.push(chunk[1]); // G
                    bgra_data.push(chunk[0]); // R
                    bgra_data.push(chunk[3]); // A
                }
            }

            let bitmap = render_target.CreateBitmap(
                size_u,
                Some(bgra_data.as_ptr() as *const std::ffi::c_void),
                size * 4, // stride
                &bitmap_properties,
            )?;

            Ok(bitmap)
        }
    }

    pub fn get_rendered_icon(
        &self,
        button: ToolbarButton,
        color: Option<(u8, u8, u8)>,
    ) -> Option<ID2D1Bitmap> {
        let cache_key = (button, color);
        self.rendered_icons.borrow().get(&cache_key).cloned()
    }
}
