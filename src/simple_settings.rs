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

/// 简化的应用程序设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleSettings {
    // 基础设置
    pub line_thickness: f32,
    pub font_size: f32,
    pub auto_copy: bool,
    pub show_cursor: bool,
    pub delay_ms: u32,
}

impl Default for SimpleSettings {
    fn default() -> Self {
        Self {
            line_thickness: 3.0,
            font_size: 20.0,
            auto_copy: false,
            show_cursor: false,
            delay_ms: 0,
        }
    }
}

impl SimpleSettings {
    /// 获取设置文件路径
    fn get_settings_path() -> PathBuf {
        let mut path = std::env::current_exe().unwrap_or_default();
        path.set_file_name("simple_settings.json");
        path
    }

    /// 从文件加载设置
    pub fn load() -> Self {
        let path = Self::get_settings_path();

        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(settings) = serde_json::from_str::<SimpleSettings>(&content) {
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
}

/// 简单设置窗口
pub struct SimpleSettingsWindow {
    hwnd: HWND,
    settings: SimpleSettings,
    // 控件句柄
    line_thickness_edit: HWND,
    font_size_edit: HWND,
    auto_copy_check: HWND,
    show_cursor_check: HWND,
    delay_edit: HWND,
    ok_button: HWND,
    cancel_button: HWND,
}

// 控件ID
const ID_LINE_THICKNESS: i32 = 1001;
const ID_FONT_SIZE: i32 = 1002;
const ID_AUTO_COPY: i32 = 1003;
const ID_SHOW_CURSOR: i32 = 1004;
const ID_DELAY: i32 = 1005;
const ID_OK: i32 = 1006;
const ID_CANCEL: i32 = 1007;

impl SimpleSettingsWindow {
    /// 显示设置窗口
    pub fn show(parent_hwnd: HWND) -> Result<()> {
        unsafe {
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

            // 创建现代化设置窗口
            let hwnd = CreateWindowExW(
                WS_EX_DLGMODALFRAME | WS_EX_CONTROLPARENT,
                PCWSTR(class_name.as_ptr()),
                PCWSTR(to_wide_chars("🎨 截图工具 - 设置").as_ptr()),
                WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                650, // 更宽的窗口
                520, // 更高的窗口
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
                let settings = SimpleSettings::load();
                let mut window = SimpleSettingsWindow {
                    hwnd,
                    settings,
                    line_thickness_edit: HWND::default(),
                    font_size_edit: HWND::default(),
                    auto_copy_check: HWND::default(),
                    show_cursor_check: HWND::default(),
                    delay_edit: HWND::default(),
                    ok_button: HWND::default(),
                    cancel_button: HWND::default(),
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
                    window.handle_command((wparam.0 & 0xFFFF) as i32);
                }
                LRESULT(0)
            }

            WM_CLOSE => {
                let window_ptr =
                    GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SimpleSettingsWindow;
                if !window_ptr.is_null() {
                    let _window = Box::from_raw(window_ptr);
                }
                let _ = DestroyWindow(hwnd);
                LRESULT(0)
            }

            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    /// 创建控件
    fn create_controls(&mut self) {
        unsafe {
            let instance = GetModuleHandleW(None).unwrap().into();

            // 现代化标题
            let _ = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("STATIC").as_ptr()),
                PCWSTR(to_wide_chars("🎨 截图工具 - 现代化设置").as_ptr()),
                WS_VISIBLE | WS_CHILD,
                20,
                20,
                610,
                35,
                Some(self.hwnd),
                None,
                Some(instance),
                None,
            );

            // 分隔线
            let _ = CreateWindowExW(
                WS_EX_STATICEDGE,
                PCWSTR(to_wide_chars("STATIC").as_ptr()),
                PCWSTR::null(),
                WS_VISIBLE | WS_CHILD,
                20,
                60,
                610,
                2,
                Some(self.hwnd),
                None,
                Some(instance),
                None,
            );

            // 🖌️ 绘图工具分组标题
            let _ = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("STATIC").as_ptr()),
                PCWSTR(to_wide_chars("🖌️ 绘图工具").as_ptr()),
                WS_VISIBLE | WS_CHILD,
                40,
                80,
                200,
                25,
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
                60,
                115,
                150,
                20,
                Some(self.hwnd),
                None,
                Some(instance),
                None,
            );

            self.line_thickness_edit = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                PCWSTR(to_wide_chars("EDIT").as_ptr()),
                PCWSTR::null(),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP,
                230,
                113,
                100,
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
                30,
                95,
                120,
                20,
                Some(self.hwnd),
                None,
                Some(instance),
                None,
            );

            self.font_size_edit = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                PCWSTR(to_wide_chars("EDIT").as_ptr()),
                PCWSTR::null(),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP,
                160,
                93,
                80,
                24,
                Some(self.hwnd),
                Some(HMENU(ID_FONT_SIZE as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();

            // 复选框
            self.auto_copy_check = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("BUTTON").as_ptr()),
                PCWSTR(to_wide_chars("自动复制到剪贴板").as_ptr()),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP,
                30,
                130,
                200,
                24,
                Some(self.hwnd),
                Some(HMENU(ID_AUTO_COPY as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();

            self.show_cursor_check = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("BUTTON").as_ptr()),
                PCWSTR(to_wide_chars("截图时显示光标").as_ptr()),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP,
                30,
                160,
                200,
                24,
                Some(self.hwnd),
                Some(HMENU(ID_SHOW_CURSOR as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();

            // 延迟设置
            let _ = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("STATIC").as_ptr()),
                PCWSTR(to_wide_chars("截图延迟 (毫秒):").as_ptr()),
                WS_VISIBLE | WS_CHILD,
                30,
                195,
                120,
                20,
                Some(self.hwnd),
                None,
                Some(instance),
                None,
            );

            self.delay_edit = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                PCWSTR(to_wide_chars("EDIT").as_ptr()),
                PCWSTR::null(),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP,
                160,
                193,
                80,
                24,
                Some(self.hwnd),
                Some(HMENU(ID_DELAY as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();

            // 按钮
            self.ok_button = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("BUTTON").as_ptr()),
                PCWSTR(to_wide_chars("确定").as_ptr()),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP,
                200,
                250,
                80,
                32,
                Some(self.hwnd),
                Some(HMENU(ID_OK as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();

            self.cancel_button = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("BUTTON").as_ptr()),
                PCWSTR(to_wide_chars("取消").as_ptr()),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP,
                290,
                250,
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
    fn load_values(&self) {
        unsafe {
            // 加载数值
            let thickness_text = to_wide_chars(&self.settings.line_thickness.to_string());
            let _ = SetWindowTextW(self.line_thickness_edit, PCWSTR(thickness_text.as_ptr()));

            let font_size_text = to_wide_chars(&self.settings.font_size.to_string());
            let _ = SetWindowTextW(self.font_size_edit, PCWSTR(font_size_text.as_ptr()));

            let delay_text = to_wide_chars(&self.settings.delay_ms.to_string());
            let _ = SetWindowTextW(self.delay_edit, PCWSTR(delay_text.as_ptr()));

            // 设置复选框状态
            let _ = SendMessageW(
                self.auto_copy_check,
                BM_SETCHECK,
                Some(WPARAM(if self.settings.auto_copy { 1 } else { 0 })),
                None,
            );

            let _ = SendMessageW(
                self.show_cursor_check,
                BM_SETCHECK,
                Some(WPARAM(if self.settings.show_cursor { 1 } else { 0 })),
                None,
            );
        }
    }

    /// 处理命令消息
    fn handle_command(&mut self, command_id: i32) {
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
            _ => {}
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

            // 读取字体大小
            if GetWindowTextW(self.font_size_edit, &mut buffer) > 0 {
                let text = String::from_utf16_lossy(&buffer);
                let text = text.trim_end_matches('\0');
                if let Ok(value) = text.parse::<f32>() {
                    self.settings.font_size = value.max(8.0).min(72.0);
                }
            }

            // 读取延迟
            if GetWindowTextW(self.delay_edit, &mut buffer) > 0 {
                let text = String::from_utf16_lossy(&buffer);
                let text = text.trim_end_matches('\0');
                if let Ok(value) = text.parse::<u32>() {
                    self.settings.delay_ms = value.min(5000);
                }
            }

            // 读取复选框状态
            let auto_copy_state = SendMessageW(self.auto_copy_check, BM_GETCHECK, None, None);
            self.settings.auto_copy = auto_copy_state.0 != 0;

            let show_cursor_state = SendMessageW(self.show_cursor_check, BM_GETCHECK, None, None);
            self.settings.show_cursor = show_cursor_state.0 != 0;
        }
    }
}
