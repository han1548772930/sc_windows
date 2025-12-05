//! Core settings data structure and persistence
//!
//! This module contains the Settings struct definition,
//! serialization/deserialization, and core methods.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::fs;
use windows::core::*;
use windows::Win32::Foundation::E_FAIL;

use super::defaults::*;

/// 应用程序设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    // 基础设置
    pub line_thickness: f32,
    pub font_size: f32,
    pub auto_copy: bool,
    pub show_cursor: bool,
    pub delay_ms: u32,

    // 字体设置
    #[serde(default = "default_font_name")]
    pub font_name: String,
    #[serde(default = "default_font_weight")]
    pub font_weight: i32,
    #[serde(default = "default_font_italic")]
    pub font_italic: bool,
    #[serde(default = "default_font_underline")]
    pub font_underline: bool,
    #[serde(default = "default_font_strikeout")]
    pub font_strikeout: bool,
    #[serde(default = "default_font_color")]
    pub font_color: (u8, u8, u8),
    // 配置文件路径
    #[serde(default = "default_config_path")]
    pub config_path: String,

    // OCR语言设置
    #[serde(default = "default_ocr_language")]
    pub ocr_language: String,

    // 绘图颜色设置
    #[serde(default = "default_drawing_color_red")]
    pub drawing_color_red: u8,
    #[serde(default = "default_drawing_color_green")]
    pub drawing_color_green: u8,
    #[serde(default = "default_drawing_color_blue")]
    pub drawing_color_blue: u8,

    #[serde(default = "default_text_color_red")]
    pub text_color_red: u8,
    #[serde(default = "default_text_color_green")]
    pub text_color_green: u8,
    #[serde(default = "default_text_color_blue")]
    pub text_color_blue: u8,

    // 热键设置
    #[serde(default = "default_hotkey_modifiers")]
    pub hotkey_modifiers: u32,
    #[serde(default = "default_hotkey_key")]
    pub hotkey_key: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            line_thickness: 3.0,
            font_size: 20.0,
            auto_copy: false,
            show_cursor: false,
            delay_ms: 0,
            drawing_color_red: 255,
            drawing_color_green: 0,
            drawing_color_blue: 0,
            text_color_red: 255,
            text_color_green: 255,
            text_color_blue: 255,
            hotkey_modifiers: {
                use windows::Win32::UI::Input::KeyboardAndMouse::*;
                (MOD_CONTROL | MOD_ALT).0
            },
            hotkey_key: 'S' as u32,
            font_name: default_font_name(),
            font_weight: default_font_weight(),
            font_italic: default_font_italic(),
            font_underline: default_font_underline(),
            font_strikeout: default_font_strikeout(),
            font_color: default_font_color(),
            config_path: default_config_path(),
            ocr_language: default_ocr_language(),
        }
    }
}

impl Settings {
    /// 获取设置文件路径
    fn get_settings_path() -> PathBuf {
        // 优先使用用户配置目录（USERPROFILE）
        let default_path = if let Ok(user_profile) = std::env::var("USERPROFILE") {
            let mut path = PathBuf::from(user_profile);
            path.push(".ocr_screenshot_tool");
            // 确保目录存在
            if std::fs::create_dir_all(&path).is_err() {
                // 如果创建失败，回退到程序目录
                let mut fallback_path = std::env::current_exe().unwrap_or_default();
                fallback_path.set_file_name("simple_settings.json");
                return fallback_path;
            }
            path.push("simple_settings.json");
            path
        } else {
            // 如果无法获取USERPROFILE，使用程序目录
            let mut path = std::env::current_exe().unwrap_or_default();
            path.set_file_name("simple_settings.json");
            path
        };

        // 如果配置文件存在，尝试读取其中的config_path设置
        if let Ok(content) = fs::read_to_string(&default_path)
            && let Ok(settings) = serde_json::from_str::<Settings>(&content)
            && !settings.config_path.is_empty()
        {
            let mut custom_path = PathBuf::from(&settings.config_path);
            custom_path.push("simple_settings.json");
            return custom_path;
        }

        // 如果无法读取或路径为空，使用默认路径
        default_path
    }

    /// 从文件加载设置
    pub fn load() -> Self {
        let path = Self::get_settings_path();

        if let Ok(content) = fs::read_to_string(&path)
            && let Ok(settings) = serde_json::from_str::<Settings>(&content)
        {
            return settings;
        }

        // 如果加载失败，返回默认设置并保存
        let default_settings = Self::default();
        let _ = default_settings.save();
        default_settings
    }

    /// 保存设置到文件
    pub fn save(&self) -> Result<()> {
        let path = Self::get_settings_path();
        let content = serde_json::to_string_pretty(self).map_err(|_| Error::from(E_FAIL))?;

        fs::write(&path, content).map_err(|_| Error::from(E_FAIL))?;

        Ok(())
    }

    /// 获取热键的字符串表示
    pub fn get_hotkey_string(&self) -> String {
        use windows::Win32::UI::Input::KeyboardAndMouse::*;

        let mut parts = Vec::new();

        if self.hotkey_modifiers & MOD_CONTROL.0 != 0 {
            parts.push("Ctrl");
        }
        if self.hotkey_modifiers & MOD_ALT.0 != 0 {
            parts.push("Alt");
        }
        if self.hotkey_modifiers & MOD_SHIFT.0 != 0 {
            parts.push("Shift");
        }

        // 将虚拟键码转换为字符
        let key_char = match self.hotkey_key {
            key if key >= 'A' as u32 && key <= 'Z' as u32 => {
                char::from_u32(key).unwrap_or('?').to_string()
            }
            key if key >= '0' as u32 && key <= '9' as u32 => {
                char::from_u32(key).unwrap_or('?').to_string()
            }
            _ => format!("Key{}", self.hotkey_key),
        };

        parts.push(&key_char);
        parts.join("+")
    }

    /// 从热键字符串解析设置
    pub fn parse_hotkey_string(&mut self, hotkey_str: &str) -> bool {
        use windows::Win32::UI::Input::KeyboardAndMouse::*;

        let parts: Vec<&str> = hotkey_str.split('+').map(|s| s.trim()).collect();
        if parts.is_empty() {
            return false;
        }

        let mut modifiers = 0u32;
        let mut key = 0u32;

        for part in &parts {
            match part.to_lowercase().as_str() {
                "ctrl" | "control" => modifiers |= MOD_CONTROL.0,
                "alt" => modifiers |= MOD_ALT.0,
                "shift" => modifiers |= MOD_SHIFT.0,
                key_str if key_str.len() == 1 => {
                    if let Some(ch) = key_str.chars().next() {
                        let ch = ch.to_ascii_uppercase();
                        if ch.is_ascii_alphanumeric() {
                            key = ch as u32;
                        }
                    }
                }
                _ => return false,
            }
        }

        if key == 0 || modifiers == 0 {
            return false;
        }

        self.hotkey_modifiers = modifiers;
        self.hotkey_key = key;
        true
    }
}
