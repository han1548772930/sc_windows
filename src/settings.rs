use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use windows::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::{LibraryLoader::*, SystemServices::*},
        UI::{Controls::*, WindowsAndMessaging::*},
    },
    core::*,
};

use crate::utils::to_wide_chars;

/// 应用程序设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    // 绘图设置
    pub default_line_thickness: f32,
    pub default_font_size: f32,
    pub default_color_red: f32,
    pub default_color_green: f32,
    pub default_color_blue: f32,

    // 截图设置
    pub auto_copy_to_clipboard: bool,
    pub show_cursor_in_screenshot: bool,
    pub screenshot_delay_ms: u32,

    // 界面设置
    pub toolbar_opacity: f32,
    pub selection_border_width: f32,
    pub handle_size: f32,

    // 热键设置
    pub hotkey_modifier: u32, // MOD_ALT, MOD_CTRL, etc.
    pub hotkey_key: u32,      // Virtual key code

    // 文件设置
    pub default_save_format: String, // "PNG", "JPEG", "BMP"
    pub jpeg_quality: u32,           // 1-100
    pub auto_save_to_folder: bool,
    pub save_folder_path: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            // 绘图设置默认值
            default_line_thickness: 3.0,
            default_font_size: 20.0,
            default_color_red: 1.0,
            default_color_green: 0.0,
            default_color_blue: 0.0,

            // 截图设置默认值
            auto_copy_to_clipboard: false,
            show_cursor_in_screenshot: false,
            screenshot_delay_ms: 0,

            // 界面设置默认值
            toolbar_opacity: 0.9,
            selection_border_width: 2.0,
            handle_size: 8.0,

            // 热键设置默认值
            hotkey_modifier: 1, // MOD_ALT
            hotkey_key: 'S' as u32,

            // 文件设置默认值
            default_save_format: "PNG".to_string(),
            jpeg_quality: 90,
            auto_save_to_folder: false,
            save_folder_path: String::new(),
        }
    }
}

impl AppSettings {
    /// 获取设置文件路径
    fn get_settings_path() -> PathBuf {
        let mut path = std::env::current_exe().unwrap_or_default();
        path.set_file_name("settings.json");
        path
    }

    /// 从文件加载设置
    pub fn load() -> Self {
        let path = Self::get_settings_path();

        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(settings) = serde_json::from_str::<AppSettings>(&content) {
                return settings;
            }
        }

        // 如果加载失败，返回默认设置并保存
        let default_settings = Self::default();
        let _ = default_settings.save();
        default_settings
    }

    /// 保存设置到文件
    pub fn save(&self) -> Result<()> {
        let path = Self::get_settings_path();
        let content = serde_json::to_string_pretty(self).map_err(|_| Error::from(E_FAIL))?;

        fs::write(&path, content).map_err(|_| Error::from(E_FAIL))?;

        Ok(())
    }

    /// 获取热键描述文本
    pub fn get_hotkey_text(&self) -> String {
        let modifier_text = match self.hotkey_modifier {
            1 => "Alt",   // MOD_ALT
            2 => "Ctrl",  // MOD_CONTROL
            4 => "Shift", // MOD_SHIFT
            8 => "Win",   // MOD_WIN
            _ => "Alt",
        };

        let key_char = char::from_u32(self.hotkey_key).unwrap_or('S');
        format!("{}+{}", modifier_text, key_char)
    }
}

/// 设置窗口管理器
pub struct SettingsWindow {
    hwnd: HWND,
    parent_hwnd: HWND,
    settings: AppSettings,
    controls: SettingsControls,
}

/// 设置窗口的控件句柄
#[derive(Default)]
struct SettingsControls {
    // 绘图设置控件
    line_thickness_edit: HWND,
    font_size_edit: HWND,
    color_red_edit: HWND,
    color_green_edit: HWND,
    color_blue_edit: HWND,

    // 截图设置控件
    auto_copy_checkbox: HWND,
    show_cursor_checkbox: HWND,
    delay_edit: HWND,

