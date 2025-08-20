use std::cell::RefCell;
use std::sync::Arc;
use windows::core::Result;
use windows_core::{HSTRING, IInspectable, Interface, h};
use winui3::{
    Activatable,
    Microsoft::UI::Xaml::{
        Application,
        Controls::{
            Button, ColorPicker, ComboBox, Expander, Frame, Grid, NumberBox, Page, ScrollViewer,
            Slider, StackPanel, TextBlock, TextBox, ToggleSwitch,
        },
        GridLength, GridLengthHelper, GridUnitType, HorizontalAlignment, LaunchActivatedEventArgs,
        Markup::IXamlType,
        Media::{Brush, MicaBackdrop, SolidColorBrush},
        Navigation::{NavigatingCancelEventArgs, NavigationEventArgs},
        Style, Thickness,
        UI::Colors,
        VerticalAlignment, Visibility, Window,
    },
    XamlApp, XamlAppOverrides, XamlPage, XamlPageOverrides, xaml_typename,
};

use crate::simple_settings::SimpleSettings;

/// WinUI3风格的设置页面
pub struct SettingsPage {
    settings: Arc<SimpleSettings>,
    // 控件引用
    line_thickness_slider: Option<Slider>,
    line_thickness_number: Option<NumberBox>,
    font_size_slider: Option<Slider>,
    font_size_number: Option<NumberBox>,
    auto_copy_toggle: Option<ToggleSwitch>,
    show_cursor_toggle: Option<ToggleSwitch>,
    delay_number: Option<NumberBox>,
    drawing_color_picker: Option<ColorPicker>,
    text_color_picker: Option<ColorPicker>,
    config_path_textbox: Option<TextBox>,
    ocr_language_combo: Option<ComboBox>,
}

impl SettingsPage {
    pub fn new() -> Self {
        Self {
            settings: Arc::new(SimpleSettings::load()),
            line_thickness_slider: None,
            line_thickness_number: None,
            font_size_slider: None,
            font_size_number: None,
            auto_copy_toggle: None,
            show_cursor_toggle: None,
            delay_number: None,
            drawing_color_picker: None,
            text_color_picker: None,
            config_path_textbox: None,
            ocr_language_combo: None,
        }
    }
}

impl Activatable for SettingsPage {
    fn activate() -> Result<IInspectable> {
        let page = XamlPage::compose(SettingsPage::new())?;
        Ok(page.into())
    }
}

impl XamlPageOverrides for SettingsPage {
    fn OnNavigatedTo(&self, page: &Page, _args: Option<&NavigationEventArgs>) -> Result<()> {
        // 创建主滚动容器
        let scroll_viewer = ScrollViewer::new()?;
        scroll_viewer.SetVerticalScrollBarVisibility(
            winui3::Microsoft::UI::Xaml::Controls::ScrollBarVisibility::Auto,
        )?;
        scroll_viewer.SetHorizontalScrollBarVisibility(
            winui3::Microsoft::UI::Xaml::Controls::ScrollBarVisibility::Disabled,
        )?;

        // 创建主Grid布局
        let main_grid = Grid::new()?;
        main_grid.SetMargin(Thickness {
            Left: 20.0,
            Top: 20.0,
            Right: 20.0,
            Bottom: 20.0,
        })?;

        // 创建主StackPanel
        let main_stack = StackPanel::new()?;
        main_stack.SetSpacing(16.0)?;

        // 添加标题
        let title = TextBlock::new()?;
        title.SetText(h!("应用程序设置"))?;
        title.SetFontSize(28.0)?;
        title.SetFontWeight(winui3::Microsoft::UI::Text::FontWeights::Bold()?)?;
        title.SetMargin(Thickness {
            Left: 0.0,
            Top: 0.0,
            Right: 0.0,
            Bottom: 20.0,
        })?;

        main_stack.Children()?.Append(&title)?;

        // 创建基础设置组
        self.create_basic_settings_group(&main_stack)?;

        // 创建颜色设置组
        self.create_color_settings_group(&main_stack)?;

        // 创建路径设置组
        self.create_path_settings_group(&main_stack)?;

        // 创建OCR设置组
        self.create_ocr_settings_group(&main_stack)?;

        // 创建按钮组
        self.create_button_group(&main_stack)?;

        main_grid.Children()?.Append(&main_stack)?;
        scroll_viewer.SetContent(&main_grid)?;
        page.SetContent(&scroll_viewer)?;

        Ok(())
    }

