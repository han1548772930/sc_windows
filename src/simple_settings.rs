use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::{ffi::c_void, fs};

use windows::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::LibraryLoader::*,
        UI::{Controls::Dialogs::*, Controls::*, WindowsAndMessaging::*},
    },
    core::*,
};

use crate::utils::to_wide_chars;

/// 简化的应用程序设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleSettings {
    // 基础设置
    pub line_thickness: f32,
    pub font_size: f32,
    pub auto_copy: bool,
    pub show_cursor: bool,
    pub delay_ms: u32,

    // 字体设置
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
    // 配置文件路径
    #[serde(default = "default_config_path")]
    pub config_path: String,

    // OCR语言设置
    #[serde(default = "default_ocr_language")]
    pub ocr_language: String,

    // 颜色设置 - 保留旧字段以向后兼容
    #[serde(default = "default_color_red")]
    pub color_red: u8,
    #[serde(default = "default_color_green")]
    pub color_green: u8,
    #[serde(default = "default_color_blue")]
    pub color_blue: u8,

    // 新的分离颜色设置
    #[serde(default = "default_drawing_color_red")]
    pub drawing_color_red: u8,
    #[serde(default = "default_drawing_color_green")]
    pub drawing_color_green: u8,
    #[serde(default = "default_drawing_color_blue")]
    pub drawing_color_blue: u8,

    #[serde(default = "default_text_color_red")]
    pub text_color_red: u8,
    #[serde(default = "default_text_color_green")]
    pub text_color_green: u8,
    #[serde(default = "default_text_color_blue")]
    pub text_color_blue: u8,

    // 热键设置
    #[serde(default = "default_hotkey_modifiers")]
    pub hotkey_modifiers: u32,
    #[serde(default = "default_hotkey_key")]
    pub hotkey_key: u32,
}

// 默认值函数
fn default_color_red() -> u8 {
    255
}
fn default_color_green() -> u8 {
    0
}
fn default_color_blue() -> u8 {
    0
}

fn default_drawing_color_red() -> u8 {
    255
}
fn default_drawing_color_green() -> u8 {
    0
}
fn default_drawing_color_blue() -> u8 {
    0
}

fn default_text_color_red() -> u8 {
    255
}
fn default_text_color_green() -> u8 {
    255
}
fn default_text_color_blue() -> u8 {
    255
}

fn default_hotkey_modifiers() -> u32 {
    use windows::Win32::UI::Input::KeyboardAndMouse::*;
    (MOD_CONTROL | MOD_ALT).0
}
fn default_hotkey_key() -> u32 {
    'S' as u32
}

// 字体默认值函数
fn default_font_name() -> String {
    "Microsoft Sans Serif".to_string()
}
fn default_font_weight() -> i32 {
    400 // FW_NORMAL
}
fn default_font_italic() -> bool {
    false
}
fn default_font_underline() -> bool {
    false
}
fn default_font_strikeout() -> bool {
    false
}
fn default_font_color() -> (u8, u8, u8) {
    (0, 0, 0) // 黑色
}
fn default_config_path() -> String {
    // 优先使用用户主目录
    if let Ok(home_dir) = std::env::var("USERPROFILE") {
        return home_dir;
    }

    // 备用方案：获取当前程序所在目录
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            return exe_dir.to_string_lossy().to_string();
        }
    }

    // 最后的备用路径：当前工作目录
    std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .to_string_lossy()
        .to_string()
}

fn default_ocr_language() -> String {
    "chinese".to_string() // 默认使用简体中文
}

impl Default for SimpleSettings {
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
            drawing_color_red: 255,
            drawing_color_green: 0,
            drawing_color_blue: 0,
            text_color_red: 255,
            text_color_green: 255,
            text_color_blue: 255,
            hotkey_modifiers: {
                use windows::Win32::UI::Input::KeyboardAndMouse::*;
                (MOD_CONTROL | MOD_ALT).0
            },
            hotkey_key: 'S' as u32,
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

impl SimpleSettings {
    /// 获取设置文件路径
    fn get_settings_path() -> PathBuf {
        // 优先使用用户配置目录（USERPROFILE）
        let default_path = if let Ok(user_profile) = std::env::var("USERPROFILE") {
            let mut path = PathBuf::from(user_profile);
            path.push(".ocr_screenshot_tool");
            // 确保目录存在
            if let Err(_) = std::fs::create_dir_all(&path) {
                // 如果创建失败，回退到程序目录
                let mut fallback_path = std::env::current_exe().unwrap_or_default();
                fallback_path.set_file_name("simple_settings.json");
                return fallback_path;
            }
            path.push("simple_settings.json");
            path
        } else {
            // 如果无法获取USERPROFILE，使用程序目录
            let mut path = std::env::current_exe().unwrap_or_default();
            path.set_file_name("simple_settings.json");
            path
        };

        // 如果配置文件存在，尝试读取其中的config_path设置
        if let Ok(content) = fs::read_to_string(&default_path) {
            if let Ok(settings) = serde_json::from_str::<SimpleSettings>(&content) {
                if !settings.config_path.is_empty() {
                    let mut custom_path = PathBuf::from(&settings.config_path);
                    custom_path.push("simple_settings.json");
                    return custom_path;
                }
            }
        }

        // 如果无法读取或路径为空，使用默认路径
        default_path
    }

    /// 从文件加载设置
    pub fn load() -> Self {
        let path = Self::get_settings_path();

        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(mut settings) = serde_json::from_str::<SimpleSettings>(&content) {
                // 数据迁移：如果新字段使用默认值，但旧字段有自定义值，则迁移
                settings.migrate_from_legacy();
                return settings;
            }
        }

        // 如果加载失败，返回默认设置并保存
        let default_settings = Self::default();
        let _ = default_settings.save();
        default_settings
    }

    /// 从旧版本设置迁移数据
    fn migrate_from_legacy(&mut self) {
        // 如果绘图颜色是默认值，但旧颜色不是默认值，则迁移
        if self.drawing_color_red == default_drawing_color_red()
            && self.drawing_color_green == default_drawing_color_green()
            && self.drawing_color_blue == default_drawing_color_blue()
            && (self.color_red != default_color_red()
                || self.color_green != default_color_green()
                || self.color_blue != default_color_blue())
        {
            self.drawing_color_red = self.color_red;
            self.drawing_color_green = self.color_green;
            self.drawing_color_blue = self.color_blue;
        }
    }

    /// 保存设置到文件
    pub fn save(&self) -> Result<()> {
        let path = Self::get_settings_path();
        let content = serde_json::to_string_pretty(self).map_err(|_| Error::from(E_FAIL))?;

        fs::write(&path, content).map_err(|_| Error::from(E_FAIL))?;

        Ok(())
    }

