//! OCR 引擎工具函数
//!
//! 提供 OCR 相关的工具函数，不包含引擎状态管理。
//! 引擎实例由 `SystemManager` 统一管理。

use std::path::PathBuf;

use anyhow::Result;
use ocr_rs::OcrEngine;
use windows::Win32::Foundation::*;

use super::types::{BoundingBox, OcrResult};
use crate::settings::Settings;

/// OCR 语言信息
#[derive(Debug, Clone)]
pub struct OcrLanguageInfo {
    /// 语言标识符 (如 "chinese", "english")
    pub id: String,
    /// 显示名称 (如 "简体中文", "English")
    pub display_name: String,
    /// 识别模型文件名
    pub rec_model: String,
    /// 字符集文件名
    pub charset_file: String,
}

/// 获取模型文件路径
pub fn get_model_paths() -> Result<(PathBuf, PathBuf, PathBuf)> {
    let settings = Settings::load();
    let language = &settings.ocr_language;

    let models_dir = PathBuf::from("models");

    // 检测模型 (所有语言共用)
    let det_path = models_dir.join("PP-OCRv5_mobile_det.mnn");

    // 动态查找语言对应的模型
    let available_languages = get_available_languages();
    let lang_info = available_languages
        .iter()
        .find(|l| l.id == *language)
        .or_else(|| available_languages.first()); // 回退到第一个可用语言

    let (rec_model, charset) = match lang_info {
        Some(info) => (info.rec_model.clone(), info.charset_file.clone()),
        None => return Err(anyhow::anyhow!("没有可用的 OCR 语言模型")),
    };

    let rec_path = models_dir.join(&rec_model);
    let charset_path = models_dir.join(&charset);

    // 验证文件存在
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

/// 动态检测可用的 OCR 语言
pub fn get_available_languages() -> Vec<OcrLanguageInfo> {
    let models_dir = PathBuf::from("models");
    let mut languages = Vec::new();

    // 语言配置: (id, display_name, rec_model_pattern, charset_pattern)
    let lang_configs = [
        ("chinese", "简体中文", "PP-OCRv5_mobile_rec.mnn", "ppocr_keys_v5.txt"),
        ("english", "English", "en_PP-OCRv5_mobile_rec_infer.mnn", "ppocr_keys_en.txt"),
        ("korean", "한국어", "korean_PP-OCRv5_mobile_rec_infer.mnn", "ppocr_keys_korean.txt"),
        ("arabic", "العربية", "arabic_PP-OCRv5_mobile_rec_infer.mnn", "ppocr_keys_arabic.txt"),
        ("cyrillic", "Кириллица", "cyrillic_PP-OCRv5_mobile_rec_infer.mnn", "ppocr_keys_cyrillic.txt"),
        ("devanagari", "देवनागरी", "devanagari_PP-OCRv5_mobile_rec_infer.mnn", "ppocr_keys_devanagari.txt"),
        ("latin", "Latin", "latin_PP-OCRv5_mobile_rec_infer.mnn", "ppocr_keys_latin.txt"),
        ("greek", "Ελληνικά", "el_PP-OCRv5_mobile_rec_infer.mnn", "ppocr_keys_el.txt"),
        ("thai", "ไทย", "th_PP-OCRv5_mobile_rec_infer.mnn", "ppocr_keys_th.txt"),
        ("tamil", "தமிழ்", "ta_PP-OCRv5_mobile_rec_infer.mnn", "ppocr_keys_ta.txt"),
        ("telugu", "తెలుగు", "te_PP-OCRv5_mobile_rec_infer.mnn", "ppocr_keys_te.txt"),
    ];

    for (id, display_name, rec_model, charset) in lang_configs {
        let rec_path = models_dir.join(rec_model);
        let charset_path = models_dir.join(charset);

        // 只有当两个文件都存在时才添加该语言
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

/// 创建 OCR 引擎实例
pub fn create_engine() -> Result<OcrEngine> {
    let (det_path, rec_path, charset_path) = get_model_paths()?;
    
    OcrEngine::new(&det_path, &rec_path, &charset_path, None)
        .map_err(|e| anyhow::anyhow!("创建 OCR 引擎失败: {}", e))
}

/// 检查模型文件是否存在
pub fn models_exist() -> bool {
    get_model_paths().is_ok()
}

/// 使用指定引擎从内存中的图像数据识别文本
pub fn recognize_from_memory(engine: &OcrEngine, image_data: &[u8]) -> Result<Vec<OcrResult>> {
    // 解码图像
    let img = image::load_from_memory(image_data)
        .map_err(|e| anyhow::anyhow!("图像解码失败: {}", e))?;

    // 执行 OCR
    let raw_results = engine
        .recognize(&img)
        .map_err(|e| anyhow::anyhow!("OCR 识别失败: {}", e))?;

    // 转换结果格式
    let results: Vec<OcrResult> = raw_results
        .into_iter()
        .filter(|r| !r.text.trim().is_empty())
        .map(|r| OcrResult {
            text: r.text,
            confidence: r.confidence,
            bounding_box: BoundingBox {
                x: r.bbox.rect.left() as i32,
                y: r.bbox.rect.top() as i32,
                width: r.bbox.rect.width() as i32,
                height: r.bbox.rect.height() as i32,
            },
        })
        .collect();

    if results.is_empty() {
        // 没有检测到文本
        Ok(vec![OcrResult {
            text: "未识别到任何文字".to_string(),
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

/// 整体识别文本然后根据坐标换行
pub fn recognize_text_by_lines(
    engine: &OcrEngine,
    image_data: &[u8],
    selection_rect: RECT,
) -> Result<Vec<OcrResult>> {
    let all_results = recognize_from_memory(engine, image_data)?;

    // 调整坐标到原始屏幕坐标系
    let mut adjusted_results = Vec::new();
    for mut result in all_results {
        result.bounding_box.x += selection_rect.left;
        result.bounding_box.y += selection_rect.top;
        adjusted_results.push(result);
    }

    // 按 Y 坐标排序
    adjusted_results.sort_by(|a, b| a.bounding_box.y.cmp(&b.bounding_box.y));

    // 根据 Y 坐标将文本块分组为行
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

    // 处理每一行：按 X 坐标排序并合并文本
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

    // 按 Y 坐标最终排序
    final_results.sort_by(|a, b| a.bounding_box.y.cmp(&b.bounding_box.y));

    Ok(final_results)
}
