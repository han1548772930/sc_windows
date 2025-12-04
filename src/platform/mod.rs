//! 平台抽象层
//!
//! 提供跨平台的抽象接口，当前主要支持 Windows 平台。
//!
//! # 模块结构
//! - [`traits`]: 平台无关的 trait 定义
//! - [`events`]: 平台无关的输入事件定义
//! - [`windows`]: Windows 平台实现（D2D 渲染、GDI 捕获等）

pub mod events;
pub mod traits;

#[cfg(target_os = "windows")]
pub mod windows;

pub use events::*;
pub use traits::*;

#[cfg(target_os = "windows")]
pub use windows::*;
