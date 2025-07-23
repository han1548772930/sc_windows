use crate::ocr_result_window::OcrResultWindow;
use anyhow::Result;
use uni_ocr::{OcrEngine, OcrProvider};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;

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

/// UniOCR 引擎，使用 uni_ocr 库
pub struct UniOcrEngine {
    engine: OcrEngine,
}

/// 清理 OCR 识别结果中的明显错误字符
fn clean_ocr_text(text: &str) -> String {
    let mut cleaned = text.to_string();

    // 移除明显的错误字符模式
    cleaned = cleaned.replace("€", "e");
    cleaned = cleaned.replace("！", "!");
    cleaned = cleaned.replace("（", "(");
    cleaned = cleaned.replace("）", ")");
    cleaned = cleaned.replace("：", ":");
    cleaned = cleaned.replace("。", ".");
    cleaned = cleaned.replace("、", ",");
    cleaned = cleaned.replace("「", "[");
    cleaned = cleaned.replace("」", "]");
    cleaned = cleaned.replace("且", "and");

    // 移除过多的特殊字符
    let chars: Vec<char> = cleaned.chars().collect();
    let mut result = String::new();
    let mut special_count = 0;

    for ch in chars {
        if ch.is_alphanumeric() || ch.is_whitespace() || ".,!?:;()[]{}\"'-".contains(ch) {
            result.push(ch);
            special_count = 0;
        } else {
            special_count += 1;
            if special_count < 3 {
                // 允许少量特殊字符
                result.push(ch);
            }
        }
    }

    // 清理多余的空格
    result.split_whitespace().collect::<Vec<&str>>().join(" ")
}

impl UniOcrEngine {
    /// 创建新的 OCR 引擎实例
    pub fn new() -> Result<Self> {
        // 尝试不同的 OCR 提供商
        let engine = match OcrEngine::new(OcrProvider::Windows) {
            Ok(engine) => engine,
            Err(_) => OcrEngine::new(OcrProvider::Auto)?,
        };

        Ok(Self { engine })
    }

    /// 从文件路径识别文本（使用 uni_ocr）
    pub async fn recognize_file(&self, path: &std::path::Path) -> Result<Vec<OcrResult>> {
        // 检查文件是否存在
        if !path.exists() {
            return Err(anyhow::anyhow!("文件不存在: {:?}", path));
        }

        // 使用 uni_ocr 进行文本识别
        let path_str = path.to_string_lossy();
        let (text, _words, confidence) = self.engine.recognize_file(&path_str).await?;

        let mut results = Vec::new();

        if text.trim().is_empty() {
            // 如果没有检测到文本
            results.push(OcrResult {
                text: "No text detected in the selected area".to_string(),
                confidence: 0.0,
                bounding_box: BoundingBox {
                    x: 0,
                    y: 0,
                    width: 300,
                    height: 25,
                },
            });
        } else {
            // 清理识别到的文本
            let cleaned_text = clean_ocr_text(&text);

            // 按行处理文本
            let lines: Vec<&str> = cleaned_text.lines().collect();
            for (i, line) in lines.iter().enumerate() {
                let line_text = line.trim();
                if !line_text.is_empty() && line_text.len() > 2 {
                    // 过滤太短的行
                    // 进一步验证文本质量
                    let alpha_count = line_text.chars().filter(|c| c.is_alphabetic()).count();
                    let total_count = line_text.chars().count();
                    let alpha_ratio = alpha_count as f32 / total_count as f32;

                    // 只保留包含合理字母比例的文本
                    if alpha_ratio > 0.3 || line_text.chars().any(|c| c.is_ascii_digit()) {
                        results.push(OcrResult {
                            text: line_text.to_string(),
                            confidence: confidence.unwrap_or(0.8) as f32,
                            bounding_box: BoundingBox {
                                x: 10,
                                y: 10 + (i as i32 * 30),
                                width: line_text.len() as i32 * 12,
                                height: 25,
                            },
                        });
                    }
                }
            }

            // 如果清理后没有有效文本，显示原始结果但标记为低质量
            if results.is_empty() && !text.trim().is_empty() {
                results.push(OcrResult {
                    text: format!(
                        "Low quality OCR result: {}",
                        text.chars().take(100).collect::<String>()
                    ),
                    confidence: 0.2,
                    bounding_box: BoundingBox {
                        x: 10,
                        y: 10,
                        width: 500,
                        height: 25,
                    },
                });
            }
        }

        Ok(results)
    }

