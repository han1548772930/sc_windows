//! ConfigManager - 统一配置管理器
//!
//! 提供配置的集中管理，避免多处重复加载设置文件。
//!
//! # 使用方式
//! ```ignore
//! use crate::config::ConfigManager;
//!
//! // 获取配置（自动使用全局单例）
//! let thickness = ConfigManager::global().line_thickness();
//!
//! // 重新加载配置
//! ConfigManager::reload_global();
//! ```

use crate::settings::Settings;
use std::sync::{Arc, RwLock};

/// 配置变更回调类型
pub type ConfigWatcher = Box<dyn Fn(&Settings) + Send + Sync>;

/// 统一配置管理器
///
/// 提供以下功能:
/// - 缓存配置，避免重复从文件加载
/// - 提供配置变更通知机制
/// - 线程安全的配置访问
pub struct ConfigManager {
    /// 缓存的设置
    settings: Arc<RwLock<Settings>>,
    /// 设置变更监听器
    watchers: Vec<ConfigWatcher>,
}

impl ConfigManager {
    /// 创建新的配置管理器
    ///
    /// 初始化时从文件加载设置并缓存
    pub fn new() -> Self {
        Self {
            settings: Arc::new(RwLock::new(Settings::load())),
            watchers: Vec::new(),
        }
    }

    /// 获取当前设置的只读引用
    ///
    /// # 返回值
    /// 返回设置的克隆副本，避免长时间持有锁
    pub fn get(&self) -> Settings {
        self.settings
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_else(|_| Settings::default())
    }

    /// 获取设置的 Arc 引用（用于需要共享的场景）
    pub fn get_shared(&self) -> Arc<RwLock<Settings>> {
        Arc::clone(&self.settings)
    }

    /// 注册配置变更监听器
    ///
    /// 当配置重新加载时，所有注册的监听器都会被调用
    pub fn watch<F>(&mut self, callback: F)
    where
        F: Fn(&Settings) + Send + Sync + 'static,
    {
        self.watchers.push(Box::new(callback));
    }

    /// 重新加载配置
    ///
    /// 从文件重新加载设置，并通知所有监听器
    pub fn reload(&mut self) {
        let new_settings = Settings::load();

        // 更新缓存
        if let Ok(mut guard) = self.settings.write() {
            *guard = new_settings.clone();
        }

        // 通知所有监听器
        for watcher in &self.watchers {
            watcher(&new_settings);
        }
    }

    /// 更新并保存设置
    ///
    /// 更新内存中的设置并持久化到文件，然后通知监听器
    pub fn update(&mut self, updater: impl FnOnce(&mut Settings)) -> windows::core::Result<()> {
        let new_settings = {
            let mut guard = self
                .settings
                .write()
                .map_err(|_| windows::core::Error::from(windows::Win32::Foundation::E_FAIL))?;
            updater(&mut guard);
            guard.save()?;
            guard.clone()
        };

        // 通知所有监听器
        for watcher in &self.watchers {
            watcher(&new_settings);
        }

        Ok(())
    }

    // ========== 便捷访问方法 ==========

    /// 获取线条粗细
    #[inline]
    pub fn line_thickness(&self) -> f32 {
        self.get().line_thickness
    }

    /// 获取字体大小
    #[inline]
    pub fn font_size(&self) -> f32 {
        self.get().font_size
    }

    /// 获取字体名称
    #[inline]
    pub fn font_name(&self) -> String {
        self.get().font_name
    }

    /// 获取字体粗细
    #[inline]
    pub fn font_weight(&self) -> i32 {
        self.get().font_weight
    }

    /// 获取字体是否斜体
    #[inline]
    pub fn font_italic(&self) -> bool {
        self.get().font_italic
    }

    /// 获取字体是否有下划线
    #[inline]
    pub fn font_underline(&self) -> bool {
        self.get().font_underline
    }

    /// 获取字体是否有删除线
    #[inline]
    pub fn font_strikeout(&self) -> bool {
        self.get().font_strikeout
    }

    /// 获取字体颜色
    #[inline]
    pub fn font_color(&self) -> (u8, u8, u8) {
        self.get().font_color
    }

    /// 获取绘图颜色（RGB）
    #[inline]
    pub fn drawing_color(&self) -> (u8, u8, u8) {
        let s = self.get();
        (s.drawing_color_red, s.drawing_color_green, s.drawing_color_blue)
    }

    /// 获取绘图颜色（D2D格式）
    #[inline]
    pub fn drawing_color_d2d(&self) -> windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F {
        let (r, g, b) = self.drawing_color();
        windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: 1.0,
        }
    }

    /// 获取文本颜色（RGB）
    #[inline]
    pub fn text_color(&self) -> (u8, u8, u8) {
        let s = self.get();
        (s.text_color_red, s.text_color_green, s.text_color_blue)
    }

    /// 获取热键配置
    #[inline]
    pub fn hotkey(&self) -> (u32, u32) {
        let s = self.get();
        (s.hotkey_modifiers, s.hotkey_key)
    }

    /// 获取自动复制设置
    #[inline]
    pub fn auto_copy(&self) -> bool {
        self.get().auto_copy
    }

    /// 获取延迟时间(毫秒)
    #[inline]
    pub fn delay_ms(&self) -> u32 {
        self.get().delay_ms
    }

    /// 获取OCR语言
    #[inline]
    pub fn ocr_language(&self) -> String {
        self.get().ocr_language
    }
}

impl Default for ConfigManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_manager_new() {
        let manager = ConfigManager::new();
        let settings = manager.get();
        // 验证默认值
        assert!(settings.line_thickness > 0.0);
        assert!(settings.font_size > 0.0);
    }

    #[test]
    fn test_config_manager_convenience_methods() {
        let manager = ConfigManager::new();
        
        // 测试便捷方法
        assert!(manager.line_thickness() > 0.0);
        assert!(manager.font_size() > 0.0);
        assert!(!manager.font_name().is_empty());
    }

    #[test]
    fn test_config_manager_reload() {
        let mut manager = ConfigManager::new();
        let settings_before = manager.get();
        
        // 重新加载
        manager.reload();
        let settings_after = manager.get();
        
        // 设置应该保持一致（因为没有修改文件）
        assert_eq!(settings_before.line_thickness, settings_after.line_thickness);
    }
}
