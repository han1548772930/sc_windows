//! 预览窗口模块
//! 
//! 将原来的 `preview_window.rs` 拆分为：
//! - `types.rs`: 类型定义（D2DIconBitmaps, SvgIcon, MARGINS, IconCache）
//! - `renderer.rs`: 渲染器（PreviewRenderer）
//! - `window.rs`: 窗口主体（PreviewWindow）

mod renderer;
mod types;
mod window;

// 重新导出公共接口
pub use window::PreviewWindow;
