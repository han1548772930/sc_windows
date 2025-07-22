use native_windows_derive as nwd;
use native_windows_gui as nwg;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// 现代化应用程序设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModernSettings {
    // 基础设置
    pub line_thickness: f32,
    pub font_size: f32,
    pub auto_copy: bool,
    pub show_cursor: bool,
    pub delay_ms: u32,

    // 颜色设置
    pub color_red: u8,
    pub color_green: u8,
    pub color_blue: u8,

    // 界面设置
    pub toolbar_opacity: f32,
    pub border_width: u32,

    // 文件设置
    pub save_format: String,
    pub jpeg_quality: u32,
}

impl Default for ModernSettings {
    fn default() -> Self {
        Self {
            line_thickness: 3.0,
            font_size: 20.0,
            auto_copy: false,
            show_cursor: false,
            delay_ms: 0,
            color_red: 255,
            color_green: 0,
            color_blue: 0,
            toolbar_opacity: 0.9,
            border_width: 2,
            save_format: "PNG".to_string(),
            jpeg_quality: 90,
        }
    }
}

impl ModernSettings {
    /// 获取设置文件路径
    fn get_settings_path() -> PathBuf {
        let mut path = std::env::current_exe().unwrap_or_default();
        path.set_file_name("modern_settings.json");
        path
    }

    /// 从文件加载设置
    pub fn load() -> Self {
        let path = Self::get_settings_path();

        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(settings) = serde_json::from_str::<ModernSettings>(&content) {
                return settings;
            }
        }

        // 如果加载失败，返回默认设置并保存
        let default_settings = Self::default();
        let _ = default_settings.save();
        default_settings
    }

    /// 保存设置到文件
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::get_settings_path();
        let content = serde_json::to_string_pretty(self)?;
        fs::write(&path, content)?;
        Ok(())
    }
}

/// 现代化设置窗口
#[derive(Default, nwd::NwgUi)]
pub struct ModernSettingsApp {
    settings: ModernSettings,

    #[nwg_control(size: (600, 500), position: (300, 300), title: "🎨 截图工具 - 设置", flags: "WINDOW|VISIBLE")]
    #[nwg_events( OnWindowClose: [ModernSettingsApp::close] )]
    window: nwg::Window,

    #[nwg_layout(parent: window, spacing: 1)]
    grid: nwg::GridLayout,

    // 标题
    #[nwg_control(text: "🎨 截图工具设置", h_align: nwg::HTextAlign::Center)]
    #[nwg_layout_item(layout: grid, row: 0, col: 0, col_span: 4)]
    title_label: nwg::Label,

    // 绘图设置标题
    #[nwg_control(text: "🖌️ 绘图工具", h_align: nwg::HTextAlign::Left)]
    #[nwg_layout_item(layout: grid, row: 1, col: 0, col_span: 2)]
    drawing_title: nwg::Label,

    #[nwg_control(text: "线条粗细 (1-20):")]
    #[nwg_layout_item(layout: grid, row: 2, col: 0)]
    thickness_label: nwg::Label,

    #[nwg_control(text: "3")]
    #[nwg_layout_item(layout: grid, row: 2, col: 1)]
    thickness_input: nwg::TextInput,

    #[nwg_control(text: "字体大小 (8-72):")]
    #[nwg_layout_item(layout: grid, row: 3, col: 0)]
    font_label: nwg::Label,

    #[nwg_control(text: "20")]
    #[nwg_layout_item(layout: grid, row: 3, col: 1)]
    font_input: nwg::TextInput,

    // 颜色设置标题
    #[nwg_control(text: "🎨 默认颜色", h_align: nwg::HTextAlign::Left)]
    #[nwg_layout_item(layout: grid, row: 1, col: 2, col_span: 2)]
    color_title: nwg::Label,

    #[nwg_control(text: "红色 (0-255):")]
    #[nwg_layout_item(layout: grid, row: 2, col: 2)]
    red_label: nwg::Label,

    #[nwg_control(text: "255")]
    #[nwg_layout_item(layout: grid, row: 2, col: 3)]
    red_input: nwg::TextInput,

    #[nwg_control(text: "绿色 (0-255):")]
    #[nwg_layout_item(layout: grid, row: 3, col: 2)]
    green_label: nwg::Label,

    #[nwg_control(text: "0")]
    #[nwg_layout_item(layout: grid, row: 3, col: 3)]
    green_input: nwg::TextInput,

    #[nwg_control(text: "蓝色 (0-255):")]
    #[nwg_layout_item(layout: grid, row: 4, col: 2)]
    blue_label: nwg::Label,