    fn OnNavigatedFrom(&self, _base: &Page, _args: Option<&NavigationEventArgs>) -> Result<()> {
        Ok(())
    }

    fn OnNavigatingFrom(
        &self,
        _base: &Page,
        _args: Option<&NavigatingCancelEventArgs>,
    ) -> Result<()> {
        Ok(())
    }
}

impl SettingsPage {
    /// 创建基础设置组
    fn create_basic_settings_group(&self, parent: &StackPanel) -> Result<()> {
        let expander = Expander::new()?;
        expander.SetHeader(&h!("基础设置"))?;
        expander.SetIsExpanded(true)?;

        let content_stack = StackPanel::new()?;
        content_stack.SetSpacing(12.0)?;
        content_stack.SetMargin(Thickness {
            Left: 16.0,
            Top: 8.0,
            Right: 16.0,
            Bottom: 8.0,
        })?;

        // 线条粗细设置
        self.create_slider_setting(
            &content_stack,
            "线条粗细",
            1.0,
            10.0,
            self.settings.line_thickness as f64,
            "px",
        )?;

        // 字体大小设置
        self.create_slider_setting(
            &content_stack,
            "字体大小",
            8.0,
            72.0,
            self.settings.font_size as f64,
            "pt",
        )?;

        // 延迟设置
        self.create_number_setting(
            &content_stack,
            "截图延迟",
            0.0,
            5000.0,
            self.settings.delay_ms as f64,
            "ms",
        )?;

        // 开关设置
        self.create_toggle_setting(&content_stack, "自动复制", self.settings.auto_copy)?;
        self.create_toggle_setting(&content_stack, "显示光标", self.settings.show_cursor)?;

        expander.SetContent(&content_stack)?;
        parent.Children()?.Append(&expander)?;

        Ok(())
    }

    /// 创建颜色设置组
    fn create_color_settings_group(&self, parent: &StackPanel) -> Result<()> {
        let expander = Expander::new()?;
        expander.SetHeader(&h!("颜色设置")?)?;
        expander.SetIsExpanded(true)?;

        let content_stack = StackPanel::new()?;
        content_stack.SetSpacing(12.0)?;
        content_stack.SetMargin(Thickness {
            Left: 16.0,
            Top: 8.0,
            Right: 16.0,
            Bottom: 8.0,
        })?;

        // 绘图颜色
        self.create_color_setting(
            &content_stack,
            "绘图颜色",
            (
                self.settings.drawing_color_red,
                self.settings.drawing_color_green,
                self.settings.drawing_color_blue,
            ),
        )?;

        // 文本颜色
        self.create_color_setting(
            &content_stack,
            "文本颜色",
            (
                self.settings.text_color_red,
                self.settings.text_color_green,
                self.settings.text_color_blue,
            ),
        )?;

        expander.SetContent(&content_stack)?;
        parent.Children()?.Append(&expander)?;

        Ok(())
    }

    /// 创建路径设置组
    fn create_path_settings_group(&self, parent: &StackPanel) -> Result<()> {
        let expander = Expander::new()?;
        expander.SetHeader(&h!("路径设置")?)?;
        expander.SetIsExpanded(false)?;

        let content_stack = StackPanel::new()?;
        content_stack.SetSpacing(12.0)?;
        content_stack.SetMargin(Thickness {
            Left: 16.0,
            Top: 8.0,
            Right: 16.0,
            Bottom: 8.0,
        })?;

        // 配置文件路径
        self.create_path_setting(&content_stack, "配置文件路径", &self.settings.config_path)?;

        expander.SetContent(&content_stack)?;
        parent.Children()?.Append(&expander)?;

        Ok(())
    }

