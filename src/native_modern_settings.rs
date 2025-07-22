use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use windows::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::LibraryLoader::*,
        UI::{Controls::*, WindowsAndMessaging::*},
    },
    core::*,
};

use crate::utils::to_wide_chars;

/// 现代化应用程序设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeModernSettings {
    pub line_thickness: f32,
    pub font_size: f32,
    pub auto_copy: bool,
    pub show_cursor: bool,
    pub delay_ms: u32,
    pub color_red: u8,
    pub color_green: u8,
    pub color_blue: u8,
    pub toolbar_opacity: f32,
    pub border_width: u32,
    pub save_format: String,
    pub jpeg_quality: u32,
}

impl Default for NativeModernSettings {
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

impl NativeModernSettings {
    fn get_settings_path() -> PathBuf {
        let mut path = std::env::current_exe().unwrap_or_default();
        path.set_file_name("native_modern_settings.json");
        path
    }

    pub fn load() -> Self {
        let path = Self::get_settings_path();
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(settings) = serde_json::from_str::<NativeModernSettings>(&content) {
                return settings;
            }
        }
        let default_settings = Self::default();
        let _ = default_settings.save();
        default_settings
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::get_settings_path();
        let content = serde_json::to_string_pretty(self).map_err(|_| Error::from(E_FAIL))?;
        fs::write(&path, content).map_err(|_| Error::from(E_FAIL))?;
        Ok(())
    }
}

/// 现代化设置窗口控件
#[derive(Default)]
struct ModernControls {
    // 绘图设置
    thickness_edit: HWND,
    font_size_edit: HWND,

    // 颜色设置
    color_red_edit: HWND,
    color_green_edit: HWND,
    color_blue_edit: HWND,
    color_preview: HWND,

    // 截图设置
    auto_copy_check: HWND,
    show_cursor_check: HWND,
    delay_edit: HWND,

    // 界面设置
    opacity_edit: HWND,
    border_width_edit: HWND,

    // 文件设置
    format_combo: HWND,
    quality_edit: HWND,

    // 按钮
    ok_button: HWND,
    cancel_button: HWND,
    reset_button: HWND,
    apply_button: HWND,
}

/// 现代化设置窗口
pub struct NativeModernSettingsWindow {
    hwnd: HWND,
    settings: NativeModernSettings,
    controls: ModernControls,
}

// 控件ID常量
const ID_THICKNESS: i32 = 2001;
const ID_FONT_SIZE: i32 = 2002;
const ID_COLOR_RED: i32 = 2003;
const ID_COLOR_GREEN: i32 = 2004;
const ID_COLOR_BLUE: i32 = 2005;
const ID_AUTO_COPY: i32 = 2006;
const ID_SHOW_CURSOR: i32 = 2007;
const ID_DELAY: i32 = 2008;
const ID_OPACITY: i32 = 2009;
const ID_BORDER_WIDTH: i32 = 2010;
const ID_FORMAT_COMBO: i32 = 2011;
const ID_QUALITY: i32 = 2012;
const ID_OK: i32 = 2013;
const ID_CANCEL: i32 = 2014;
const ID_RESET: i32 = 2015;
const ID_APPLY: i32 = 2016;
const ID_COLOR_PREVIEW: i32 = 2017;

