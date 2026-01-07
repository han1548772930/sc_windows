//! 系统集成模块
//!
//! 提供与 Windows 系统的集成功能。
//!
//! # 主要组件
//! - [`SystemManager`]: 系统管理器，统一管理系统集成
//! - [`TrayManager`](tray::TrayManager): 系统托盘管理
//! - [`HotkeyManager`](hotkeys::HotkeyManager): 全局热键管理
//! - [`WindowDetectionManager`](window_detection::WindowDetectionManager): 窗口检测

use std::sync::{Arc, Mutex, RwLock};

use ocr_rs::OcrEngine;
use windows::Win32::Foundation::{HWND, LPARAM, RECT, WPARAM};

use crate::message::{Command, SystemMessage};
use crate::ocr::{self, OcrResult, BoundingBox};
use crate::screenshot::ScreenshotManager;
use crate::settings::Settings;
use crate::utils::image_processing::crop_bmp;

pub mod hotkeys;
pub mod tray;
pub mod window_detection;

use hotkeys::HotkeyManager;
use tray::TrayManager;
use window_detection::WindowDetectionManager;

/// 系统管理器
pub struct SystemManager {
    /// 共享的配置引用
    #[allow(dead_code)]
    settings: Arc<RwLock<Settings>>,
    /// 托盘管理器
    tray: TrayManager,
    /// 热键管理器
    hotkeys: HotkeyManager,
    /// 窗口检测管理器
    window_detection: WindowDetectionManager,
    /// OCR 引擎实例
    ocr_engine: Arc<Mutex<Option<OcrEngine>>>,
}

impl SystemManager {
    /// 创建新的系统管理器
    ///
    /// # 参数
    /// - `settings`: 共享的配置引用
    pub fn new(settings: Arc<RwLock<Settings>>) -> Result<Self, SystemError> {
        Ok(Self {
            tray: TrayManager::new()?,
            hotkeys: HotkeyManager::new(Arc::clone(&settings))?,
            window_detection: WindowDetectionManager::new()?,
            settings,
            ocr_engine: Arc::new(Mutex::new(None)),
        })
    }

    /// 处理系统消息
    pub fn handle_message(&mut self, message: SystemMessage) -> Vec<Command> {
        match message {
            SystemMessage::TrayMessage(wparam, lparam) => self.tray.handle_message(wparam, lparam),
            SystemMessage::HotkeyTriggered => self.hotkeys.handle_hotkey_triggered(),
            SystemMessage::WindowDetected(_window_title) => {
                // TODO: 实现窗口检测处理逻辑
                vec![]
            }
            SystemMessage::OcrStatusUpdate(_available) => {
                // OCR 引擎状态由 SystemManager 管理
                vec![Command::RequestRedraw]
            }
        }
    }

    /// 处理键盘输入（全局快捷键）
    pub fn handle_key_input(&mut self, key: u32) -> Vec<Command> {
        self.hotkeys.handle_key_input(key)
    }

    /// 初始化系统集成
    pub fn initialize(&mut self, hwnd: HWND) -> Result<(), SystemError> {
        // 初始化系统托盘
        self.tray.initialize(hwnd)?;

        // 注册全局热键
        self.hotkeys.register_hotkeys(hwnd)?;

        // 启动窗口检测
        self.window_detection.start_detection()?;

        // 异步启动 OCR 引擎
        self.start_ocr_engine_async(hwnd);

        Ok(())
    }

    /// 清理系统资源
    pub fn cleanup(&mut self) {
        self.tray.cleanup();
        self.hotkeys.cleanup();
        self.window_detection.stop_detection();
        self.stop_ocr_engine_sync();
    }

    /// 处理托盘消息
    pub fn handle_tray_message(&mut self, wparam: u32, lparam: u32) -> Vec<Command> {
        self.tray.handle_message(wparam, lparam)
    }

    // ========== OCR 引擎管理 ==========

