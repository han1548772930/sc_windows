use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::defaults::*;

/// Application settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    // Basic settings
    pub line_thickness: f32,
    pub font_size: f32,
    pub auto_copy: bool,
    pub show_cursor: bool,
    pub delay_ms: u32,

    // Font settings
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

    /// Output directory (e.g. where screenshots are saved).
    #[serde(default = "default_config_path")]
    pub config_path: String,

    // OCR language
    #[serde(default = "default_ocr_language")]
    pub ocr_language: String,

    // Drawing color
    #[serde(default = "default_drawing_color_red")]
    pub drawing_color_red: u8,
    #[serde(default = "default_drawing_color_green")]
    pub drawing_color_green: u8,
    #[serde(default = "default_drawing_color_blue")]
    pub drawing_color_blue: u8,

    // Text color
    #[serde(default = "default_text_color_red")]
    pub text_color_red: u8,
    #[serde(default = "default_text_color_green")]
    pub text_color_green: u8,
    #[serde(default = "default_text_color_blue")]
    pub text_color_blue: u8,

    // Hotkey settings (Win32-style modifier bitmask + virtual key)
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

            hotkey_modifiers: default_hotkey_modifiers(),
            hotkey_key: default_hotkey_key(),

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
    fn settings_dir() -> PathBuf {
        PathBuf::from(default_config_path()).join(".ocr_screenshot_tool")
    }

    fn primary_settings_path() -> PathBuf {
        Self::settings_dir().join("simple_settings.json")
    }

    fn legacy_settings_paths() -> Vec<PathBuf> {
        // Historical behavior sometimes wrote settings to USERPROFILE\simple_settings.json.
        let mut paths = Vec::new();
        if let Ok(user_profile) = std::env::var("USERPROFILE") {
            paths.push(PathBuf::from(user_profile).join("simple_settings.json"));
        }
        paths
    }

    /// Load settings from disk.
    ///
    /// Falls back to defaults if loading fails.
    pub fn load() -> Self {
        let primary = Self::primary_settings_path();

        // 1) Try primary path.
        if let Ok(content) = fs::read_to_string(&primary)
            && let Ok(settings) = serde_json::from_str::<Settings>(&content)
        {
            return settings;
        }

        // 2) Try legacy paths.
        for legacy in Self::legacy_settings_paths() {
            if let Ok(content) = fs::read_to_string(&legacy)
                && let Ok(settings) = serde_json::from_str::<Settings>(&content)
            {
                // Best-effort migration: persist into the primary path.
                let _ = settings.save();
                return settings;
            }
        }

        // 3) Default + persist.
        let default_settings = Self::default();
        let _ = default_settings.save();
        default_settings
    }

    /// Save settings to disk.
    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::primary_settings_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        fs::write(&path, content)?;
        Ok(())
    }

    /// Get hotkey display string (e.g. "Ctrl+Alt+S").
    pub fn get_hotkey_string(&self) -> String {
        // Win32 modifier bitmask.
        const MOD_ALT: u32 = 0x0001;
        const MOD_CONTROL: u32 = 0x0002;
        const MOD_SHIFT: u32 = 0x0004;

        let mut parts = Vec::new();

        if self.hotkey_modifiers & MOD_CONTROL != 0 {
            parts.push("Ctrl");
        }
        if self.hotkey_modifiers & MOD_ALT != 0 {
            parts.push("Alt");
        }
        if self.hotkey_modifiers & MOD_SHIFT != 0 {
            parts.push("Shift");
        }

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

    /// Parse a hotkey string (e.g. "Ctrl+Alt+S") into fields.
    ///
    /// Returns `true` if parsing succeeded.
    pub fn parse_hotkey_string(&mut self, hotkey_str: &str) -> bool {
        // Win32 modifier bitmask.
        const MOD_ALT: u32 = 0x0001;
        const MOD_CONTROL: u32 = 0x0002;
        const MOD_SHIFT: u32 = 0x0004;

        let parts: Vec<&str> = hotkey_str.split('+').map(|s| s.trim()).collect();
        if parts.is_empty() {
            return false;
        }

        let mut modifiers = 0u32;
        let mut key = 0u32;

        for part in &parts {
            match part.to_lowercase().as_str() {
                "ctrl" | "control" => modifiers |= MOD_CONTROL,
                "alt" => modifiers |= MOD_ALT,
                "shift" => modifiers |= MOD_SHIFT,
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