    #[nwg_control(text: "0")]
    #[nwg_layout_item(layout: grid, row: 4, col: 3)]
    blue_input: nwg::TextInput,

    // 截图设置分组
    #[nwg_control(text: "📷 截图选项")]
    #[nwg_layout_item(layout: grid, row: 5, col: 0, col_span: 4)]
    screenshot_group: nwg::GroupBox,

    #[nwg_control(text: "自动复制到剪贴板", parent: screenshot_group)]
    #[nwg_layout_item(layout: grid, row: 6, col: 0, col_span: 2)]
    auto_copy_check: nwg::CheckBox,

    #[nwg_control(text: "截图时显示光标", parent: screenshot_group)]
    #[nwg_layout_item(layout: grid, row: 6, col: 2, col_span: 2)]
    show_cursor_check: nwg::CheckBox,

    #[nwg_control(text: "截图延迟 (毫秒):", parent: screenshot_group)]
    #[nwg_layout_item(layout: grid, row: 7, col: 0)]
    delay_label: nwg::Label,

    #[nwg_control(text: "0", parent: screenshot_group)]
    #[nwg_layout_item(layout: grid, row: 7, col: 1)]
    delay_input: nwg::TextInput,

    // 界面设置分组
    #[nwg_control(text: "🎛️ 界面设置")]
    #[nwg_layout_item(layout: grid, row: 8, col: 0, col_span: 4)]
    ui_group: nwg::GroupBox,

    #[nwg_control(text: "工具栏透明度 (0.1-1.0):", parent: ui_group)]
    #[nwg_layout_item(layout: grid, row: 9, col: 0)]
    opacity_label: nwg::Label,

    #[nwg_control(text: "0.9", parent: ui_group)]
    #[nwg_layout_item(layout: grid, row: 9, col: 1)]
    opacity_input: nwg::TextInput,

    #[nwg_control(text: "边框宽度 (1-5):", parent: ui_group)]
    #[nwg_layout_item(layout: grid, row: 9, col: 2)]
    border_label: nwg::Label,

    #[nwg_control(text: "2", parent: ui_group)]
    #[nwg_layout_item(layout: grid, row: 9, col: 3)]
    border_input: nwg::TextInput,

    // 文件设置分组
    #[nwg_control(text: "💾 文件设置")]
    #[nwg_layout_item(layout: grid, row: 10, col: 0, col_span: 4)]
    file_group: nwg::GroupBox,

    #[nwg_control(text: "保存格式:", parent: file_group)]
    #[nwg_layout_item(layout: grid, row: 11, col: 0)]
    format_label: nwg::Label,

    #[nwg_control(collection: vec!["PNG", "JPEG", "BMP"], selected_index: Some(0), parent: file_group)]
    #[nwg_layout_item(layout: grid, row: 11, col: 1)]
    format_combo: nwg::ComboBox<&'static str>,

    #[nwg_control(text: "JPEG质量 (1-100):", parent: file_group)]
    #[nwg_layout_item(layout: grid, row: 11, col: 2)]
    quality_label: nwg::Label,

    #[nwg_control(text: "90", parent: file_group)]
    #[nwg_layout_item(layout: grid, row: 11, col: 3)]
    quality_input: nwg::TextInput,

    // 底部按钮
    #[nwg_control(text: "🔄 重置默认")]
    #[nwg_layout_item(layout: grid, row: 12, col: 0)]
    #[nwg_events( OnButtonClick: [ModernSettingsApp::reset_defaults] )]
    reset_button: nwg::Button,

    #[nwg_control(text: "✅ 确定")]
    #[nwg_layout_item(layout: grid, row: 12, col: 2)]
    #[nwg_events( OnButtonClick: [ModernSettingsApp::save_and_close] )]
    ok_button: nwg::Button,

    #[nwg_control(text: "❌ 取消")]
    #[nwg_layout_item(layout: grid, row: 12, col: 3)]
    #[nwg_events( OnButtonClick: [ModernSettingsApp::close] )]
    cancel_button: nwg::Button,
}

impl ModernSettingsApp {
    /// 显示设置窗口
    pub fn show() -> Result<(), Box<dyn std::error::Error>> {
        nwg::init()?;

        let mut app = ModernSettingsApp::default();
        app.settings = ModernSettings::load();

        let _ui = ModernSettingsApp::build_ui(app)?;

        // 加载设置值到控件
        // 这里需要在build_ui之后设置值，但由于所有权问题，我们需要重新设计

        nwg::dispatch_thread_events();
        Ok(())
    }

