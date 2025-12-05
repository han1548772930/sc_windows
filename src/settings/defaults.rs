//! Settings default value functions for serde
//!
//! This module contains all the default value functions used by serde
//! for deserializing Settings fields.

use std::path::PathBuf;

// Drawing color defaults
pub fn default_drawing_color_red() -> u8 {
    255
}
pub fn default_drawing_color_green() -> u8 {
    0
}
pub fn default_drawing_color_blue() -> u8 {
    0
}

// Text color defaults
pub fn default_text_color_red() -> u8 {
    255
}
pub fn default_text_color_green() -> u8 {
    255
}
pub fn default_text_color_blue() -> u8 {
    255
}

// Hotkey defaults
pub fn default_hotkey_modifiers() -> u32 {
    use windows::Win32::UI::Input::KeyboardAndMouse::*;
    (MOD_CONTROL | MOD_ALT).0
}
pub fn default_hotkey_key() -> u32 {
    'S' as u32
}

// Font defaults
pub fn default_font_name() -> String {
    "Microsoft Sans Serif".to_string()
}
pub fn default_font_weight() -> i32 {
    400 // FW_NORMAL
}
pub fn default_font_italic() -> bool {
    false
}
pub fn default_font_underline() -> bool {
    false
}
pub fn default_font_strikeout() -> bool {
    false
}
pub fn default_font_color() -> (u8, u8, u8) {
    (0, 0, 0) // 黑色
}

// Config path default
pub fn default_config_path() -> String {
    // 优先使用用户主目录
    if let Ok(home_dir) = std::env::var("USERPROFILE") {
        return home_dir;
    }

    // 备用方案：获取当前程序所在目录
    if let Ok(exe_path) = std::env::current_exe()
        && let Some(exe_dir) = exe_path.parent()
    {
        return exe_dir.to_string_lossy().to_string();
    }

    // 最后的备用路径：当前工作目录
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .to_string_lossy()
        .to_string()
}

// OCR language default
pub fn default_ocr_language() -> String {
    "chinese".to_string() // 默认使用简体中文
}
