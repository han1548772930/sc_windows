//! OCR 模块
//!
//! 提供 OCR 识别功能，包括：
//! - `engine` - PaddleOCR 引擎封装
//! - `manager` - OCR 状态管理和业务逻辑
//! - `types` - OCR 相关类型定义

pub mod engine;
pub mod manager;
pub mod types;

// 重导出常用类型，保持外部接口兼容
pub use engine::{
    PaddleOcrEngine,
    bitmap_to_bmp_data,
    crop_bmp,
    recognize_text_by_lines,
};
pub use manager::OcrManager;
pub use types::{BoundingBox, OcrCompletionData, OcrResult};
