// 新架构的模块声明
pub mod app;
pub mod constants;
pub mod drawing;
pub mod file_dialog;
pub mod message;
pub mod ocr; // 使用原始文件
pub mod ocr_result_window; // 使用原始文件
pub mod platform;

pub mod screenshot;
pub mod simple_settings; // 从原始代码复制
pub mod system;
pub mod types;
pub mod ui;
pub mod utils;

// 重新导出主要类型
pub use app::App;
pub use message::{Command, Message};
pub use types::*;

// 常量定义
pub const WINDOW_CLASS_NAME: &str = "SC_WINDOWS_NEW";
