//! OCR 相关类型定义
//!
//! 包含 OCR 识别结果、边界框等数据结构。

use windows::Win32::Foundation::RECT;

/// OCR 结果结构体，包含识别的文本和坐标信息
#[derive(Debug, Clone)]
pub struct OcrResult {
    pub text: String,
    pub confidence: f32,
    pub bounding_box: BoundingBox,
}

/// 边界框结构体，表示文本在图像中的位置
#[derive(Debug, Clone)]
pub struct BoundingBox {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// OCR结果数据传输结构
pub struct OcrCompletionData {
    pub image_data: Vec<u8>,
    pub ocr_results: Vec<OcrResult>,
    pub selection_rect: RECT,
}
