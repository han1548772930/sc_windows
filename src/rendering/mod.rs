//! 渲染模块
//!
//! 提供渲染相关的优化功能，包括：
//! - 渲染列表：统一收集和排序渲染图元
//! - 脏矩形追踪：只重绘变化区域
//! - 图层缓存：缓存静态内容到离屏位图

pub mod dirty_rect;
pub mod layer_cache;
pub mod render_list;

// Re-export commonly used types
pub use dirty_rect::{DirtyRectTracker, DirtyType};
pub use layer_cache::{LayerCache, LayerType, LayerState, CachedLayer};
pub use render_list::{RenderItem, RenderList, RenderListBuilder, z_order};
