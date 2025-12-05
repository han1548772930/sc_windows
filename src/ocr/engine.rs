//! PaddleOCR 引擎封装
//!
//! 提供 OCR 引擎的启动、停止、识别等功能。

use std::path::PathBuf;
use std::process::Command;
use std::sync::{Mutex, OnceLock};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};
use paddleocr::{ImageData, Ppocr};
use serde_json::Value;
use windows::Win32::Foundation::*;

use super::types::{BoundingBox, OcrResult};
use crate::settings::Settings;

// 当前活跃的OCR引擎（按需启动和关闭）
static CURRENT_OCR_ENGINE: OnceLock<Mutex<Option<Ppocr>>> = OnceLock::new();

/// PaddleOCR 引擎，使用全局单例避免重复启动进程
pub struct PaddleOcrEngine;

impl PaddleOcrEngine {
    /// 创建新的 OCR 引擎实例（按需启动）
    pub fn new() -> Result<Self> {
        Ok(Self)
    }

    /// 确保OCR引擎已启动（推荐使用的统一接口）
    /// 如果引擎未启动则异步启动，如果已启动则直接返回
    pub fn ensure_engine_started() {
        Self::start_ocr_engine_async();
    }

    /// 异步启动OCR引擎（截图开始时调用，不阻塞）
    /// 注意：推荐使用 ensure_engine_started() 方法
    pub fn start_ocr_engine_async() {
        Self::start_ocr_engine_async_with_callback(None);
    }

    /// 异步启动OCR引擎，完成后通知指定窗口
    pub fn start_ocr_engine_async_with_hwnd(hwnd: HWND) {
        Self::start_ocr_engine_async_with_callback(Some(hwnd.0 as usize));
    }