    // 界面设置控件
    toolbar_opacity_edit: HWND,
    border_width_edit: HWND,
    handle_size_edit: HWND,

    // 文件设置控件
    format_combo: HWND,
    quality_edit: HWND,
    auto_save_checkbox: HWND,
    folder_edit: HWND,

    // 按钮
    ok_button: HWND,
    cancel_button: HWND,
    reset_button: HWND,
}

// 控件ID常量
const ID_LINE_THICKNESS: i32 = 1001;
const ID_FONT_SIZE: i32 = 1002;
const ID_COLOR_RED: i32 = 1003;
const ID_COLOR_GREEN: i32 = 1004;
const ID_COLOR_BLUE: i32 = 1005;
const ID_AUTO_COPY: i32 = 1006;
const ID_SHOW_CURSOR: i32 = 1007;
const ID_DELAY: i32 = 1008;
const ID_TOOLBAR_OPACITY: i32 = 1009;
const ID_BORDER_WIDTH: i32 = 1010;
const ID_HANDLE_SIZE: i32 = 1011;
const ID_FORMAT_COMBO: i32 = 1012;
const ID_QUALITY: i32 = 1013;
const ID_AUTO_SAVE: i32 = 1014;
const ID_FOLDER: i32 = 1015;
const ID_OK: i32 = 1016;
const ID_CANCEL: i32 = 1017;
const ID_RESET: i32 = 1018;