    fn load_values(&self) {
        self.thickness_input
            .set_text(&self.settings.line_thickness.to_string());
        self.font_input
            .set_text(&self.settings.font_size.to_string());
        self.red_input
            .set_text(&self.settings.color_red.to_string());
        self.green_input
            .set_text(&self.settings.color_green.to_string());
        self.blue_input
            .set_text(&self.settings.color_blue.to_string());
        self.auto_copy_check
            .set_check_state(if self.settings.auto_copy {
                nwg::CheckBoxState::Checked
            } else {
                nwg::CheckBoxState::Unchecked
            });
        self.show_cursor_check
            .set_check_state(if self.settings.show_cursor {
                nwg::CheckBoxState::Checked
            } else {
                nwg::CheckBoxState::Unchecked
            });
        self.delay_input
            .set_text(&self.settings.delay_ms.to_string());
        self.opacity_input
            .set_text(&self.settings.toolbar_opacity.to_string());
        self.border_input
            .set_text(&self.settings.border_width.to_string());
        self.quality_input
            .set_text(&self.settings.jpeg_quality.to_string());

        // 设置格式下拉框
        match self.settings.save_format.as_str() {
            "PNG" => self.format_combo.set_selection(Some(0)),
            "JPEG" => self.format_combo.set_selection(Some(1)),
            "BMP" => self.format_combo.set_selection(Some(2)),
            _ => self.format_combo.set_selection(Some(0)),
        }
    }

    fn save_values(&mut self) {
        // 读取并验证输入值
        if let Ok(value) = self.thickness_input.text().parse::<f32>() {
            self.settings.line_thickness = value.max(1.0).min(20.0);
        }

        if let Ok(value) = self.font_input.text().parse::<f32>() {
            self.settings.font_size = value.max(8.0).min(72.0);
        }

        if let Ok(value) = self.red_input.text().parse::<u8>() {
            self.settings.color_red = value;
        }

        if let Ok(value) = self.green_input.text().parse::<u8>() {
            self.settings.color_green = value;
        }

        if let Ok(value) = self.blue_input.text().parse::<u8>() {
            self.settings.color_blue = value;
        }

        self.settings.auto_copy = self.auto_copy_check.check_state() == nwg::CheckBoxState::Checked;
        self.settings.show_cursor =
            self.show_cursor_check.check_state() == nwg::CheckBoxState::Checked;

        if let Ok(value) = self.delay_input.text().parse::<u32>() {
            self.settings.delay_ms = value.min(5000);
        }

        if let Ok(value) = self.opacity_input.text().parse::<f32>() {
            self.settings.toolbar_opacity = value.max(0.1).min(1.0);
        }

        if let Ok(value) = self.border_input.text().parse::<u32>() {
            self.settings.border_width = value.max(1).min(5);
        }

        if let Ok(value) = self.quality_input.text().parse::<u32>() {
            self.settings.jpeg_quality = value.max(1).min(100);
        }

        // 保存格式
        if let Some(index) = self.format_combo.selection() {
            self.settings.save_format = match index {
                0 => "PNG".to_string(),
                1 => "JPEG".to_string(),
                2 => "BMP".to_string(),
                _ => "PNG".to_string(),
            };
        }
    }

    fn reset_defaults(&self) {
        let defaults = ModernSettings::default();
        self.thickness_input
            .set_text(&defaults.line_thickness.to_string());
        self.font_input.set_text(&defaults.font_size.to_string());
        self.red_input.set_text(&defaults.color_red.to_string());
        self.green_input.set_text(&defaults.color_green.to_string());
        self.blue_input.set_text(&defaults.color_blue.to_string());
        self.auto_copy_check
            .set_check_state(nwg::CheckBoxState::Unchecked);
        self.show_cursor_check
            .set_check_state(nwg::CheckBoxState::Unchecked);
        self.delay_input.set_text(&defaults.delay_ms.to_string());
        self.opacity_input
            .set_text(&defaults.toolbar_opacity.to_string());
        self.border_input
            .set_text(&defaults.border_width.to_string());
        self.quality_input
            .set_text(&defaults.jpeg_quality.to_string());
        self.format_combo.set_selection(Some(0));
    }

    fn save_and_close(&mut self) {
        self.save_values();
        let _ = self.settings.save();
        nwg::stop_thread_dispatch();
    }

    fn close(&self) {
        nwg::stop_thread_dispatch();
    }
}