    /// 创建OCR设置组
    fn create_ocr_settings_group(&self, parent: &StackPanel) -> Result<()> {
        let expander = Expander::new()?;
        expander.SetHeader(&h!("OCR设置")?)?;
        expander.SetIsExpanded(false)?;

        let content_stack = StackPanel::new()?;
        content_stack.SetSpacing(12.0)?;
        content_stack.SetMargin(Thickness {
            Left: 16.0,
            Top: 8.0,
            Right: 16.0,
            Bottom: 8.0,
        })?;

        // OCR语言选择
        self.create_combo_setting(&content_stack, "OCR语言", &self.settings.ocr_language)?;

        expander.SetContent(&content_stack)?;
        parent.Children()?.Append(&expander)?;

        Ok(())
    }

    /// 创建按钮组
    fn create_button_group(&self, parent: &StackPanel) -> Result<()> {
        let button_stack = StackPanel::new()?;
        button_stack
            .SetOrientation(winui3::Microsoft::UI::Xaml::Controls::Orientation::Horizontal)?;
        button_stack.SetSpacing(12.0)?;
        button_stack.SetHorizontalAlignment(HorizontalAlignment::Right)?;
        button_stack.SetMargin(Thickness {
            Left: 0.0,
            Top: 20.0,
            Right: 0.0,
            Bottom: 0.0,
        })?;

        // 确定按钮
        let ok_button = Button::new()?;
        ok_button.SetContent(&h!("确定")?)?;
        ok_button.SetMinWidth(100.0)?;
        ok_button.SetStyle(&self.get_accent_button_style()?)?;

        // 取消按钮
        let cancel_button = Button::new()?;
        cancel_button.SetContent(&h!("取消")?)?;
        cancel_button.SetMinWidth(100.0)?;

        button_stack.Children()?.Append(&ok_button)?;
        button_stack.Children()?.Append(&cancel_button)?;
        parent.Children()?.Append(&button_stack)?;

        Ok(())
    }

    /// 创建滑块设置项
    fn create_slider_setting(
        &self,
        parent: &StackPanel,
        label: &str,
        min: f64,
        max: f64,
        value: f64,
        unit: &str,
    ) -> Result<()> {
        let container = StackPanel::new()?;
        container.SetSpacing(8.0)?;

        // 标签和值显示
        let header_stack = StackPanel::new()?;
        header_stack
            .SetOrientation(winui3::Microsoft::UI::Xaml::Controls::Orientation::Horizontal)?;
        header_stack.SetSpacing(8.0)?;

        let label_text = TextBlock::new()?;
        label_text.SetText(&HSTRING::from(label))?;
        label_text.SetVerticalAlignment(VerticalAlignment::Center)?;
        label_text.SetMinWidth(120.0)?;

        let value_number = NumberBox::new()?;
        value_number.SetValue(Some(value))?;
        value_number.SetMinimum(Some(min))?;
        value_number.SetMaximum(Some(max))?;
        value_number.SetWidth(80.0)?;
        value_number.SetSpinButtonPlacementMode(
            winui3::Microsoft::UI::Xaml::Controls::NumberBoxSpinButtonPlacementMode::Inline,
        )?;

        let unit_text = TextBlock::new()?;
        unit_text.SetText(&HSTRING::from(unit))?;
        unit_text.SetVerticalAlignment(VerticalAlignment::Center)?;

        header_stack.Children()?.Append(&label_text)?;
        header_stack.Children()?.Append(&value_number)?;
        header_stack.Children()?.Append(&unit_text)?;

        // 滑块
        let slider = Slider::new()?;
        slider.SetMinimum(min)?;
        slider.SetMaximum(max)?;
        slider.SetValue(value)?;
        slider.SetStepFrequency(if max <= 10.0 { 0.1 } else { 1.0 })?;

        container.Children()?.Append(&header_stack)?;
        container.Children()?.Append(&slider)?;
        parent.Children()?.Append(&container)?;

        Ok(())
    }