    /// 内部实现：异步启动引擎，可选回调
    fn start_ocr_engine_async_with_callback(hwnd_ptr: Option<usize>) {
        std::thread::spawn(move || {
            let success = Self::start_ocr_engine_sync().is_ok();

            // 启动完成后发送状态更新消息
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

    /// 同步启动OCR引擎（内部使用）
    fn start_ocr_engine_sync() -> Result<()> {
        let engine_mutex = CURRENT_OCR_ENGINE.get_or_init(|| Mutex::new(None));
        let mut engine_guard = engine_mutex
            .lock()
            .map_err(|_| anyhow::anyhow!("OCR 引擎互斥锁已中毒"))?;

        if engine_guard.is_none() {
            #[cfg(debug_assertions)]
            println!("正在后台启动OCR引擎...");

            #[cfg(debug_assertions)]
            let start_time = std::time::Instant::now();

            // 获取 PaddleOCR-json.exe 的路径
            let exe_path = Self::get_paddle_ocr_exe_path()?;

            // 获取语言配置路径（可以根据需要修改）
            let config_path = Self::get_language_config_path();

            // 创建PaddleOCR引擎（已修改源代码支持隐藏窗口）
            let engine = Ppocr::new(exe_path, config_path)
                .map_err(|e| anyhow::anyhow!("创建 PaddleOCR 引擎失败: {}", e))?;

            *engine_guard = Some(engine);

            #[cfg(debug_assertions)]
            println!("OCR引擎启动成功，耗时: {:?}", start_time.elapsed());
        }

        Ok(())
    }

    /// 停止OCR引擎（统一的停止接口）
    /// 根据 immediate 参数决定是异步还是同步停止
    pub fn stop_engine(immediate: bool) {
        if immediate {
            Self::stop_ocr_engine_sync();
        } else {
            Self::stop_ocr_engine_async();
        }
    }

    /// 异步停止OCR引擎（截图结束时调用，不阻塞）
    /// 注意：推荐使用 stop_engine(false) 方法
    pub fn stop_ocr_engine_async() {
        // 在后台线程中异步停止OCR引擎
        std::thread::spawn(|| {
            Self::stop_ocr_engine_sync();
        });
    }

    /// 同步停止OCR引擎（内部使用）
    fn stop_ocr_engine_sync() {
        if let Some(engine_mutex) = CURRENT_OCR_ENGINE.get()
            && let Ok(mut engine_guard) = engine_mutex.lock()
            && let Some(engine) = engine_guard.take()
        {
            #[cfg(debug_assertions)]
            let start_time = std::time::Instant::now();
            #[cfg(debug_assertions)]
            println!("正在后台停止OCR引擎...");

            // 正常关闭引擎
            drop(engine);

            // 等待进程退出
            std::thread::sleep(std::time::Duration::from_millis(300));

            // 强制清理残留进程
            Self::force_kill_paddle_processes();

            #[cfg(debug_assertions)]
            println!("OCR引擎已停止，耗时: {:?}", start_time.elapsed());
        }
    }

    /// 立即停止OCR引擎（程序退出时使用，同步）
    /// 注意：推荐使用 stop_engine(true) 方法
    pub fn stop_ocr_engine_immediate() {
        Self::stop_ocr_engine_sync();
    }

    /// 使用当前OCR引擎进行识别（不等待，立即检查状态）
    fn call_global_ocr(image_data: ImageData) -> Result<String> {
        // 立即检查OCR引擎是否就绪
        if let Some(engine_mutex) = CURRENT_OCR_ENGINE.get()
            && let Ok(mut engine_guard) = engine_mutex.lock()
            && let Some(engine) = engine_guard.as_mut()
        {
            // 引擎已就绪，执行OCR
            #[cfg(debug_assertions)]
            println!("OCR引擎就绪，开始识别...");

            return engine
                .ocr(image_data)
                .map_err(|e| anyhow::anyhow!("PaddleOCR 识别失败: {}", e));
        }

        // 引擎未就绪，直接返回错误
        Err(anyhow::anyhow!("OCR引擎未就绪，请等待引擎启动完成"))
    }

    /// 检查OCR引擎是否已经准备就绪（非阻塞，UI线程使用）
    ///
    /// 使用 `try_lock` 避免阻塞主线程。如果锁被占用（引擎正在启动/停止），
    /// 返回 `false` 而不是等待。
    pub fn is_engine_ready() -> bool {
        if let Some(engine_mutex) = CURRENT_OCR_ENGINE.get()
            && let Ok(engine_guard) = engine_mutex.try_lock()
        {
            return engine_guard.is_some();
        }
        false
    }

    /// 检查OCR引擎是否已经准备就绪（阻塞版本，后台线程使用）
    fn is_engine_ready_blocking() -> bool {
        if let Some(engine_mutex) = CURRENT_OCR_ENGINE.get()
            && let Ok(engine_guard) = engine_mutex.lock()
        {
            return engine_guard.is_some();
        }
        false
    }

    /// 检查OCR引擎是否可用（包括检查可执行文件是否存在）
    pub fn is_engine_available() -> bool {
        // 首先检查PaddleOCR可执行文件是否存在
        if Self::find_paddle_exe().is_err() {
            return false;
        }

        // 然后检查引擎是否已启动并就绪
        Self::is_engine_ready()
    }

    /// 获取OCR引擎状态描述
    pub fn get_engine_status() -> String {
        // 首先检查可执行文件是否存在
        if Self::find_paddle_exe().is_err() {
            return "PaddleOCR可执行文件未找到".to_string();
        }

        if Self::is_engine_ready() {
            "OCR引擎已就绪".to_string()
        } else if CURRENT_OCR_ENGINE.get().is_some() {
            "OCR引擎正在启动中...".to_string()
        } else {
            "OCR引擎未启动".to_string()
        }
    }

    /// 异步检查OCR引擎是否可用（非阻塞）
    pub fn check_engine_available_async<F>(callback: F)
    where
        F: Fn(bool) + Send + 'static,
    {
        std::thread::spawn(move || {
            // 在后台线程中检查引擎状态
            let available = Self::is_engine_available();
            callback(available);
        });
    }

    /// 异步检查OCR引擎状态并返回详细信息（后台线程执行）
    ///
    /// 注意：此方法仅检查状态，不会启动引擎。
    /// 引擎启动应通过 `ensure_engine_started()` 单独调用。
    pub fn check_engine_status_async<F>(callback: F)
    where
        F: Fn(bool, bool, String) + Send + 'static,
    {
        std::thread::spawn(move || {
            // 在后台线程中检查引擎状态（使用阻塞版本，等待锁释放）
            let exe_exists = Self::find_paddle_exe().is_ok();
            let engine_ready = Self::is_engine_ready_blocking();
            let status = Self::get_engine_status();
            callback(exe_exists, engine_ready, status);
        });
    }

    /// 清理OCR引擎（程序退出时调用）
    pub fn cleanup_global_engine() {
        // 程序退出时使用立即停止方法
        Self::stop_ocr_engine_immediate();
    }

    /// 强制终止所有PaddleOCR进程
    fn force_kill_paddle_processes() {
        #[cfg(target_os = "windows")]
        {
            // 使用taskkill命令强制终止PaddleOCR进程
            let _result = Command::new("taskkill")
                .args(["/F", "/IM", "PaddleOCR-json.exe"])
                .creation_flags(0x08000000) // CREATE_NO_WINDOW
                .output();

            #[cfg(debug_assertions)]
            match _result {
                Ok(output) => {
                    if output.status.success() {
                        println!("已强制终止所有PaddleOCR进程");
                    } else {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        if !stderr.contains("找不到") && !stderr.contains("not found") {
                            println!("终止PaddleOCR进程时出现警告: {stderr}");
                        }
                    }
                }
                Err(e) => {
                    println!("执行taskkill命令失败: {e}");
                }
            }
        }
    }

    /// 获取PaddleOCR-json.exe的路径
    fn get_paddle_ocr_exe_path() -> Result<PathBuf> {
        Self::find_paddle_exe()
    }

    /// 获取 PaddleOCR-json.exe 的固定路径
    fn find_paddle_exe() -> Result<PathBuf> {
        // 写死路径：当前目录下的 PaddleOCR-json_v1.4.1 文件夹
        let exe_path = PathBuf::from("PaddleOCR-json_v1.4.1").join("PaddleOCR-json.exe");

        if exe_path.exists() {
            Ok(exe_path)
        } else {
            Err(anyhow::anyhow!(
                "找不到 PaddleOCR-json.exe 文件。\n请确保 PaddleOCR-json_v1.4.1 文件夹与程序在同一目录中。\n期望路径: {}",
                exe_path.display()
            ))
        }
    }

    /// 获取语言配置文件路径
    /// 根据用户设置选择对应的OCR语言配置
    fn get_language_config_path() -> Option<PathBuf> {
        // 从设置中读取用户选择的语言
        let settings = Settings::load();
        let language = &settings.ocr_language;

        match language.as_str() {
            "english" => Some(PathBuf::from("models\\config_en.txt")),
            "chinese_cht" => Some(PathBuf::from("models\\config_chinese_cht.txt")),
            "japan" => Some(PathBuf::from("models\\config_japan.txt")),
            "korean" => Some(PathBuf::from("models\\config_korean.txt")),
            _ => None, // "chinese" or any unknown language uses default config
        }
    }

    /// 从内存中的图像数据识别文本
    pub fn recognize_from_memory(&mut self, image_data: &[u8]) -> Result<Vec<OcrResult>> {
        // 使用Base64方式（纯内存操作，无需磁盘IO）
        self.recognize_from_base64(image_data)
    }

    /// 使用Base64方式进行OCR识别（真正的内存识别）
    fn recognize_from_base64(&mut self, image_data: &[u8]) -> Result<Vec<OcrResult>> {
        // 将图像数据编码为Base64
        let base64_string = general_purpose::STANDARD.encode(image_data);

        // 创建ImageData对象
        let image_data_obj = ImageData::ImageBase64Dict {
            image_base64: base64_string,
        };

        // 使用全局PaddleOCR引擎进行识别（隐藏窗口）
        match Self::call_global_ocr(image_data_obj) {
            Ok(result_json) => {
                // 解析JSON结果
                self.parse_paddle_ocr_result(&result_json)
            }
            Err(e) => Err(anyhow::anyhow!("Base64 OCR失败: {}", e)),
        }
    }

    /// 解析PaddleOCR的JSON结果
    fn parse_paddle_ocr_result(&self, json_str: &str) -> Result<Vec<OcrResult>> {
        let mut results = Vec::new();

        // 解析JSON
        let json_value: Value =
            serde_json::from_str(json_str).map_err(|e| anyhow::anyhow!("JSON解析失败: {}", e))?;

        // 检查返回码
        if let Some(code) = json_value.get("code").and_then(|v| v.as_i64())
            && code != 100
        {
            return Err(anyhow::anyhow!("PaddleOCR返回错误码: {}", code));
        }

        // 获取data数组
        if let Some(data_array) = json_value.get("data").and_then(|v| v.as_array()) {
            if data_array.is_empty() {
                // 没有检测到文本，添加提示信息
                #[cfg(debug_assertions)]
                println!("OCR未检测到任何文本");
                results.push(OcrResult {
                    text: "未识别到任何文字".to_string(),
                    confidence: 0.0,
                    bounding_box: BoundingBox {
                        x: 0,
                        y: 0,
                        width: 200,
                        height: 25,
                    },
                });
            } else {
                // 处理每个识别结果
                for item in data_array {
                    if let (Some(text), Some(score), Some(box_coords)) = (
                        item.get("text").and_then(|v| v.as_str()),
                        item.get("score").and_then(|v| v.as_f64()),
                        item.get("box").and_then(|v| v.as_array()),
                    ) {
                        // 解析边界框坐标
                        let bounding_box = if box_coords.len() >= 4 {
                            let coords: Vec<Vec<i32>> = box_coords
                                .iter()
                                .filter_map(|coord| {
                                    coord.as_array().and_then(|arr| {
                                        if arr.len() >= 2 {
                                            Some(vec![
                                                arr[0].as_i64().unwrap_or(0) as i32,
                                                arr[1].as_i64().unwrap_or(0) as i32,
                                            ])
                                        } else {
                                            None
                                        }
                                    })
                                })
                                .collect();

                            if coords.len() >= 4 {
                                let min_x = coords.iter().map(|c| c[0]).min().unwrap_or(0);
                                let max_x = coords.iter().map(|c| c[0]).max().unwrap_or(0);
                                let min_y = coords.iter().map(|c| c[1]).min().unwrap_or(0);
                                let max_y = coords.iter().map(|c| c[1]).max().unwrap_or(0);

                                BoundingBox {
                                    x: min_x,
                                    y: min_y,
                                    width: max_x - min_x,
                                    height: max_y - min_y,
                                }
                            } else {
                                BoundingBox {
                                    x: 0,
                                    y: 0,
                                    width: 100,
                                    height: 20,
                                }
                            }
                        } else {
                            BoundingBox {
                                x: 0,
                                y: 0,
                                width: 100,
                                height: 20,
                            }
                        };

                        // 直接使用原始文本
                        if !text.trim().is_empty() {
                            results.push(OcrResult {
                                text: text.to_string(),
                                confidence: score as f32,
                                bounding_box,
                            });
                        }
                    }
                }
            }
        } else {
            return Err(anyhow::anyhow!("JSON格式错误：缺少data字段"));
        }

        Ok(results)
    }
}

/// 整体识别文本然后根据坐标换行
pub fn recognize_text_by_lines(image_data: &[u8], selection_rect: RECT) -> Result<Vec<OcrResult>> {
    // 使用整体识别
    let mut ocr_engine = PaddleOcrEngine::new()?;

    let all_results = ocr_engine.recognize_from_memory(image_data)?;

    // 不要在这里检查空结果，让OCR引擎的parse_paddle_ocr_result方法处理
    // 这样可以确保"未识别到任何文字"的提示能够正确显示

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
    let line_height_threshold = 20; // 行间距阈值

    for result in adjusted_results {
        let mut added_to_existing_line = false;

        // 尝试将当前文本块添加到现有行
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

        // 如果没有添加到现有行，创建新行
        if !added_to_existing_line {
            text_lines.push(vec![result]);
        }
    }

    // 处理每一行：按 X 坐标排序并合并文本
    let mut final_results = Vec::new();

    for mut line_blocks in text_lines.into_iter() {
        // 按 X 坐标排序
        line_blocks.sort_by(|a, b| a.bounding_box.x.cmp(&b.bounding_box.x));

        // 合并这一行的所有文本
        let mut line_text = String::new();
        let mut min_x = i32::MAX;
        let mut min_y = i32::MAX;
        let mut max_x = i32::MIN;
        let mut max_y = i32::MIN;
        let mut total_confidence = 0.0;

        for (i, text_block) in line_blocks.iter().enumerate() {
            if i > 0 {
                line_text.push(' '); // 在文本块之间添加空格
            }
            line_text.push_str(&text_block.text);

            // 计算整行的边界框
            min_x = min_x.min(text_block.bounding_box.x);
            min_y = min_y.min(text_block.bounding_box.y);
            max_x = max_x.max(text_block.bounding_box.x + text_block.bounding_box.width);
            max_y = max_y.max(text_block.bounding_box.y + text_block.bounding_box.height);

            total_confidence += text_block.confidence;
        }

        // 创建行结果
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

    // 按 Y 坐标最终排序，确保行的顺序正确
    final_results.sort_by(|a, b| a.bounding_box.y.cmp(&b.bounding_box.y));

    Ok(final_results)
}
