use std::path::{Path, PathBuf};

use anyhow::Result;
use ocr_rs::OcrEngine;
use sc_app::selection::RectI32;

use crate::types::{BoundingBox, OcrResult};

/// OCR language information.
#[derive(Debug, Clone)]
pub struct OcrLanguageInfo {
    /// Language identifier (e.g. "chinese", "english").
    pub id: String,
    /// Display name.
    pub display_name: String,
    /// Recognition model filename.
    pub rec_model: String,
    /// Charset filename.
    pub charset_file: String,
}

/// Host-provided OCR configuration.
#[derive(Debug, Clone)]
pub struct OcrConfig {
    /// Directory containing the model files.
    pub models_dir: PathBuf,
    /// Language identifier.
    pub language: String,
}

impl OcrConfig {
    pub fn new(models_dir: impl Into<PathBuf>, language: impl Into<String>) -> Self {
        Self {
            models_dir: models_dir.into(),
            language: language.into(),
        }
    }
}

/// Get model paths for the given config.
pub fn get_model_paths(config: &OcrConfig) -> Result<(PathBuf, PathBuf, PathBuf)> {
    let language = config.language.as_str();

    // Detection model (shared by all languages).
    let det_path = config.models_dir.join("PP-OCRv5_mobile_det.mnn");

    // Dynamic language model selection.
    let available_languages = get_available_languages(&config.models_dir);
    let lang_info = available_languages
        .iter()
        .find(|l| l.id == language)
        .or_else(|| available_languages.first());

    let (rec_model, charset) = match lang_info {
        Some(info) => (info.rec_model.clone(), info.charset_file.clone()),
        None => return Err(anyhow::anyhow!("没有可用的 OCR 语言模型")),
    };

    let rec_path = config.models_dir.join(&rec_model);
    let charset_path = config.models_dir.join(&charset);

    // Validate existence.
    if !det_path.exists() {
        return Err(anyhow::anyhow!("检测模型不存在: {}", det_path.display()));
    }
    if !rec_path.exists() {
        return Err(anyhow::anyhow!("识别模型不存在: {}", rec_path.display()));
    }
    if !charset_path.exists() {
        return Err(anyhow::anyhow!(
            "字符集文件不存在: {}",
            charset_path.display()
        ));
    }

    Ok((det_path, rec_path, charset_path))
}

/// Detect available OCR languages by inspecting the models directory.
pub fn get_available_languages(models_dir: &Path) -> Vec<OcrLanguageInfo> {
    let mut languages = Vec::new();

    // Language config: (id, display_name, rec_model, charset)
    let lang_configs = [
        (
            "chinese",
            "简体中文",
            "PP-OCRv5_mobile_rec.mnn",
            "ppocr_keys_v5.txt",
        ),
        (
            "english",
            "English",
            "en_PP-OCRv5_mobile_rec_infer.mnn",
            "ppocr_keys_en.txt",
        ),
        (
            "korean",
            "한국어",
            "korean_PP-OCRv5_mobile_rec_infer.mnn",
            "ppocr_keys_korean.txt",
        ),
        (
            "arabic",
            "العربية",
            "arabic_PP-OCRv5_mobile_rec_infer.mnn",
            "ppocr_keys_arabic.txt",
        ),
        (
            "cyrillic",
            "Кириллица",
            "cyrillic_PP-OCRv5_mobile_rec_infer.mnn",
            "ppocr_keys_cyrillic.txt",
        ),
        (
            "devanagari",
            "देवनागरी",
            "devanagari_PP-OCRv5_mobile_rec_infer.mnn",
            "ppocr_keys_devanagari.txt",
        ),
        (
            "latin",
            "Latin",
            "latin_PP-OCRv5_mobile_rec_infer.mnn",
            "ppocr_keys_latin.txt",
        ),
        (
            "greek",
            "Ελληνικά",
            "el_PP-OCRv5_mobile_rec_infer.mnn",
            "ppocr_keys_el.txt",
        ),
        (
            "thai",
            "ไทย",
            "th_PP-OCRv5_mobile_rec_infer.mnn",
            "ppocr_keys_th.txt",
        ),
        (
            "tamil",
            "தமிழ்",
            "ta_PP-OCRv5_mobile_rec_infer.mnn",
            "ppocr_keys_ta.txt",
        ),
        (
            "telugu",
            "తెలుగు",
            "te_PP-OCRv5_mobile_rec_infer.mnn",
            "ppocr_keys_te.txt",
        ),
    ];

    for (id, display_name, rec_model, charset) in lang_configs {
        let rec_path = models_dir.join(rec_model);
        let charset_path = models_dir.join(charset);

        // Only include languages with both files present.
        if rec_path.exists() && charset_path.exists() {
            languages.push(OcrLanguageInfo {
                id: id.to_string(),
                display_name: display_name.to_string(),
                rec_model: rec_model.to_string(),
                charset_file: charset.to_string(),
            });
        }
    }

    languages
}

/// Create an OCR engine instance.
pub fn create_engine(config: &OcrConfig) -> Result<OcrEngine> {
    let (det_path, rec_path, charset_path) = get_model_paths(config)?;

    OcrEngine::new(&det_path, &rec_path, &charset_path, None)
        .map_err(|e| anyhow::anyhow!("创建 OCR 引擎失败: {}", e))
}

/// Check whether model files exist for the given config.
pub fn models_exist(config: &OcrConfig) -> bool {
    get_model_paths(config).is_ok()
}

