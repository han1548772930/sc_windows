// SVG图标管理器
//
// 负责加载、渲染和缓存SVG图标

use crate::types::ToolbarButton;
use std::cell::RefCell;
use std::collections::HashMap;
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
        // 使用 include_str! 宏将SVG文件嵌入到二进制文件中
        let embedded_icons = [
            (
                ToolbarButton::Arrow,
                include_str!("../../icons/move-up-right.svg"),
            ),
            (
                ToolbarButton::Rectangle,
                include_str!("../../icons/square.svg"),
            ),
            (ToolbarButton::Circle, include_str!("../../icons/circle.svg")),
            (ToolbarButton::Pen, include_str!("../../icons/pen.svg")),
            (ToolbarButton::Text, include_str!("../../icons/type.svg")),
            (ToolbarButton::Undo, include_str!("../../icons/undo-2.svg")),
            (
                ToolbarButton::ExtractText,
                include_str!("../../icons/extracttext.svg"),
            ),
            (
                ToolbarButton::Languages,
                include_str!("../../icons/languages.svg"),
            ),
            (ToolbarButton::Save, include_str!("../../icons/download.svg")),
            (ToolbarButton::Pin, include_str!("../../icons/pin.svg")),
            (ToolbarButton::Confirm, include_str!("../../icons/check.svg")),
            (ToolbarButton::Cancel, include_str!("../../icons/x.svg")),
        ];

        for (button, svg_content) in &embedded_icons {
            let svg_data = svg_content.as_bytes().to_vec();
            self.icons.insert(*button, svg_data);
        }

        Ok(())
    }

    pub fn render_icon_to_bitmap(
        &self,
        button: ToolbarButton,
        render_target: &ID2D1RenderTarget,
        size: u32,
        color: Option<(u8, u8, u8)>,
    ) -> Result<Option<ID2D1Bitmap>, Box<dyn std::error::Error>> {
        let cache_key = (button, color);
        if let Some(bitmap) = self.rendered_icons.borrow().get(&cache_key) {
            return Ok(Some(bitmap.clone()));
        }

        let svg_data = match self.icons.get(&button) {
            Some(data) => data,
            None => return Ok(None),
        };
        let mut svg_str = String::from_utf8_lossy(svg_data).to_string();

        if let Some((r, g, b)) = color {
            let color_hex = format!("#{:02x}{:02x}{:02x}", r, g, b);
            svg_str = svg_str.replace(
                "stroke=\"currentColor\"",
                &format!("stroke=\"{}\"", color_hex),
            );
            svg_str = svg_str.replace("fill=\"currentColor\"", &format!("fill=\"{}\"", color_hex));
        }

        let opt = usvg::Options::default();
        let tree = usvg::Tree::from_str(&svg_str, &opt)?;

        let mut pixmap = tiny_skia::Pixmap::new(size, size).unwrap();

        let transform = tiny_skia::Transform::from_scale(
            size as f32 / tree.size().width(),
            size as f32 / tree.size().height(),
        );

        resvg::render(&tree, transform, &mut pixmap.as_mut());

        let bitmap = self.create_d2d_bitmap_from_pixmap(render_target, &pixmap, size)?;
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