impl SettingsWindow {
    /// 显示设置窗口
    pub fn show(parent_hwnd: HWND) -> Result<()> {
        unsafe {
            let instance = GetModuleHandleW(None)?;
            let class_name = to_wide_chars("ModernSettingsWindow");

            // 注册窗口类
            let window_class = WNDCLASSW {
                lpfnWndProc: Some(Self::window_proc),
                hInstance: instance.into(),
                lpszClassName: PCWSTR(class_name.as_ptr()),
                hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as *mut _),
                hCursor: LoadCursorW(None, IDC_ARROW)?,
                style: CS_HREDRAW | CS_VREDRAW,
                ..Default::default()
            };

            RegisterClassW(&window_class);

            // 创建现代化设置窗口
            let hwnd = CreateWindowExW(
                WS_EX_DLGMODALFRAME | WS_EX_CONTROLPARENT,
                PCWSTR(class_name.as_ptr()),
                PCWSTR(to_wide_chars("截图工具 - 设置").as_ptr()),
                WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                580,
                480,
                Some(parent_hwnd),
                None,
                Some(instance.into()),
                None,
            )?;

            // 居中显示窗口
            let mut rect = RECT::default();
            GetWindowRect(hwnd, &mut rect);
            let width = rect.right - rect.left;
            let height = rect.bottom - rect.top;
            let screen_width = GetSystemMetrics(SM_CXSCREEN);
            let screen_height = GetSystemMetrics(SM_CYSCREEN);
            let x = (screen_width - width) / 2;
            let y = (screen_height - height) / 2;
            SetWindowPos(hwnd, None, x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER);

            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = UpdateWindow(hwnd);

            Ok(())
        }
    }

    /// 设置窗口过程
    unsafe extern "system" fn window_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        unsafe {
            match msg {
                WM_CREATE => {
                    // 创建设置窗口实例
                    let settings = AppSettings::load();
                    let mut window = SettingsWindow {
                        hwnd,
                        parent_hwnd: HWND::default(),
                        settings,
                        controls: SettingsControls::default(),
                    };

                    // 创建控件
                    window.create_controls();
                    window.load_values_to_controls();

                    // 保存窗口实例到窗口数据
                    let window_box = Box::new(window);
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(window_box) as isize);

                    LRESULT(0)
                }

                WM_COMMAND => {
                    let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SettingsWindow;
                    if !window_ptr.is_null() {
                        let window = &mut *window_ptr;
                        window.handle_command((wparam.0 & 0xFFFF) as i32);
                    }
                    LRESULT(0)
                }

                WM_CLOSE => {
                    let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SettingsWindow;
                    if !window_ptr.is_null() {
                        let _window = Box::from_raw(window_ptr);
                    }
                    DestroyWindow(hwnd);
                    LRESULT(0)
                }

                _ => DefWindowProcW(hwnd, msg, wparam, lparam),
            }
        }
    }

    /// 创建控件
    fn create_controls(&mut self) {
        unsafe {
            let instance = GetModuleHandleW(None).unwrap().into();

            // 创建分组框 - 绘图设置
            let _ = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("BUTTON").as_ptr()),
                PCWSTR(to_wide_chars("🎨 绘图工具设置").as_ptr()),
                WINDOW_STYLE(WS_VISIBLE.0 | WS_CHILD.0 | BS_GROUPBOX.0),
                20,
                20,
                260,
                140,
                Some(self.hwnd),
                None,
                Some(instance),
                None,
            );

            // 线条粗细
            let _ = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("STATIC").as_ptr()),
                PCWSTR(to_wide_chars("线条粗细 (1-20):").as_ptr()),
                WS_VISIBLE | WS_CHILD,
                35,
                50,
                120,
                20,
                Some(self.hwnd),
                None,
                Some(instance),
                None,
            );

            self.controls.line_thickness_edit = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                PCWSTR(to_wide_chars("EDIT").as_ptr()),
                PCWSTR::null(),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP | ES_NUMBER,
                170,
                48,
                80,
                24,
                Some(self.hwnd),
                Some(HMENU(ID_LINE_THICKNESS as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();

            // 字体大小
            let _ = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("STATIC").as_ptr()),
                PCWSTR(to_wide_chars("字体大小 (8-72):").as_ptr()),
                WS_VISIBLE | WS_CHILD,
                35,
                80,
                120,
                20,
                Some(self.hwnd),
                None,
                Some(instance),
                None,
            );

            self.controls.font_size_edit = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                PCWSTR(to_wide_chars("EDIT").as_ptr()),
                PCWSTR::null(),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP | ES_NUMBER,
                170,
                78,
                80,
                24,
                Some(self.hwnd),
                Some(HMENU(ID_FONT_SIZE as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();

            // 默认颜色
            let _ = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("STATIC").as_ptr()),
                PCWSTR(to_wide_chars("默认颜色 (RGB):").as_ptr()),
                WS_VISIBLE | WS_CHILD,
                35,
                110,
                120,
                20,
                Some(self.hwnd),
                None,
                Some(instance),
                None,
            );

            // RGB输入框
            self.controls.color_red_edit = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                PCWSTR(to_wide_chars("EDIT").as_ptr()),
                PCWSTR::null(),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP | ES_NUMBER,
                170,
                108,
                40,
                24,
                Some(self.hwnd),
                Some(HMENU(ID_COLOR_RED as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();

            self.controls.color_green_edit = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                PCWSTR(to_wide_chars("EDIT").as_ptr()),
                PCWSTR::null(),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP | ES_NUMBER,
                215,
                108,
                40,
                24,
                Some(self.hwnd),
                Some(HMENU(ID_COLOR_GREEN as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();

            // 截图设置分组
            let _ = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("BUTTON").as_ptr()),
                PCWSTR(to_wide_chars("📷 截图选项").as_ptr()),
                WS_VISIBLE | WS_CHILD | BS_GROUPBOX,
                300,
                20,
                260,
                140,
                Some(self.hwnd),
                None,
                Some(instance),
                None,
            );

            // 自动复制到剪贴板
            self.controls.auto_copy_checkbox = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("BUTTON").as_ptr()),
                PCWSTR(to_wide_chars("自动复制到剪贴板").as_ptr()),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP | BS_AUTOCHECKBOX,
                315,
                50,
                200,
                24,
                Some(self.hwnd),
                Some(HMENU(ID_AUTO_COPY as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();

            // 显示鼠标光标
            self.controls.show_cursor_checkbox = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("BUTTON").as_ptr()),
                PCWSTR(to_wide_chars("截图时显示光标").as_ptr()),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP | BS_AUTOCHECKBOX,
                315,
                80,
                200,
                24,
                Some(self.hwnd),
                Some(HMENU(ID_SHOW_CURSOR as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();

            // 截图延迟
            let _ = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("STATIC").as_ptr()),
                PCWSTR(to_wide_chars("延迟 (毫秒):").as_ptr()),
                WS_VISIBLE | WS_CHILD,
                315,
                115,
                100,
                20,
                Some(self.hwnd),
                None,
                Some(instance),
                None,
            );

            self.controls.delay_edit = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                PCWSTR(to_wide_chars("EDIT").as_ptr()),
                PCWSTR::null(),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP | ES_NUMBER,
                430,
                113,
                80,
                24,
                Some(self.hwnd),
                Some(HMENU(ID_DELAY as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();

            // 界面设置分组
            let _ = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("BUTTON").as_ptr()),
                PCWSTR(to_wide_chars("🎛️ 界面设置").as_ptr()),
                WS_VISIBLE | WS_CHILD | BS_GROUPBOX,
                20,
                180,
                540,
                100,
                Some(self.hwnd),
                None,
                Some(instance),
                None,
            );

            // 工具栏透明度
            let _ = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("STATIC").as_ptr()),
                PCWSTR(to_wide_chars("工具栏透明度 (0.1-1.0):").as_ptr()),
                WS_VISIBLE | WS_CHILD,
                35,
                210,
                150,
                20,
                Some(self.hwnd),
                None,
                Some(instance),
                None,
            );

            self.controls.toolbar_opacity_edit = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                PCWSTR(to_wide_chars("EDIT").as_ptr()),
                PCWSTR::null(),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP,
                200,
                208,
                80,
                24,
                Some(self.hwnd),
                Some(HMENU(ID_TOOLBAR_OPACITY as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();

            // 选择框边框宽度
            let _ = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("STATIC").as_ptr()),
                PCWSTR(to_wide_chars("边框宽度 (1-5):").as_ptr()),
                WS_VISIBLE | WS_CHILD,
                300,
                210,
                120,
                20,
                Some(self.hwnd),
                None,
                Some(instance),
                None,
            );

            self.controls.border_width_edit = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                PCWSTR(to_wide_chars("EDIT").as_ptr()),
                PCWSTR::null(),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP | ES_NUMBER,
                430,
                208,
                80,
                24,
                Some(self.hwnd),
                Some(HMENU(ID_BORDER_WIDTH as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();

            // 控制点大小
            let _ = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("STATIC").as_ptr()),
                PCWSTR(to_wide_chars("控制点大小 (4-16):").as_ptr()),
                WS_VISIBLE | WS_CHILD,
                35,
                245,
                150,
                20,
                Some(self.hwnd),
                None,
                Some(instance),
                None,
            );

            self.controls.handle_size_edit = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                PCWSTR(to_wide_chars("EDIT").as_ptr()),
                PCWSTR::null(),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP | ES_NUMBER,
                200,
                243,
                80,
                24,
                Some(self.hwnd),
                Some(HMENU(ID_HANDLE_SIZE as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();

            // 文件设置分组
            let _ = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("BUTTON").as_ptr()),
                PCWSTR(to_wide_chars("💾 文件设置").as_ptr()),
                WS_VISIBLE | WS_CHILD | BS_GROUPBOX,
                20,
                300,
                540,
                80,
                Some(self.hwnd),
                None,
                Some(instance),
                None,
            );

            // 默认保存格式
            let _ = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("STATIC").as_ptr()),
                PCWSTR(to_wide_chars("默认格式:").as_ptr()),
                WS_VISIBLE | WS_CHILD,
                35,
                330,
                80,
                20,
                Some(self.hwnd),
                None,
                Some(instance),
                None,
            );

            self.controls.format_combo = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                PCWSTR(to_wide_chars("COMBOBOX").as_ptr()),
                PCWSTR::null(),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP | CBS_DROPDOWNLIST,
                130,
                328,
                100,
                100,
                Some(self.hwnd),
                Some(HMENU(ID_FORMAT_COMBO as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();

            // 添加格式选项
            let formats = ["PNG", "JPEG", "BMP"];
            for format in &formats {
                let format_wide = to_wide_chars(format);
                let _ = SendMessageW(
                    self.controls.format_combo,
                    CB_ADDSTRING,
                    WPARAM(0),
                    LPARAM(format_wide.as_ptr() as isize),
                );
            }

            // JPEG质量
            let _ = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("STATIC").as_ptr()),
                PCWSTR(to_wide_chars("JPEG质量 (1-100):").as_ptr()),
                WS_VISIBLE | WS_CHILD,
                250,
                330,
                120,
                20,
                Some(self.hwnd),
                None,
                Some(instance),
                None,
            );

            self.controls.quality_edit = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                PCWSTR(to_wide_chars("EDIT").as_ptr()),
                PCWSTR::null(),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP | ES_NUMBER,
                380,
                328,
                80,
                24,
                Some(self.hwnd),
                Some(HMENU(ID_QUALITY as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();

            // 底部按钮
            self.controls.reset_button = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("BUTTON").as_ptr()),
                PCWSTR(to_wide_chars("🔄 重置默认").as_ptr()),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP,
                30,
                400,
                100,
                32,
                Some(self.hwnd),
                Some(HMENU(ID_RESET as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();

            self.controls.ok_button = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("BUTTON").as_ptr()),
                PCWSTR(to_wide_chars("✅ 确定").as_ptr()),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP | BS_DEFPUSHBUTTON,
                370,
                400,
                80,
                32,
                Some(self.hwnd),
                Some(HMENU(ID_OK as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();

            self.controls.cancel_button = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("BUTTON").as_ptr()),
                PCWSTR(to_wide_chars("❌ 取消").as_ptr()),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP,
                460,
                400,
                80,
                32,
                Some(self.hwnd),
                Some(HMENU(ID_CANCEL as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();
        }
    }

    /// 加载设置值到控件
    fn load_values_to_controls(&self) {
        unsafe {
            // 加载线条粗细
            let thickness_text = to_wide_chars(&self.settings.default_line_thickness.to_string());
            let _ = SetWindowTextW(
                self.controls.line_thickness_edit,
                PCWSTR(thickness_text.as_ptr()),
            );

            // 加载字体大小
            let font_size_text = to_wide_chars(&self.settings.default_font_size.to_string());
            let _ = SetWindowTextW(
                self.controls.font_size_edit,
                PCWSTR(font_size_text.as_ptr()),
            );
        }
    }

    /// 处理命令消息
    fn handle_command(&mut self, command_id: i32) {
        match command_id {
            ID_OK => {
                // 保存设置并关闭窗口
                self.save_settings_from_controls();
                unsafe {
                    let _ = self.settings.save();
                    let _ = DestroyWindow(self.hwnd);
                }
            }
            ID_CANCEL => {
                // 取消并关闭窗口
                unsafe {
                    let _ = DestroyWindow(self.hwnd);
                }
            }
            ID_RESET => {
                // 重置为默认值
                self.settings = AppSettings::default();
                self.load_values_to_controls();
            }
            _ => {}
        }
    }

    /// 从控件保存设置
    fn save_settings_from_controls(&mut self) {
        unsafe {
            // 读取线条粗细
            let mut buffer = [0u16; 32];
            if GetWindowTextW(self.controls.line_thickness_edit, &mut buffer) > 0 {
                let text = String::from_utf16_lossy(&buffer);
                let text = text.trim_end_matches('\0');
                if let Ok(value) = text.parse::<f32>() {
                    self.settings.default_line_thickness = value.max(1.0).min(20.0);
                }
            }

            // 读取字体大小
            if GetWindowTextW(self.controls.font_size_edit, &mut buffer) > 0 {
                let text = String::from_utf16_lossy(&buffer);
                let text = text.trim_end_matches('\0');
                if let Ok(value) = text.parse::<f32>() {
                    self.settings.default_font_size = value.max(8.0).min(72.0);
                }
            }
        }
    }
}
