pub mod dirty_rect;
pub mod layer_cache;
pub mod render_list;
pub mod types;

// 重新导出常用类型
pub use dirty_rect::{DirtyRectTracker, DirtyType};
pub use layer_cache::{CacheLayer, CacheState, LayerCache};
pub use render_list::{RenderBackend, RenderItem, RenderList, RenderListBuilder, z_order};
pub use types::{BitmapId, Color, DrawStyle, Point, Rectangle, TextStyle};
