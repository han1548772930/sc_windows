//! rust-paddle-ocr 引擎封装
//!
//! 提供 OCR 引擎的启动、停止、识别等功能。
//! 使用 MNN 推理框架，无需外部进程。

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use anyhow::Result;
use ocr_rs::OcrEngine;
use windows::Win32::Foundation::*;

use super::types::{BoundingBox, OcrResult};
use crate::settings::Settings;

// 全局 OCR 引擎单例
static CURRENT_OCR_ENGINE: OnceLock<Mutex<Option<OcrEngine>>> = OnceLock::new();

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

/// PaddleOCR 引擎，使用全局单例
pub struct PaddleOcrEngine;

impl PaddleOcrEngine {
    /// 创建新的 OCR 引擎实例
    pub fn new() -> Result<Self> {
        Ok(Self)
    }

    /// 确保 OCR 引擎已启动
    pub fn ensure_engine_started() {
        Self::start_ocr_engine_async();
    }

    /// 异步启动 OCR 引擎
    pub fn start_ocr_engine_async() {
        Self::start_ocr_engine_async_with_callback(None);
    }

    /// 异步启动 OCR 引擎，完成后通知指定窗口
    pub fn start_ocr_engine_async_with_hwnd(hwnd: HWND) {
        Self::start_ocr_engine_async_with_callback(Some(hwnd.0 as usize));
    }