    /// 创建数字输入设置项
    fn create_number_setting(
        &self,
        parent: &StackPanel,
        label: &str,
        min: f64,
        max: f64,
        value: f64,
        unit: &str,
    ) -> Result<()> {
        let container = StackPanel::new()?;
        container.SetOrientation(winui3::Microsoft::UI::Xaml::Controls::Orientation::Horizontal)?;
        container.SetSpacing(12.0)?;

        let label_text = TextBlock::new()?;
        label_text.SetText(&HSTRING::from(label))?;
        label_text.SetVerticalAlignment(VerticalAlignment::Center)?;
        label_text.SetMinWidth(120.0)?;

        let number_box = NumberBox::new()?;
        number_box.SetValue(Some(value))?;
        number_box.SetMinimum(Some(min))?;
        number_box.SetMaximum(Some(max))?;
        number_box.SetWidth(100.0)?;
        number_box.SetSpinButtonPlacementMode(
            winui3::Microsoft::UI::Xaml::Controls::NumberBoxSpinButtonPlacementMode::Inline,
        )?;

        let unit_text = TextBlock::new()?;
        unit_text.SetText(&HSTRING::from(unit))?;
        unit_text.SetVerticalAlignment(VerticalAlignment::Center)?;

        container.Children()?.Append(&label_text)?;
        container.Children()?.Append(&number_box)?;
        container.Children()?.Append(&unit_text)?;
        parent.Children()?.Append(&container)?;

        Ok(())
    }

    /// 创建开关设置项
    fn create_toggle_setting(&self, parent: &StackPanel, label: &str, value: bool) -> Result<()> {
        let container = StackPanel::new()?;
        container.SetOrientation(winui3::Microsoft::UI::Xaml::Controls::Orientation::Horizontal)?;
        container.SetSpacing(12.0)?;

        let label_text = TextBlock::new()?;
        label_text.SetText(&HSTRING::from(label))?;
        label_text.SetVerticalAlignment(VerticalAlignment::Center)?;
        label_text.SetMinWidth(120.0)?;

        let toggle = ToggleSwitch::new()?;
        toggle.SetIsOn(value)?;

        container.Children()?.Append(&label_text)?;
        container.Children()?.Append(&toggle)?;
        parent.Children()?.Append(&container)?;

        Ok(())
    }

    /// 创建颜色设置项
    fn create_color_setting(
        &self,
        parent: &StackPanel,
        label: &str,
        color: (u8, u8, u8),
    ) -> Result<()> {
        let container = StackPanel::new()?;
        container.SetOrientation(winui3::Microsoft::UI::Xaml::Controls::Orientation::Horizontal)?;
        container.SetSpacing(12.0)?;

        let label_text = TextBlock::new()?;
        label_text.SetText(&HSTRING::from(label))?;
        label_text.SetVerticalAlignment(VerticalAlignment::Center)?;
        label_text.SetMinWidth(120.0)?;

        let color_picker = ColorPicker::new()?;
        // 设置初始颜色
        let initial_color =
            winui3::Microsoft::UI::Colors::FromArgb(255, color.0, color.1, color.2)?;
        color_picker.SetColor(initial_color)?;
        color_picker.SetWidth(200.0)?;

        container.Children()?.Append(&label_text)?;
        container.Children()?.Append(&color_picker)?;
        parent.Children()?.Append(&container)?;

        Ok(())
    }

    /// 创建路径设置项
    fn create_path_setting(&self, parent: &StackPanel, label: &str, path: &str) -> Result<()> {
        let container = StackPanel::new()?;
        container.SetSpacing(8.0)?;

        let label_text = TextBlock::new()?;
        label_text.SetText(&HSTRING::from(label))?;

        let path_container = StackPanel::new()?;
        path_container
            .SetOrientation(winui3::Microsoft::UI::Xaml::Controls::Orientation::Horizontal)?;
        path_container.SetSpacing(8.0)?;

        let path_textbox = TextBox::new()?;
        path_textbox.SetText(&HSTRING::from(path))?;
        path_textbox.SetWidth(300.0)?;

        let browse_button = Button::new()?;
        browse_button.SetContent(&h!("浏览...")?)?;

        path_container.Children()?.Append(&path_textbox)?;
        path_container.Children()?.Append(&browse_button)?;

        container.Children()?.Append(&label_text)?;
        container.Children()?.Append(&path_container)?;
        parent.Children()?.Append(&container)?;

        Ok(())
    }

