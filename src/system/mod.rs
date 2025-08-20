// 系统集成管理器模块
//
// 负责系统级功能：托盘、热键、窗口检测、OCR等

use crate::message::{Command, SystemMessage};

pub mod hotkeys;
pub mod ocr;
pub mod tray;
pub mod window_detection;

use hotkeys::HotkeyManager;
use ocr::OcrManager;
use tray::TrayManager;
use window_detection::WindowDetectionManager;

/// 系统管理器
pub struct SystemManager {
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
    pub fn new() -> Result<Self, SystemError> {
        Ok(Self {
            tray: TrayManager::new()?,
            hotkeys: HotkeyManager::new()?,
            window_detection: WindowDetectionManager::new()?,
            ocr: OcrManager::new()?,
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

        // 启动OCR引擎
        self.ocr.start_engine()?;

        Ok(())
    }

    /// 清理系统资源
    pub fn cleanup(&mut self) {
        self.tray.cleanup();
        self.hotkeys.cleanup();
        self.window_detection.stop_detection();
        self.ocr.stop_engine();
    }

    /// 处理托盘消息（从原始代码迁移）
    pub fn handle_tray_message(&mut self, wparam: u32, lparam: u32) -> Vec<Command> {
        self.tray.handle_message(wparam, lparam)
    }

    /// 启动异步OCR引擎状态检查（从原始代码迁移）
    pub fn start_async_ocr_check(&mut self, hwnd: windows::Win32::Foundation::HWND) {
        // 通过OCR管理器启动异步状态检查
        self.ocr.start_async_status_check(hwnd);
    }

    /// 异步停止OCR引擎（从原始代码迁移）
    pub fn stop_ocr_engine_async(&mut self) {
        self.ocr.stop_engine_async();
    }

    /// 重新加载设置（从原始代码迁移）
    pub fn reload_settings(&mut self) {
        // 重新加载热键设置
        self.hotkeys.reload_settings();
        // 重新加载托盘设置
        self.tray.reload_settings();
        // 重新加载OCR设置
        self.ocr.reload_settings();
    }

    /// 重新注册热键（从原始代码迁移）
    pub fn reregister_hotkey(
        &mut self,
        hwnd: windows::Win32::Foundation::HWND,
    ) -> windows::core::Result<()> {
        self.hotkeys.reregister_hotkey(hwnd)
    }

    /// 更新OCR引擎状态（从原始代码迁移）
    pub fn update_ocr_engine_status(
        &mut self,
        available: bool,
        hwnd: windows::Win32::Foundation::HWND,
    ) {
        self.ocr.update_status(available);
        // 可以在这里添加状态更新后的其他逻辑
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
            SystemError::TrayError(msg) => write!(f, "Tray error: {}", msg),
            SystemError::HotkeyError(msg) => write!(f, "Hotkey error: {}", msg),
            SystemError::WindowDetectionError(msg) => write!(f, "Window detection error: {}", msg),
            SystemError::OcrError(msg) => write!(f, "OCR error: {}", msg),
            SystemError::InitError(msg) => write!(f, "System init error: {}", msg),
            SystemError::WindowEnumerationFailed => write!(f, "Window enumeration failed"),
        }
    }
}

impl std::error::Error for SystemError {}

impl From<windows::core::Error> for SystemError {
    fn from(err: windows::core::Error) -> Self {
        SystemError::TrayError(format!("Windows API error: {:?}", err))
    }
}
