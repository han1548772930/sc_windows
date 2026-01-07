//! Settings window UI implementation
//!
//! This module contains the Win32 settings window implementation,
//! using a tabbed interface similar to native-windows-gui demo.

use std::ffi::c_void;

use windows::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::LibraryLoader::*,
        UI::{Controls::Dialogs::*, Controls::*, WindowsAndMessaging::*},
    },
    core::*,
};

use std::sync::atomic::{AtomicIsize, Ordering};

use super::core::Settings;
use crate::WINDOW_CLASS_NAME;
use crate::ocr::get_available_languages;
use crate::ui::controls::{Font, Tab, TabsContainer};
use crate::utils::to_wide_chars;

/// 全局设置窗口句柄，确保只能打开一个设置窗口
static SETTINGS_WINDOW: AtomicIsize = AtomicIsize::new(0);

/// 布局常量 - 放大以适应字体
const MARGIN: i32 = 15;
const ROW_HEIGHT: i32 = 32;
const ROW_SPACING: i32 = 8;
const LABEL_WIDTH: i32 = 80;
const CONTROL_HEIGHT: i32 = 28;
const BUTTON_WIDTH: i32 = 90;
const BUTTON_HEIGHT: i32 = 30;

/// 设置窗口
pub struct SettingsWindow {
    hwnd: HWND,
    settings: Settings,
    // Tab 控件
    tabs_container: HWND,
    tab_drawing: HWND, // 绘图设置 Tab
    tab_system: HWND,  // 系统设置 Tab
    // 绘图设置控件
    line_thickness_edit: HWND,
    font_choose_button: HWND,
    drawing_color_button: HWND,
    drawing_color_preview: HWND,
    text_color_preview: HWND,
    // 系统设置控件
    hotkey_edit: HWND,
    config_path_edit: HWND,
    config_path_browse_button: HWND,
    ocr_language_combo: HWND,
    // 底部按钮
    ok_button: HWND,
    cancel_button: HWND,
    // 字体句柄
    font: HFONT,
}

// 控件 ID
const ID_LINE_THICKNESS: i32 = 1001;
const ID_FONT_CHOOSE_BUTTON: i32 = 1003;
const ID_DRAWING_COLOR_BUTTON: i32 = 1006;
const ID_HOTKEY_EDIT: i32 = 1008;
const ID_CONFIG_PATH_EDIT: i32 = 1011;
const ID_CONFIG_PATH_BROWSE: i32 = 1012;
const ID_OCR_LANGUAGE_COMBO: i32 = 1013;
const ID_OK: i32 = 1009;
const ID_CANCEL: i32 = 1010;

