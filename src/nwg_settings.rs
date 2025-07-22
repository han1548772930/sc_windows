use native_windows_gui as nwg;
use native_windows_derive as nwd;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// 现代化应用程序设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NwgSettings {
    pub line_thickness: f32,
    pub font_size: f32,
    pub auto_copy: bool,
    pub show_cursor: bool,
    pub delay_ms: u32,
    pub color_red: u8,
    pub color_green: u8,
    pub color_blue: u8,
}

impl Default for NwgSettings {
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
        }
    }
}

impl NwgSettings {
    fn get_settings_path() -> PathBuf {
        let mut path = std::env::current_exe().unwrap_or_default();
        path.set_file_name("nwg_settings.json");
        path
    }
    
    pub fn load() -> Self {
        let path = Self::get_settings_path();
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(settings) = serde_json::from_str::<NwgSettings>(&content) {
                return settings;
            }
        }
        let default_settings = Self::default();
        let _ = default_settings.save();
        default_settings
    }
    
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::get_settings_path();
        let content = serde_json::to_string_pretty(self)?;
        fs::write(&path, content)?;
        Ok(())
    }
}

/// 现代化设置窗口
#[derive(Default, nwd::NwgUi)]
pub struct NwgSettingsApp {
    settings: NwgSettings,
    
    #[nwg_control(size: (500, 400), position: (300, 300), title: "🎨 截图工具设置", flags: "WINDOW|VISIBLE")]
    #[nwg_events( OnWindowClose: [NwgSettingsApp::close] )]
    window: nwg::Window,

    #[nwg_layout(parent: window, spacing: 1)]
    grid: nwg::GridLayout,

    // 标题
    #[nwg_control(text: "🎨 截图工具设置", h_align: nwg::HTextAlign::Center)]
    #[nwg_layout_item(layout: grid, row: 0, col: 0, col_span: 4)]
    title_label: nwg::Label,

    // 绘图设置
    #[nwg_control(text: "🖌️ 绘图工具", h_align: nwg::HTextAlign::Left)]
    #[nwg_layout_item(layout: grid, row: 1, col: 0, col_span: 4)]
    drawing_title: nwg::Label,

    #[nwg_control(text: "线条粗细 (1-20):")]
    #[nwg_layout_item(layout: grid, row: 2, col: 0)]
    thickness_label: nwg::Label,

    #[nwg_control(text: "3")]
    #[nwg_layout_item(layout: grid, row: 2, col: 1)]
    thickness_input: nwg::TextInput,

    #[nwg_control(text: "字体大小 (8-72):")]
    #[nwg_layout_item(layout: grid, row: 2, col: 2)]
    font_label: nwg::Label,

    #[nwg_control(text: "20")]
    #[nwg_layout_item(layout: grid, row: 2, col: 3)]
    font_input: nwg::TextInput,

    // 颜色设置
    #[nwg_control(text: "🎨 默认颜色", h_align: nwg::HTextAlign::Left)]
    #[nwg_layout_item(layout: grid, row: 3, col: 0, col_span: 4)]
    color_title: nwg::Label,

    #[nwg_control(text: "红色 (0-255):")]
    #[nwg_layout_item(layout: grid, row: 4, col: 0)]
    red_label: nwg::Label,

    #[nwg_control(text: "255")]
    #[nwg_layout_item(layout: grid, row: 4, col: 1)]
    red_input: nwg::TextInput,

    #[nwg_control(text: "绿色 (0-255):")]
    #[nwg_layout_item(layout: grid, row: 4, col: 2)]
    green_label: nwg::Label,

    #[nwg_control(text: "0")]
    #[nwg_layout_item(layout: grid, row: 4, col: 3)]
    green_input: nwg::TextInput,

    #[nwg_control(text: "蓝色 (0-255):")]
    #[nwg_layout_item(layout: grid, row: 5, col: 0)]
    blue_label: nwg::Label,

    #[nwg_control(text: "0")]
    #[nwg_layout_item(layout: grid, row: 5, col: 1)]
    blue_input: nwg::TextInput,