    /// 内部实现：异步启动引擎，可选回调
    fn start_ocr_engine_async_with_callback(hwnd_ptr: Option<usize>) {
        std::thread::spawn(move || {
            let success = Self::start_ocr_engine_sync().is_ok();

            if let Some(ptr) = hwnd_ptr {
                unsafe {
                    use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_USER};
                    let hwnd = HWND(ptr as *mut std::ffi::c_void);
                    // WM_USER + 10 = WM_OCR_STATUS_UPDATE
                    let _ = PostMessageW(
                        Some(hwnd),
                        WM_USER + 10,
                        WPARAM(if success { 1 } else { 0 }),
                        LPARAM(0),
                    );
                }
            }
        });
    }

    /// 同步启动 OCR 引擎
    fn start_ocr_engine_sync() -> Result<()> {
        let engine_mutex = CURRENT_OCR_ENGINE.get_or_init(|| Mutex::new(None));
        let mut engine_guard = engine_mutex
            .lock()
            .map_err(|_| anyhow::anyhow!("OCR 引擎互斥锁已中毒"))?;

        if engine_guard.is_none() {
            #[cfg(debug_assertions)]
            println!("正在启动 OCR 引擎...");

            #[cfg(debug_assertions)]
            let start_time = std::time::Instant::now();

            // 获取模型路径
            let (det_path, rec_path, charset_path) = Self::get_model_paths()?;

            // 创建 OCR 引擎
            let engine = OcrEngine::new(&det_path, &rec_path, &charset_path, None)
                .map_err(|e| anyhow::anyhow!("创建 OCR 引擎失败: {}", e))?;

            *engine_guard = Some(engine);

            #[cfg(debug_assertions)]
            println!("OCR 引擎启动成功，耗时: {:?}", start_time.elapsed());
        }

        Ok(())
    }

    /// 获取模型文件路径
    fn get_model_paths() -> Result<(PathBuf, PathBuf, PathBuf)> {
        let settings = Settings::load();
        let language = &settings.ocr_language;

        let models_dir = PathBuf::from("models");

        // 检测模型 (所有语言共用)
        let det_path = models_dir.join("PP-OCRv5_mobile_det.mnn");

        // 动态查找语言对应的模型
        let available_languages = Self::get_available_languages();
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

    /// 停止 OCR 引擎
    pub fn stop_engine(immediate: bool) {
        if immediate {
            Self::stop_ocr_engine_sync();
        } else {
            Self::stop_ocr_engine_async();
        }
    }

    /// 异步停止 OCR 引擎
    pub fn stop_ocr_engine_async() {
        std::thread::spawn(|| {
            Self::stop_ocr_engine_sync();
        });
    }

    /// 同步停止 OCR 引擎
    fn stop_ocr_engine_sync() {
        if let Some(engine_mutex) = CURRENT_OCR_ENGINE.get()
            && let Ok(mut engine_guard) = engine_mutex.lock()
            && let Some(engine) = engine_guard.take()
        {
            #[cfg(debug_assertions)]
            println!("正在停止 OCR 引擎...");

            drop(engine);

            #[cfg(debug_assertions)]
            println!("OCR 引擎已停止");
        }
        // 注意: 纯 Rust 实现，不再需要 force_kill_paddle_processes()
    }

    /// 立即停止 OCR 引擎
    pub fn stop_ocr_engine_immediate() {
        Self::stop_ocr_engine_sync();
    }

    /// 检查 OCR 引擎是否已经准备就绪（非阻塞）
    pub fn is_engine_ready() -> bool {
        if let Some(engine_mutex) = CURRENT_OCR_ENGINE.get()
            && let Ok(engine_guard) = engine_mutex.try_lock()
        {
            return engine_guard.is_some();
        }
        false
    }

    /// 检查 OCR 引擎是否已经准备就绪（阻塞版本）
    fn is_engine_ready_blocking() -> bool {
        if let Some(engine_mutex) = CURRENT_OCR_ENGINE.get()
            && let Ok(engine_guard) = engine_mutex.lock()
        {
            return engine_guard.is_some();
        }
        false
    }

    /// 检查 OCR 引擎是否可用（包括模型文件是否存在）
    pub fn is_engine_available() -> bool {
        if Self::get_model_paths().is_err() {
            return false;
        }
        Self::is_engine_ready()
    }

    /// 获取 OCR 引擎状态描述
    pub fn get_engine_status() -> String {
        if Self::get_model_paths().is_err() {
            return "OCR 模型文件未找到".to_string();
        }

        if Self::is_engine_ready() {
            "OCR 引擎已就绪".to_string()
        } else if CURRENT_OCR_ENGINE.get().is_some() {
            "OCR 引擎正在启动中...".to_string()
        } else {
            "OCR 引擎未启动".to_string()
        }
    }

    /// 异步检查 OCR 引擎状态并返回详细信息
    pub fn check_engine_status_async<F>(callback: F)
    where
        F: Fn(bool, bool, String) + Send + 'static,
    {
        std::thread::spawn(move || {
            let models_exist = Self::get_model_paths().is_ok();
            let engine_ready = Self::is_engine_ready_blocking();
            let status = Self::get_engine_status();
            callback(models_exist, engine_ready, status);
        });
    }

    /// 清理 OCR 引擎
    pub fn cleanup_global_engine() {
        Self::stop_ocr_engine_immediate();
    }

    /// 从内存中的图像数据识别文本
    pub fn recognize_from_memory(&mut self, image_data: &[u8]) -> Result<Vec<OcrResult>> {
        // 解码图像
        let img = image::load_from_memory(image_data)
            .map_err(|e| anyhow::anyhow!("图像解码失败: {}", e))?;

        // 获取引擎并执行识别
        let engine_mutex = CURRENT_OCR_ENGINE
            .get()
            .ok_or_else(|| anyhow::anyhow!("OCR 引擎未初始化"))?;

        let engine_guard = engine_mutex
            .lock()
            .map_err(|_| anyhow::anyhow!("OCR 引擎互斥锁已中毒"))?;

        let engine = engine_guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("OCR 引擎未就绪"))?;

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
}

/// 整体识别文本然后根据坐标换行
pub fn recognize_text_by_lines(image_data: &[u8], selection_rect: RECT) -> Result<Vec<OcrResult>> {
    let mut ocr_engine = PaddleOcrEngine::new()?;
    let all_results = ocr_engine.recognize_from_memory(image_data)?;

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
