// 全局热键管理
//
// 负责全局热键的注册、注销和处理

use super::SystemError;
use crate::message::Command;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    HOT_KEY_MODIFIERS, RegisterHotKey, UnregisterHotKey,
};

/// 热键管理器
pub struct HotkeyManager {
    /// 已注册的热键
    registered_hotkeys: Vec<u32>,
}

impl HotkeyManager {
    /// 创建新的热键管理器
    pub fn new() -> Result<Self, SystemError> {
        Ok(Self {
            registered_hotkeys: Vec::new(),
        })
    }

    /// 注册全局热键（从原始代码迁移）
    pub fn register_hotkeys(&mut self, hwnd: HWND) -> Result<(), SystemError> {
        // 从设置中读取热键配置
        let settings = crate::settings::Settings::load();
        let hotkey_id = 1001;

        // 注册全局热键
        unsafe {
            let result = RegisterHotKey(
                Some(hwnd),
                hotkey_id,
                HOT_KEY_MODIFIERS(settings.hotkey_modifiers),
                settings.hotkey_key,
            );

            if result.is_ok() {
                self.registered_hotkeys.push(hotkey_id as u32);
                Ok(())
            } else {
                Err(SystemError::HotkeyError(
                    "Failed to register hotkey".to_string(),
                ))
            }
        }
    }

    /// 处理热键触发（从原始代码迁移）
    pub fn handle_hotkey_triggered(&mut self) -> Vec<Command> {
        // 热键触发时执行截图（按照原始代码逻辑）
        vec![Command::TakeScreenshot]
    }

    /// 处理键盘输入（检查是否为快捷键）
    pub fn handle_key_input(&mut self, _key: u32) -> Vec<Command> {
        // 热键管理器不处理普通键盘输入，只处理全局热键
        vec![]
    }

    /// 重新加载设置（从原始代码迁移）
    pub fn reload_settings(&mut self) {
        // 热键设置重新加载通常需要重新注册热键
        // 这里只是标记需要重新注册，实际重新注册在reregister_hotkey中进行
    }

    /// 重新注册热键（从原始代码迁移）
    pub fn reregister_hotkey(&mut self, hwnd: HWND) -> windows::core::Result<()> {
        // 先注销现有热键
        self.cleanup();

        // 重新注册热键
        match self.register_hotkeys(hwnd) {
            Ok(()) => Ok(()),
            Err(_) => Err(windows::core::Error::from_win32()),
        }
    }

    /// 清理热键（从原始代码迁移）
    pub fn cleanup(&mut self) {
        // 注销所有注册的热键
        unsafe {
            for &hotkey_id in &self.registered_hotkeys {
                let _ = UnregisterHotKey(None, hotkey_id as i32);
            }
        }
        self.registered_hotkeys.clear();
    }
}