    /// 创建下拉框设置项
    fn create_combo_setting(&self, parent: &StackPanel, label: &str, value: &str) -> Result<()> {
        let container = StackPanel::new()?;
        container.SetOrientation(winui3::Microsoft::UI::Xaml::Controls::Orientation::Horizontal)?;
        container.SetSpacing(12.0)?;

        let label_text = TextBlock::new()?;
        label_text.SetText(&HSTRING::from(label))?;
        label_text.SetVerticalAlignment(VerticalAlignment::Center)?;
        label_text.SetMinWidth(120.0)?;

        let combo_box = ComboBox::new()?;
        combo_box.SetWidth(200.0)?;

        // 添加OCR语言选项
        let languages = vec![
            ("zh-CN", "简体中文"),
            ("zh-TW", "繁体中文"),
            ("en-US", "英语"),
            ("ja-JP", "日语"),
            ("ko-KR", "韩语"),
        ];

        for (code, name) in languages {
            let item = winui3::Microsoft::UI::Xaml::Controls::ComboBoxItem::new()?;
            item.SetContent(&HSTRING::from(name)?)?;
            item.SetTag(&HSTRING::from(code)?)?;
            combo_box.Items()?.Append(&item)?;

            if code == value {
                combo_box.SetSelectedItem(&item)?;
            }
        }

        container.Children()?.Append(&label_text)?;
        container.Children()?.Append(&combo_box)?;
        parent.Children()?.Append(&container)?;

        Ok(())
    }

    /// 获取强调按钮样式
    fn get_accent_button_style(&self) -> Result<Style> {
        let app = Application::Current()?;
        let resources = app.Resources()?;
        let style: Style = resources
            .Lookup(&HSTRING::from("AccentButtonStyle")?)?
            .cast()?;
        Ok(style)
    }
}

/// WinUI3设置窗口
pub struct SettingsWindow {
    window: Window,
}

impl SettingsWindow {
    pub fn new() -> Result<Self> {
        Ok(Self {
            window: Window::new()?,
        })
    }

    pub fn initialize_component(&self) -> Result<()> {
        // 设置窗口属性
        self.window.SetTitle(h!("应用程序设置"))?;

        // 设置Mica背景
        if let Ok(mica_backdrop) = MicaBackdrop::new() {
            let _ = self.window.SetSystemBackdrop(&mica_backdrop);
        }

        // 创建Frame来承载设置页面
        let frame = Frame::new()?;

        // 导航到设置页面
        let page_type = xaml_typename("SettingsPage");
        frame.Navigate2(&page_type)?;

        self.window.SetContent(&frame)?;
        Ok(())
    }

    pub fn activate(&self) -> Result<()> {
        self.window.Activate()
    }
}

/// WinUI3设置应用程序
pub struct SettingsApp {
    window: RefCell<Option<SettingsWindow>>,
}

impl SettingsApp {
    pub fn create() -> Result<Application> {
        let app = SettingsApp {
            window: RefCell::new(None),
        };
        XamlApp::compose(app)
    }
}

impl XamlAppOverrides for SettingsApp {
    fn OnLaunched(
        &self,
        _base: &Application,
        _args: Option<&LaunchActivatedEventArgs>,
    ) -> Result<()> {
        let window = SettingsWindow::new()?;
        window.initialize_component()?;
        window.activate()?;

        self.window.borrow_mut().replace(window);
        Ok(())
    }

    fn TryResolveXamlType(&self, full_name: &HSTRING) -> Result<IXamlType> {
        if full_name == "SettingsPage" {
            winui3::XamlCustomType::<SettingsPage>::new(full_name)
        } else {
            Err(windows::core::Error::empty())
        }
    }
}

/// 显示WinUI3设置窗口的便利函数（替换simple_settings中的函数）
pub fn show_settings_window() -> Result<()> {
    // 临时实现：启动独立的设置进程
    // 这样可以避免在主应用程序中初始化WinUI3的复杂性

    use std::process::Command;

    // 启动独立的设置应用程序
    let _child = Command::new("cargo")
        .args(&["run", "--bin", "settings-app"])
        .spawn();

    // 如果启动失败，回退到原始设置窗口
    if _child.is_err() {
        // 回退到simple_settings
        return crate::simple_settings::show_settings_window();
    }

    Ok(())
}

/// 检查设置窗口是否已经打开
pub fn is_settings_window_open() -> bool {
    // 临时实现：由于WinUI3应用程序是独立进程，这里总是返回false
    false
}