/// Recognize text from image bytes in memory.
pub fn recognize_from_memory(engine: &OcrEngine, image_data: &[u8]) -> Result<Vec<OcrResult>> {
    // Decode image.
    let img =
        image::load_from_memory(image_data).map_err(|e| anyhow::anyhow!("图像解码失败: {}", e))?;

    // OCR.
    let raw_results = engine
        .recognize(&img)
        .map_err(|e| anyhow::anyhow!("OCR 识别失败: {}", e))?;

    // Convert.
    let results: Vec<OcrResult> = raw_results
        .into_iter()
        .filter(|r| !r.text.trim().is_empty())
        .map(|r| OcrResult {
            text: r.text,
            confidence: r.confidence,
            bounding_box: BoundingBox {
                x: r.bbox.rect.left(),
                y: r.bbox.rect.top(),
                width: r.bbox.rect.width() as i32,
                height: r.bbox.rect.height() as i32,
            },
        })
        .collect();

    if results.is_empty() {
        Ok(vec![OcrResult {
            text: crate::OCR_NO_TEXT_PLACEHOLDER.to_string(),
            confidence: 0.0,
            bounding_box: BoundingBox {
                x: 0,
                y: 0,
                width: 200,
                height: 25,
            },
        }])
    } else {
        Ok(results)
    }
}

/// Recognize text and group by lines based on coordinates.
pub fn recognize_text_by_lines(
    engine: &OcrEngine,
    image_data: &[u8],
    selection_rect: RectI32,
) -> Result<Vec<OcrResult>> {
    let all_results = recognize_from_memory(engine, image_data)?;

    // Adjust coordinates into original screen coordinate system.
    let mut adjusted_results = Vec::new();
    for mut result in all_results {
        result.bounding_box.x += selection_rect.left;
        result.bounding_box.y += selection_rect.top;
        adjusted_results.push(result);
    }

    // Sort by Y.
    adjusted_results.sort_by(|a, b| a.bounding_box.y.cmp(&b.bounding_box.y));

    // Group by lines.
    let mut text_lines: Vec<Vec<OcrResult>> = Vec::new();
    let line_height_threshold = 20;

    for result in adjusted_results {
        let mut added_to_existing_line = false;

        for line in &mut text_lines {
            if let Some(first_in_line) = line.first() {
                let y_diff = (result.bounding_box.y - first_in_line.bounding_box.y).abs();
                if y_diff <= line_height_threshold {
                    line.push(result.clone());
                    added_to_existing_line = true;
                    break;
                }
            }
        }

        if !added_to_existing_line {
            text_lines.push(vec![result]);
        }
    }

    // For each line: sort by X and merge text.
    let mut final_results = Vec::new();

    for mut line_blocks in text_lines.into_iter() {
        line_blocks.sort_by(|a, b| a.bounding_box.x.cmp(&b.bounding_box.x));

        let mut line_text = String::new();
        let mut min_x = i32::MAX;
        let mut min_y = i32::MAX;
        let mut max_x = i32::MIN;
        let mut max_y = i32::MIN;
        let mut total_confidence = 0.0;

        for (i, text_block) in line_blocks.iter().enumerate() {
            if i > 0 {
                line_text.push(' ');
            }
            line_text.push_str(&text_block.text);

            min_x = min_x.min(text_block.bounding_box.x);
            min_y = min_y.min(text_block.bounding_box.y);
            max_x = max_x.max(text_block.bounding_box.x + text_block.bounding_box.width);
            max_y = max_y.max(text_block.bounding_box.y + text_block.bounding_box.height);

            total_confidence += text_block.confidence;
        }

        if !line_text.trim().is_empty() {
            let line_result = OcrResult {
                text: line_text.trim().to_string(),
                confidence: total_confidence / line_blocks.len() as f32,
                bounding_box: BoundingBox {
                    x: min_x,
                    y: min_y,
                    width: max_x - min_x,
                    height: max_y - min_y,
                },
            };

            final_results.push(line_result);
        }
    }

    // Final sort by Y.
    final_results.sort_by(|a, b| a.bounding_box.y.cmp(&b.bounding_box.y));

    Ok(final_results)
}

/// Summarize OCR results into (has_results, is_failed, text).
///
/// This preserves existing host semantics:
/// - `has_results` when the vector is non-empty
/// - `is_failed` when the only result is the failure placeholder
/// - `text` is a newline-joined concatenation of all result texts
pub fn summarize_results(ocr_results: &[OcrResult]) -> (bool, bool, String) {
    summarize_outcome(ocr_results).into_summary()
}

/// Summarize OCR results into a structured outcome.
pub fn summarize_outcome(ocr_results: &[OcrResult]) -> crate::OcrOutcome {
    if ocr_results.is_empty() {
        return crate::OcrOutcome::None;
    }

    if ocr_results.len() == 1 {
        let text = ocr_results[0].text.clone();
        if text == crate::OCR_FAILED_PLACEHOLDER {
            return crate::OcrOutcome::Failed { text };
        }
        if text == crate::OCR_NO_TEXT_PLACEHOLDER {
            return crate::OcrOutcome::Empty { text };
        }
    }

    let text = ocr_results
        .iter()
        .map(|r| r.text.clone())
        .collect::<Vec<_>>()
        .join("\n");
    crate::OcrOutcome::Success { text }
}
/// Join OCR result texts with newline, trimming trailing whitespace per line.
pub fn join_result_texts_trimmed(ocr_results: &[OcrResult]) -> String {
    ocr_results
        .iter()
        .map(|r| r.text.trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}
