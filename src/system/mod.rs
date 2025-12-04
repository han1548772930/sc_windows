//! 系统集成模块
//!
//! 提供与 Windows 系统的集成功能。
//!
//! # 主要组件
//! - [`SystemManager`]: 系统管理器，统一管理系统集成
//! - [`TrayManager`](tray::TrayManager): 系统托盘管理
//! - [`HotkeyManager`](hotkeys::HotkeyManager): 全局热键管理
//! - [`OcrManager`](ocr::OcrManager): OCR 文字识别
//! - [`WindowDetectionManager`](window_detection::WindowDetectionManager): 窗口检测

use crate::message::{Command, SystemMessage};
use crate::settings::Settings;
use std::sync::{Arc, RwLock};

pub mod hotkeys;
pub mod tray;
pub mod window_detection;

use hotkeys::HotkeyManager;
use crate::ocr::OcrManager;
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
    /// OCR管理器
    ocr: OcrManager,
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
            ocr: OcrManager::new()?,
            settings,
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
            SystemMessage::OcrStatusUpdate(available) => {
                self.ocr.update_status(available);
                vec![Command::RequestRedraw]
            }
        }
    }

    /// 处理键盘输入（全局快捷键）
    pub fn handle_key_input(&mut self, key: u32) -> Vec<Command> {
        self.hotkeys.handle_key_input(key)
    }

    /// 初始化系统集成
    pub fn initialize(
        &mut self,
        hwnd: windows::Win32::Foundation::HWND,
    ) -> Result<(), SystemError> {
        // 初始化系统托盘
        self.tray.initialize(hwnd)?;

        // 注册全局热键
        self.hotkeys.register_hotkeys(hwnd)?;

        // 启动窗口检测
        self.window_detection.start_detection()?;

        // 确保OCR引擎已启动
        self.ocr.ensure_engine_started()?;

        Ok(())
    }

    /// 清理系统资源
    pub fn cleanup(&mut self) {
        self.tray.cleanup();
        self.hotkeys.cleanup();
        self.window_detection.stop_detection();
        self.ocr.stop_engine();
    }

    /// 处理托盘消息
    pub fn handle_tray_message(&mut self, wparam: u32, lparam: u32) -> Vec<Command> {
        self.tray.handle_message(wparam, lparam)
    }

    /// 启动异步OCR引擎状态检查
    pub fn start_async_ocr_check(&mut self, hwnd: windows::Win32::Foundation::HWND) {
        // 通过OCR管理器启动异步状态检查
        self.ocr.start_async_status_check(hwnd);
    }

    /// 异步停止OCR引擎
    pub fn stop_ocr_engine_async(&mut self) {
        self.ocr.stop_engine();
    }

    /// 重新加载设置
    pub fn reload_settings(&mut self) {
        // 重新加载热键设置
        self.hotkeys.reload_settings();
        // 重新加载托盘设置
        self.tray.reload_settings();
        // 重新加载OCR设置
        self.ocr.reload_settings();
    }

    /// 重新注册热键
    pub fn reregister_hotkey(
        &mut self,
        hwnd: windows::Win32::Foundation::HWND,
    ) -> windows::core::Result<()> {
        self.hotkeys.reregister_hotkey(hwnd)
    }

    /// 更新OCR引擎状态
    pub fn update_ocr_engine_status(
        &mut self,
        available: bool,
        _hwnd: windows::Win32::Foundation::HWND,
    ) {
        self.ocr.update_status(available);
        // 可以在这里添加状态更新后的其他逻辑
    }

    /// 查询OCR引擎可用性缓存（供UI非阻塞使用）
    pub fn ocr_is_available(&self) -> bool {
        self.ocr.is_available()
    }

    /// 从选择区域识别文本（委托给OcrManager）
    pub fn recognize_text_from_selection(
        &mut self,
        selection_rect: windows::Win32::Foundation::RECT,
        hwnd: windows::Win32::Foundation::HWND,
        screenshot_manager: &mut crate::screenshot::ScreenshotManager,
    ) -> Result<(), SystemError> {
        self.ocr
            .recognize_text_from_selection(selection_rect, hwnd, screenshot_manager)
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
