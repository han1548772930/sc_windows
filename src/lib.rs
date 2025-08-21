// 新架构的模块声明
pub mod app;
pub mod constants;
pub mod drawing;
pub mod file_dialog;
pub mod message;
pub mod ocr; // 使用原始文件
pub mod ocr_result_window; // 使用原始文件
pub mod platform;

pub mod interaction;
pub mod screenshot;
pub mod settings;
pub mod system;
pub mod types;
pub mod ui;
pub mod utils;
// pub mod winrt_settings_window; // WinRT 设置窗口

// 重新导出主要类型
pub use app::App;
pub use message::{Command, Message};
pub use types::*;

// 常量定义统一到 constants.rs
pub use crate::constants::WINDOW_CLASS_NAME;
