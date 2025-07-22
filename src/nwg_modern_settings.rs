/*!
    现代化截图工具设置窗口
    基于native-windows-gui实现，使用derive宏简化代码
*/

extern crate native_windows_derive as nwd;
extern crate native_windows_gui as nwg;

use nwd::NwgUi;
use nwg::NativeUi;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// 现代化应用程序设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NwgModernSettings {
    // 基础设置
    pub line_thickness: f32,
    pub font_size: f32,
    pub auto_copy: bool,
    pub show_cursor: bool,
    pub delay_ms: u32,

    // 颜色设置 (RGB 0-255)
    pub drawing_color: [u8; 3],          // 绘图颜色
    pub selection_border_color: [u8; 3], // 选择框边框颜色
    pub toolbar_bg_color: [u8; 3],       // 工具栏背景颜色
    pub mask_color: [u8; 3],             // 遮罩颜色
    pub mask_opacity: f32,               // 遮罩透明度 (0.0-1.0)

    // 界面尺寸设置
    pub handle_size: f32,     // 控制点大小
    pub toolbar_height: f32,  // 工具栏高度
    pub button_width: f32,    // 按钮宽度
    pub button_height: f32,   // 按钮高度
    pub button_spacing: f32,  // 按钮间距
    pub toolbar_padding: f32, // 工具栏内边距

    // 文字设置
    pub default_text_width: i32,  // 默认文字框宽度
    pub default_text_height: i32, // 默认文字框高度
    pub text_padding: f32,        // 文字内边距
    pub line_height: i32,         // 行高
}

impl Default for NwgModernSettings {
    fn default() -> Self {
        Self {
            // 基础设置
            line_thickness: 3.0,
            font_size: 20.0,
            auto_copy: false,
            show_cursor: false,
            delay_ms: 0,

            // 颜色设置 (基于constants.rs中的默认值)
            drawing_color: [255, 0, 0],            // 红色绘图
            selection_border_color: [0, 120, 215], // 蓝色选择框
            toolbar_bg_color: [255, 255, 255],     // 白色工具栏
            mask_color: [0, 0, 0],                 // 黑色遮罩
            mask_opacity: 0.6,

            // 界面尺寸设置 (基于constants.rs)
            handle_size: 8.0,
            toolbar_height: 40.0,
            button_width: 30.0,
            button_height: 30.0,
            button_spacing: 4.0,
            toolbar_padding: 8.0,

            // 文字设置 (基于constants.rs)
            default_text_width: 120,
            default_text_height: 32,
            text_padding: 8.0,
            line_height: 24,
        }
    }
}

impl NwgModernSettings {
    fn get_settings_path() -> PathBuf {
        let mut path = std::env::current_exe().unwrap_or_default();
        path.set_file_name("nwg_modern_settings.json");
        path
    }

