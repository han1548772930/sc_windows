use sc_app::selection::RectI32;

/// OCR result (text + coordinates).
#[derive(Debug, Clone)]
pub struct OcrResult {
    pub text: String,
    pub confidence: f32,
    pub bounding_box: BoundingBox,
}

/// Bounding box for a text block.
#[derive(Debug, Clone)]
pub struct BoundingBox {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// OCR completion payload (for host preview windows).
pub struct OcrCompletionData {
    pub image_data: Vec<u8>,
    pub ocr_results: Vec<OcrResult>,
    pub selection_rect: RectI32,
}