    /// 异步启动 OCR 引擎
    pub fn start_ocr_engine_async(&self, hwnd: HWND) {
        let engine_arc = Arc::clone(&self.ocr_engine);
        let hwnd_ptr = hwnd.0 as usize;

        std::thread::spawn(move || {
            let success = Self::start_ocr_engine_sync_inner(&engine_arc);

            // 通知窗口引擎状态
            unsafe {
                use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_USER};
                let hwnd = HWND(hwnd_ptr as *mut std::ffi::c_void);
                let _ = PostMessageW(
                    Some(hwnd),
                    WM_USER + 10, // WM_OCR_STATUS_UPDATE
                    WPARAM(if success { 1 } else { 0 }),
                    LPARAM(0),
                );
            }
        });
    }

    /// 同步启动 OCR 引擎（内部实现）
    fn start_ocr_engine_sync_inner(engine_arc: &Arc<Mutex<Option<OcrEngine>>>) -> bool {
        let mut engine_guard = match engine_arc.lock() {
            Ok(guard) => guard,
            Err(_) => return false,
        };

        if engine_guard.is_some() {
            return true; // 已经启动
        }

        #[cfg(debug_assertions)]
        println!("正在启动 OCR 引擎...");

        #[cfg(debug_assertions)]
        let start_time = std::time::Instant::now();

        match ocr::create_engine() {
            Ok(engine) => {
                *engine_guard = Some(engine);
                #[cfg(debug_assertions)]
                println!("OCR 引擎启动成功，耗时: {:?}", start_time.elapsed());
                true
            }
            Err(e) => {
                eprintln!("OCR 引擎启动失败: {}", e);
                false
            }
        }
    }

    /// 同步停止 OCR 引擎
    fn stop_ocr_engine_sync(&self) {
        if let Ok(mut engine_guard) = self.ocr_engine.lock() {
            if let Some(engine) = engine_guard.take() {
                #[cfg(debug_assertions)]
                println!("正在停止 OCR 引擎...");
                drop(engine);
                #[cfg(debug_assertions)]
                println!("OCR 引擎已停止");
            }
        }
    }

    /// 异步停止 OCR 引擎
    pub fn stop_ocr_engine_async(&self) {
        let engine_arc = Arc::clone(&self.ocr_engine);
        std::thread::spawn(move || {
            if let Ok(mut engine_guard) = engine_arc.lock() {
                if let Some(engine) = engine_guard.take() {
                    drop(engine);
                }
            }
        });
    }

    /// 检查 OCR 引擎是否已准备就绪
    pub fn is_ocr_engine_ready(&self) -> bool {
        if let Ok(engine_guard) = self.ocr_engine.try_lock() {
            return engine_guard.is_some();
        }
        false
    }

    /// 检查 OCR 引擎是否可用（模型存在且引擎就绪）
    pub fn ocr_is_available(&self) -> bool {
        ocr::models_exist() && self.is_ocr_engine_ready()
    }

    /// 启动异步 OCR 引擎状态检查
    pub fn start_async_ocr_check(&self, hwnd: HWND) {
        let engine_arc = Arc::clone(&self.ocr_engine);
        let hwnd_ptr = hwnd.0 as usize;

        std::thread::spawn(move || {
            let models_exist = ocr::models_exist();
            let engine_ready = if let Ok(guard) = engine_arc.lock() {
                guard.is_some()
            } else {
                false
            };
            let available = models_exist && engine_ready;

            unsafe {
                use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_USER};
                let hwnd = HWND(hwnd_ptr as *mut std::ffi::c_void);
                let _ = PostMessageW(
                    Some(hwnd),
                    WM_USER + 10,
                    WPARAM(if available { 1 } else { 0 }),
                    LPARAM(0),
                );
            }
        });
    }

    /// 重新加载设置
    pub fn reload_settings(&mut self) {
        self.hotkeys.reload_settings();
        self.tray.reload_settings();
    }

    /// 重新注册热键
    pub fn reregister_hotkey(&mut self, hwnd: HWND) -> windows::core::Result<()> {
        self.hotkeys.reregister_hotkey(hwnd)
    }

    /// 更新 OCR 引擎状态后的回调处理
    pub fn on_ocr_engine_status_changed(&mut self, _available: bool, _hwnd: HWND) {
        // 可以在这里添加状态更新后的其他逻辑
    }

    /// 从选择区域识别文本
    pub fn recognize_text_from_selection(
        &self,
        selection_rect: RECT,
        hwnd: HWND,
        screenshot_manager: &mut ScreenshotManager,
    ) -> Result<(), SystemError> {
        // 检查 OCR 引擎是否可用
        if !self.ocr_is_available() {
            let message = "OCR引擎不可用。\n\n请确保 OCR 引擎正常运行。";
            let message_w: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();
            let title_w: Vec<u16> = "OCR错误".encode_utf16().chain(std::iter::once(0)).collect();

            unsafe {
                use windows::Win32::UI::WindowsAndMessaging::*;
                MessageBoxW(
                    Some(hwnd),
                    windows::core::PCWSTR(message_w.as_ptr()),
                    windows::core::PCWSTR(title_w.as_ptr()),
                    MB_OK | MB_ICONERROR,
                );
            }
            return Ok(());
        }

        // 获取缓存的图像数据
        let cached_image = screenshot_manager.get_current_image_data().map(|d| d.to_vec());

        // 隐藏窗口
        unsafe {
            use windows::Win32::UI::WindowsAndMessaging::*;
            let _ = ShowWindow(hwnd, SW_HIDE);
        }

        let hwnd_ptr = hwnd.0 as usize;
        let engine_arc = Arc::clone(&self.ocr_engine);

        // 异步执行 OCR
        std::thread::spawn(move || {
            use windows::Win32::UI::WindowsAndMessaging::*;
            let hwnd = HWND(hwnd_ptr as *mut std::ffi::c_void);

            // 检查选区大小
            let width = selection_rect.right - selection_rect.left;
            let height = selection_rect.bottom - selection_rect.top;
            if width <= 0 || height <= 0 {
                unsafe {
                    let _ = PostMessageW(Some(hwnd), WM_USER + 2, WPARAM(0), LPARAM(0));
                }
                return;
            }

            // 获取图像数据
            let Some(ref data) = cached_image else {
                eprintln!("没有缓存图像数据");
                unsafe {
                    let _ = PostMessageW(Some(hwnd), WM_USER + 2, WPARAM(0), LPARAM(0));
                }
                return;
            };

            // 裁剪图像
            let cropped = match crop_bmp(data, &selection_rect) {
                Ok(cropped) => cropped,
                Err(e) => {
                    eprintln!("裁剪图像失败: {:?}", e);
                    unsafe {
                        let _ = PostMessageW(Some(hwnd), WM_USER + 2, WPARAM(0), LPARAM(0));
                    }
                    return;
                }
            };

            // 执行 OCR 识别
            let line_results = {
                let engine_guard = match engine_arc.lock() {
                    Ok(guard) => guard,
                    Err(_) => {
                        eprintln!("OCR 引擎锁已中毒");
                        unsafe {
                            let _ = PostMessageW(Some(hwnd), WM_USER + 2, WPARAM(0), LPARAM(0));
                        }
                        return;
                    }
                };

                let Some(ref engine) = *engine_guard else {
                    eprintln!("OCR 引擎未就绪");
                    unsafe {
                        let _ = PostMessageW(Some(hwnd), WM_USER + 2, WPARAM(0), LPARAM(0));
                    }
                    return;
                };

                match ocr::recognize_text_by_lines(engine, &cropped, selection_rect) {
                    Ok(results) => results,
                    Err(_) => vec![OcrResult {
                        text: "OCR识别失败".to_string(),
                        confidence: 0.0,
                        bounding_box: BoundingBox {
                            x: 0,
                            y: 0,
                            width: 200,
                            height: 25,
                        },
                    }],
                }
            };

            // 构建完成数据包
            let completion_data = Box::new(crate::ocr::OcrCompletionData {
                image_data: cropped,
                ocr_results: line_results,
                selection_rect,
            });

            // 通知主线程显示结果
            unsafe {
                let _ = PostMessageW(
                    Some(hwnd),
                    WM_USER + 11, // WM_OCR_COMPLETED
                    WPARAM(0),
                    LPARAM(Box::into_raw(completion_data) as isize),
                );
            }
        });

        Ok(())
    }

    /// 获取 OCR 引擎 Arc 引用（用于外部调用）
    pub fn get_ocr_engine(&self) -> Arc<Mutex<Option<OcrEngine>>> {
        Arc::clone(&self.ocr_engine)
    }
}

/// 系统错误类型
#[derive(Debug)]
pub enum SystemError {
    /// 托盘错误
    TrayError(String),
    /// 热键错误
    HotkeyError(String),
    /// 窗口检测错误
    WindowDetectionError(String),
    /// OCR错误
    OcrError(String),
    /// 初始化错误
    InitError(String),
    /// 窗口枚举失败
    WindowEnumerationFailed,
}

impl std::fmt::Display for SystemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SystemError::TrayError(msg) => write!(f, "Tray error: {msg}"),
            SystemError::HotkeyError(msg) => write!(f, "Hotkey error: {msg}"),
            SystemError::WindowDetectionError(msg) => write!(f, "Window detection error: {msg}"),
            SystemError::OcrError(msg) => write!(f, "OCR error: {msg}"),
            SystemError::InitError(msg) => write!(f, "System init error: {msg}"),
            SystemError::WindowEnumerationFailed => write!(f, "Window enumeration failed"),
        }
    }
}

impl std::error::Error for SystemError {}

impl From<windows::core::Error> for SystemError {
    fn from(err: windows::core::Error) -> Self {
        SystemError::TrayError(format!("Windows API error: {err:?}"))
    }
}
