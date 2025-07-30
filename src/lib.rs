// 模块声明
pub mod constants;
pub mod d2d;
pub mod drawing;
pub mod file_dialog;
pub mod input;
pub mod ocr; // OCR 文本识别模块
pub mod ocr_result_window; // OCR 结果显示窗口

pub mod simple_settings; // 原生 Windows API 设置窗口
pub mod svg_icons;
pub mod system_tray;
pub mod toolbar;
pub mod translation; // 翻译模块
pub mod types;
pub mod utils;
pub mod window_detection; // 窗口检测模块

// 重新导出常用类型
pub use constants::*;
pub use types::*;