    // 截图设置
    #[nwg_control(text: "📷 截图选项", h_align: nwg::HTextAlign::Left)]
    #[nwg_layout_item(layout: grid, row: 6, col: 0, col_span: 4)]
    screenshot_title: nwg::Label,

    #[nwg_control(text: "自动复制到剪贴板")]
    #[nwg_layout_item(layout: grid, row: 7, col: 0, col_span: 2)]
    auto_copy_check: nwg::CheckBox,

    #[nwg_control(text: "截图时显示光标")]
    #[nwg_layout_item(layout: grid, row: 7, col: 2, col_span: 2)]
    show_cursor_check: nwg::CheckBox,

    #[nwg_control(text: "截图延迟 (毫秒):")]
    #[nwg_layout_item(layout: grid, row: 8, col: 0)]
    delay_label: nwg::Label,

    #[nwg_control(text: "0")]
    #[nwg_layout_item(layout: grid, row: 8, col: 1)]
    delay_input: nwg::TextInput,

    // 底部按钮
    #[nwg_control(text: "🔄 重置默认")]
    #[nwg_layout_item(layout: grid, row: 10, col: 0)]
    #[nwg_events( OnButtonClick: [NwgSettingsApp::reset_defaults] )]
    reset_button: nwg::Button,

    #[nwg_control(text: "✅ 确定")]
    #[nwg_layout_item(layout: grid, row: 10, col: 2)]
    #[nwg_events( OnButtonClick: [NwgSettingsApp::save_and_close] )]
    ok_button: nwg::Button,

    #[nwg_control(text: "❌ 取消")]
    #[nwg_layout_item(layout: grid, row: 10, col: 3)]
    #[nwg_events( OnButtonClick: [NwgSettingsApp::close] )]
    cancel_button: nwg::Button,
}

impl NwgSettingsApp {
    /// 显示设置窗口
    pub fn show() -> Result<(), Box<dyn std::error::Error>> {
        nwg::init()?;
        
        let mut app = NwgSettingsApp::default();
        app.settings = NwgSettings::load();
        
        let _ui = NwgSettingsApp::build_ui(app)?;
        
        nwg::dispatch_thread_events();
        Ok(())
    }

    fn load_values(&self) {
        self.thickness_input.set_text(&self.settings.line_thickness.to_string());
        self.font_input.set_text(&self.settings.font_size.to_string());
        self.red_input.set_text(&self.settings.color_red.to_string());
        self.green_input.set_text(&self.settings.color_green.to_string());
        self.blue_input.set_text(&self.settings.color_blue.to_string());
        self.auto_copy_check.set_check_state(if self.settings.auto_copy { 
            nwg::CheckBoxState::Checked 
        } else { 
            nwg::CheckBoxState::Unchecked 
        });
        self.show_cursor_check.set_check_state(if self.settings.show_cursor { 
            nwg::CheckBoxState::Checked 
        } else { 
            nwg::CheckBoxState::Unchecked 
        });
        self.delay_input.set_text(&self.settings.delay_ms.to_string());
    }

    fn save_values(&mut self) {
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
        self.settings.show_cursor = self.show_cursor_check.check_state() == nwg::CheckBoxState::Checked;
        
        if let Ok(value) = self.delay_input.text().parse::<u32>() {
            self.settings.delay_ms = value.min(5000);
        }
    }

    fn reset_defaults(&self) {
        let defaults = NwgSettings::default();
        self.thickness_input.set_text(&defaults.line_thickness.to_string());
        self.font_input.set_text(&defaults.font_size.to_string());
        self.red_input.set_text(&defaults.color_red.to_string());
        self.green_input.set_text(&defaults.color_green.to_string());
        self.blue_input.set_text(&defaults.color_blue.to_string());
        self.auto_copy_check.set_check_state(nwg::CheckBoxState::Unchecked);
        self.show_cursor_check.set_check_state(nwg::CheckBoxState::Unchecked);
        self.delay_input.set_text(&defaults.delay_ms.to_string());
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
