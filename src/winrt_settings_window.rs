// WinRT 原生 API 实现的 WinUI3 设置窗口
//
// 基于 winui3-rs 库，使用正确的 WinRT 调用方法

use std::cell::RefCell;
use std::sync::Arc;
use windows::Foundation::Uri;
use windows::core::Result;
use windows_core::{HSTRING, IInspectable, Interface, h};
use winui3::{
    Activatable,
    Microsoft::UI::Xaml::{
        Application, ApplicationInitializationCallback, ApplicationInitializationCallbackParams,
        Controls::{
            Button, ColorPicker, ComboBox, ComboBoxItem, Expander, Frame, Grid, NumberBox,
            Orientation, Page, RowDefinition, ScrollBarVisibility, ScrollViewer, Slider,
            StackPanel, TextBlock, TextBox, TitleBar, ToggleSwitch, XamlControlsResources,
        },
        DispatcherShutdownMode, GridLength, GridLengthHelper, GridUnitType, HorizontalAlignment,
        LaunchActivatedEventArgs,
        Markup::IXamlType,
        Media::{Brush, MicaBackdrop, SolidColorBrush},
        Navigation::{NavigatingCancelEventArgs, NavigationEventArgs},
        ResourceDictionary, Style, Thickness, ThicknessHelper, UnhandledExceptionEventArgs,
        UnhandledExceptionEventHandler, VerticalAlignment, Visibility, Window,
    },
    XamlApp, XamlAppOverrides, XamlPage, XamlPageOverrides,
    bootstrap::{PackageDependency as WinUIDependency, WindowsAppSDKVersion},
    xaml_typename,
};

use crate::settings::Settings;

// 辅助函数，类似于 rust-winui 项目中的 utils
use windows::Foundation::{IReference, PropertyValue};

#[allow(non_snake_case)]
fn HStringReference(text: &HSTRING) -> Result<IReference<HSTRING>> {
    PropertyValue::CreateString(text)?.cast()
}

/// WinRT 设置应用程序
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
    fn OnLaunched(&self, base: &Application, _: Option<&LaunchActivatedEventArgs>) -> Result<()> {
        // 设置资源
        let resources = base.Resources()?;
        let merged_dictionaries = resources.MergedDictionaries()?;
        let xaml_controls_resources = XamlControlsResources::new()?;
        merged_dictionaries.Append(&xaml_controls_resources)?;

        // 添加紧凑样式
        let compact_resources = ResourceDictionary::new()?;
        compact_resources.SetSource(&Uri::CreateUri(h!(
            "ms-appx:///Microsoft.UI.Xaml/DensityStyles/Compact.xaml"
        ))?)?;
        merged_dictionaries.Append(&compact_resources)?;

        // 创建设置窗口
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
            Err(windows_core::Error::empty())
        }
    }
}

/// WinRT 设置窗口
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
        self.window.SetExtendsContentIntoTitleBar(true)?;

        // 设置 Mica 背景
        if let Ok(mica_backdrop) = MicaBackdrop::new() {
            let _ = self.window.SetSystemBackdrop(&mica_backdrop);
        }

        // 创建主布局
        let grid = Grid::new()?;
        let row_definitions = grid.RowDefinitions()?;
        let grid_children = grid.Children()?;

        // 标题栏行
        let row0 = RowDefinition::new()?;
        row0.SetHeight(GridLengthHelper::Auto()?)?;
        row_definitions.Append(&row0)?;

        // 内容行
        let row1 = RowDefinition::new()?;
        row1.SetHeight(GridLengthHelper::FromValueAndType(1.0, GridUnitType::Star)?)?;
        row_definitions.Append(&row1)?;

        // 创建标题栏
        let titlebar = TitleBar::new()?;
        Grid::SetRow(&titlebar, 0)?;
        titlebar.SetTitle(h!("应用程序设置"))?;
        grid_children.Append(&titlebar)?;
        self.window.SetTitleBar(&titlebar)?;

        // 创建 Frame 来承载设置页面
        let frame = Frame::new()?;
        Grid::SetRow(&frame, 1)?;
        grid_children.Append(&frame)?;

        // 导航到设置页面
        let page_type = xaml_typename("SettingsPage");
        let _ = frame.Navigate2(&page_type);

        self.window.SetContent(&grid)?;
        Ok(())
    }

    pub fn activate(&self) -> Result<()> {
        self.window.Activate()
    }
}

