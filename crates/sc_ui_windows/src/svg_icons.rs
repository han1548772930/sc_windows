use super::ToolbarButton;
use crate::svg::{PixelFormat, SvgRenderOptions, render_svg_to_pixels};
use std::cell::RefCell;
use std::collections::HashMap;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;

#[derive(Debug)]
pub struct SvgIconManager {
    icons: HashMap<ToolbarButton, Vec<u8>>,
    rendered_icons: RefCell<HashMap<(ToolbarButton, Option<(u8, u8, u8)>), ID2D1Bitmap>>,
}

impl Default for SvgIconManager {
    fn default() -> Self {
        Self::new()
    }
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
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../apps/sc_windows/icons/move-up-right.svg"
                )),
            ),
            (
                ToolbarButton::Rectangle,
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../apps/sc_windows/icons/square.svg"
                )),
            ),
            (
                ToolbarButton::Circle,
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../apps/sc_windows/icons/circle.svg"
                )),
            ),
            (
                ToolbarButton::Pen,
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../apps/sc_windows/icons/pen.svg"
                )),
            ),
            (
                ToolbarButton::Text,
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../apps/sc_windows/icons/type.svg"
                )),
            ),
            (
                ToolbarButton::Undo,
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../apps/sc_windows/icons/undo-2.svg"
                )),
            ),
            (
                ToolbarButton::ExtractText,
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../apps/sc_windows/icons/extracttext.svg"
                )),
            ),
            (
                ToolbarButton::Languages,
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../apps/sc_windows/icons/languages.svg"
                )),
            ),
            (
                ToolbarButton::Save,
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../apps/sc_windows/icons/download.svg"
                )),
            ),
            (
                ToolbarButton::Pin,
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../apps/sc_windows/icons/pin.svg"
                )),
            ),
            (
                ToolbarButton::Confirm,
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../apps/sc_windows/icons/check.svg"
                )),
            ),
            (
                ToolbarButton::Cancel,
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../apps/sc_windows/icons/x.svg"
                )),
            ),
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
        let svg_str = String::from_utf8_lossy(svg_data);

        // 使用统一的 SVG 渲染工具
        let options = SvgRenderOptions {
            size,
            scale: 1.0,
            color_override: color,
            output_format: PixelFormat::Bgra,
        };
        let (bgra_data, render_size, _) = render_svg_to_pixels(&svg_str, &options)?;

        let bitmap = Self::create_d2d_bitmap(render_target, &bgra_data, render_size)?;
        self.rendered_icons
            .borrow_mut()
            .insert(cache_key, bitmap.clone());

        Ok(Some(bitmap))
    }

    fn create_d2d_bitmap(
        render_target: &ID2D1RenderTarget,
        bgra_data: &[u8],
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
