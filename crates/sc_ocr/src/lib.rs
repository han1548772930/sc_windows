pub mod engine;
pub mod types;

// Re-export the engine type so downstream crates don't need to depend on `ocr-rs` directly.
pub use ocr_rs::OcrEngine;
/// Default directory name for OCR models (relative to app working dir).
pub const DEFAULT_MODELS_DIR: &str = "models";
/// Placeholder text inserted when OCR finds no text.
pub const OCR_NO_TEXT_PLACEHOLDER: &str = "未识别到任何文字";
/// Placeholder text inserted when OCR fails.
pub const OCR_FAILED_PLACEHOLDER: &str = "OCR识别失败";

pub use engine::*;
pub use types::*;
