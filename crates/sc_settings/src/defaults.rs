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
    // Win32-style modifier bitmask (MOD_ALT=0x0001, MOD_CONTROL=0x0002).
    0x0001 | 0x0002
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
    (0, 0, 0) // black
}

// Output/config path default
pub fn default_config_path() -> String {
    // Prefer a user home directory.
    if let Ok(home_dir) = std::env::var("USERPROFILE") {
        return home_dir;
    }
    if let Ok(home_dir) = std::env::var("HOME") {
        return home_dir;
    }

    // Fallback: program directory.
    if let Ok(exe_path) = std::env::current_exe()
        && let Some(exe_dir) = exe_path.parent()
    {
        return exe_dir.to_string_lossy().to_string();
    }

    // Last resort: cwd.
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .to_string_lossy()
        .to_string()
}

// OCR language default
pub fn default_ocr_language() -> String {
    "chinese".to_string()
}
