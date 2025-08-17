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

    /// 启动OCR引擎
    pub fn start_engine(&mut self) -> Result<(), SystemError> {
        // 使用原始OCR模块启动引擎
        crate::ocr::PaddleOcrEngine::start_ocr_engine_async();
        self.engine_available = true;
        Ok(())
    }

    /// 停止OCR引擎
    pub fn stop_engine(&mut self) {
        // 使用原始OCR模块停止引擎
        crate::ocr::PaddleOcrEngine::stop_ocr_engine_async();
        self.engine_available = false;
    }

    /// 异步停止OCR引擎（从原始代码迁移）
    pub fn stop_engine_async(&mut self) {
        crate::ocr::PaddleOcrEngine::stop_ocr_engine_async();
        self.engine_available = false;
    }

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
}
