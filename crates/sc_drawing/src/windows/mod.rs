pub mod cache;
pub mod context;
pub mod drawing_renderer;
pub mod elements;
pub mod ext;
pub mod renderable;
pub mod text;

pub use cache::{CacheStats, ElementId, GeometryCache};
pub use context::{BorderStyle, RenderContext, RenderOptions};
pub use drawing_renderer::{DrawingRenderer, TextCursorState};
pub use elements::{ArrowRenderer, CircleRenderer, PenRenderer, RectangleRenderer, TextRenderer};
pub use ext::{PointExt, RectExt};
pub use renderable::{RenderError, RenderResult, Renderable, RendererRegistry};
pub use text::{text_padding_for_font_size, update_text_element_size_dwrite};

// 重新导出 Windows 类型，方便主项目使用
pub use windows::Win32::Foundation::{POINT, RECT};
pub use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;
