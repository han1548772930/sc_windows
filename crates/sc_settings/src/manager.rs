use std::sync::{Arc, RwLock};

use crate::Settings;

/// Unified config manager.
pub struct ConfigManager {
    settings: Arc<RwLock<Settings>>,
}

impl ConfigManager {
    /// Create a new config manager (loads settings once and caches them).
    pub fn new() -> Self {
        Self {
            settings: Arc::new(RwLock::new(Settings::load())),
        }
    }

    /// Get a snapshot copy of current settings.
    pub fn get(&self) -> Settings {
        self.settings
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_else(|_| Settings::default())
    }

    /// Get the shared settings reference.
    pub fn get_shared(&self) -> Arc<RwLock<Settings>> {
        Arc::clone(&self.settings)
    }

    /// Reload settings from disk.
    pub fn reload(&mut self) {
        let new_settings = Settings::load();
        if let Ok(mut guard) = self.settings.write() {
            *guard = new_settings;
        }
    }

    // Convenience accessors.

    #[inline]
    pub fn line_thickness(&self) -> f32 {
        self.get().line_thickness
    }

    #[inline]
    pub fn font_size(&self) -> f32 {
        self.get().font_size
    }

    #[inline]
    pub fn font_name(&self) -> String {
        self.get().font_name
    }

    #[inline]
    pub fn font_weight(&self) -> i32 {
        self.get().font_weight
    }

    #[inline]
    pub fn font_italic(&self) -> bool {
        self.get().font_italic
    }

    #[inline]
    pub fn font_underline(&self) -> bool {
        self.get().font_underline
    }

    #[inline]
    pub fn font_strikeout(&self) -> bool {
        self.get().font_strikeout
    }

    #[inline]
    pub fn font_color(&self) -> (u8, u8, u8) {
        self.get().font_color
    }

    #[inline]
    pub fn drawing_color(&self) -> (u8, u8, u8) {
        let s = self.get();
        (
            s.drawing_color_red,
            s.drawing_color_green,
            s.drawing_color_blue,
        )
    }

    #[inline]
    pub fn text_color(&self) -> (u8, u8, u8) {
        let s = self.get();
        (s.text_color_red, s.text_color_green, s.text_color_blue)
    }

    #[inline]
    pub fn hotkey(&self) -> (u32, u32) {
        let s = self.get();
        (s.hotkey_modifiers, s.hotkey_key)
    }

    #[inline]
    pub fn auto_copy(&self) -> bool {
        self.get().auto_copy
    }

    #[inline]
    pub fn delay_ms(&self) -> u32 {
        self.get().delay_ms
    }

    #[inline]
    pub fn ocr_language(&self) -> String {
        self.get().ocr_language
    }

    #[inline]
    pub fn config_path(&self) -> String {
        self.get().config_path
    }
}

impl Default for ConfigManager {
    fn default() -> Self {
        Self::new()
    }
}
