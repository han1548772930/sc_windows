use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

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
    // 颜色设置
    pub color_red: u8,
    pub color_green: u8,
    pub color_blue: u8,
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

/// 全局设置窗口句柄，确保只能打开一个设置窗口
static mut SETTINGS_WINDOW: Option<HWND> = None;

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
    color_button: HWND,
    color_preview: HWND,
    ok_button: HWND,
    cancel_button: HWND,
    // 字体句柄
    font: HFONT,
}

// 控件ID
const ID_LINE_THICKNESS: i32 = 1001;
const ID_FONT_SIZE: i32 = 1002;
const ID_AUTO_COPY: i32 = 1003;
const ID_SHOW_CURSOR: i32 = 1004;
const ID_DELAY: i32 = 1005;
const ID_COLOR_BUTTON: i32 = 1006;
const ID_OK: i32 = 1007;
const ID_CANCEL: i32 = 1008;

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

            // 创建现代化设置窗口
            let hwnd = CreateWindowExW(
                WS_EX_DLGMODALFRAME | WS_EX_CONTROLPARENT,
                PCWSTR(class_name.as_ptr()),
                PCWSTR(to_wide_chars("🎨 截图工具 - 设置").as_ptr()),
                WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                480, // 窗口宽度
                450, // 窗口高度 - 增加高度以显示按钮
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
                        auto_copy_check: HWND::default(),
                        show_cursor_check: HWND::default(),
                        delay_edit: HWND::default(),
                        color_button: HWND::default(),
                        color_preview: HWND::default(),
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
                        window.handle_command((wparam.0 & 0xFFFF) as i32);
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

                        // 检查是否是颜色预览控件
                        if control_hwnd == window.color_preview {
                            // 创建颜色画刷
                            let color = (window.settings.color_red as u32)
                                | ((window.settings.color_green as u32) << 8)
                                | ((window.settings.color_blue as u32) << 16);
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

    /// 创建控件
    fn create_controls(&mut self) {
        unsafe {
            let instance = GetModuleHandleW(None).unwrap().into();

            // 创建微软雅黑字体
            self.font = CreateFontW(
                -14,                                        // 字体高度
                0,                                          // 字体宽度
                0,                                          // 角度
                0,                                          // 基线角度
                FW_NORMAL.0 as i32,                         // 字体粗细
                0,                                          // 斜体
                0,                                          // 下划线
                0,                                          // 删除线
                DEFAULT_CHARSET,                            // 字符集
                OUT_DEFAULT_PRECIS,                         // 输出精度
                CLIP_DEFAULT_PRECIS,                        // 裁剪精度
                DEFAULT_QUALITY,                            // 输出质量
                (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,   // 字体族
                PCWSTR(to_wide_chars("微软雅黑").as_ptr()), // 字体名称
            );

            // 🖌️ 绘图设置分组
            let group1 = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("STATIC").as_ptr()),
                PCWSTR(to_wide_chars("🖌️ 绘图设置").as_ptr()),
                WS_VISIBLE | WS_CHILD,
                20,
                20,
                200,
                20,
                Some(self.hwnd),
                None,
                Some(instance),
                None,
            )
            .unwrap_or_default();
            SendMessageW(group1, WM_SETFONT, Some(WPARAM(self.font.0 as usize)), None);

            // 线条粗细
            let thickness_label = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("STATIC").as_ptr()),
                PCWSTR(to_wide_chars("线条粗细:").as_ptr()),
                WS_VISIBLE | WS_CHILD,
                40,
                50,
                80,
                20,
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

            self.line_thickness_edit = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                PCWSTR(to_wide_chars("EDIT").as_ptr()),
                PCWSTR::null(),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP,
                130,
                48,
                60,
                22,
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

            // 字体大小
            let font_label = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("STATIC").as_ptr()),
                PCWSTR(to_wide_chars("字体大小:").as_ptr()),
                WS_VISIBLE | WS_CHILD,
                240,
                50,
                80,
                20,
                Some(self.hwnd),
                None,
                Some(instance),
                None,
            )
            .unwrap_or_default();
            SendMessageW(
                font_label,
                WM_SETFONT,
                Some(WPARAM(self.font.0 as usize)),
                None,
            );

            self.font_size_edit = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                PCWSTR(to_wide_chars("EDIT").as_ptr()),
                PCWSTR::null(),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP,
                330,
                48,
                60,
                22,
                Some(self.hwnd),
                Some(HMENU(ID_FONT_SIZE as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();
            SendMessageW(
                self.font_size_edit,
                WM_SETFONT,
                Some(WPARAM(self.font.0 as usize)),
                None,
            );

            // 选项设置分组
            let options_group = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("STATIC").as_ptr()),
                PCWSTR(to_wide_chars("⚙️ 选项设置").as_ptr()),
                WS_VISIBLE | WS_CHILD,
                20,
                90,
                200,
                20,
                Some(self.hwnd),
                None,
                Some(instance),
                None,
            )
            .unwrap_or_default();
            SendMessageW(
                options_group,
                WM_SETFONT,
                Some(WPARAM(self.font.0 as usize)),
                None,
            );

            // 复选框 - 使用普通样式，然后子类化
            self.auto_copy_check = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("BUTTON").as_ptr()),
                PCWSTR(to_wide_chars("自动复制到剪贴板").as_ptr()),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
                40,
                120,
                180,
                22,
                Some(self.hwnd),
                Some(HMENU(ID_AUTO_COPY as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();
            SendMessageW(
                self.auto_copy_check,
                WM_SETFONT,
                Some(WPARAM(self.font.0 as usize)),
                None,
            );

            // 移除复选框的主题以获得更好的控制
            let _ = SetWindowTheme(self.auto_copy_check, PCWSTR::null(), PCWSTR::null());

            // 强制重绘复选框
            let _ = InvalidateRect(Some(self.auto_copy_check), None, true);
            let _ = UpdateWindow(self.auto_copy_check);

            self.show_cursor_check = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("BUTTON").as_ptr()),
                PCWSTR(to_wide_chars("截图时显示光标").as_ptr()),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
                240,
                120,
                180,
                22,
                Some(self.hwnd),
                Some(HMENU(ID_SHOW_CURSOR as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();
            SendMessageW(
                self.show_cursor_check,
                WM_SETFONT,
                Some(WPARAM(self.font.0 as usize)),
                None,
            );

            // 移除复选框的主题以获得更好的控制
            let _ = SetWindowTheme(self.show_cursor_check, PCWSTR::null(), PCWSTR::null());

            // 强制重绘复选框
            let _ = InvalidateRect(Some(self.show_cursor_check), None, true);
            let _ = UpdateWindow(self.show_cursor_check);

            // 延迟设置
            let delay_label = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("STATIC").as_ptr()),
                PCWSTR(to_wide_chars("截图延迟(毫秒):").as_ptr()),
                WS_VISIBLE | WS_CHILD,
                40,
                195,
                120,
                20,
                Some(self.hwnd),
                None,
                Some(instance),
                None,
            )
            .unwrap_or_default();
            SendMessageW(
                delay_label,
                WM_SETFONT,
                Some(WPARAM(self.font.0 as usize)),
                None,
            );

            self.delay_edit = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                PCWSTR(to_wide_chars("EDIT").as_ptr()),
                PCWSTR::null(),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP,
                170,
                193,
                60,
                22,
                Some(self.hwnd),
                Some(HMENU(ID_DELAY as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();
            SendMessageW(
                self.delay_edit,
                WM_SETFONT,
                Some(WPARAM(self.font.0 as usize)),
                None,
            );

            // 🎨 颜色设置分组标题
            let color_group = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("STATIC").as_ptr()),
                PCWSTR(to_wide_chars("🎨 颜色设置").as_ptr()),
                WS_VISIBLE | WS_CHILD,
                20,
                240,
                200,
                20,
                Some(self.hwnd),
                None,
                Some(instance),
                None,
            )
            .unwrap_or_default();
            SendMessageW(
                color_group,
                WM_SETFONT,
                Some(WPARAM(self.font.0 as usize)),
                None,
            );

            // 颜色标签
            let color_label = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("STATIC").as_ptr()),
                PCWSTR(to_wide_chars("绘图颜色:").as_ptr()),
                WS_VISIBLE | WS_CHILD,
                40,
                270,
                80,
                20,
                Some(self.hwnd),
                None,
                Some(instance),
                None,
            )
            .unwrap_or_default();
            SendMessageW(
                color_label,
                WM_SETFONT,
                Some(WPARAM(self.font.0 as usize)),
                None,
            );

            // 颜色预览框
            self.color_preview = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                PCWSTR(to_wide_chars("STATIC").as_ptr()),
                PCWSTR::null(),
                WS_VISIBLE | WS_CHILD,
                130,
                268,
                40,
                24,
                Some(self.hwnd),
                None,
                Some(instance),
                None,
            )
            .unwrap_or_default();

            // 颜色选择按钮
            self.color_button = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("BUTTON").as_ptr()),
                PCWSTR(to_wide_chars("选择颜色...").as_ptr()),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP,
                180,
                268,
                90,
                24,
                Some(self.hwnd),
                Some(HMENU(ID_COLOR_BUTTON as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();
            Self::set_modern_theme(self.color_button);
            SendMessageW(
                self.color_button,
                WM_SETFONT,
                Some(WPARAM(self.font.0 as usize)),
                None,
            );

            // 按钮
            self.ok_button = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("BUTTON").as_ptr()),
                PCWSTR(to_wide_chars("确定").as_ptr()),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP,
                280,
                350,
                80,
                30,
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

            self.cancel_button = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("BUTTON").as_ptr()),
                PCWSTR(to_wide_chars("取消").as_ptr()),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP,
                370,
                350,
                80,
                30,
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

            // 更新颜色预览
            self.update_color_preview();
        }
    }

    /// 处理命令消息
    fn handle_command(&mut self, command_id: i32) {
        match command_id {
            ID_OK => {
                self.save_settings();
                unsafe {
                    let _ = self.settings.save();
                    let _ = PostMessageW(Some(self.hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
                }
            }
            ID_CANCEL => unsafe {
                let _ = PostMessageW(Some(self.hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
            },
            ID_COLOR_BUTTON => {
                self.show_color_dialog();
            }
            _ => {}
        }
    }

    /// 显示颜色选择对话框
    fn show_color_dialog(&mut self) {
        unsafe {
            // 创建自定义颜色数组
            let mut custom_colors = [COLORREF(0); 16];

            let mut cc = CHOOSECOLORW {
                lStructSize: std::mem::size_of::<CHOOSECOLORW>() as u32,
                hwndOwner: self.hwnd,
                hInstance: HWND::default(),
                rgbResult: COLORREF(
                    (self.settings.color_red as u32)
                        | ((self.settings.color_green as u32) << 8)
                        | ((self.settings.color_blue as u32) << 16),
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
                self.settings.color_red = (color & 0xFF) as u8;
                self.settings.color_green = ((color >> 8) & 0xFF) as u8;
                self.settings.color_blue = ((color >> 16) & 0xFF) as u8;

                // 更新颜色预览
                self.update_color_preview();
            }
        }
    }

    /// 更新颜色预览
    fn update_color_preview(&self) {
        unsafe {
            // 强制重绘颜色预览控件
            let _ = InvalidateRect(Some(self.color_preview), None, true);
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

/// 显示设置窗口的便利函数
pub fn show_settings_window() -> Result<()> {
    SimpleSettingsWindow::show(HWND::default())
}
