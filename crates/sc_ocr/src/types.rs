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

/// OCR outcome summary derived from a list of results.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OcrOutcome {
    /// No OCR results.
    None,
    /// OCR succeeded with text content.
    Success { text: String },
    /// OCR ran but found no text (placeholder).
    Empty { text: String },
    /// OCR failed (placeholder).
    Failed { text: String },
}

impl OcrOutcome {
    pub fn has_results(&self) -> bool {
        !matches!(self, OcrOutcome::None)
    }

    pub fn is_failed(&self) -> bool {
        matches!(self, OcrOutcome::Failed { .. })
    }

    pub fn text(&self) -> &str {
        match self {
            OcrOutcome::None => "",
            OcrOutcome::Success { text }
            | OcrOutcome::Empty { text }
            | OcrOutcome::Failed { text } => text,
        }
    }

    pub fn into_summary(self) -> (bool, bool, String) {
        let has_results = self.has_results();
        let is_failed = self.is_failed();
        let text = self.text().to_string();
        (has_results, is_failed, text)
    }
}