    /// 获取热键的字符串表示
    pub fn get_hotkey_string(&self) -> String {
        use windows::Win32::UI::Input::KeyboardAndMouse::*;

        let mut parts = Vec::new();

        if self.hotkey_modifiers & MOD_CONTROL.0 != 0 {
            parts.push("Ctrl");
        }
        if self.hotkey_modifiers & MOD_ALT.0 != 0 {
            parts.push("Alt");
        }
        if self.hotkey_modifiers & MOD_SHIFT.0 != 0 {
            parts.push("Shift");
        }

        // 将虚拟键码转换为字符
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

    /// 从热键字符串解析设置
    pub fn parse_hotkey_string(&mut self, hotkey_str: &str) -> bool {
        use windows::Win32::UI::Input::KeyboardAndMouse::*;

        let parts: Vec<&str> = hotkey_str.split('+').map(|s| s.trim()).collect();
        if parts.is_empty() {
            return false;
        }

        let mut modifiers = 0u32;
        let mut key = 0u32;

        for part in &parts {
            match part.to_lowercase().as_str() {
                "ctrl" | "control" => modifiers |= MOD_CONTROL.0,
                "alt" => modifiers |= MOD_ALT.0,
                "shift" => modifiers |= MOD_SHIFT.0,
                key_str if key_str.len() == 1 => {
                    let ch = key_str.chars().next().unwrap().to_ascii_uppercase();
                    if ch.is_ascii_alphanumeric() {
                        key = ch as u32;
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

/// 全局设置窗口句柄，确保只能打开一个设置窗口
static mut SETTINGS_WINDOW: Option<HWND> = None;

/// 简单设置窗口
pub struct SimpleSettingsWindow {
    hwnd: HWND,
    settings: SimpleSettings,
    // 控件句柄
    line_thickness_edit: HWND,
    font_size_edit: HWND,
    font_choose_button: HWND,
    // 颜色相关控件
    drawing_color_button: HWND,
    drawing_color_preview: HWND,
    text_color_button: HWND,
    text_color_preview: HWND,
    // 热键控件
    hotkey_edit: HWND,
    // 配置路径控件
    config_path_edit: HWND,
    config_path_browse_button: HWND,
    // OCR语言选择控件
    ocr_language_combo: HWND,
    // 按钮
    ok_button: HWND,
    cancel_button: HWND,
    // 字体句柄
    font: HFONT,
}

// 控件ID
const ID_LINE_THICKNESS: i32 = 1001;
const ID_FONT_SIZE: i32 = 1002;
const ID_FONT_CHOOSE_BUTTON: i32 = 1003;
const ID_DRAWING_COLOR_BUTTON: i32 = 1006;
const ID_TEXT_COLOR_BUTTON: i32 = 1007;
const ID_HOTKEY_EDIT: i32 = 1008;
const ID_CONFIG_PATH_EDIT: i32 = 1011;
const ID_CONFIG_PATH_BROWSE: i32 = 1012;
const ID_OCR_LANGUAGE_COMBO: i32 = 1013;
const ID_OK: i32 = 1009;
const ID_CANCEL: i32 = 1010;

impl SimpleSettingsWindow {
    /// 检查设置窗口是否已经打开
    pub fn is_open() -> bool {
        unsafe {
            if let Some(hwnd) = SETTINGS_WINDOW {
                IsWindow(Some(hwnd)).as_bool()
            } else {
                false
            }
        }
    }
    /// 热键输入框的窗口过程
    unsafe extern "system" fn hotkey_edit_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        use windows::Win32::UI::Input::KeyboardAndMouse::*;
        use windows::Win32::UI::WindowsAndMessaging::*;

        unsafe {
            match msg {
                WM_LBUTTONDOWN => {
                    // 保存当前文本作为原始文本
                    let mut buffer = [0u16; 64];
                    let len = GetWindowTextW(hwnd, &mut buffer);
                    if len > 0 {
                        let current_text = String::from_utf16_lossy(&buffer[..len as usize]);
                        let text_wide = to_wide_chars(&current_text);
                        let prop_name = to_wide_chars("OriginalText");
                        // 使用SetPropW存储原始文本指针
                        let text_box = Box::new(text_wide);
                        let text_ptr = Box::into_raw(text_box);
                        let _ = SetPropW(
                            hwnd,
                            PCWSTR(prop_name.as_ptr()),
                            Some(HANDLE(text_ptr as *mut c_void)),
                        );
                    }

                    // 当用户点击输入框时，清空内容并设置placeholder文本
                    let placeholder_text = to_wide_chars("按下快捷键");
                    let _ = SetWindowTextW(hwnd, PCWSTR(placeholder_text.as_ptr()));

                    // 设置焦点到输入框以便接收按键事件
                    let _ = SetFocus(Some(hwnd));

                    return LRESULT(0);
                }
                WM_KILLFOCUS => {
                    // 检查当前文本是否是有效的热键
                    let mut buffer = [0u16; 64];
                    let len = GetWindowTextW(hwnd, &mut buffer);
                    let current_text = if len > 0 {
                        String::from_utf16_lossy(&buffer[..len as usize])
                    } else {
                        String::new()
                    };

                    // 如果当前文本是placeholder或者空，则恢复原始文本
                    if current_text.trim() == "按下快捷键" || current_text.trim().is_empty() {
                        let prop_name = to_wide_chars("OriginalText");
                        let text_handle = GetPropW(hwnd, PCWSTR(prop_name.as_ptr()));
                        if !text_handle.is_invalid() {
                            let text_ptr = text_handle.0 as *mut Vec<u16>;
                            if !text_ptr.is_null() {
                                let text_box = Box::from_raw(text_ptr);
                                let _ = SetWindowTextW(hwnd, PCWSTR(text_box.as_ptr()));
                                // 清理属性
                                let _ = RemovePropW(hwnd, PCWSTR(prop_name.as_ptr()));
                            }
                        }
                    } else {
                        // 如果是有效的热键文本，清理存储的原始文本
                        let prop_name = to_wide_chars("OriginalText");
                        let text_handle = GetPropW(hwnd, PCWSTR(prop_name.as_ptr()));
                        if !text_handle.is_invalid() {
                            let text_ptr = text_handle.0 as *mut Vec<u16>;
                            if !text_ptr.is_null() {
                                let _ = Box::from_raw(text_ptr); // 释放内存
                            }
                            let _ = RemovePropW(hwnd, PCWSTR(prop_name.as_ptr()));
                        }
                    }

                    // 调用原始窗口过程
                    let original_proc = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
                    if original_proc != 0 {
                        return CallWindowProcW(
                            Some(std::mem::transmute(original_proc)),
                            hwnd,
                            msg,
                            wparam,
                            lparam,
                        );
                    }
                    return LRESULT(0);
                }
                WM_KEYDOWN | WM_SYSKEYDOWN => {
                    // 获取修饰键状态
                    let mut modifiers = 0u32;
                    if GetKeyState(VK_CONTROL.0 as i32) < 0 {
                        modifiers |= MOD_CONTROL.0;
                    }
                    if GetKeyState(VK_MENU.0 as i32) < 0 {
                        // VK_MENU 是 Alt 键
                        modifiers |= MOD_ALT.0;
                    }
                    if GetKeyState(VK_SHIFT.0 as i32) < 0 {
                        modifiers |= MOD_SHIFT.0;
                    }

                    let key = wparam.0 as u32;

                    // 只处理字母和数字键
                    if (key >= 'A' as u32 && key <= 'Z' as u32)
                        || (key >= '0' as u32 && key <= '9' as u32)
                    {
                        if modifiers != 0 {
                            // 构建热键字符串
                            let mut hotkey_parts = Vec::new();
                            if modifiers & MOD_CONTROL.0 != 0 {
                                hotkey_parts.push("Ctrl".to_string());
                            }
                            if modifiers & MOD_ALT.0 != 0 {
                                hotkey_parts.push("Alt".to_string());
                            }
                            if modifiers & MOD_SHIFT.0 != 0 {
                                hotkey_parts.push("Shift".to_string());
                            }

                            let key_char = char::from_u32(key).unwrap_or('?');
                            hotkey_parts.push(key_char.to_string());

                            let hotkey_string = hotkey_parts.join("+");
                            let hotkey_wide = to_wide_chars(&hotkey_string);

                            // 更新输入框文本
                            let _ = SetWindowTextW(hwnd, PCWSTR(hotkey_wide.as_ptr()));

                            return LRESULT(0);
                        }
                    }

                    // 忽略其他按键
                    return LRESULT(0);
                }
                WM_CHAR => {
                    // 拦截所有字符输入，防止手动编辑
                    return LRESULT(0);
                }
                _ => {
                    // 调用原始窗口过程
                    let original_proc = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
                    if original_proc != 0 {
                        return CallWindowProcW(
                            Some(std::mem::transmute(original_proc)),
                            hwnd,
                            msg,
                            wparam,
                            lparam,
                        );
                    }
                }
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
    }

    /// 显示设置窗口
    pub fn show(parent_hwnd: HWND) -> Result<()> {
        unsafe {
            // 检查是否已经有设置窗口打开
            if let Some(existing_hwnd) = SETTINGS_WINDOW {
                if IsWindow(Some(existing_hwnd)).as_bool() {
                    // 如果窗口已存在，将其置于前台
                    let _ = ShowWindow(existing_hwnd, SW_RESTORE);
                    let _ = SetForegroundWindow(existing_hwnd);
                    let _ = BringWindowToTop(existing_hwnd);
                    return Ok(());
                } else {
                    // 窗口句柄无效，清除它
                    SETTINGS_WINDOW = None;
                }
            }
            // 初始化Common Controls 6.0以启用现代样式
            let mut icc = INITCOMMONCONTROLSEX {
                dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
                dwICC: ICC_STANDARD_CLASSES
                    | ICC_WIN95_CLASSES
                    | ICC_TAB_CLASSES
                    | ICC_PROGRESS_CLASS
                    | ICC_LISTVIEW_CLASSES,
            };
            let _ = InitCommonControlsEx(&mut icc);

            let instance = GetModuleHandleW(None)?;
            let class_name = to_wide_chars("ModernSettingsWindow");

            // 注册窗口类
            let window_class = WNDCLASSW {
                lpfnWndProc: Some(Self::window_proc),
                hInstance: instance.into(),
                lpszClassName: PCWSTR(class_name.as_ptr()),
                hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as *mut _), // 白色现代背景
                hCursor: LoadCursorW(None, IDC_ARROW)?,
                style: CS_HREDRAW | CS_VREDRAW,
                ..Default::default()
            };

            RegisterClassW(&window_class);

            // 创建支持弹性布局的设置窗口
            let hwnd = CreateWindowExW(
                WS_EX_DLGMODALFRAME | WS_EX_CONTROLPARENT,
                PCWSTR(class_name.as_ptr()),
                PCWSTR(to_wide_chars("🎨 截图工具 - 设置").as_ptr()),
                WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_THICKFRAME,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                450, // 初始窗口宽度
                400, // 初始窗口高度
                Some(parent_hwnd),
                None,
                Some(instance.into()),
                None,
            )?;

            // 居中显示
            let mut rect = RECT::default();
            let _ = GetWindowRect(hwnd, &mut rect);
            let width = rect.right - rect.left;
            let height = rect.bottom - rect.top;
            let screen_width = GetSystemMetrics(SM_CXSCREEN);
            let screen_height = GetSystemMetrics(SM_CYSCREEN);
            let x = (screen_width - width) / 2;
            let y = (screen_height - height) / 2;
            let _ = SetWindowPos(hwnd, None, x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER);

            // 保存窗口句柄到全局变量
            SETTINGS_WINDOW = Some(hwnd);

            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = UpdateWindow(hwnd);

            // 模态对话框消息循环 - 只处理这个窗口的消息
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                // 检查消息是否是给我们的窗口或其子窗口的
                if msg.hwnd == hwnd || IsChild(hwnd, msg.hwnd).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                } else {
                    // 如果不是给我们窗口的消息，转发给默认处理
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }

                // 如果窗口被销毁，退出循环
                if !IsWindow(Some(hwnd)).as_bool() {
                    break;
                }
            }

            Ok(())
        }
    }

    /// 窗口过程
    unsafe extern "system" fn window_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        unsafe {
            match msg {
                WM_CREATE => {
                    let settings = SimpleSettings::load();
                    let mut window = SimpleSettingsWindow {
                        hwnd,
                        settings,
                        line_thickness_edit: HWND::default(),
                        font_size_edit: HWND::default(),
                        font_choose_button: HWND::default(),
                        drawing_color_button: HWND::default(),
                        drawing_color_preview: HWND::default(),
                        text_color_button: HWND::default(),
                        text_color_preview: HWND::default(),
                        hotkey_edit: HWND::default(),
                        config_path_edit: HWND::default(),
                        config_path_browse_button: HWND::default(),
                        ocr_language_combo: HWND::default(),
                        ok_button: HWND::default(),
                        cancel_button: HWND::default(),
                        font: HFONT::default(),
                    };

                    window.create_controls();
                    window.load_values();

                    let window_box = Box::new(window);
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(window_box) as isize);

                    LRESULT(0)
                }

                WM_COMMAND => {
                    let window_ptr =
                        GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SimpleSettingsWindow;
                    if !window_ptr.is_null() {
                        let window = &mut *window_ptr;
                        let command_id = (wparam.0 & 0xFFFF) as i32;
                        let notification = ((wparam.0 >> 16) & 0xFFFF) as i32;

                        // 处理编辑框变化通知
                        if notification == 0x0300 {
                            // EN_CHANGE
                            window.handle_edit_change(command_id);
                        } else {
                            window.handle_command(command_id);
                        }
                    }
                    LRESULT(0)
                }

                WM_CLOSE => {
                    let window_ptr =
                        GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SimpleSettingsWindow;
                    if !window_ptr.is_null() {
                        let _window = Box::from_raw(window_ptr);
                        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                    }
                    SETTINGS_WINDOW = None;
                    let _ = DestroyWindow(hwnd);
                    LRESULT(0)
                }

                WM_CTLCOLORSTATIC => {
                    let hdc = HDC(wparam.0 as *mut _);
                    let control_hwnd = HWND(lparam.0 as *mut _);

                    let window_ptr =
                        GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SimpleSettingsWindow;
                    if !window_ptr.is_null() {
                        let window = &*window_ptr;

                        // 检查是否是绘图颜色预览控件
                        if control_hwnd == window.drawing_color_preview {
                            // 创建绘图颜色画刷
                            let color = (window.settings.drawing_color_red as u32)
                                | ((window.settings.drawing_color_green as u32) << 8)
                                | ((window.settings.drawing_color_blue as u32) << 16);
                            let brush = CreateSolidBrush(COLORREF(color));

                            // 设置背景色
                            SetBkColor(hdc, COLORREF(color));

                            return LRESULT(brush.0 as isize);
                        }

                        // 检查是否是文字颜色预览控件
                        if control_hwnd == window.text_color_preview {
                            // 创建文字颜色画刷
                            let color = (window.settings.text_color_red as u32)
                                | ((window.settings.text_color_green as u32) << 8)
                                | ((window.settings.text_color_blue as u32) << 16);
                            let brush = CreateSolidBrush(COLORREF(color));

                            // 设置背景色
                            SetBkColor(hdc, COLORREF(color));

                            return LRESULT(brush.0 as isize);
                        }
                    }

                    // 对于所有其他静态文本控件，设置透明背景
                    SetBkMode(hdc, TRANSPARENT);

                    // 返回空画刷，让父窗口绘制背景
                    return LRESULT(GetStockObject(HOLLOW_BRUSH).0 as isize);
                }

                WM_CTLCOLOREDIT => {
                    // 处理编辑框背景色，确保热键输入框不会变黑
                    let hdc = HDC(wparam.0 as *mut _);

                    // 强制设置白色背景和黑色文字
                    SetBkColor(hdc, COLORREF(0xFFFFFF)); // 白色背景
                    SetTextColor(hdc, COLORREF(0x000000)); // 黑色文字
                    SetBkMode(hdc, OPAQUE); // 不透明背景

                    // 返回白色画刷
                    return LRESULT(GetStockObject(WHITE_BRUSH).0 as isize);
                }

                WM_CTLCOLORBTN => {
                    // 处理复选框背景 - 返回NULL画刷强制透明
                    let hdc = HDC(wparam.0 as *mut _);
                    SetBkMode(hdc, TRANSPARENT);
                    return LRESULT(GetStockObject(NULL_BRUSH).0 as isize);
                }

                WM_ERASEBKGND => {
                    // 处理背景擦除 - 确保复选框区域透明
                    let hdc = HDC(wparam.0 as *mut _);
                    let mut rect = RECT::default();
                    let _ = GetClientRect(hwnd, &mut rect);

                    // 使用系统背景色填充
                    let bg_brush = GetSysColorBrush(COLOR_BTNFACE);
                    FillRect(hdc, &rect, bg_brush);

                    return LRESULT(1); // 表示我们处理了背景擦除
                }

                WM_PAINT => {
                    // 强制重绘所有编辑框，确保它们保持正确的颜色
                    let window_ptr =
                        GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SimpleSettingsWindow;
                    if !window_ptr.is_null() {
                        let window = &*window_ptr;

                        // 强制重绘热键输入框
                        let _ = InvalidateRect(Some(window.hotkey_edit), None, TRUE.into());
                        let _ = UpdateWindow(window.hotkey_edit);
                    }

                    // 调用默认处理
                    return DefWindowProcW(hwnd, msg, wparam, lparam);
                }

                WM_SIZE => {
                    // 处理窗口大小变化，重新布局控件
                    let window_ptr =
                        GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SimpleSettingsWindow;
                    if !window_ptr.is_null() {
                        let window = &mut *window_ptr;
                        window.layout_controls();
                    }
                    LRESULT(0)
                }

                WM_GETMINMAXINFO => {
                    // 设置最小窗口大小
                    let min_max_info = lparam.0 as *mut MINMAXINFO;
                    if !min_max_info.is_null() {
                        (*min_max_info).ptMinTrackSize.x = 400; // 最小宽度
                        (*min_max_info).ptMinTrackSize.y = 350; // 最小高度
                    }
                    LRESULT(0)
                }

                WM_DESTROY => {
                    SETTINGS_WINDOW = None;
                    LRESULT(0)
                }

                _ => DefWindowProcW(hwnd, msg, wparam, lparam),
            }
        }
    }

    /// 设置控件现代主题
    unsafe fn set_modern_theme(hwnd: HWND) {
        unsafe {
            // 尝试设置现代主题
            let theme_name = to_wide_chars("Explorer");
            let _ = SetWindowTheme(hwnd, PCWSTR(theme_name.as_ptr()), PCWSTR::null());
        }
    }

    /// 查找具有指定文本的子控件
    fn find_control_by_text(&self, text: &str) -> Option<HWND> {
        unsafe {
            // 使用GetWindow来查找子控件
            if let Ok(mut child) = GetWindow(self.hwnd, GW_CHILD) {
                while !child.is_invalid() {
                    let mut buffer = [0u16; 256];
                    let len = GetWindowTextW(child, &mut buffer);
                    if len > 0 {
                        let window_text = String::from_utf16_lossy(&buffer[..len as usize]);
                        if window_text == text {
                            return Some(child);
                        }
                    }
                    if let Ok(next_child) = GetWindow(child, GW_HWNDNEXT) {
                        child = next_child;
                    } else {
                        break;
                    }
                }
            }

            None
        }
    }

 

    /// 专业的Windows标准布局 - 参考标准控件演示
    fn layout_controls(&mut self) {
        unsafe {
            let mut client_rect = RECT::default();
            let _ = GetClientRect(self.hwnd, &mut client_rect);

            let window_width = client_rect.right - client_rect.left;
            let window_height = client_rect.bottom - client_rect.top;

            // 标准Windows布局参数
            let margin = 15;
            let item_spacing = 30;
            let group_spacing = 20;
            let label_width = 80;
            let input_width = 80;
            let button_width = 90;
            let button_height = 25;
            let edit_height = 21;
            let label_height = 18;

            let mut current_y = margin;

            // === 绘图设置分组 ===

            // 绘图设置分组标题
            if let Some(group_title) = self.find_control_by_text("绘图设置") {
                let _ = SetWindowPos(
                    group_title,
                    None,
                    margin,
                    current_y,
                    200,
                    label_height,
                    SWP_NOZORDER,
                );
            }
            current_y += label_height + 10;

            // 线条粗细标签和输入框 (第一行)
            if let Some(thickness_label) = self.find_control_by_text("线条粗细:") {
                let _ = SetWindowPos(
                    thickness_label,
                    None,
                    margin + 10,
                    current_y,
                    label_width,
                    label_height,
                    SWP_NOZORDER,
                );
            }
            let _ = SetWindowPos(
                self.line_thickness_edit,
                None,
                margin + 10 + label_width + 5,
                current_y - 2,
                input_width,
                edit_height,
                SWP_NOZORDER,
            );

            current_y += item_spacing;

            // 字体设置标签和按钮 (第二行)
            if let Some(font_label) = self.find_control_by_text("字体设置:") {
                let _ = SetWindowPos(
                    font_label,
                    None,
                    margin + 10,
                    current_y,
                    label_width,
                    label_height,
                    SWP_NOZORDER,
                );
            }

            // 字体选择按钮 (同一行，在标签右侧)
            let _ = SetWindowPos(
                self.font_choose_button,
                None,
                margin + 10 + label_width + 5,
                current_y - 2,
                120, // 按钮宽度
                button_height,
                SWP_NOZORDER,
            );

            current_y += item_spacing;

            // === 颜色设置分组 ===

            // 颜色设置分组标题
            if let Some(color_group_title) = self.find_control_by_text("颜色设置") {
                let _ = SetWindowPos(
                    color_group_title,
                    None,
                    margin,
                    current_y,
                    200,
                    label_height,
                    SWP_NOZORDER,
                );
            }
            current_y += label_height + 10;

            // 绘图颜色标签和按钮 (第一行)
            if let Some(drawing_color_label) = self.find_control_by_text("绘图颜色:") {
                let _ = SetWindowPos(
                    drawing_color_label,
                    None,
                    margin + 10,
                    current_y,
                    label_width,
                    label_height,
                    SWP_NOZORDER,
                );
            }
            let _ = SetWindowPos(
                self.drawing_color_button,
                None,
                margin + 10 + label_width + 5,
                current_y - 2,
                button_width,
                button_height,
                SWP_NOZORDER,
            );
            let _ = SetWindowPos(
                self.drawing_color_preview,
                None,
                margin + 10 + label_width + 5 + button_width + 8,
                current_y,
                30,
                label_height,
                SWP_NOZORDER,
            );

            current_y += item_spacing;

            // === 热键设置分组 ===

            // 热键设置分组标题
            if let Some(hotkey_group_title) = self.find_control_by_text("热键设置") {
                let _ = SetWindowPos(
                    hotkey_group_title,
                    None,
                    margin,
                    current_y,
                    200,
                    label_height,
                    SWP_NOZORDER,
                );
            }
            current_y += label_height + 10;

            // 热键标签和输入框
            if let Some(hotkey_label) = self.find_control_by_text("截图热键:") {
                let _ = SetWindowPos(
                    hotkey_label,
                    None,
                    margin + 10,
                    current_y,
                    label_width,
                    label_height,
                    SWP_NOZORDER,
                );
            }
            let hotkey_width = window_width - margin * 2 - 20 - label_width - 10;
            let _ = SetWindowPos(
                self.hotkey_edit,
                None,
                margin + 10 + label_width + 5,
                current_y - 2,
                hotkey_width,
                edit_height,
                SWP_NOZORDER,
            );

            current_y += item_spacing;

            // === 配置路径设置分组 ===

            // 配置路径设置分组标题
            if let Some(config_path_group_title) = self.find_control_by_text("配置文件路径") {
                let _ = SetWindowPos(
                    config_path_group_title,
                    None,
                    margin,
                    current_y,
                    200,
                    label_height,
                    SWP_NOZORDER,
                );
            }
            current_y += label_height + 10;

            // 配置路径标签、输入框和浏览按钮
            if let Some(config_path_label) = self.find_control_by_text("保存路径:") {
                let _ = SetWindowPos(
                    config_path_label,
                    None,
                    margin + 10,
                    current_y,
                    label_width,
                    label_height,
                    SWP_NOZORDER,
                );
            }

            let browse_button_width = 80;
            let config_path_width =
                window_width - margin * 2 - 20 - label_width - 10 - browse_button_width - 5;
            let _ = SetWindowPos(
                self.config_path_edit,
                None,
                margin + 10 + label_width + 5,
                current_y - 2,
                config_path_width,
                edit_height,
                SWP_NOZORDER,
            );

            let _ = SetWindowPos(
                self.config_path_browse_button,
                None,
                margin + 10 + label_width + 5 + config_path_width + 5,
                current_y - 2,
                browse_button_width,
                button_height,
                SWP_NOZORDER,
            );

            current_y += item_spacing;

            // === OCR语言设置 ===

            // OCR语言标签
            if let Some(ocr_label) = self.find_control_by_text("OCR识别语言:") {
                let _ = SetWindowPos(
                    ocr_label,
                    None,
                    margin + 10,
                    current_y,
                    label_width + 20, // 稍微宽一点以容纳中文
                    label_height,
                    SWP_NOZORDER,
                );
            }

            // OCR语言下拉框
            let _ = SetWindowPos(
                self.ocr_language_combo,
                None,
                margin + 10 + label_width + 25,
                current_y - 2,
                150, // ComboBox宽度
                200, // ComboBox高度（包含下拉部分）
                SWP_NOZORDER,
            );

            current_y += item_spacing;

            // === 底部按钮 ===
            let button_spacing = 10;
            let buttons_total_width = button_width * 2 + button_spacing;
            let buttons_start_x = (window_width - buttons_total_width) / 2;

            let _ = SetWindowPos(
                self.ok_button,
                None,
                buttons_start_x,
                window_height - button_height - margin,
                button_width,
                button_height,
                SWP_NOZORDER,
            );

            let _ = SetWindowPos(
                self.cancel_button,
                None,
                buttons_start_x + button_width + button_spacing,
                window_height - button_height - margin,
                button_width,
                button_height,
                SWP_NOZORDER,
            );

            // 强制重绘窗口
            let _ = InvalidateRect(Some(self.hwnd), None, TRUE.into());
        }
    }

    /// 创建控件 - 专业Windows标准布局
    fn create_controls(&mut self) {
        unsafe {
            let instance = GetModuleHandleW(None).unwrap().into();

            // 创建标准Windows字体
            self.font = CreateFontW(
                -12,                                                    // 字体高度 (标准大小)
                0,                                                      // 字体宽度
                0,                                                      // 角度
                0,                                                      // 基线角度
                FW_NORMAL.0 as i32,                                     // 字体粗细
                0,                                                      // 斜体
                0,                                                      // 下划线
                0,                                                      // 删除线
                DEFAULT_CHARSET,                                        // 字符集
                OUT_DEFAULT_PRECIS,                                     // 输出精度
                CLIP_DEFAULT_PRECIS,                                    // 裁剪精度
                DEFAULT_QUALITY,                                        // 输出质量
                (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,               // 字体族
                PCWSTR(to_wide_chars("Microsoft Sans Serif").as_ptr()), // 标准字体
            );

            // === 创建分组标题 ===

            // 绘图设置分组标题
            let drawing_group_title = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("STATIC").as_ptr()),
                PCWSTR(to_wide_chars("绘图设置").as_ptr()),
                WS_VISIBLE | WS_CHILD,
                0,
                0,
                0,
                0, // 位置将在layout_controls中设置
                Some(self.hwnd),
                None,
                Some(instance),
                None,
            )
            .unwrap_or_default();
            SendMessageW(
                drawing_group_title,
                WM_SETFONT,
                Some(WPARAM(self.font.0 as usize)),
                None,
            );

            // === 创建标签控件 ===

            // 线条粗细标签
            let thickness_label = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("STATIC").as_ptr()),
                PCWSTR(to_wide_chars("线条粗细:").as_ptr()),
                WS_VISIBLE | WS_CHILD,
                0,
                0,
                0,
                0, // 位置将在layout_controls中设置
                Some(self.hwnd),
                None,
                Some(instance),
                None,
            )
            .unwrap_or_default();
            SendMessageW(
                thickness_label,
                WM_SETFONT,
                Some(WPARAM(self.font.0 as usize)),
                None,
            );

            // 字体设置标签
            let font_settings_label = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("STATIC").as_ptr()),
                PCWSTR(to_wide_chars("字体设置:").as_ptr()),
                WS_VISIBLE | WS_CHILD,
                0,
                0,
                0,
                0, // 位置将在layout_controls中设置
                Some(self.hwnd),
                None,
                Some(instance),
                None,
            )
            .unwrap_or_default();
            SendMessageW(
                font_settings_label,
                WM_SETFONT,
                Some(WPARAM(self.font.0 as usize)),
                None,
            );

