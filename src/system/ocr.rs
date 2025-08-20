// OCR管理
//
// 负责OCR引擎的管理和文本识别

use super::SystemError;
use windows::Win32::UI::WindowsAndMessaging::WM_USER;

/// OCR管理器
pub struct OcrManager {
    /// OCR引擎是否可用
    engine_available: bool,
}

impl OcrManager {
    /// 创建新的OCR管理器
    pub fn new() -> Result<Self, SystemError> {
        Ok(Self {
            engine_available: false,
        })
    }

    /// 确保OCR引擎已启动（统一的引擎管理接口）
    pub fn ensure_engine_started(&mut self) -> Result<(), SystemError> {
        if !self.engine_available {
            // 使用统一的OCR引擎管理接口
            crate::ocr::PaddleOcrEngine::ensure_engine_started();
            self.engine_available = true;
        }
        Ok(())
    }

    /// 停止OCR引擎（统一的停止接口）
    pub fn stop_engine(&mut self) {
        if self.engine_available {
            // 使用统一的OCR引擎停止接口（异步停止）
            crate::ocr::PaddleOcrEngine::stop_engine(false);
            self.engine_available = false;
        }
    }

    /// 立即停止OCR引擎（程序退出时使用）
    pub fn stop_engine_immediate(&mut self) {
        if self.engine_available {
            // 使用统一的OCR引擎停止接口（同步停止）
            crate::ocr::PaddleOcrEngine::stop_engine(true);
            self.engine_available = false;
        }
    }

    // 注意：以下方法已被移除，使用统一接口代替：
    // - start_engine: 使用 ensure_engine_started 代替
    // - stop_engine_async: 与 stop_engine 重复，已合并

    /// 启动异步状态检查（从原始代码迁移）
    pub fn start_async_status_check(&mut self, hwnd: windows::Win32::Foundation::HWND) {
        // 使用原始代码的完整实现
        let hwnd_ptr = hwnd.0 as usize;

        // 启动异步检查，使用原始代码的check_engine_status_async方法
        crate::ocr::PaddleOcrEngine::check_engine_status_async(
            move |exe_exists, engine_ready, _status| {
                // 在后台线程中检查完成后，发送消息到主线程更新状态
                let available = exe_exists && engine_ready;
                unsafe {
                    use windows::Win32::UI::WindowsAndMessaging::PostMessageW;
                    // 重新构造HWND
                    let hwnd = windows::Win32::Foundation::HWND(hwnd_ptr as *mut std::ffi::c_void);

                    // 使用自定义消息通知主线程更新OCR状态
                    // WM_USER + 10 用于OCR状态更新
                    let _ = PostMessageW(
                        Some(hwnd),
                        WM_USER + 10,
                        windows::Win32::Foundation::WPARAM(if available { 1 } else { 0 }),
                        windows::Win32::Foundation::LPARAM(0),
                    );
                }
            },
        );
    }

    /// 重新加载设置（从原始代码迁移）
    pub fn reload_settings(&mut self) {
        // OCR设置通常不需要重新加载，但可以在这里添加相关逻辑
    }

    /// 更新OCR引擎状态
    pub fn update_status(&mut self, available: bool) {
        self.engine_available = available;
    }

    /// 执行OCR识别
    pub fn perform_ocr(&mut self, image_data: &[u8]) -> Result<String, SystemError> {
        if !self.engine_available {
            return Err(SystemError::OcrError(
                "OCR engine not available".to_string(),
            ));
        }

        // 使用原始OCR模块进行识别
        match crate::ocr::PaddleOcrEngine::new() {
            Ok(mut engine) => {
                match engine.recognize_from_memory(image_data) {
                    Ok(results) => {
                        // 合并所有识别结果
                        let text = results
                            .iter()
                            .map(|r| r.text.as_str())
                            .collect::<Vec<_>>()
                            .join("\n");
                        Ok(text)
                    }
                    Err(e) => Err(SystemError::OcrError(format!(
                        "OCR recognition failed: {}",
                        e
                    ))),
                }
            }
            Err(e) => Err(SystemError::OcrError(format!(
                "Failed to create OCR engine: {}",
                e
            ))),
        }
    }

