//! OCR 模块
//!
//! 提供 OCR 识别功能，包括：
//! - `engine` - PaddleOCR 引擎封装
//! - `manager` - OCR 业务流程处理
//! - `types` - OCR 相关类型定义

pub mod engine;
pub mod manager;
pub mod types;

// 重导出常用类型
pub use engine::{PaddleOcrEngine, recognize_text_by_lines};
pub use manager::recognize_text_from_selection;
pub use types::{BoundingBox, OcrCompletionData, OcrResult};
