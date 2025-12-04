use super::SystemError;
use windows::Win32::UI::WindowsAndMessaging::WM_USER;

/// OCR管理器
pub struct OcrManager {
    /// OCR引擎是否可用
    engine_available: bool,
}

// OCR结果数据传输结构
pub struct OcrCompletionData {
    pub image_data: Vec<u8>,
    pub ocr_results: Vec<crate::ocr::OcrResult>,
    pub selection_rect: windows::Win32::Foundation::RECT,
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

    /// 启动异步状态检查
    pub fn start_async_status_check(&mut self, hwnd: windows::Win32::Foundation::HWND) {
        let hwnd_ptr = hwnd.0 as usize;

        // 启动异步检查
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

    /// 重新加载设置
    pub fn reload_settings(&mut self) {
        // OCR设置通常不需要重新加载，但可以在这里添加相关逻辑
    }

    /// 更新OCR引擎状态
    pub fn update_status(&mut self, available: bool) {
        self.engine_available = available;
    }

    /// 查询缓存的OCR引擎可用性（不阻塞，供UI使用）
    pub fn is_available(&self) -> bool {
        self.engine_available
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
        screenshot_manager: &mut crate::screenshot::ScreenshotManager,
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

        // 异步执行耗时的OCR操作，避免阻塞UI线程
        std::thread::spawn(move || {
            // 重构 HWND
            let hwnd = windows::Win32::Foundation::HWND(hwnd_ptr as *mut std::ffi::c_void);

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

                match crate::ocr::crop_bmp(data, &selection_rect) {
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
}