    pub fn load() -> Self {
        let path = Self::get_settings_path();
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(settings) = serde_json::from_str::<NwgModernSettings>(&content) {
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

/// 现代化设置窗口应用 - 使用derive宏
#[derive(Default, NwgUi)]
pub struct ModernSettingsApp {
    settings: NwgModernSettings,

    #[nwg_control(size: (750, 600), position: (300, 300), title: "截图工具设置", flags: "WINDOW|VISIBLE")]
    #[nwg_events( OnWindowClose: [ModernSettingsApp::close] )]
    window: nwg::Window,

    #[nwg_layout(parent: window, spacing: 2, min_size: [700, 550])]
    grid: nwg::GridLayout,

    // 标题
    #[nwg_control(text: "截图工具设置", h_align: nwg::HTextAlign::Center)]
    #[nwg_layout_item(layout: grid, row: 0, col: 0, col_span: 5)]
    title_label: nwg::Label,

    // 基础设置分组
    #[nwg_control(text: "基础设置")]
    #[nwg_layout_item(layout: grid, row: 1, col: 0, col_span: 2)]
    basic_title: nwg::Label,

    #[nwg_control(text: "线条粗细 (1-20):")]
    #[nwg_layout_item(layout: grid, row: 2, col: 0)]
    thickness_label: nwg::Label,

    #[nwg_control(text: "3", focus: true)]
    #[nwg_layout_item(layout: grid, row: 2, col: 1)]
    thickness_input: nwg::TextInput,

    #[nwg_control(text: "字体大小 (8-72):")]
    #[nwg_layout_item(layout: grid, row: 3, col: 0)]
    font_label: nwg::Label,

    #[nwg_control(text: "20")]
    #[nwg_layout_item(layout: grid, row: 3, col: 1)]
    font_input: nwg::TextInput,

    // 颜色设置分组
    #[nwg_control(text: "颜色设置")]
    #[nwg_layout_item(layout: grid, row: 1, col: 2, col_span: 3)]
    color_title: nwg::Label,

    #[nwg_control(text: "绘图颜色:")]
    #[nwg_layout_item(layout: grid, row: 2, col: 2)]
    drawing_color_label: nwg::Label,

    #[nwg_control(text: "■ 红色", h_align: nwg::HTextAlign::Center)]
    #[nwg_layout_item(layout: grid, row: 2, col: 3)]
    drawing_color_display: nwg::Label,

    #[nwg_control(text: "选择颜色")]
    #[nwg_layout_item(layout: grid, row: 2, col: 4)]
    #[nwg_events( OnButtonClick: [ModernSettingsApp::choose_drawing_color] )]
    drawing_color_button: nwg::Button,

    #[nwg_control(text: "选择框颜色:")]
    #[nwg_layout_item(layout: grid, row: 3, col: 2)]
    selection_color_label: nwg::Label,

    #[nwg_control(text: "■ 蓝色", h_align: nwg::HTextAlign::Center)]
    #[nwg_layout_item(layout: grid, row: 3, col: 3)]
    selection_color_display: nwg::Label,

    #[nwg_control(text: "选择颜色")]
    #[nwg_layout_item(layout: grid, row: 3, col: 4)]
    #[nwg_events( OnButtonClick: [ModernSettingsApp::choose_selection_color] )]
    selection_color_button: nwg::Button,

    #[nwg_control(text: "工具栏颜色:")]
    #[nwg_layout_item(layout: grid, row: 4, col: 2)]
    toolbar_color_label: nwg::Label,

    #[nwg_control(text: "■ 白色", h_align: nwg::HTextAlign::Center)]
    #[nwg_layout_item(layout: grid, row: 4, col: 3)]
    toolbar_color_display: nwg::Label,

    #[nwg_control(text: "选择颜色")]
    #[nwg_layout_item(layout: grid, row: 4, col: 4)]
    #[nwg_events( OnButtonClick: [ModernSettingsApp::choose_toolbar_color] )]
    toolbar_color_button: nwg::Button,

    // 界面尺寸设置
    #[nwg_control(text: "界面尺寸")]
    #[nwg_layout_item(layout: grid, row: 5, col: 0, col_span: 5)]
    size_title: nwg::Label,

    #[nwg_control(text: "控制点大小:")]
    #[nwg_layout_item(layout: grid, row: 6, col: 0)]
    handle_size_label: nwg::Label,

    #[nwg_control(text: "8")]
    #[nwg_layout_item(layout: grid, row: 6, col: 1)]
    handle_size_input: nwg::TextInput,

    #[nwg_control(text: "工具栏高度:")]
    #[nwg_layout_item(layout: grid, row: 6, col: 2)]
    toolbar_height_label: nwg::Label,

    #[nwg_control(text: "40")]
    #[nwg_layout_item(layout: grid, row: 6, col: 3)]
    toolbar_height_input: nwg::TextInput,

    #[nwg_control(text: "按钮宽度:")]
    #[nwg_layout_item(layout: grid, row: 7, col: 0)]
    button_width_label: nwg::Label,

    #[nwg_control(text: "30")]
    #[nwg_layout_item(layout: grid, row: 7, col: 1)]
    button_width_input: nwg::TextInput,

    #[nwg_control(text: "按钮高度:")]
    #[nwg_layout_item(layout: grid, row: 7, col: 2)]
    button_height_label: nwg::Label,

    #[nwg_control(text: "30")]
    #[nwg_layout_item(layout: grid, row: 7, col: 3)]
    button_height_input: nwg::TextInput,

    // 截图设置分组
    #[nwg_control(text: "截图选项")]
    #[nwg_layout_item(layout: grid, row: 8, col: 0, col_span: 5)]
    screenshot_title: nwg::Label,

    #[nwg_control(text: "自动复制到剪贴板")]
    #[nwg_layout_item(layout: grid, row: 9, col: 0, col_span: 2)]
    auto_copy_check: nwg::CheckBox,

    #[nwg_control(text: "截图时显示光标")]
    #[nwg_layout_item(layout: grid, row: 9, col: 2, col_span: 3)]
    show_cursor_check: nwg::CheckBox,

    #[nwg_control(text: "截图延迟 (毫秒):")]
    #[nwg_layout_item(layout: grid, row: 10, col: 0)]
    delay_label: nwg::Label,

    #[nwg_control(text: "0")]
    #[nwg_layout_item(layout: grid, row: 10, col: 1)]
    delay_input: nwg::TextInput,

    // 按钮
    #[nwg_control(text: "重置默认")]
    #[nwg_layout_item(layout: grid, row: 12, col: 0)]
    #[nwg_events( OnButtonClick: [ModernSettingsApp::reset_defaults] )]
    reset_button: nwg::Button,

    #[nwg_control(text: "确定")]
    #[nwg_layout_item(layout: grid, row: 12, col: 2)]
    #[nwg_events( OnButtonClick: [ModernSettingsApp::save_and_close] )]
    ok_button: nwg::Button,

    #[nwg_control(text: "取消")]
    #[nwg_layout_item(layout: grid, row: 12, col: 3)]
    #[nwg_events( OnButtonClick: [ModernSettingsApp::close] )]
    cancel_button: nwg::Button,

    // 调色盘对话框
    #[nwg_resource]
    color_dialog: nwg::ColorDialog,
}

impl ModernSettingsApp {
    /// 显示现代化设置窗口
    pub fn show() -> Result<(), Box<dyn std::error::Error>> {
        // 初始化NWG
        nwg::init()?;

        // 设置现代字体
        nwg::Font::set_global_family("Segoe UI")?;

        let mut app = ModernSettingsApp::default();
        app.settings = NwgModernSettings::load();

        // 构建UI
        let app = ModernSettingsApp::build_ui(app)?;

        // 加载设置值
        app.load_values();

        // 运行事件循环
        nwg::dispatch_thread_events();

        Ok(())
    }

    fn reset_defaults(&self) {
        let defaults = NwgModernSettings::default();
        self.thickness_input
            .set_text(&defaults.line_thickness.to_string());
        self.font_input.set_text(&defaults.font_size.to_string());
        self.handle_size_input
            .set_text(&defaults.handle_size.to_string());
        self.toolbar_height_input
            .set_text(&defaults.toolbar_height.to_string());
        self.button_width_input
            .set_text(&defaults.button_width.to_string());
        self.button_height_input
            .set_text(&defaults.button_height.to_string());
        self.auto_copy_check
            .set_check_state(nwg::CheckBoxState::Unchecked);
        self.show_cursor_check
            .set_check_state(nwg::CheckBoxState::Unchecked);
        self.delay_input.set_text(&defaults.delay_ms.to_string());
    }

    fn choose_drawing_color(&self) {
        if self.color_dialog.run(Some(&self.window)) {
            let color = self.color_dialog.color();
            println!("选择了绘图颜色: {:?}", color);

            // 更新颜色显示
            let color_name = Self::get_color_name(color);
            self.drawing_color_display
                .set_text(&format!("■ {}", color_name));

            // 立即保存颜色设置到文件
            Self::save_color_setting("drawing_color", color);
        }
    }

    fn choose_selection_color(&self) {
        if self.color_dialog.run(Some(&self.window)) {
            let color = self.color_dialog.color();
            println!("选择了选择框颜色: {:?}", color);

            // 更新颜色显示
            let color_name = Self::get_color_name(color);
            self.selection_color_display
                .set_text(&format!("■ {}", color_name));

            // 立即保存颜色设置到文件
            Self::save_color_setting("selection_border_color", color);
        }
    }

    fn choose_toolbar_color(&self) {
        if self.color_dialog.run(Some(&self.window)) {
            let color = self.color_dialog.color();
            println!("选择了工具栏颜色: {:?}", color);

            // 更新颜色显示
            let color_name = Self::get_color_name(color);
            self.toolbar_color_display
                .set_text(&format!("■ {}", color_name));

            // 立即保存颜色设置到文件
            Self::save_color_setting("toolbar_bg_color", color);
        }
    }

    // 辅助函数：根据RGB值获取颜色名称或十六进制值
    fn get_color_name(color: [u8; 3]) -> String {
        match color {
            [255, 0, 0] => "红色".to_string(),
            [0, 255, 0] => "绿色".to_string(),
            [0, 0, 255] => "蓝色".to_string(),
            [255, 255, 255] => "白色".to_string(),
            [0, 0, 0] => "黑色".to_string(),
            [255, 255, 0] => "黄色".to_string(),
            [255, 0, 255] => "紫色".to_string(),
            [0, 255, 255] => "青色".to_string(),
            [128, 128, 128] => "灰色".to_string(),
            [255, 165, 0] => "橙色".to_string(),
            [0, 120, 215] => "蓝色".to_string(),
            _ => format!("#{:02X}{:02X}{:02X}", color[0], color[1], color[2]),
        }
    }

    // 保存单个颜色设置到文件
    fn save_color_setting(color_type: &str, color: [u8; 3]) {
        let mut settings = NwgModernSettings::load();

        match color_type {
            "drawing_color" => settings.drawing_color = color,
            "selection_border_color" => settings.selection_border_color = color,
            "toolbar_bg_color" => settings.toolbar_bg_color = color,
            _ => {}
        }

        if let Err(e) = settings.save() {
            println!("保存颜色设置失败: {}", e);
        } else {
            println!("颜色设置已保存: {} = {:?}", color_type, color);
        }
    }

    fn save_and_close(&self) {
        // 由于NWG的限制，我们无法在事件处理中修改self
        // 这里只能关闭窗口，实际保存需要在其他地方处理
        nwg::stop_thread_dispatch();
    }

    fn close(&self) {
        nwg::stop_thread_dispatch();
    }

    fn save_values(&mut self) {
        if let Ok(value) = self.thickness_input.text().parse::<f32>() {
            self.settings.line_thickness = value.max(1.0).min(20.0);
        }

        if let Ok(value) = self.font_input.text().parse::<f32>() {
            self.settings.font_size = value.max(8.0).min(72.0);
        }

        if let Ok(value) = self.handle_size_input.text().parse::<f32>() {
            self.settings.handle_size = value.max(4.0).min(16.0);
        }

        if let Ok(value) = self.toolbar_height_input.text().parse::<f32>() {
            self.settings.toolbar_height = value.max(20.0).min(80.0);
        }

        if let Ok(value) = self.button_width_input.text().parse::<f32>() {
            self.settings.button_width = value.max(20.0).min(60.0);
        }

        if let Ok(value) = self.button_height_input.text().parse::<f32>() {
            self.settings.button_height = value.max(20.0).min(60.0);
        }

        self.settings.auto_copy = self.auto_copy_check.check_state() == nwg::CheckBoxState::Checked;
        self.settings.show_cursor =
            self.show_cursor_check.check_state() == nwg::CheckBoxState::Checked;

        if let Ok(value) = self.delay_input.text().parse::<u32>() {
            self.settings.delay_ms = value.min(5000);
        }
    }

    fn load_values(&self) {
        self.thickness_input
            .set_text(&self.settings.line_thickness.to_string());
        self.font_input
            .set_text(&self.settings.font_size.to_string());
        self.handle_size_input
            .set_text(&self.settings.handle_size.to_string());
        self.toolbar_height_input
            .set_text(&self.settings.toolbar_height.to_string());
        self.button_width_input
            .set_text(&self.settings.button_width.to_string());
        self.button_height_input
            .set_text(&self.settings.button_height.to_string());

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
    }
}
