//! OCR 管理器
//!
//! 负责 OCR 业务流程处理，引擎状态由 `PaddleOcrEngine` 直接管理。

use windows::Win32::Foundation::{HWND, RECT};

use super::engine::{recognize_text_by_lines, PaddleOcrEngine};
use super::types::{BoundingBox, OcrCompletionData, OcrResult};
use crate::screenshot::ScreenshotManager;
use crate::system::SystemError;
use crate::utils::image_processing::crop_bmp;

/// 从选择区域识别文本（统一的OCR流程处理）
/// 这是一个"一站式"服务，处理整个OCR流程：
/// 1. 检查引擎可用性
/// 2. 隐藏UI进行截图
/// 3. 执行OCR识别
/// 4. 显示结果窗口
/// 5. 通知主窗口流程结束
pub fn recognize_text_from_selection(
    selection_rect: RECT,
    hwnd: HWND,
    screenshot_manager: &mut ScreenshotManager,
) -> Result<(), SystemError> {
        use windows::Win32::Foundation::*;
        use windows::Win32::UI::WindowsAndMessaging::*;

    // 检查OCR引擎是否可用
    if !PaddleOcrEngine::is_engine_available() {
            let message = "OCR引擎不可用。\n\n请确保PaddleOCR引擎正常运行。";
            let message_w: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();
            let title_w: Vec<u16> = "OCR错误".encode_utf16().chain(std::iter::once(0)).collect();

            unsafe {
                MessageBoxW(
                    Some(hwnd),
                    windows::core::PCWSTR(message_w.as_ptr()),
                    windows::core::PCWSTR(title_w.as_ptr()),
                    MB_OK | MB_ICONERROR,
                );
            }
        return Ok(());
    }

    // 获取缓存的图像数据（如果有），避免重复截图
    // 注意：必须在隐藏窗口之前获取，虽然数据已经在内存中，但这是一个逻辑点
    let cached_image = screenshot_manager.get_current_image_data().map(|d| d.to_vec());

    // 彻底隐藏窗口进行干净的截图 (UI线程执行)
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::*;
        let _ = ShowWindow(hwnd, SW_HIDE);
    }
    
    // HWND 包含 raw pointer，不能直接 Send。转换为 usize 传递。
    let hwnd_ptr = hwnd.0 as usize;

    // 异步执行耗时的OCR操作，避免阻塞主UI线程
    std::thread::spawn(move || {
        // 重构 HWND
        let hwnd = HWND(hwnd_ptr as *mut std::ffi::c_void);

        // 如果没有缓存图像，给予系统足够时间重绘被遮挡的桌面区域
        if cached_image.is_none() {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        let result = {
            let width = selection_rect.right - selection_rect.left;
            let height = selection_rect.bottom - selection_rect.top;

            if width <= 0 || height <= 0 {
                // 恢复窗口
                unsafe {
                    let _ = PostMessageW(Some(hwnd), WM_USER + 2, WPARAM(0), LPARAM(0));
                }
                return;
            }

            // 使用缓存数据进行裁剪
            let Some(ref data) = cached_image else {
                eprintln!("没有缓存图像数据");
                unsafe {
                    let _ = PostMessageW(Some(hwnd), WM_USER + 2, WPARAM(0), LPARAM(0));
                }
                return;
            };

            match crop_bmp(data, &selection_rect) {
                Ok(cropped) => cropped,
                Err(e) => {
                    eprintln!("裁剪图像失败: {:?}", e);
                    unsafe {
                        let _ = PostMessageW(Some(hwnd), WM_USER + 2, WPARAM(0), LPARAM(0));
                    }
                    return;
                }
            }
        };

        // 分行识别文本 (耗时操作)
        let line_results = match recognize_text_by_lines(&result, selection_rect) {
            Ok(results) => results,
            Err(_) => {
                // 即使识别失败，也要显示结果窗口
                vec![OcrResult {
                    text: "OCR识别失败".to_string(),
                    confidence: 0.0,
                    bounding_box: BoundingBox {
                        x: 0,
                        y: 0,
                        width: 200,
                        height: 25,
                    },
                }]
            }
        };

        // 构建完成数据包
        let completion_data = Box::new(OcrCompletionData {
            image_data: result,
            ocr_results: line_results,
            selection_rect,
        });

        // 通知主线程显示结果 (WM_USER + 11)
        unsafe {
            let _ = PostMessageW(
                Some(hwnd), 
                WM_USER + 11, 
                WPARAM(0), 
                LPARAM(Box::into_raw(completion_data) as isize)
            );
        }
    });

    Ok(())
}
