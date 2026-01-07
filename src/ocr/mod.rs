//! OCR 模块
//!
//! 提供 OCR 识别功能，包括：
//! - `engine` - OCR 引擎工具函数
//! - `types` - OCR 相关类型定义
//!
//! 引擎实例由 `SystemManager` 统一管理。

pub mod engine;
pub mod types;

// 重导出常用类型和函数
pub use engine::{
    OcrLanguageInfo, create_engine, get_available_languages, get_model_paths, models_exist,
    recognize_from_memory, recognize_text_by_lines,
};
pub use types::{BoundingBox, OcrCompletionData, OcrResult};
