use anyhow::Result;
use base64::{Engine as _, engine::general_purpose};
use paddleocr::{ImageData, Ppocr};
use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Mutex, OnceLock};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;

// 简化版本：只使用本地路径访问

// 当前活跃的OCR引擎（按需启动和关闭）
static CURRENT_OCR_ENGINE: OnceLock<Mutex<Option<Ppocr>>> = OnceLock::new();

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
        // 在后台线程中异步启动OCR引擎
        std::thread::spawn(|| {
            if let Err(e) = Self::start_ocr_engine_sync() {
                #[cfg(debug_assertions)]
                eprintln!("异步启动OCR引擎失败: {e}");
            }
        });
    }

    /// 同步启动OCR引擎（内部使用）
    fn start_ocr_engine_sync() -> Result<()> {
        let engine_mutex = CURRENT_OCR_ENGINE.get_or_init(|| Mutex::new(None));
        let mut engine_guard = engine_mutex.lock().unwrap();

        if engine_guard.is_none() {
            #[cfg(debug_assertions)]
            println!("正在后台启动OCR引擎...");

            let start_time = std::time::Instant::now();

            // 获取 PaddleOCR-json.exe 的路径
            let exe_path = Self::get_paddle_ocr_exe_path()?;

            // 获取语言配置路径（可以根据需要修改）
            let config_path = Self::get_language_config_path();

            // 创建PaddleOCR引擎（已修改源代码支持隐藏窗口）
            let engine = Ppocr::new(exe_path, config_path)
                .map_err(|e| anyhow::anyhow!("创建 PaddleOCR 引擎失败: {}", e))?;

            *engine_guard = Some(engine);

            let elapsed = start_time.elapsed();
            #[cfg(debug_assertions)]
            println!("OCR引擎启动成功，耗时: {elapsed:?}");
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
        if let Some(engine_mutex) = CURRENT_OCR_ENGINE.get() {
            if let Ok(mut engine_guard) = engine_mutex.lock() {
                if let Some(engine) = engine_guard.take() {
                    #[cfg(debug_assertions)]
                    println!("正在后台停止OCR引擎...");

                    let start_time = std::time::Instant::now();

                    // 正常关闭引擎
                    drop(engine);

                    // 等待进程退出
                    std::thread::sleep(std::time::Duration::from_millis(300));

                    // 强制清理残留进程
                    Self::force_kill_paddle_processes();

                    let elapsed = start_time.elapsed();
                    #[cfg(debug_assertions)]
                    println!("OCR引擎已停止，耗时: {elapsed:?}");
                }
            }
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
        if let Some(engine_mutex) = CURRENT_OCR_ENGINE.get() {
            if let Ok(mut engine_guard) = engine_mutex.lock() {
                if let Some(engine) = engine_guard.as_mut() {
                    // 引擎已就绪，执行OCR
                    #[cfg(debug_assertions)]
                    println!("OCR引擎就绪，开始识别...");

                    return engine
                        .ocr(image_data)
                        .map_err(|e| anyhow::anyhow!("PaddleOCR 识别失败: {}", e));
                }
            }
        }

        // 引擎未就绪，直接返回错误
        Err(anyhow::anyhow!("OCR引擎未就绪，请等待引擎启动完成"))
    }

    /// 预启动OCR引擎（已废弃，改为按需启动）
    #[deprecated(note = "使用start_ocr_engine()代替")]
    pub fn prestart_engine() {
        // 不再预启动，改为按需启动
    }

    /// 检查OCR引擎是否已经准备就绪
    pub fn is_engine_ready() -> bool {
        if let Some(engine_mutex) = CURRENT_OCR_ENGINE.get() {
            if let Ok(engine_guard) = engine_mutex.lock() {
                return engine_guard.is_some();
            }
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

    /// 异步检查OCR引擎状态并返回详细信息（非阻塞）
    pub fn check_engine_status_async<F>(callback: F)
    where
        F: Fn(bool, bool, String) + Send + 'static,
    {
        std::thread::spawn(move || {
            // 在后台线程中检查引擎状态
            let exe_exists = Self::find_paddle_exe().is_ok();
            let mut engine_ready = Self::is_engine_ready();

            // 如果可执行文件存在但引擎未启动，则尝试启动引擎
            if exe_exists && !engine_ready {
                #[cfg(debug_assertions)]
                println!("检测到PaddleOCR可执行文件，正在启动OCR引擎...");

                // 尝试启动引擎
                if let Err(e) = Self::start_ocr_engine_sync() {
                    #[cfg(debug_assertions)]
                    eprintln!("启动OCR引擎失败: {e}");
                } else {
                    // 启动成功，重新检查状态
                    engine_ready = Self::is_engine_ready();
                    #[cfg(debug_assertions)]
                    println!("OCR引擎启动成功，状态: {engine_ready}");
                }
            }

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
            let result = Command::new("taskkill")
                .args(["/F", "/IM", "PaddleOCR-json.exe"])
                .creation_flags(0x08000000) // CREATE_NO_WINDOW
                .output();

            #[cfg(debug_assertions)]
            match result {
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
        let settings = crate::settings::Settings::load();
        let language = &settings.ocr_language;

        match language.as_str() {
            "english" => Some(PathBuf::from("models\\config_en.txt")),
            "chinese_cht" => Some(PathBuf::from("models\\config_chinese_cht.txt")),
            "japan" => Some(PathBuf::from("models\\config_japan.txt")),
            "korean" => Some(PathBuf::from("models\\config_korean.txt")),
            "chinese" | _ => None,
        }
    }

    /// 从文件路径识别文本（使用 PaddleOCR）
    pub fn recognize_file(&mut self, path: &std::path::Path) -> Result<Vec<OcrResult>> {
        // 检查文件是否存在
        if !path.exists() {
            return Err(anyhow::anyhow!("文件不存在: {:?}", path));
        }

        // 使用全局 PaddleOCR 引擎进行文本识别（隐藏窗口）
        let ocr_result = Self::call_global_ocr(path.into())?;

        // 解析JSON结果
        let results = self.parse_paddle_ocr_result(&ocr_result)?;

        Ok(results)
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

    /// 批量识别多个图像数据
    pub fn recognize_batch_from_memory(
        &mut self,
        images_data: &[Vec<u8>],
    ) -> Result<Vec<(String, Option<f32>)>> {
        let mut results = Vec::new();

        // 为每个图像使用Base64方式识别，避免磁盘IO
        for image_data in images_data.iter() {
            // 将图像数据编码为Base64
            let base64_string = general_purpose::STANDARD.encode(image_data);
            let image_data_obj = ImageData::ImageBase64Dict {
                image_base64: base64_string,
            };

            // 使用全局 PaddleOCR 引擎识别
            match Self::call_global_ocr(image_data_obj) {
                Ok(json_str) => {
                    // 解析结果
                    if let Ok(ocr_results) = self.parse_paddle_ocr_result(&json_str) {
                        // 提取文本和置信度
                        let mut text = String::new();
                        let mut total_confidence = 0.0;
                        let count = ocr_results.len();

                        for (i, res) in ocr_results.iter().enumerate() {
                            if i > 0 {
                                text.push(' ');
                            }
                            text.push_str(&res.text);
                            total_confidence += res.confidence;
                        }

                        let avg_confidence = if count > 0 {
                            Some(total_confidence / count as f32)
                        } else {
                            // 检查是否是"未识别到任何文字"的情况
                            if ocr_results.len() == 1 && ocr_results[0].text == "未识别到任何文字" {
                                None
                            } else {
                                Some(0.0)
                            }
                        };

                        // 如果结果是空的或者全是占位符，视为空
                        if text.is_empty() || text == "未识别到任何文字" {
                             results.push((String::new(), Some(0.0)));
                        } else {
                             results.push((text, avg_confidence));
                        }
                    } else {
                        results.push((String::new(), Some(0.0)));
                    }
                }
                Err(_) => {
                    results.push((String::new(), Some(0.0))); // 识别失败
                }
            }
        }

        Ok(results)
    }

    /// 解析PaddleOCR的JSON结果
    fn parse_paddle_ocr_result(&self, json_str: &str) -> Result<Vec<OcrResult>> {
        let mut results = Vec::new();

        // 解析JSON
        let json_value: Value =
            serde_json::from_str(json_str).map_err(|e| anyhow::anyhow!("JSON解析失败: {}", e))?;

        // 检查返回码
        if let Some(code) = json_value.get("code").and_then(|v| v.as_i64()) {
            if code != 100 {
                return Err(anyhow::anyhow!("PaddleOCR返回错误码: {}", code));
            }
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

/// 从原图中提取指定区域的图片数据
pub fn crop_bmp(original_image_data: &[u8], crop_rect: &RECT) -> Result<Vec<u8>> {
    // 解析 BMP 头部信息
    if original_image_data.len() < 54 {
        return Err(anyhow::anyhow!("BMP 数据太小"));
    }

    // 读取 BMP 头部信息
    let width = i32::from_le_bytes([
        original_image_data[18],
        original_image_data[19],
        original_image_data[20],
        original_image_data[21],
    ]);
    let height = i32::from_le_bytes([
        original_image_data[22],
        original_image_data[23],
        original_image_data[24],
        original_image_data[25],
    ])
    .abs();

    let bits_per_pixel = u16::from_le_bytes([original_image_data[28], original_image_data[29]]);

    // 计算每行的字节数（需要4字节对齐）
    let bytes_per_pixel = (bits_per_pixel / 8) as i32;
    let row_size = ((width * bytes_per_pixel + 3) / 4) * 4;

    // 计算裁剪区域
    let crop_x = crop_rect.left.max(0).min(width - 1);
    let crop_y = crop_rect.top.max(0).min(height - 1);
    let crop_width = (crop_rect.right - crop_rect.left)
        .max(1)
        .min(width - crop_x);
    let crop_height = (crop_rect.bottom - crop_rect.top)
        .max(1)
        .min(height - crop_y);

    // 如果裁剪区域无效，返回原图
    if crop_width <= 0 || crop_height <= 0 {
        return Ok(original_image_data.to_vec());
    }

    // 创建新的 BMP 头部
    let new_row_size = ((crop_width * bytes_per_pixel + 3) / 4) * 4;
    let new_image_size = new_row_size * crop_height;
    let new_file_size = 54 + new_image_size;

    let mut new_bmp = Vec::with_capacity(new_file_size as usize);

    // 复制并修改 BMP 头部
    new_bmp.extend_from_slice(&original_image_data[0..18]); // 文件头
    new_bmp.extend_from_slice(&crop_width.to_le_bytes()); // 新宽度
    new_bmp.extend_from_slice(&(-crop_height).to_le_bytes()); // 新高度 (保持负值，Top-Down)
    new_bmp.extend_from_slice(&original_image_data[26..54]); // 其余头部信息

    // 修改文件大小
    new_bmp[2..6].copy_from_slice(&(new_file_size as u32).to_le_bytes());
    // 修改图像数据大小
    new_bmp[34..38].copy_from_slice(&(new_image_size as u32).to_le_bytes());

    // 复制像素数据
    let pixel_data_offset = 54;

    for y in 0..crop_height {
        let src_y = crop_y + y;
        let src_row_start = pixel_data_offset + (src_y * row_size) as usize;
        let src_pixel_start = src_row_start + (crop_x * bytes_per_pixel) as usize;
        let src_pixel_end = src_pixel_start + (crop_width * bytes_per_pixel) as usize;

        if src_pixel_end <= original_image_data.len() {
            new_bmp.extend_from_slice(&original_image_data[src_pixel_start..src_pixel_end]);

            // 添加行填充
            let padding = new_row_size - crop_width * bytes_per_pixel;
            for _ in 0..padding {
                new_bmp.push(0);
            }
        }
    }

    Ok(new_bmp)
}

/// 将位图转换为 BMP 数据
pub fn bitmap_to_bmp_data(
    mem_dc: HDC,
    bitmap: HBITMAP,
    width: i32,
    height: i32,
) -> Result<Vec<u8>> {
    unsafe {
        // 获取位图信息
        let mut bitmap_info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height, // 负值表示自顶向下
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            bmiColors: [RGBQUAD::default(); 1],
        };

        // 计算图像数据大小
        let data_size = (width * height * 4) as usize;
        let mut pixel_data = vec![0u8; data_size];

        // 获取位图数据
        let result = GetDIBits(
            mem_dc,
            bitmap,
            0,
            height as u32,
            Some(pixel_data.as_mut_ptr() as *mut _),
            &mut bitmap_info,
            DIB_RGB_COLORS,
        );

        if result == 0 {
            return Err(anyhow::anyhow!("获取位图数据失败"));
        }

        // 将 BGRA 数据转换为简单的 BMP 格式
        // 创建 BMP 文件头
        let file_size = 54 + data_size as u32; // BMP 头部 + 数据
        let mut bmp_data = Vec::with_capacity(file_size as usize);

        // BMP 文件头 (14 字节)
        bmp_data.extend_from_slice(b"BM"); // 签名
        bmp_data.extend_from_slice(&file_size.to_le_bytes()); // 文件大小
        bmp_data.extend_from_slice(&[0u8; 4]); // 保留字段
        bmp_data.extend_from_slice(&54u32.to_le_bytes()); // 数据偏移

        // BMP 信息头 (40 字节)
        bmp_data.extend_from_slice(&40u32.to_le_bytes()); // 信息头大小
        bmp_data.extend_from_slice(&width.to_le_bytes()); // 宽度
        bmp_data.extend_from_slice(&(-height).to_le_bytes()); // 高度（负值，表示自顶向下，与 GetDIBits 一致）
        bmp_data.extend_from_slice(&1u16.to_le_bytes()); // 平面数
        bmp_data.extend_from_slice(&32u16.to_le_bytes()); // 位深度
        bmp_data.extend_from_slice(&[0u8; 24]); // 其他字段填充为 0

        // 添加像素数据
        bmp_data.extend_from_slice(&pixel_data);

        Ok(bmp_data)
    }
}
