pub mod engine;
pub mod types;

// Re-export the engine type so downstream crates don't need to depend on `ocr-rs` directly.
pub use ocr_rs::OcrEngine;

pub use engine::*;
pub use types::*;