impl SettingsWindow {
    /// 检查设置窗口是否已经打开
    pub fn is_open() -> bool {
        let hwnd_value = SETTINGS_WINDOW.load(Ordering::Acquire);
        if hwnd_value != 0 {
            let hwnd = HWND(hwnd_value as *mut _);
            unsafe { IsWindow(Some(hwnd)).as_bool() }
        } else {
            false
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
                        let wndproc: WNDPROC = std::mem::transmute(original_proc);
                        return CallWindowProcW(wndproc, hwnd, msg, wparam, lparam);
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
                    if ((key >= 'A' as u32 && key <= 'Z' as u32)
                        || (key >= '0' as u32 && key <= '9' as u32))
                        && modifiers != 0
                    {
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
                        let wndproc: WNDPROC = std::mem::transmute(original_proc);
                        return CallWindowProcW(wndproc, hwnd, msg, wparam, lparam);
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
            let existing_hwnd_value = SETTINGS_WINDOW.load(Ordering::Acquire);
            if existing_hwnd_value != 0 {
                let existing_hwnd = HWND(existing_hwnd_value as *mut _);
                if IsWindow(Some(existing_hwnd)).as_bool() {
                    // 如果窗口已存在，将其置于前台
                    let _ = ShowWindow(existing_hwnd, SW_RESTORE);
                    let _ = SetForegroundWindow(existing_hwnd);
                    let _ = BringWindowToTop(existing_hwnd);
                    return Ok(());
                } else {
                    // 窗口句柄无效，清除它
                    SETTINGS_WINDOW.store(0, Ordering::Release);
                }
            }
            // 初始化Common Controls 6.0以启用现代样式
            let icc = INITCOMMONCONTROLSEX {
                dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
                dwICC: ICC_STANDARD_CLASSES
                    | ICC_WIN95_CLASSES
                    | ICC_TAB_CLASSES
                    | ICC_PROGRESS_CLASS
                    | ICC_LISTVIEW_CLASSES,
            };
            let _ = InitCommonControlsEx(&icc);

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

            // 创建固定大小的设置窗口
            let hwnd = CreateWindowExW(
                WS_EX_DLGMODALFRAME | WS_EX_CONTROLPARENT,
                PCWSTR(class_name.as_ptr()),
                PCWSTR(to_wide_chars("🎨 截图工具 - 设置").as_ptr()),
                WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX, // 移除 WS_THICKFRAME
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                480, // 窗口宽度
                480, // 窗口高度
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
            SETTINGS_WINDOW.store(hwnd.0 as isize, Ordering::Release);

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
                    let settings = Settings::load();
                    let mut window = SettingsWindow {
                        hwnd,
                        settings,
                        tabs_container: HWND::default(),
                        tab_drawing: HWND::default(),
                        tab_system: HWND::default(),
                        line_thickness_edit: HWND::default(),
                        font_choose_button: HWND::default(),
                        drawing_color_button: HWND::default(),
                        drawing_color_preview: HWND::default(),
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

                WM_NOTIFY => {
                    let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SettingsWindow;
                    if !window_ptr.is_null() {
                        let window = &*window_ptr;
                        let nmhdr = &*(lparam.0 as *const NMHDR);
                        // TCN_SELCHANGE = TCN_FIRST - 1 = -551 = 0xFFFFFDD9 (as u32)
                        if nmhdr.code == 0xFFFFFDD9_u32 {
                            window.handle_tab_change();
                        }
                    }
                    LRESULT(0)
                }

                WM_SIZE => DefWindowProcW(hwnd, msg, wparam, lparam),

                WM_COMMAND => {
                    let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SettingsWindow;
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
                    let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SettingsWindow;
                    if !window_ptr.is_null() {
                        let _window = Box::from_raw(window_ptr);
                        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                    }
                    SETTINGS_WINDOW.store(0, Ordering::Release);
                    let _ = DestroyWindow(hwnd);
                    LRESULT(0)
                }

                WM_CTLCOLORSTATIC => {
                    let hdc = HDC(wparam.0 as *mut _);
                    let control_hwnd = HWND(lparam.0 as *mut _);

                    let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SettingsWindow;
                    if !window_ptr.is_null() {
                        let window = &*window_ptr;

                        // 检查是否是绘图颜色预览控件
                        if control_hwnd == window.drawing_color_preview {
                            let color = (window.settings.drawing_color_red as u32)
                                | ((window.settings.drawing_color_green as u32) << 8)
                                | ((window.settings.drawing_color_blue as u32) << 16);
                            let brush = CreateSolidBrush(COLORREF(color));
                            SetBkColor(hdc, COLORREF(color));
                            return LRESULT(brush.0 as isize);
                        }

                        // 检查是否是文字颜色预览控件
                        if control_hwnd == window.text_color_preview {
                            let color = (window.settings.text_color_red as u32)
                                | ((window.settings.text_color_green as u32) << 8)
                                | ((window.settings.text_color_blue as u32) << 16);
                            let brush = CreateSolidBrush(COLORREF(color));
                            SetBkColor(hdc, COLORREF(color));
                            return LRESULT(brush.0 as isize);
                        }
                    }

                    // Tab 页面内的标签使用白色背景
                    SetBkMode(hdc, TRANSPARENT);
                    SetTextColor(hdc, COLORREF(0x000000)); // 黑色文字
                    LRESULT(GetStockObject(WHITE_BRUSH).0 as isize)
                }

                WM_CTLCOLOREDIT => {
                    // 处理编辑框背景色，确保热键输入框不会变黑
                    let hdc = HDC(wparam.0 as *mut _);

                    // 强制设置白色背景和黑色文字
                    SetBkColor(hdc, COLORREF(0xFFFFFF)); // 白色背景
                    SetTextColor(hdc, COLORREF(0x000000)); // 黑色文字
                    SetBkMode(hdc, OPAQUE); // 不透明背景

                    // 返回白色画刷
                    LRESULT(GetStockObject(WHITE_BRUSH).0 as isize)
                }

                WM_CTLCOLORBTN => {
                    // 处理复选框背景 - 返回NULL画刷强制透明
                    let hdc = HDC(wparam.0 as *mut _);
                    SetBkMode(hdc, TRANSPARENT);
                    LRESULT(GetStockObject(NULL_BRUSH).0 as isize)
                }

                WM_ERASEBKGND => {
                    // 处理背景擦除 - 确保复选框区域透明
                    let hdc = HDC(wparam.0 as *mut _);
                    let mut rect = RECT::default();
                    let _ = GetClientRect(hwnd, &mut rect);

                    // 使用系统背景色填充
                    let bg_brush = GetSysColorBrush(COLOR_BTNFACE);
                    FillRect(hdc, &rect, bg_brush);

                    LRESULT(1) // 表示我们处理了背景擦除
                }

                WM_PAINT => {
                    // 强制重绘所有编辑框，确保它们保持正确的颜色
                    let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SettingsWindow;
                    if !window_ptr.is_null() {
                        let window = &*window_ptr;

                        // 强制重绘热键输入框
                        let _ = InvalidateRect(Some(window.hotkey_edit), None, TRUE.into());
                        let _ = UpdateWindow(window.hotkey_edit);
                    }

                    // 调用默认处理
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }

                WM_DESTROY => {
                    SETTINGS_WINDOW.store(0, Ordering::Release);
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

    /// 查找具有指定文本的子控件（在指定父窗口内）
    fn find_control_by_text_in_parent(&self, text: &str, parent: HWND) -> Option<HWND> {
        unsafe {
            if let Ok(mut child) = GetWindow(parent, GW_CHILD) {
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

    /// Tab 布局 - 现代化 Tab 界面
    fn layout_controls(&mut self) {
        unsafe {
            let mut client_rect = RECT::default();
            let _ = GetClientRect(self.hwnd, &mut client_rect);
            let window_width = client_rect.right - client_rect.left;
            let window_height = client_rect.bottom - client_rect.top;

            // 布局参数
            let button_height = BUTTON_HEIGHT;
            let button_width = BUTTON_WIDTH;

            // ═══════════════════════════════════════════════════
            // TabsContainer 布局
            // ═══════════════════════════════════════════════════
            let tabs_height = window_height - button_height - MARGIN * 3;
            let tabs_width = window_width - MARGIN * 2;
            if !self.tabs_container.is_invalid() {
                let _ = SetWindowPos(
                    self.tabs_container,
                    None,
                    MARGIN,
                    MARGIN,
                    tabs_width,
                    tabs_height,
                    SWP_NOZORDER,
                );

                // Tab 页面位置和大小（参考 NWG）
                let page_x = 5;
                let page_y = 25;
                let page_width = tabs_width - 11;
                let page_height = tabs_height - 33;

                if !self.tab_drawing.is_invalid() {
                    let _ = SetWindowPos(
                        self.tab_drawing,
                        None,
                        page_x,
                        page_y,
                        page_width,
                        page_height,
                        SWP_NOZORDER,
                    );
                    self.layout_drawing_tab(page_width);
                }

                if !self.tab_system.is_invalid() {
                    let _ = SetWindowPos(
                        self.tab_system,
                        None,
                        page_x,
                        page_y,
                        page_width,
                        page_height,
                        SWP_NOZORDER,
                    );
                    self.layout_system_tab(page_width);
                }
            }

            // ═══════════════════════════════════════════════════
            // 底部按钮布局
            // ═══════════════════════════════════════════════════
            let button_spacing = 15;
            let buttons_total_width = button_width * 2 + button_spacing;
            let buttons_x = (window_width - buttons_total_width) / 2;
            let buttons_y = window_height - button_height - MARGIN;

            let _ = SetWindowPos(
                self.ok_button,
                None,
                buttons_x,
                buttons_y,
                button_width,
                button_height,
                SWP_NOZORDER,
            );
            let _ = SetWindowPos(
                self.cancel_button,
                None,
                buttons_x + button_width + button_spacing,
                buttons_y,
                button_width,
                button_height,
                SWP_NOZORDER,
            );

            // 强制重绘
            let _ = InvalidateRect(Some(self.hwnd), None, TRUE.into());
        }
    }

    /// 布局绘图设置 Tab 内的控件
    fn layout_drawing_tab(&self, _tab_width: i32) {
        unsafe {
            let margin = 10;
            let row_height = ROW_HEIGHT;
            let label_width = LABEL_WIDTH;
            let control_x = margin + label_width + 10;
            let edit_height = CONTROL_HEIGHT;
            let button_height = BUTTON_HEIGHT;

            let mut y = margin;

            // 线条粗细
            if let Some(label) = self.find_control_by_text_in_parent("线条粗细:", self.tab_drawing)
            {
                let _ = SetWindowPos(label, None, margin, y + 3, label_width, 18, SWP_NOZORDER);
            }
            let _ = SetWindowPos(
                self.line_thickness_edit,
                None,
                control_x,
                y,
                60,
                edit_height,
                SWP_NOZORDER,
            );
            y += row_height + ROW_SPACING;

            // 字体设置
            if let Some(label) = self.find_control_by_text_in_parent("字体设置:", self.tab_drawing)
            {
                let _ = SetWindowPos(label, None, margin, y + 3, label_width, 18, SWP_NOZORDER);
            }
            let _ = SetWindowPos(
                self.font_choose_button,
                None,
                control_x,
                y,
                110,
                button_height,
                SWP_NOZORDER,
            );
            y += row_height + ROW_SPACING;

            // 绘图颜色
            if let Some(label) = self.find_control_by_text_in_parent("绘图颜色:", self.tab_drawing)
            {
                let _ = SetWindowPos(label, None, margin, y + 3, label_width, 18, SWP_NOZORDER);
            }
            let _ = SetWindowPos(
                self.drawing_color_preview,
                None,
                control_x,
                y + 2,
                24,
                20,
                SWP_NOZORDER,
            );
            let _ = SetWindowPos(
                self.drawing_color_button,
                None,
                control_x + 32,
                y,
                100,
                button_height,
                SWP_NOZORDER,
            );
        }
    }

    /// 布局系统设置 Tab 内的控件
    fn layout_system_tab(&self, tab_width: i32) {
        unsafe {
            let margin = 10;
            let row_height = ROW_HEIGHT;
            let label_width = LABEL_WIDTH;
            let control_x = margin + label_width + 10;
            let edit_height = CONTROL_HEIGHT;
            let button_height = BUTTON_HEIGHT;
            let available_width = tab_width - margin * 2;

            let mut y = margin;

            // 截图热键
            if let Some(label) = self.find_control_by_text_in_parent("截图热键:", self.tab_system)
            {
                let _ = SetWindowPos(label, None, margin, y + 3, label_width, 18, SWP_NOZORDER);
            }
            let hotkey_width = available_width - label_width - 20;
            let _ = SetWindowPos(
                self.hotkey_edit,
                None,
                control_x,
                y,
                hotkey_width,
                edit_height,
                SWP_NOZORDER,
            );
            y += row_height + ROW_SPACING;

            // 保存路径
            if let Some(label) = self.find_control_by_text_in_parent("保存路径:", self.tab_system)
            {
                let _ = SetWindowPos(label, None, margin, y + 3, label_width, 18, SWP_NOZORDER);
            }
            let browse_width = 80;
            let path_width = available_width - label_width - browse_width - 30;
            let _ = SetWindowPos(
                self.config_path_edit,
                None,
                control_x,
                y,
                path_width,
                edit_height,
                SWP_NOZORDER,
            );
            let _ = SetWindowPos(
                self.config_path_browse_button,
                None,
                control_x + path_width + 8,
                y,
                browse_width,
                button_height,
                SWP_NOZORDER,
            );
            y += row_height + ROW_SPACING;

            // OCR语言
            if let Some(label) = self.find_control_by_text_in_parent("OCR语言", self.tab_system) {
                let _ = SetWindowPos(label, None, margin, y + 3, label_width, 18, SWP_NOZORDER);
            }
            let _ = SetWindowPos(
                self.ocr_language_combo,
                None,
                control_x,
                y,
                160,
                200,
                SWP_NOZORDER,
            );
        }
    }

    /// 创建控件 - 使用 Tab 布局
    fn create_controls(&mut self) {
        unsafe {
            let instance = GetModuleHandleW(None).unwrap_or_default().into();

            // 系统默认 GUI 字体
            self.font = HFONT(GetStockObject(DEFAULT_GUI_FONT).0);

            // 获取窗口客户区大小
            let mut client_rect = RECT::default();
            let _ = GetClientRect(self.hwnd, &mut client_rect);
            let window_width = client_rect.right - client_rect.left;
            let window_height = client_rect.bottom - client_rect.top;

            // ═══════════════════════════════════════════════════════════════════════════════
            // 创建 TabsContainer
            // ═══════════════════════════════════════════════════════════════════════════════
            let tabs_height = window_height - BUTTON_HEIGHT - MARGIN * 3;
            let tabs = TabsContainer::builder()
                .position(MARGIN, MARGIN)
                .size(window_width - MARGIN * 2, tabs_height)
                .parent(self.hwnd)
                .build()
                .unwrap();
            tabs.set_font(&Font { handle: self.font });
            self.tabs_container = tabs.handle;

            // ═══════════════════════════════════════════════════════════════════════════════
            // 创建绘图设置 Tab
            // ═══════════════════════════════════════════════════════════════════════════════
            let tab_drawing = Tab::builder()
                .text("绘图设置")
                .parent(self.hwnd)
                .build(&tabs)
                .unwrap();
            self.tab_drawing = tab_drawing.handle;

            // ═══════════════════════════════════════════════════════════════════════════════
            // 创建系统设置 Tab
            // ═══════════════════════════════════════════════════════════════════════════════
            let tab_system = Tab::builder()
                .text("系统设置")
                .parent(self.hwnd)
                .build(&tabs)
                .unwrap();
            self.tab_system = tab_system.handle;

            std::mem::forget(tabs);
            std::mem::forget(tab_drawing);
            std::mem::forget(tab_system);

            // ═══════════════════════════════════════════════════════════════════════════════
            // 绘图设置 Tab 内的控件
            // ═══════════════════════════════════════════════════════════════════════════════

            // 线条粗细标签
            let _ = self.create_label("线条粗细:", self.tab_drawing, instance);

            // 线条粗细输入框
            self.line_thickness_edit = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                PCWSTR(to_wide_chars("EDIT").as_ptr()),
                PCWSTR::null(),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP,
                0,
                0,
                0,
                0,
                Some(self.tab_drawing),
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

            // 字体设置标签
            let _ = self.create_label("字体设置:", self.tab_drawing, instance);

            // 字体选择按钮
            self.font_choose_button = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("BUTTON").as_ptr()),
                PCWSTR(to_wide_chars("选择字体...").as_ptr()),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP,
                0,
                0,
                0,
                0,
                Some(self.tab_drawing),
                Some(HMENU(ID_FONT_CHOOSE_BUTTON as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();
            SendMessageW(
                self.font_choose_button,
                WM_SETFONT,
                Some(WPARAM(self.font.0 as usize)),
                None,
            );

            // 绘图颜色标签
            let _ = self.create_label("绘图颜色:", self.tab_drawing, instance);

            // 绘图颜色预览框
            self.drawing_color_preview = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                PCWSTR(to_wide_chars("STATIC").as_ptr()),
                PCWSTR::null(),
                WS_VISIBLE | WS_CHILD,
                0,
                0,
                0,
                0,
                Some(self.tab_drawing),
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
                0,
                Some(self.tab_drawing),
                Some(HMENU(ID_DRAWING_COLOR_BUTTON as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();
            SendMessageW(
                self.drawing_color_button,
                WM_SETFONT,
                Some(WPARAM(self.font.0 as usize)),
                None,
            );

            // ═══════════════════════════════════════════════════════════════════════════════
            // 系统设置 Tab 内的控件
            // ═══════════════════════════════════════════════════════════════════════════════

            // 热键标签
            let _ = self.create_label("截图热键:", self.tab_system, instance);

            // 热键输入框
            self.hotkey_edit = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                PCWSTR(to_wide_chars("EDIT").as_ptr()),
                PCWSTR::null(),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP,
                0,
                0,
                0,
                0,
                Some(self.tab_system),
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
            Self::set_modern_theme(self.hotkey_edit);

            // 子类化热键输入框
            let original_proc = SetWindowLongPtrW(
                self.hotkey_edit,
                GWLP_WNDPROC,
                Self::hotkey_edit_proc as isize,
            );
            SetWindowLongPtrW(self.hotkey_edit, GWLP_USERDATA, original_proc);

            // 配置路径标签
            let _ = self.create_label("保存路径:", self.tab_system, instance);

            // 配置路径输入框
            self.config_path_edit = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                PCWSTR(to_wide_chars("EDIT").as_ptr()),
                PCWSTR::null(),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP,
                0,
                0,
                0,
                0,
                Some(self.tab_system),
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
                0,
                Some(self.tab_system),
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

            // OCR语言标签
            let _ = self.create_label("OCR语言", self.tab_system, instance);

            // OCR语言选择下拉框
            self.ocr_language_combo = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                PCWSTR(to_wide_chars("COMBOBOX").as_ptr()),
                PCWSTR::null(),
                WINDOW_STYLE(WS_VISIBLE.0 | WS_CHILD.0 | WS_TABSTOP.0 | 0x0003),
                0,
                0,
                0,
                0,
                Some(self.tab_system),
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

            // 加载 OCR 语言
            self.load_ocr_languages();

            // ═══════════════════════════════════════════════════════════════════════════════
            // 底部按钮（在主窗口中）
            // ═══════════════════════════════════════════════════════════════════════════════
            self.ok_button = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("BUTTON").as_ptr()),
                PCWSTR(to_wide_chars("确定").as_ptr()),
                WS_VISIBLE | WS_CHILD | WS_TABSTOP,
                0,
                0,
                0,
                0,
                Some(self.hwnd),
                Some(HMENU(ID_OK as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();
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
                0,
                0,
                0,
                0,
                Some(self.hwnd),
                Some(HMENU(ID_CANCEL as *mut _)),
                Some(instance),
                None,
            )
            .unwrap_or_default();
            SendMessageW(
                self.cancel_button,
                WM_SETFONT,
                Some(WPARAM(self.font.0 as usize)),
                None,
            );

            // 初始布局
            self.layout_controls();
        }
    }

    /// 创建标签辅助方法
    fn create_label(&self, text: &str, parent: HWND, instance: HINSTANCE) -> HWND {
        unsafe {
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("STATIC").as_ptr()),
                PCWSTR(to_wide_chars(text).as_ptr()),
                WS_VISIBLE | WS_CHILD,
                0,
                0,
                0,
                0,
                Some(parent),
                None,
                Some(instance),
                None,
            )
            .unwrap_or_default();
            SendMessageW(hwnd, WM_SETFONT, Some(WPARAM(self.font.0 as usize)), None);
            hwnd
        }
    }

    /// 加载 OCR 语言列表
    fn load_ocr_languages(&self) {
        unsafe {
            let available_languages = get_available_languages();

            if available_languages.is_empty() {
                let text = to_wide_chars("未找到 OCR 模型");
                SendMessageW(
                    self.ocr_language_combo,
                    0x0143,
                    Some(WPARAM(0)),
                    Some(LPARAM(text.as_ptr() as isize)),
                );
            } else {
                for (i, lang) in available_languages.iter().enumerate() {
                    let display = if i == 0 {
                        format!("{} (默认)", lang.display_name)
                    } else {
                        lang.display_name.clone()
                    };

                    let text = to_wide_chars(&display);
                    let index = SendMessageW(
                        self.ocr_language_combo,
                        0x0143,
                        Some(WPARAM(0)),
                        Some(LPARAM(text.as_ptr() as isize)),
                    );

                    let value_text = to_wide_chars(&lang.id);
                    let value_box = Box::new(value_text);
                    let value_ptr = Box::into_raw(value_box);
                    SendMessageW(
                        self.ocr_language_combo,
                        0x0151,
                        Some(WPARAM(index.0 as usize)),
                        Some(LPARAM(value_ptr as isize)),
                    );
                }
            }
        }
    }

    /// 处理 Tab 切换 - 直接切换页面可见性
    fn handle_tab_change(&self) {
        unsafe {
            if self.tabs_container.is_invalid() {
                return;
            }
            // 获取当前选中的 Tab 索引
            let index = SendMessageW(
                self.tabs_container,
                TCM_GETCURSEL,
                Some(WPARAM(0)),
                Some(LPARAM(0)),
            )
            .0 as i32;

            // 切换页面可见性
            if !self.tab_drawing.is_invalid() {
                let _ = ShowWindow(self.tab_drawing, if index == 0 { SW_SHOW } else { SW_HIDE });
            }
            if !self.tab_system.is_invalid() {
                let _ = ShowWindow(self.tab_system, if index == 1 { SW_SHOW } else { SW_HIDE });
            }
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
    fn handle_edit_change(&mut self, _control_id: i32) {
        {}
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
                        PCWSTR(to_wide_chars(WINDOW_CLASS_NAME).as_ptr()),
                        PCWSTR::null(),
                    ) && !main_hwnd.0.is_null()
                    {
                        // 发送自定义消息通知设置已更改 (WM_USER + 3)
                        let _ = PostMessageW(Some(main_hwnd), WM_USER + 3, WPARAM(0), LPARAM(0));
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
                    self.settings.line_thickness = value.clamp(1.0, 20.0);
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
                if folder_dialog.Show(Some(self.hwnd)).is_ok()
                    && let Ok(result) = folder_dialog.GetResult()
                    && let Ok(path) = result.GetDisplayName(SIGDN_FILESYSPATH)
                {
                    let path_str = path.to_string().unwrap_or_default();

                    // 更新输入框
                    let path_wide = to_wide_chars(&path_str);
                    let _ = SetWindowTextW(self.config_path_edit, PCWSTR(path_wide.as_ptr()));
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
            let message = format!("当前路径: {current_path}\n\n请手动在输入框中修改路径");
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
    // 直接使用传统的 Win32 设置窗口
    SettingsWindow::show(HWND::default())
}

/// 关闭设置窗口（如果已打开）
pub fn close_settings_window() {
    let hwnd_value = SETTINGS_WINDOW.load(Ordering::Acquire);
    if hwnd_value != 0 {
        let hwnd = HWND(hwnd_value as *mut _);
        unsafe {
            if IsWindow(Some(hwnd)).as_bool() {
                let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
            }
        }
    }
}