    /// 从内存中的图像数据识别文本
    pub async fn recognize_from_memory(&self, image_data: &[u8]) -> Result<Vec<OcrResult>> {
        // 将 BMP 数据保存到临时文件进行 OCR 识别
        let temp_path = std::env::temp_dir().join("screenshot_ocr_temp.bmp");
        std::fs::write(&temp_path, image_data)?;

        // 使用临时文件进行 OCR 识别
        let results = self.recognize_file(&temp_path).await?;

        // 清理临时文件
        let _ = std::fs::remove_file(&temp_path);

        Ok(results)
    }

    /// 批量识别多个图像数据
    pub async fn recognize_batch_from_memory(
        &self,
        images_data: &[Vec<u8>],
    ) -> Result<Vec<(String, Option<f32>)>> {
        // 为每个图像创建临时文件
        let mut temp_files = Vec::new();
        let mut file_paths = Vec::new();

        for (i, image_data) in images_data.iter().enumerate() {
            let temp_path = std::env::temp_dir().join(format!("screenshot_ocr_line_{}.bmp", i));
            std::fs::write(&temp_path, image_data)?;
            temp_files.push(temp_path.clone());
            file_paths.push(temp_path.to_string_lossy().to_string());
        }

        // 将 String 转换为 &str
        let file_path_refs: Vec<&str> = file_paths.iter().map(|s| s.as_str()).collect();

        // 使用 uni_ocr 的批量识别功能
        let batch_results = self.engine.recognize_batch(file_path_refs).await?;

        // 清理临时文件
        for temp_file in temp_files {
            let _ = std::fs::remove_file(&temp_file);
        }

        // 转换结果格式
        let results: Vec<(String, Option<f32>)> = batch_results
            .into_iter()
            .map(|(text, _, confidence)| (text, confidence.map(|c| c as f32)))
            .collect();

        Ok(results)
    }

    /// 检查 OCR 引擎是否可用
    pub fn is_available(&self) -> bool {
        // 如果能创建实例，说明 uni_ocr 可用
        true
    }
}

/// 从选择区域创建图像并进行 OCR 识别
pub async fn extract_text_from_selection(
    screenshot_dc: HDC,
    selection_rect: RECT,
    current_window: Option<HWND>,
) -> Result<Vec<OcrResult>> {
    unsafe {
        let width = selection_rect.right - selection_rect.left;
        let height = selection_rect.bottom - selection_rect.top;

        if width <= 0 || height <= 0 {
            return Ok(vec![]);
        }

        // 创建兼容的内存 DC
        let mem_dc = CreateCompatibleDC(Some(screenshot_dc));
        if mem_dc.is_invalid() {
            return Err(anyhow::anyhow!("创建内存 DC 失败"));
        }

        // 创建位图
        let bitmap = CreateCompatibleBitmap(screenshot_dc, width, height);
        if bitmap.is_invalid() {
            let _ = DeleteDC(mem_dc);
            return Err(anyhow::anyhow!("创建位图失败"));
        }

        // 选择位图到内存 DC
        let old_bitmap = SelectObject(mem_dc, bitmap.into());

        // 复制选择区域到内存 DC
        let result = BitBlt(
            mem_dc,
            0,
            0,
            width,
            height,
            Some(screenshot_dc),
            selection_rect.left,
            selection_rect.top,
            SRCCOPY,
        );

        if result.is_err() {
            let _ = SelectObject(mem_dc, old_bitmap);
            let _ = DeleteObject(bitmap.into());
            let _ = DeleteDC(mem_dc);
            return Err(anyhow::anyhow!("复制图像失败"));
        }

        // 将位图保存为 PNG 数据
        let image_data = bitmap_to_png_data(mem_dc, bitmap, width, height)?;

        // 清理 GDI 资源
        let _ = SelectObject(mem_dc, old_bitmap);
        let _ = DeleteObject(bitmap.into());
        let _ = DeleteDC(mem_dc);

        // 分行识别文本
        let line_results = recognize_text_by_lines(&image_data, selection_rect).await?;

        // 显示 OCR 结果窗口
        let _ = OcrResultWindow::show(image_data, line_results.clone(), selection_rect);

        // 关闭截图窗口（如果提供了窗口句柄）
        if let Some(hwnd) = current_window {
            use windows::Win32::UI::WindowsAndMessaging::*;
            // 使用自定义消息来通知窗口关闭截图模式，而不是关闭整个程序
            let _ = PostMessageW(Some(hwnd), WM_USER + 2, WPARAM(0), LPARAM(0));
        }

        Ok(line_results)
    }
}