            // === 创建输入控件 ===

            // 线条粗细输入框
            self.line_thickness_edit = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                PCWSTR(to_wide_chars("EDIT").as_ptr()),
                PCWSTR::null(),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP,
                0,
                0,
                0,
                0, // 位置将在layout_controls中设置
                Some(self.hwnd),
                Some(HMENU(ID_LINE_THICKNESS as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();
            SendMessageW(
                self.line_thickness_edit,
                WM_SETFONT,
                Some(WPARAM(self.font.0 as usize)),
                None,
            );

            // 字体选择按钮
            self.font_choose_button = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("BUTTON").as_ptr()),
                PCWSTR(to_wide_chars("选择字体...").as_ptr()),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP,
                0,
                0,
                0,
                0, // 位置将在layout_controls中设置
                Some(self.hwnd),
                Some(HMENU(ID_FONT_CHOOSE_BUTTON as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();
            Self::set_modern_theme(self.font_choose_button);
            SendMessageW(
                self.font_choose_button,
                WM_SETFONT,
                Some(WPARAM(self.font.0 as usize)),
                None,
            );

            // === 创建颜色设置标签 ===

            // 颜色设置分组标题
            let color_group_title = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("STATIC").as_ptr()),
                PCWSTR(to_wide_chars("颜色设置").as_ptr()),
                WS_VISIBLE | WS_CHILD,
                0,
                0,
                0,
                0, // 位置将在layout_controls中设置
                Some(self.hwnd),
                None,
                Some(instance),
                None,
            )
            .unwrap_or_default();
            SendMessageW(
                color_group_title,
                WM_SETFONT,
                Some(WPARAM(self.font.0 as usize)),
                None,
            );

            // 绘图颜色标签
            let drawing_color_label = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("STATIC").as_ptr()),
                PCWSTR(to_wide_chars("绘图颜色:").as_ptr()),
                WS_VISIBLE | WS_CHILD,
                0,
                0,
                0,
                0, // 位置将在layout_controls中设置
                Some(self.hwnd),
                None,
                Some(instance),
                None,
            )
            .unwrap_or_default();
            SendMessageW(
                drawing_color_label,
                WM_SETFONT,
                Some(WPARAM(self.font.0 as usize)),
                None,
            );

            // === 创建颜色控件 ===

            // 绘图颜色预览框
            self.drawing_color_preview = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                PCWSTR(to_wide_chars("STATIC").as_ptr()),
                PCWSTR::null(),
                WS_VISIBLE | WS_CHILD,
                0,
                0,
                0,
                0, // 位置将在layout_controls中设置
                Some(self.hwnd),
                None,
                Some(instance),
                None,
            )
            .unwrap_or_default();

            // 绘图颜色选择按钮
            self.drawing_color_button = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("BUTTON").as_ptr()),
                PCWSTR(to_wide_chars("选择颜色...").as_ptr()),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP,
                0,
                0,
                0,
                0, // 位置将在layout_controls中设置
                Some(self.hwnd),
                Some(HMENU(ID_DRAWING_COLOR_BUTTON as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();
            Self::set_modern_theme(self.drawing_color_button);
            SendMessageW(
                self.drawing_color_button,
                WM_SETFONT,
                Some(WPARAM(self.font.0 as usize)),
                None,
            );

            // === 创建热键设置标签 ===

            // 热键设置分组标题
            let hotkey_group_title = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("STATIC").as_ptr()),
                PCWSTR(to_wide_chars("热键设置").as_ptr()),
                WS_VISIBLE | WS_CHILD,
                0,
                0,
                0,
                0, // 位置将在layout_controls中设置
                Some(self.hwnd),
                None,
                Some(instance),
                None,
            )
            .unwrap_or_default();
            SendMessageW(
                hotkey_group_title,
                WM_SETFONT,
                Some(WPARAM(self.font.0 as usize)),
                None,
            );

            // 热键标签
            let hotkey_label = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("STATIC").as_ptr()),
                PCWSTR(to_wide_chars("截图热键:").as_ptr()),
                WS_VISIBLE | WS_CHILD,
                0,
                0,
                0,
                0, // 位置将在layout_controls中设置
                Some(self.hwnd),
                None,
                Some(instance),
                None,
            )
            .unwrap_or_default();
            SendMessageW(
                hotkey_label,
                WM_SETFONT,
                Some(WPARAM(self.font.0 as usize)),
                None,
            );

            // === 创建配置路径设置标签 ===

            // 配置路径设置分组标题
            let config_path_group_title = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("STATIC").as_ptr()),
                PCWSTR(to_wide_chars("配置文件路径").as_ptr()),
                WS_VISIBLE | WS_CHILD,
                0,
                0,
                0,
                0, // 位置将在layout_controls中设置
                Some(self.hwnd),
                None,
                Some(instance),
                None,
            )
            .unwrap_or_default();
            SendMessageW(
                config_path_group_title,
                WM_SETFONT,
                Some(WPARAM(self.font.0 as usize)),
                None,
            );

            // 配置路径标签
            let config_path_label = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("STATIC").as_ptr()),
                PCWSTR(to_wide_chars("保存路径:").as_ptr()),
                WS_VISIBLE | WS_CHILD,
                0,
                0,
                0,
                0, // 位置将在layout_controls中设置
                Some(self.hwnd),
                None,
                Some(instance),
                None,
            )
            .unwrap_or_default();
            SendMessageW(
                config_path_label,
                WM_SETFONT,
                Some(WPARAM(self.font.0 as usize)),
                None,
            );

            // === 创建热键输入控件 ===

            // 热键输入框 - 移除ES_READONLY样式，使用普通编辑框
            self.hotkey_edit = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                PCWSTR(to_wide_chars("EDIT").as_ptr()),
                PCWSTR::null(),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP, // 移除ES_READONLY
                0,
                0,
                0,
                0, // 位置将在layout_controls中设置
                Some(self.hwnd),
                Some(HMENU(ID_HOTKEY_EDIT as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();
            SendMessageW(
                self.hotkey_edit,
                WM_SETFONT,
                Some(WPARAM(self.font.0 as usize)),
                None,
            );

            // 设置热键输入框的现代主题
            Self::set_modern_theme(self.hotkey_edit);

            // 子类化热键输入框以处理按键事件
            let original_proc = SetWindowLongPtrW(
                self.hotkey_edit,
                GWLP_WNDPROC,
                Self::hotkey_edit_proc as isize,
            );
            // 存储原始窗口过程
            SetWindowLongPtrW(self.hotkey_edit, GWLP_USERDATA, original_proc);

            // === 创建配置路径控件 ===

            // 配置路径输入框
            self.config_path_edit = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                PCWSTR(to_wide_chars("EDIT").as_ptr()),
                PCWSTR::null(),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP,
                0,
                0,
                0,
                0, // 位置将在layout_controls中设置
                Some(self.hwnd),
                Some(HMENU(ID_CONFIG_PATH_EDIT as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();
            SendMessageW(
                self.config_path_edit,
                WM_SETFONT,
                Some(WPARAM(self.font.0 as usize)),
                None,
            );
            Self::set_modern_theme(self.config_path_edit);

            // 配置路径浏览按钮
            self.config_path_browse_button = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("BUTTON").as_ptr()),
                PCWSTR(to_wide_chars("浏览...").as_ptr()),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP,
                0,
                0,
                0,
                0, // 位置将在layout_controls中设置
                Some(self.hwnd),
                Some(HMENU(ID_CONFIG_PATH_BROWSE as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();
            SendMessageW(
                self.config_path_browse_button,
                WM_SETFONT,
                Some(WPARAM(self.font.0 as usize)),
                None,
            );
            Self::set_modern_theme(self.config_path_browse_button);

            // === 创建OCR语言选择控件 ===

            // OCR语言标签
            let ocr_language_label = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("STATIC").as_ptr()),
                PCWSTR(to_wide_chars("OCR识别语言:").as_ptr()),
                WS_VISIBLE | WS_CHILD,
                0,
                0,
                0,
                0, // 位置将在layout_controls中设置
                Some(self.hwnd),
                None,
                Some(instance),
                None,
            )
            .unwrap_or_default();
            SendMessageW(
                ocr_language_label,
                WM_SETFONT,
                Some(WPARAM(self.font.0 as usize)),
                None,
            );

            // OCR语言选择下拉框
            self.ocr_language_combo = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                PCWSTR(to_wide_chars("COMBOBOX").as_ptr()),
                PCWSTR::null(),
                WINDOW_STYLE(WS_VISIBLE.0 | WS_CHILD.0 | WS_TABSTOP.0 | 0x0003), // CBS_DROPDOWNLIST = 0x0003
                0,
                0,
                0,
                0, // 位置将在layout_controls中设置
                Some(self.hwnd),
                Some(HMENU(ID_OCR_LANGUAGE_COMBO as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();
            SendMessageW(
                self.ocr_language_combo,
                WM_SETFONT,
                Some(WPARAM(self.font.0 as usize)),
                None,
            );
            Self::set_modern_theme(self.ocr_language_combo);

            // 添加语言选项
            let languages = [
                ("chinese", "简体中文 (默认)"),
                ("english", "英文"),
                ("chinese_cht", "繁体中文"),
                ("japan", "日文"),
                ("korean", "韩文"),
            ];

            for (value, display) in &languages {
                let text = to_wide_chars(display);
                let index = SendMessageW(
                    self.ocr_language_combo,
                    0x0143, // CB_ADDSTRING
                    Some(WPARAM(0)),
                    Some(LPARAM(text.as_ptr() as isize)),
                );

                // 存储语言值作为项目数据
                let value_text = to_wide_chars(value);
                let value_box = Box::new(value_text);
                let value_ptr = Box::into_raw(value_box);
                SendMessageW(
                    self.ocr_language_combo,
                    0x0151, // CB_SETITEMDATA
                    Some(WPARAM(index.0 as usize)),
                    Some(LPARAM(value_ptr as isize)),
                );
            }

            // === 创建按钮 ===

            // 确定按钮
            self.ok_button = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("BUTTON").as_ptr()),
                PCWSTR(to_wide_chars("确定").as_ptr()),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP,
                0,
                0,
                0,
                0, // 位置将在layout_controls中设置
                Some(self.hwnd),
                Some(HMENU(ID_OK as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();
            Self::set_modern_theme(self.ok_button);
            SendMessageW(
                self.ok_button,
                WM_SETFONT,
                Some(WPARAM(self.font.0 as usize)),
                None,
            );

            // 取消按钮
            self.cancel_button = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("BUTTON").as_ptr()),
                PCWSTR(to_wide_chars("取消").as_ptr()),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP,
                0,
                0,
                0,
                0, // 位置将在layout_controls中设置
                Some(self.hwnd),
                Some(HMENU(ID_CANCEL as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();
            Self::set_modern_theme(self.cancel_button);
            SendMessageW(
                self.cancel_button,
                WM_SETFONT,
                Some(WPARAM(self.font.0 as usize)),
                None,
            );

            // 初始布局控件
            self.layout_controls();
        }
    }

    /// 加载设置值到控件
    fn load_values(&mut self) {
        unsafe {
            // 加载数值
            let thickness_text = to_wide_chars(&self.settings.line_thickness.to_string());
            let _ = SetWindowTextW(self.line_thickness_edit, PCWSTR(thickness_text.as_ptr()));

            // 加载热键设置
            let hotkey_text = to_wide_chars(&self.settings.get_hotkey_string());
            let _ = SetWindowTextW(self.hotkey_edit, PCWSTR(hotkey_text.as_ptr()));

            // 加载配置路径设置
            let config_path_text = to_wide_chars(&self.settings.config_path);
            let _ = SetWindowTextW(self.config_path_edit, PCWSTR(config_path_text.as_ptr()));

            // 加载OCR语言设置
            let item_count = SendMessageW(self.ocr_language_combo, 0x0146, None, None); // CB_GETCOUNT
            for i in 0..item_count.0 {
                let data_ptr = SendMessageW(
                    self.ocr_language_combo,
                    0x0150, // CB_GETITEMDATA
                    Some(WPARAM(i as usize)),
                    None,
                );

                if data_ptr.0 != 0 {
                    let value_ptr = data_ptr.0 as *const Vec<u16>;
                    if !value_ptr.is_null() {
                        let value_vec = &*value_ptr;
                        let value = String::from_utf16_lossy(value_vec)
                            .trim_end_matches('\0')
                            .to_string();

                        if value == self.settings.ocr_language {
                            SendMessageW(
                                self.ocr_language_combo,
                                0x014E, // CB_SETCURSEL
                                Some(WPARAM(i as usize)),
                                None,
                            );
                            break;
                        }
                    }
                }
            }

            // 更新颜色预览
            self.update_color_preview();
        }
    }

    /// 处理编辑框变化
    fn handle_edit_change(&mut self, control_id: i32) {
        match control_id {
            _ => {}
        }
    }

    /// 处理命令消息
    fn handle_command(&mut self, command_id: i32) {
        match command_id {
            ID_OK => {
                self.save_settings();
                unsafe {
                    let _ = self.settings.save();

                    // 通知主窗口重新加载设置和重新注册热键
                    // 查找主窗口并发送消息
                    if let Ok(main_hwnd) = FindWindowW(
                        PCWSTR(to_wide_chars("sc_windows_main").as_ptr()),
                        PCWSTR::null(),
                    ) {
                        if !main_hwnd.0.is_null() {
                            // 发送自定义消息通知设置已更改 (WM_USER + 3)
                            let _ =
                                PostMessageW(Some(main_hwnd), WM_USER + 3, WPARAM(0), LPARAM(0));
                        }
                    }

                    let _ = PostMessageW(Some(self.hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
                }
            }
            ID_CANCEL => unsafe {
                let _ = PostMessageW(Some(self.hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
            },
            ID_FONT_CHOOSE_BUTTON => {
                self.show_font_dialog();
            }
            ID_DRAWING_COLOR_BUTTON => {
                self.show_drawing_color_dialog();
            }
            ID_TEXT_COLOR_BUTTON => {
                self.show_text_color_dialog();
            }
            ID_CONFIG_PATH_BROWSE => {
                self.show_folder_browser_dialog();
            }
            _ => {}
        }
    }

    /// 显示字体选择对话框
    fn show_font_dialog(&mut self) {
        unsafe {
            use windows::Win32::Graphics::Gdi::*;
            use windows::Win32::UI::Controls::Dialogs::*;

            // 创建LOGFONTW结构体
            let mut log_font = LOGFONTW::default();

            // 设置当前字体信息
            log_font.lfHeight = -(self.settings.font_size as i32);
            log_font.lfWeight = self.settings.font_weight;
            log_font.lfItalic = if self.settings.font_italic { 1 } else { 0 };
            log_font.lfUnderline = if self.settings.font_underline { 1 } else { 0 };
            log_font.lfStrikeOut = if self.settings.font_strikeout { 1 } else { 0 };

            // 复制字体名称
            let font_name_wide = to_wide_chars(&self.settings.font_name);
            let copy_len = std::cmp::min(font_name_wide.len(), 31); // LF_FACESIZE - 1
            for i in 0..copy_len {
                log_font.lfFaceName[i] = font_name_wide[i];
            }

            // 创建CHOOSEFONTW结构体
            let mut choose_font = CHOOSEFONTW::default();
            choose_font.lStructSize = std::mem::size_of::<CHOOSEFONTW>() as u32;
            choose_font.hwndOwner = self.hwnd;
            choose_font.lpLogFont = &mut log_font;
            choose_font.Flags = CF_EFFECTS | CF_SCREENFONTS | CF_INITTOLOGFONTSTRUCT;
            // 设置当前字体颜色
            choose_font.rgbColors = COLORREF(
                (self.settings.font_color.0 as u32)
                    | ((self.settings.font_color.1 as u32) << 8)
                    | ((self.settings.font_color.2 as u32) << 16),
            );

            // 显示字体选择对话框
            if ChooseFontW(&mut choose_font).as_bool() {
                // 用户选择了字体，更新设置
                self.settings.font_size = (-log_font.lfHeight) as f32;
                self.settings.font_weight = log_font.lfWeight;
                self.settings.font_italic = log_font.lfItalic != 0;
                self.settings.font_underline = log_font.lfUnderline != 0;
                self.settings.font_strikeout = log_font.lfStrikeOut != 0;

                // 获取字体颜色
                let color_value = choose_font.rgbColors.0;
                self.settings.font_color = (
                    (color_value & 0xFF) as u8,
                    ((color_value >> 8) & 0xFF) as u8,
                    ((color_value >> 16) & 0xFF) as u8,
                );

                // 获取字体名称
                let mut font_name = String::new();
                for &ch in &log_font.lfFaceName {
                    if ch == 0 {
                        break;
                    }
                    font_name.push(char::from_u32(ch as u32).unwrap_or('?'));
                }
                self.settings.font_name = font_name;

                // 更新界面显示
                self.load_values();
            }
        }
    }

    /// 更新字体显示效果（不修改设置界面字体，只保存设置）
    fn update_font_display(&mut self) {
        // 这个方法现在只是一个占位符，实际的字体应用在框选文本时进行
        // 设置界面保持系统默认字体
    }

    /// 显示绘图颜色选择对话框
    fn show_drawing_color_dialog(&mut self) {
        unsafe {
            // 创建自定义颜色数组
            let mut custom_colors = [COLORREF(0); 16];

            let mut cc = CHOOSECOLORW {
                lStructSize: std::mem::size_of::<CHOOSECOLORW>() as u32,
                hwndOwner: self.hwnd,
                hInstance: HWND::default(),
                rgbResult: COLORREF(
                    (self.settings.drawing_color_red as u32)
                        | ((self.settings.drawing_color_green as u32) << 8)
                        | ((self.settings.drawing_color_blue as u32) << 16),
                ),
                lpCustColors: custom_colors.as_mut_ptr(),
                Flags: CC_FULLOPEN | CC_RGBINIT,
                lCustData: LPARAM(0),
                lpfnHook: None,
                lpTemplateName: PCWSTR::null(),
            };

            if ChooseColorW(&mut cc).as_bool() {
                // 用户选择了颜色，更新设置
                let color = cc.rgbResult.0;
                self.settings.drawing_color_red = (color & 0xFF) as u8;
                self.settings.drawing_color_green = ((color >> 8) & 0xFF) as u8;
                self.settings.drawing_color_blue = ((color >> 16) & 0xFF) as u8;

                // 更新颜色预览
                self.update_color_preview();
            }
        }
    }

    /// 显示文字颜色选择对话框
    fn show_text_color_dialog(&mut self) {
        unsafe {
            // 创建自定义颜色数组
            let mut custom_colors = [COLORREF(0); 16];

            let mut cc = CHOOSECOLORW {
                lStructSize: std::mem::size_of::<CHOOSECOLORW>() as u32,
                hwndOwner: self.hwnd,
                hInstance: HWND::default(),
                rgbResult: COLORREF(
                    (self.settings.text_color_red as u32)
                        | ((self.settings.text_color_green as u32) << 8)
                        | ((self.settings.text_color_blue as u32) << 16),
                ),
                lpCustColors: custom_colors.as_mut_ptr(),
                Flags: CC_FULLOPEN | CC_RGBINIT,
                lCustData: LPARAM(0),
                lpfnHook: None,
                lpTemplateName: PCWSTR::null(),
            };

            if ChooseColorW(&mut cc).as_bool() {
                // 用户选择了颜色，更新设置
                let color = cc.rgbResult.0;
                self.settings.text_color_red = (color & 0xFF) as u8;
                self.settings.text_color_green = ((color >> 8) & 0xFF) as u8;
                self.settings.text_color_blue = ((color >> 16) & 0xFF) as u8;

                // 更新颜色预览
                self.update_color_preview();
            }
        }
    }

    /// 更新颜色预览
    fn update_color_preview(&self) {
        unsafe {
            // 强制重绘绘图颜色预览控件
            let _ = InvalidateRect(Some(self.drawing_color_preview), None, true);
            // 强制重绘文字颜色预览控件
            let _ = InvalidateRect(Some(self.text_color_preview), None, true);
        }
    }

    /// 从控件保存设置
    fn save_settings(&mut self) {
        unsafe {
            let mut buffer = [0u16; 32];

            // 读取线条粗细
            if GetWindowTextW(self.line_thickness_edit, &mut buffer) > 0 {
                let text = String::from_utf16_lossy(&buffer);
                let text = text.trim_end_matches('\0');
                if let Ok(value) = text.parse::<f32>() {
                    self.settings.line_thickness = value.max(1.0).min(20.0);
                }
            }

            // 读取热键设置
            let mut hotkey_buffer = [0u16; 64];
            if GetWindowTextW(self.hotkey_edit, &mut hotkey_buffer) > 0 {
                let hotkey_text = String::from_utf16_lossy(&hotkey_buffer);
                let hotkey_text = hotkey_text.trim_end_matches('\0');
                // 尝试解析热键字符串，如果失败则保持原值
                let _ = self.settings.parse_hotkey_string(hotkey_text);
            }

            // 读取配置路径设置
            let mut config_path_buffer = [0u16; 260]; // MAX_PATH
            if GetWindowTextW(self.config_path_edit, &mut config_path_buffer) > 0 {
                let config_path_text = String::from_utf16_lossy(&config_path_buffer);
                let config_path_text = config_path_text.trim_end_matches('\0');
                if !config_path_text.is_empty() {
                    self.settings.config_path = config_path_text.to_string();
                }
            }

            // 读取OCR语言设置
            let selected_index = SendMessageW(self.ocr_language_combo, 0x0147, None, None); // CB_GETCURSEL
            if selected_index.0 != -1 {
                let data_ptr = SendMessageW(
                    self.ocr_language_combo,
                    0x0150, // CB_GETITEMDATA
                    Some(WPARAM(selected_index.0 as usize)),
                    None,
                );

                if data_ptr.0 != 0 {
                    let value_ptr = data_ptr.0 as *const Vec<u16>;
                    if !value_ptr.is_null() {
                        let value_vec = &*value_ptr;
                        let value = String::from_utf16_lossy(value_vec)
                            .trim_end_matches('\0')
                            .to_string();
                        self.settings.ocr_language = value;
                    }
                }
            }
        }
    }

    /// 显示文件夹浏览对话框
    fn show_folder_browser_dialog(&mut self) {
        unsafe {
            use windows::Win32::System::Com::*;
            use windows::Win32::UI::Shell::*;

            // 初始化COM
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);

            // 创建文件夹浏览对话框
            if let Ok(folder_dialog) =
                CoCreateInstance::<_, IFileOpenDialog>(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)
            {
                // 设置为选择文件夹模式
                let _ = folder_dialog.SetOptions(FOS_PICKFOLDERS | FOS_PATHMUSTEXIST);

                // 设置标题
                let title = to_wide_chars("选择配置文件保存路径");
                let _ = folder_dialog.SetTitle(PCWSTR(title.as_ptr()));

                // 显示对话框
                if folder_dialog.Show(Some(self.hwnd)).is_ok() {
                    if let Ok(result) = folder_dialog.GetResult() {
                        if let Ok(path) = result.GetDisplayName(SIGDN_FILESYSPATH) {
                            let path_str = path.to_string().unwrap_or_default();

                            // 更新输入框
                            let path_wide = to_wide_chars(&path_str);
                            let _ =
                                SetWindowTextW(self.config_path_edit, PCWSTR(path_wide.as_ptr()));
                        }
                    }
                }
            } else {
                // 如果创建失败，使用简单的输入框
                self.show_simple_path_input();
            }

            // 清理COM
            CoUninitialize();
        }
    }

    /// 简单的路径输入对话框（备用方案）
    fn show_simple_path_input(&mut self) {
        unsafe {
            // 获取当前路径
            let mut buffer = [0u16; 260];
            GetWindowTextW(self.config_path_edit, &mut buffer);
            let current_path = String::from_utf16_lossy(&buffer);
            let current_path = current_path.trim_end_matches('\0');

            // 显示提示信息
            let message = format!("当前路径: {}\n\n请手动在输入框中修改路径", current_path);
            let message_wide = to_wide_chars(&message);
            let title_wide = to_wide_chars("配置路径");

            MessageBoxW(
                Some(self.hwnd),
                PCWSTR(message_wide.as_ptr()),
                PCWSTR(title_wide.as_ptr()),
                MB_OK | MB_ICONINFORMATION,
            );
        }
    }
}

/// 显示设置窗口的便利函数
pub fn show_settings_window() -> Result<()> {
    SimpleSettingsWindow::show(HWND::default())
}