impl NativeModernSettingsWindow {
    /// 显示现代化设置窗口
    pub fn show(parent_hwnd: HWND) -> Result<()> {
        unsafe {
            // 初始化Common Controls以启用现代样式
            let mut icc = INITCOMMONCONTROLSEX {
                dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
                dwICC: ICC_STANDARD_CLASSES | ICC_WIN95_CLASSES | ICC_TAB_CLASSES,
            };
            InitCommonControlsEx(&mut icc);

            let instance = GetModuleHandleW(None)?;
            let class_name = to_wide_chars("NativeModernSettingsWindow");

            // 注册现代化窗口类
            let window_class = WNDCLASSW {
                lpfnWndProc: Some(Self::window_proc),
                hInstance: instance.into(),
                lpszClassName: PCWSTR(class_name.as_ptr()),
                hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as *mut _), // 白色背景
                hCursor: LoadCursorW(None, IDC_ARROW)?,
                style: CS_HREDRAW | CS_VREDRAW,
                ..Default::default()
            };

            RegisterClassW(&window_class);

            // 创建现代化设置窗口
            let hwnd = CreateWindowExW(
                WS_EX_DLGMODALFRAME | WS_EX_CONTROLPARENT,
                PCWSTR(class_name.as_ptr()),
                PCWSTR(to_wide_chars("🎨 截图工具 - 现代设置").as_ptr()),
                WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                720, // 更宽的窗口
                580, // 更高的窗口
                Some(parent_hwnd),
                None,
                Some(instance.into()),
                None,
            )?;

            // 居中显示窗口
            let mut rect = RECT::default();
            let _ = GetWindowRect(hwnd, &mut rect);
            let width = rect.right - rect.left;
            let height = rect.bottom - rect.top;
            let screen_width = GetSystemMetrics(SM_CXSCREEN);
            let screen_height = GetSystemMetrics(SM_CYSCREEN);
            let x = (screen_width - width) / 2;
            let y = (screen_height - height) / 2;
            let _ = SetWindowPos(hwnd, None, x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER);

            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = UpdateWindow(hwnd);

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
        match msg {
            WM_CREATE => {
                let settings = NativeModernSettings::load();
                let mut window = NativeModernSettingsWindow {
                    hwnd,
                    settings,
                    controls: ModernControls::default(),
                };

                window.create_modern_controls();
                window.load_values();

                let window_box = Box::new(window);
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(window_box) as isize);

                LRESULT(0)
            }

            WM_COMMAND => {
                let window_ptr =
                    GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut NativeModernSettingsWindow;
                if !window_ptr.is_null() {
                    let window = &mut *window_ptr;
                    window.handle_command((wparam.0 & 0xFFFF) as i32, (wparam.0 >> 16) as u16);
                }
                LRESULT(0)
            }

            WM_CTLCOLORSTATIC => {
                // 为静态控件设置现代颜色
                let hdc = HDC(wparam.0 as *mut _);
                let _ = SetBkMode(hdc, TRANSPARENT);
                let _ = SetTextColor(hdc, COLORREF(0x333333)); // 深灰色文字
                LRESULT(GetStockObject(WHITE_BRUSH).0 as isize)
            }

            WM_CTLCOLOREDIT => {
                // 为编辑框设置现代颜色
                let hdc = HDC(wparam.0 as *mut _);
                let _ = SetBkColor(hdc, COLORREF(0xFFFFFF)); // 白色背景
                let _ = SetTextColor(hdc, COLORREF(0x000000)); // 黑色文字
                LRESULT(GetStockObject(WHITE_BRUSH).0 as isize)
            }

            WM_CLOSE => {
                let window_ptr =
                    GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut NativeModernSettingsWindow;
                if !window_ptr.is_null() {
                    let _window = Box::from_raw(window_ptr);
                }
                let _ = DestroyWindow(hwnd);
                LRESULT(0)
            }

            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    /// 创建现代化控件
    fn create_modern_controls(&mut self) {
        unsafe {
            let instance = GetModuleHandleW(None).unwrap().into();
            let mut y_pos = 20;

            // 创建Tab控件
            let tab_control = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("SysTabControl32").as_ptr()),
                PCWSTR::null(),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP | WINDOW_STYLE(TCS_TABS.0),
                20,
                y_pos,
                660,
                450,
                Some(self.hwnd),
                Some(HMENU(3000 as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();

            // 添加Tab页
            let mut tab_item = TCITEMW {
                mask: TCIF_TEXT,
                pszText: PWSTR(to_wide_chars("🖌️ 绘图").as_mut_ptr()),
                ..Default::default()
            };
            let _ = SendMessageW(
                tab_control,
                TCM_INSERTITEMW,
                WPARAM(0),
                LPARAM(&tab_item as *const _ as isize),
            );

            tab_item.pszText = PWSTR(to_wide_chars("🎨 颜色").as_mut_ptr());
            let _ = SendMessageW(
                tab_control,
                TCM_INSERTITEMW,
                WPARAM(1),
                LPARAM(&tab_item as *const _ as isize),
            );

            tab_item.pszText = PWSTR(to_wide_chars("📷 截图").as_mut_ptr());
            let _ = SendMessageW(
                tab_control,
                TCM_INSERTITEMW,
                WPARAM(2),
                LPARAM(&tab_item as *const _ as isize),
            );

            tab_item.pszText = PWSTR(to_wide_chars("💾 文件").as_mut_ptr());
            let _ = SendMessageW(
                tab_control,
                TCM_INSERTITEMW,
                WPARAM(3),
                LPARAM(&tab_item as *const _ as isize),
            );

            // 在Tab控件内创建控件（第一个Tab页 - 绘图设置）
            y_pos = 80;

            // 线条粗细
            let _ = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("STATIC").as_ptr()),
                PCWSTR(to_wide_chars("线条粗细 (1-20):").as_ptr()),
                WS_VISIBLE | WS_CHILD,
                50,
                y_pos,
                150,
                25,
                Some(self.hwnd),
                None,
                Some(instance),
                None,
            );

            self.controls.thickness_edit = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                PCWSTR(to_wide_chars("EDIT").as_ptr()),
                PCWSTR::null(),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP | ES_NUMBER,
                220,
                y_pos,
                100,
                25,
                Some(self.hwnd),
                Some(HMENU(ID_THICKNESS as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();

            y_pos += 40;

            // 字体大小
            let _ = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("STATIC").as_ptr()),
                PCWSTR(to_wide_chars("字体大小 (8-72):").as_ptr()),
                WS_VISIBLE | WS_CHILD,
                50,
                y_pos,
                150,
                25,
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
                220,
                y_pos,
                100,
                25,
                Some(self.hwnd),
                Some(HMENU(ID_FONT_SIZE as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();

            // 颜色设置（右侧）
            y_pos = 80;

            // 颜色预览
            let _ = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("STATIC").as_ptr()),
                PCWSTR(to_wide_chars("颜色预览:").as_ptr()),
                WS_VISIBLE | WS_CHILD,
                400,
                y_pos,
                100,
                25,
                Some(self.hwnd),
                None,
                Some(instance),
                None,
            );

            self.controls.color_preview = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                PCWSTR(to_wide_chars("STATIC").as_ptr()),
                PCWSTR::null(),
                WS_VISIBLE | WS_CHILD,
                520,
                y_pos,
                80,
                60,
                Some(self.hwnd),
                Some(HMENU(ID_COLOR_PREVIEW as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();

            y_pos += 80;

            // RGB输入框
            let _ = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("STATIC").as_ptr()),
                PCWSTR(to_wide_chars("红色 (0-255):").as_ptr()),
                WS_VISIBLE | WS_CHILD,
                400,
                y_pos,
                100,
                25,
                Some(self.hwnd),
                None,
                Some(instance),
                None,
            );

            self.controls.color_red_edit = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                PCWSTR(to_wide_chars("EDIT").as_ptr()),
                PCWSTR::null(),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP | ES_NUMBER,
                520,
                y_pos,
                80,
                25,
                Some(self.hwnd),
                Some(HMENU(ID_COLOR_RED as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();

            y_pos += 35;

            let _ = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("STATIC").as_ptr()),
                PCWSTR(to_wide_chars("绿色 (0-255):").as_ptr()),
                WS_VISIBLE | WS_CHILD,
                400,
                y_pos,
                100,
                25,
                Some(self.hwnd),
                None,
                Some(instance),
                None,
            );

            self.controls.color_green_edit = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                PCWSTR(to_wide_chars("EDIT").as_ptr()),
                PCWSTR::null(),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP | ES_NUMBER,
                520,
                y_pos,
                80,
                25,
                Some(self.hwnd),
                Some(HMENU(ID_COLOR_GREEN as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();

            y_pos += 35;

            let _ = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("STATIC").as_ptr()),
                PCWSTR(to_wide_chars("蓝色 (0-255):").as_ptr()),
                WS_VISIBLE | WS_CHILD,
                400,
                y_pos,
                100,
                25,
                Some(self.hwnd),
                None,
                Some(instance),
                None,
            );

            self.controls.color_blue_edit = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                PCWSTR(to_wide_chars("EDIT").as_ptr()),
                PCWSTR::null(),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP | ES_NUMBER,
                520,
                y_pos,
                80,
                25,
                Some(self.hwnd),
                Some(HMENU(ID_COLOR_BLUE as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();

            // 截图设置
            y_pos = 280;

            self.controls.auto_copy_check = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("BUTTON").as_ptr()),
                PCWSTR(to_wide_chars("✅ 自动复制到剪贴板").as_ptr()),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP | BS_AUTOCHECKBOX,
                50,
                y_pos,
                250,
                30,
                Some(self.hwnd),
                Some(HMENU(ID_AUTO_COPY as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();

            y_pos += 40;

            self.controls.show_cursor_check = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("BUTTON").as_ptr()),
                PCWSTR(to_wide_chars("🖱️ 截图时显示光标").as_ptr()),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP | BS_AUTOCHECKBOX,
                50,
                y_pos,
                250,
                30,
                Some(self.hwnd),
                Some(HMENU(ID_SHOW_CURSOR as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();

            y_pos += 40;

            let _ = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("STATIC").as_ptr()),
                PCWSTR(to_wide_chars("⏱️ 截图延迟 (毫秒):").as_ptr()),
                WS_VISIBLE | WS_CHILD,
                50,
                y_pos,
                150,
                25,
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
                220,
                y_pos,
                100,
                25,
                Some(self.hwnd),
                Some(HMENU(ID_DELAY as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();

            // 底部按钮
            y_pos = 490;

            self.controls.reset_button = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("BUTTON").as_ptr()),
                PCWSTR(to_wide_chars("🔄 重置默认").as_ptr()),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP,
                50,
                y_pos,
                120,
                35,
                Some(self.hwnd),
                Some(HMENU(ID_RESET as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();

            self.controls.apply_button = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("BUTTON").as_ptr()),
                PCWSTR(to_wide_chars("📝 应用").as_ptr()),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP,
                190,
                y_pos,
                100,
                35,
                Some(self.hwnd),
                Some(HMENU(ID_APPLY as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();

            self.controls.ok_button = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("BUTTON").as_ptr()),
                PCWSTR(to_wide_chars("✅ 确定").as_ptr()),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP | BS_DEFPUSHBUTTON,
                450,
                y_pos,
                100,
                35,
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
                570,
                y_pos,
                100,
                35,
                Some(self.hwnd),
                Some(HMENU(ID_CANCEL as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();
        }
    }

    /// 加载设置值到控件
    fn load_values(&self) {
        unsafe {
            // 加载数值
            let thickness_text = to_wide_chars(&self.settings.line_thickness.to_string());
            let _ = SetWindowTextW(
                self.controls.thickness_edit,
                PCWSTR(thickness_text.as_ptr()),
            );

            let font_size_text = to_wide_chars(&self.settings.font_size.to_string());
            let _ = SetWindowTextW(
                self.controls.font_size_edit,
                PCWSTR(font_size_text.as_ptr()),
            );

            let red_text = to_wide_chars(&self.settings.color_red.to_string());
            let _ = SetWindowTextW(self.controls.color_red_edit, PCWSTR(red_text.as_ptr()));

            let green_text = to_wide_chars(&self.settings.color_green.to_string());
            let _ = SetWindowTextW(self.controls.color_green_edit, PCWSTR(green_text.as_ptr()));

            let blue_text = to_wide_chars(&self.settings.color_blue.to_string());
            let _ = SetWindowTextW(self.controls.color_blue_edit, PCWSTR(blue_text.as_ptr()));

            let delay_text = to_wide_chars(&self.settings.delay_ms.to_string());
            let _ = SetWindowTextW(self.controls.delay_edit, PCWSTR(delay_text.as_ptr()));

            // 设置复选框状态
            let _ = SendMessageW(
                self.controls.auto_copy_check,
                BM_SETCHECK,
                Some(WPARAM(if self.settings.auto_copy { 1 } else { 0 })),
                None,
            );

            let _ = SendMessageW(
                self.controls.show_cursor_check,
                BM_SETCHECK,
                Some(WPARAM(if self.settings.show_cursor { 1 } else { 0 })),
                None,
            );

            // 更新颜色预览
            self.update_color_preview();
        }
    }

    /// 更新颜色预览
    fn update_color_preview(&self) {
        unsafe {
            let _ = InvalidateRect(self.controls.color_preview, None, TRUE);
        }
    }

    /// 处理命令消息
    fn handle_command(&mut self, command_id: i32, notification: u16) {
        match command_id {
            ID_OK => {
                self.save_settings();
                unsafe {
                    let _ = self.settings.save();
                    let _ = DestroyWindow(self.hwnd);
                }
            }
            ID_CANCEL => unsafe {
                let _ = DestroyWindow(self.hwnd);
            },
            ID_APPLY => {
                self.save_settings();
                unsafe {
                    let _ = self.settings.save();
                }
            }
            ID_RESET => {
                self.reset_to_defaults();
            }
            ID_COLOR_RED | ID_COLOR_GREEN | ID_COLOR_BLUE => {
                if notification == EN_CHANGE as u16 {
                    // 颜色值改变时更新预览
                    self.update_color_from_inputs();
                    self.update_color_preview();
                }
            }
            _ => {}
        }
    }

    /// 从输入框更新颜色值
    fn update_color_from_inputs(&mut self) {
        unsafe {
            let mut buffer = [0u16; 16];

            // 读取红色值
            if GetWindowTextW(self.controls.color_red_edit, &mut buffer) > 0 {
                let text = String::from_utf16_lossy(&buffer);
                let text = text.trim_end_matches('\0');
                if let Ok(value) = text.parse::<u8>() {
                    self.settings.color_red = value;
                }
            }

            // 读取绿色值
            if GetWindowTextW(self.controls.color_green_edit, &mut buffer) > 0 {
                let text = String::from_utf16_lossy(&buffer);
                let text = text.trim_end_matches('\0');
                if let Ok(value) = text.parse::<u8>() {
                    self.settings.color_green = value;
                }
            }

            // 读取蓝色值
            if GetWindowTextW(self.controls.color_blue_edit, &mut buffer) > 0 {
                let text = String::from_utf16_lossy(&buffer);
                let text = text.trim_end_matches('\0');
                if let Ok(value) = text.parse::<u8>() {
                    self.settings.color_blue = value;
                }
            }
        }
    }

    /// 重置为默认值
    fn reset_to_defaults(&mut self) {
        self.settings = NativeModernSettings::default();
        self.load_values();
    }

    /// 从控件保存设置
    fn save_settings(&mut self) {
        unsafe {
            let mut buffer = [0u16; 32];

            // 读取线条粗细
            if GetWindowTextW(self.controls.thickness_edit, &mut buffer) > 0 {
                let text = String::from_utf16_lossy(&buffer);
                let text = text.trim_end_matches('\0');
                if let Ok(value) = text.parse::<f32>() {
                    self.settings.line_thickness = value.max(1.0).min(20.0);
                }
            }

            // 读取字体大小
            if GetWindowTextW(self.controls.font_size_edit, &mut buffer) > 0 {
                let text = String::from_utf16_lossy(&buffer);
                let text = text.trim_end_matches('\0');
                if let Ok(value) = text.parse::<f32>() {
                    self.settings.font_size = value.max(8.0).min(72.0);
                }
            }

            // 读取延迟
            if GetWindowTextW(self.controls.delay_edit, &mut buffer) > 0 {
                let text = String::from_utf16_lossy(&buffer);
                let text = text.trim_end_matches('\0');
                if let Ok(value) = text.parse::<u32>() {
                    self.settings.delay_ms = value.min(5000);
                }
            }

            // 读取复选框状态
            let auto_copy_state =
                SendMessageW(self.controls.auto_copy_check, BM_GETCHECK, None, None);
            self.settings.auto_copy = auto_copy_state.0 != 0;

            let show_cursor_state =
                SendMessageW(self.controls.show_cursor_check, BM_GETCHECK, None, None);
            self.settings.show_cursor = show_cursor_state.0 != 0;

            // 颜色值已经在update_color_from_inputs中更新了
        }
    }
}