/// 整体识别文本然后根据坐标换行
async fn recognize_text_by_lines(
    image_data: &[u8],
    selection_rect: RECT,
) -> Result<Vec<OcrResult>> {
    // 使用整体识别
    let ocr_engine = UniOcrEngine::new()?;
    let all_results = ocr_engine.recognize_from_memory(image_data).await?;

    if all_results.is_empty() {
        return Ok(vec![]);
    }

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

    for (line_index, mut line_blocks) in text_lines.into_iter().enumerate() {
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

/// 基于图像尺寸检测文本行区域
fn detect_text_lines(width: i32, height: i32) -> Vec<RECT> {
    let mut line_rects = Vec::new();

    // 更智能的行高估算
    let estimated_line_height = if height <= 30 {
        // 很小的图像，可能是单行
        height
    } else if height <= 60 {
        // 中等高度，可能是1-2行
        if height > 45 {
            height / 2 // 分成2行
        } else {
            height // 单行
        }
    } else if height <= 120 {
        // 较高图像，可能是2-3行
        if height > 90 {
            height / 3 // 分成3行
        } else {
            height / 2 // 分成2行
        }
    } else {
        // 很高的图像，假设每行约40-50像素高
        let typical_line_height = 45;
        let estimated_lines = (height / typical_line_height).max(1);
        height / estimated_lines
    };

    // 计算实际行数，但限制最大行数
    let max_lines = (height / 25).min(10); // 每行至少25像素，最多10行
    let line_count = (height / estimated_line_height).max(1).min(max_lines);

    // 如果只有1行，直接返回整个图像
    if line_count == 1 {
        line_rects.push(RECT {
            left: 0,
            top: 0,
            right: width,
            bottom: height,
        });
        return line_rects;
    }

    // 创建文本行区域，使用更大的重叠区域
    for i in 0..line_count {
        let y_start = (i * height / line_count) as i32;
        let y_end = (((i + 1) * height / line_count) as i32).min(height);

        // 添加重叠区域，确保不会切断文字
        let overlap = estimated_line_height / 4; // 25% 重叠
        let adjusted_y_start = if i == 0 {
            0
        } else {
            (y_start - overlap).max(0)
        };
        let adjusted_y_end = if i == line_count - 1 {
            height
        } else {
            (y_end + overlap).min(height)
        };

        // 确保行高度合理
        if adjusted_y_end - adjusted_y_start >= 20 {
            // 至少20像素高
            let line_rect = RECT {
                left: 0,
                top: adjusted_y_start,
                right: width,
                bottom: adjusted_y_end,
            };

            line_rects.push(line_rect);
        }
    }

    line_rects
}

/// 从原图中提取指定区域的图片数据
fn extract_line_image(original_image_data: &[u8], line_rect: &RECT) -> Result<Vec<u8>> {
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
    let crop_x = line_rect.left.max(0).min(width - 1);
    let crop_y = line_rect.top.max(0).min(height - 1);
    let crop_width = (line_rect.right - line_rect.left)
        .max(1)
        .min(width - crop_x);
    let crop_height = (line_rect.bottom - line_rect.top)
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
    new_bmp.extend_from_slice(&crop_height.to_le_bytes()); // 新高度
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

/// 将位图转换为 PNG 数据
fn bitmap_to_png_data(mem_dc: HDC, bitmap: HBITMAP, width: i32, height: i32) -> Result<Vec<u8>> {
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