    /// 从选择区域识别文本（统一的OCR流程处理）
    /// 这是一个"一站式"服务，处理整个OCR流程：
    /// 1. 检查引擎可用性
    /// 2. 隐藏UI进行截图
    /// 3. 执行OCR识别
    /// 4. 显示结果窗口
    /// 5. 通知主窗口流程结束
    pub fn recognize_text_from_selection(
        &mut self,
        selection_rect: windows::Win32::Foundation::RECT,
        hwnd: windows::Win32::Foundation::HWND,
        _screenshot_manager: &mut crate::screenshot::ScreenshotManager,
    ) -> Result<(), SystemError> {
        use windows::Win32::Foundation::*;
        use windows::Win32::UI::WindowsAndMessaging::*;

        // 检查OCR引擎是否可用
        if !self.engine_available {
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

        // 彻底隐藏窗口进行干净的截图
        unsafe {
            use windows::Win32::UI::WindowsAndMessaging::*;
            let _ = ShowWindow(hwnd, SW_HIDE);
        }

        // 给予系统足够时间重绘被遮挡的桌面区域
        std::thread::sleep(std::time::Duration::from_millis(100));

        let result = {
            let width = selection_rect.right - selection_rect.left;
            let height = selection_rect.bottom - selection_rect.top;

            if width <= 0 || height <= 0 {
                return Ok(());
            }

            // 使用统一的平台层截图函数，避免重复的GDI代码
            let bitmap = match unsafe {
                crate::platform::windows::gdi::capture_screen_region_to_hbitmap(selection_rect)
            } {
                Ok(bitmap) => bitmap,
                Err(e) => {
                    return Err(SystemError::OcrError(format!("截图失败: {:?}", e)));
                }
            };

            // 将位图转换为 BMP 数据
            let image_data = unsafe {
                use windows::Win32::Foundation::HWND;
                use windows::Win32::Graphics::Gdi::*;

                let screen_dc = GetDC(Some(HWND(std::ptr::null_mut())));
                let mem_dc = CreateCompatibleDC(Some(screen_dc));
                let old_bitmap = SelectObject(mem_dc, bitmap.into());

                let result = match crate::ocr::bitmap_to_bmp_data(mem_dc, bitmap, width, height) {
                    Ok(data) => Ok(data),
                    Err(e) => Err(SystemError::OcrError(format!("位图转换失败: {}", e))),
                };

                // 清理 GDI 资源
                let _ = SelectObject(mem_dc, old_bitmap);
                let _ = DeleteObject(bitmap.into());
                let _ = DeleteDC(mem_dc);
                let _ = ReleaseDC(Some(HWND(std::ptr::null_mut())), screen_dc);

                result
            };

            match image_data {
                Ok(data) => data,
                Err(e) => return Err(e),
            }
        };

        // 分行识别文本
        let line_results = match crate::ocr::recognize_text_by_lines(&result, selection_rect) {
            Ok(results) => results,
            Err(_) => {
                // 即使识别失败，也要显示结果窗口
                vec![crate::ocr::OcrResult {
                    text: "OCR识别失败".to_string(),
                    confidence: 0.0,
                    bounding_box: crate::ocr::BoundingBox {
                        x: 0,
                        y: 0,
                        width: 200,
                        height: 25,
                    },
                }]
            }
        };

        // 显示 OCR 结果窗口
        let _ = crate::ocr_result_window::OcrResultWindow::show(
            result,
            line_results.clone(),
            selection_rect,
        );

        // 关闭截图窗口（通知主窗口流程结束）
        unsafe {
            use windows::Win32::UI::WindowsAndMessaging::*;
            let _ = PostMessageW(Some(hwnd), WM_USER + 2, WPARAM(0), LPARAM(0));

            // 如果没有识别到文本，显示提示消息
            if line_results.is_empty()
                || (line_results.len() == 1 && line_results[0].text == "OCR识别失败")
            {
                let message = "未识别到文本内容。\n\n请确保选择区域包含清晰的文字。";
                let message_w: Vec<u16> =
                    message.encode_utf16().chain(std::iter::once(0)).collect();
                let title_w: Vec<u16> =
                    "OCR结果".encode_utf16().chain(std::iter::once(0)).collect();

                MessageBoxW(
                    Some(hwnd),
                    windows::core::PCWSTR(message_w.as_ptr()),
                    windows::core::PCWSTR(title_w.as_ptr()),
                    MB_OK | MB_ICONINFORMATION,
                );
            }
        }

        // 恢复窗口显示（不需要调用 show_ui_after_capture，因为窗口会在OCR完成后被隐藏）
        // screenshot_manager.show_ui_after_capture(hwnd); // 已不需要

        Ok(())
    }
}
