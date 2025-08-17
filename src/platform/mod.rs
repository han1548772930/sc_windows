// 平台抽象层
//
// 定义了平台无关的渲染接口，具体实现由各平台提供

pub mod traits;

#[cfg(target_os = "windows")]
pub mod windows;

// 重新导出主要的trait和类型
pub use traits::*;

#[cfg(target_os = "windows")]
pub use windows::*;