/// WinRT 设置页面
pub struct SettingsPage {
    settings: Arc<Settings>,
    // 控件引用
    line_thickness_slider: RefCell<Option<Slider>>,
    font_size_slider: RefCell<Option<Slider>>,
    auto_copy_toggle: RefCell<Option<ToggleSwitch>>,
    show_cursor_toggle: RefCell<Option<ToggleSwitch>>,
    delay_number: RefCell<Option<NumberBox>>,
    drawing_color_picker: RefCell<Option<ColorPicker>>,
    text_color_picker: RefCell<Option<ColorPicker>>,
    config_path_textbox: RefCell<Option<TextBox>>,
    ocr_language_combo: RefCell<Option<ComboBox>>,
}

impl SettingsPage {
    pub fn new() -> Self {
        Self {
            settings: Arc::new(Settings::load()),
            line_thickness_slider: RefCell::new(None),
            font_size_slider: RefCell::new(None),
            auto_copy_toggle: RefCell::new(None),
            show_cursor_toggle: RefCell::new(None),
            delay_number: RefCell::new(None),
            drawing_color_picker: RefCell::new(None),
            text_color_picker: RefCell::new(None),
            config_path_textbox: RefCell::new(None),
            ocr_language_combo: RefCell::new(None),
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
        scroll_viewer.SetVerticalScrollBarVisibility(ScrollBarVisibility::Auto)?;
        scroll_viewer.SetHorizontalScrollBarVisibility(ScrollBarVisibility::Disabled)?;

        // 创建主 Grid 布局
        let main_grid = Grid::new()?;
        main_grid.SetMargin(ThicknessHelper::FromUniformLength(20.0)?)?;

        // 创建主 StackPanel
        let main_stack = StackPanel::new()?;
        main_stack.SetSpacing(16.0)?;

        // 添加标题
        let title = TextBlock::new()?;
        title.SetText(h!("应用程序设置"))?;
        title.SetFontSize(28.0)?;
        title.SetFontWeight(winui3::Microsoft::UI::Text::FontWeights::Bold()?)?;
        title.SetMargin(ThicknessHelper::FromLengths(0.0, 0.0, 0.0, 20.0)?)?;

        main_stack.Children()?.Append(&title)?;

        // 创建设置组
        self.create_basic_settings_group(&main_stack)?;
        self.create_color_settings_group(&main_stack)?;
        self.create_path_settings_group(&main_stack)?;
        self.create_ocr_settings_group(&main_stack)?;
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

/// 初始化 WinRT 设置窗口
pub fn initialize_winrt_settings() -> Result<()> {
    // 初始化 WinRT 环境
    winui3::init_apartment(winui3::ApartmentType::SingleThreaded)?;

    // 初始化 WinUI3 依赖
    let _winui_dependency = WinUIDependency::initialize_version(WindowsAppSDKVersion::V1_7)
        .or_else(|_| WinUIDependency::initialize_version(WindowsAppSDKVersion::V1_6))
        .or_else(|_| WinUIDependency::initialize_version(WindowsAppSDKVersion::V1_5))?;

    // 启动应用程序
    Application::Start(&ApplicationInitializationCallback::new(app_start))?;

    Ok(())
}

fn app_start(_: windows_core::Ref<'_, ApplicationInitializationCallbackParams>) -> Result<()> {
    let app = SettingsApp::create()?;
    app.SetDispatcherShutdownMode(DispatcherShutdownMode::OnLastWindowClose)?;
    app.UnhandledException(Some(&UnhandledExceptionEventHandler::new(
        unhandled_exception_handler,
    )))?;

    Ok(())
}

fn unhandled_exception_handler(
    _sender: windows_core::Ref<'_, IInspectable>,
    _args: windows_core::Ref<'_, UnhandledExceptionEventArgs>,
) -> Result<()> {
    // 静默处理异常
    Ok(())
}

impl SettingsPage {
    /// 创建基础设置组
    fn create_basic_settings_group(&self, parent: &StackPanel) -> Result<()> {
        let expander = Expander::new()?;
        expander.SetHeader(&HStringReference(h!("基础设置"))?)?;
        expander.SetIsExpanded(true)?;

        let content_stack = StackPanel::new()?;
        content_stack.SetSpacing(12.0)?;
        content_stack.SetMargin(ThicknessHelper::FromLengths(16.0, 8.0, 16.0, 8.0)?)?;

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
        expander.SetHeader(&HStringReference(h!("颜色设置"))?)?;
        expander.SetIsExpanded(true)?;

        let content_stack = StackPanel::new()?;
        content_stack.SetSpacing(12.0)?;
        content_stack.SetMargin(ThicknessHelper::FromLengths(16.0, 8.0, 16.0, 8.0)?)?;

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
        expander.SetHeader(&HStringReference(h!("路径设置"))?)?;
        expander.SetIsExpanded(false)?;

        let content_stack = StackPanel::new()?;
        content_stack.SetSpacing(12.0)?;
        content_stack.SetMargin(ThicknessHelper::FromLengths(16.0, 8.0, 16.0, 8.0)?)?;

        // 配置文件路径
        self.create_path_setting(&content_stack, "配置文件路径", &self.settings.config_path)?;

        expander.SetContent(&content_stack)?;
        parent.Children()?.Append(&expander)?;

        Ok(())
    }

    /// 创建 OCR 设置组
    fn create_ocr_settings_group(&self, parent: &StackPanel) -> Result<()> {
        let expander = Expander::new()?;
        expander.SetHeader(&HStringReference(h!("OCR设置"))?)?;
        expander.SetIsExpanded(false)?;

        let content_stack = StackPanel::new()?;
        content_stack.SetSpacing(12.0)?;
        content_stack.SetMargin(ThicknessHelper::FromLengths(16.0, 8.0, 16.0, 8.0)?)?;

        // OCR 语言选择
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
        button_stack.SetMargin(ThicknessHelper::FromLengths(0.0, 20.0, 0.0, 0.0)?)?;

        // 确定按钮
        let ok_button = Button::new()?;
        ok_button.SetContent(&HStringReference(h!("确定"))?)?;
        ok_button.SetMinWidth(100.0)?;
        // 设置为强调按钮样式
        if let Ok(app) = Application::Current() {
            if let Ok(resources) = app.Resources() {
                if let Ok(style) = resources.Lookup(&HStringReference(h!("AccentButtonStyle"))?) {
                    if let Ok(button_style) = style.cast::<Style>() {
                        let _ = ok_button.SetStyle(&button_style);
                    }
                }
            }
        }

        // 取消按钮
        let cancel_button = Button::new()?;
        cancel_button.SetContent(&HStringReference(h!("取消"))?)?;
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
        value_number.SetValue2(value)?;
        value_number.SetMinimum(min)?;
        value_number.SetMaximum(max)?;
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
        slider.SetValue2(value)?;
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
        number_box.SetValue2(value)?;
        number_box.SetMinimum(min)?;
        number_box.SetMaximum(max)?;
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
        // 设置初始颜色 - 使用默认颜色，因为 FromArgb 方法不可用
        // TODO: 实现正确的颜色设置
        // color_picker.SetColor(initial_color)?;
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
        browse_button.SetContent(&HStringReference(h!("浏览..."))?)?;

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
            let item = ComboBoxItem::new()?;
            item.SetContent(&HStringReference(&HSTRING::from(name))?)?;
            item.SetTag(&HStringReference(&HSTRING::from(code))?)?;
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
}

/// 显示 WinRT 设置窗口的便利函数
pub fn show_winrt_settings_window() -> Result<()> {
    // 在独立线程中启动设置窗口，避免阻塞主应用程序
    std::thread::spawn(|| {
        let _ = initialize_winrt_settings();
    });

    Ok(())
}
