use std::sync::atomic::Ordering;

use anyhow::Result;
use sc_ocr::get_available_languages;
use sc_platform::{HostPlatform, WindowId};
use sc_platform_windows::win_api::to_wide_chars;
use sc_platform_windows::windows::controls::{Font, Tab, TabsContainer};
use sc_platform_windows::windows::{
    WindowsHostPlatform, file_dialog, hwnd as to_hwnd, window_id as to_window_id,
};
use sc_settings::Settings;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::*;
use windows::Win32::UI::Controls::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::PCWSTR;

use super::{
    BUTTON_HEIGHT, ID_CANCEL, ID_CONFIG_PATH_BROWSE, ID_CONFIG_PATH_EDIT, ID_DRAWING_COLOR_BUTTON,
    ID_FONT_CHOOSE_BUTTON, ID_HOTKEY_EDIT, ID_LINE_THICKNESS, ID_OCR_LANGUAGE_COMBO, ID_OK, MARGIN,
    WINDOW_DEFAULT_HEIGHT, WINDOW_DEFAULT_WIDTH,
};

/// Settings window.
pub struct SettingsWindow;

pub(super) struct SettingsWindowState {
    pub(super) hwnd: HWND,
    pub(super) settings: Settings,

    // Tab controls.
    pub(super) tabs_container: HWND,
    pub(super) tab_drawing: HWND,
    pub(super) tab_system: HWND,

    // Drawing labels.
    pub(super) line_thickness_label: HWND,
    pub(super) font_label: HWND,
    pub(super) drawing_color_label: HWND,

    // Drawing controls.
    pub(super) line_thickness_edit: HWND,
    pub(super) font_choose_button: HWND,
    pub(super) drawing_color_button: HWND,
    pub(super) drawing_color_preview: HWND,
    pub(super) text_color_preview: HWND,

    // System labels.
    pub(super) hotkey_label: HWND,
    pub(super) config_path_label: HWND,
    pub(super) ocr_language_label: HWND,

    // System controls.
    pub(super) hotkey_edit: HWND,
    pub(super) config_path_edit: HWND,
    pub(super) config_path_browse_button: HWND,
    pub(super) ocr_language_combo: HWND,

    // Bottom buttons.
    pub(super) ok_button: HWND,
    pub(super) cancel_button: HWND,

    // Resources.
    pub(super) font: HFONT,
    pub(super) drawing_color_brush: HBRUSH,
    pub(super) text_color_brush: HBRUSH,
}

impl SettingsWindow {
    pub fn is_open() -> bool {
        SettingsWindowState::is_open()
    }

    /// Show settings window. Returns `Ok(true)` if the user saved successfully.
    pub fn show(parent_window: WindowId) -> Result<bool> {
        SettingsWindowState::show(parent_window)
    }
}

impl SettingsWindowState {
    pub(super) fn new(hwnd: HWND, settings: Settings) -> Self {
        Self {
            hwnd,
            settings,
            tabs_container: HWND::default(),
            tab_drawing: HWND::default(),
            tab_system: HWND::default(),
            line_thickness_label: HWND::default(),
            font_label: HWND::default(),
            drawing_color_label: HWND::default(),
            line_thickness_edit: HWND::default(),
            font_choose_button: HWND::default(),
            drawing_color_button: HWND::default(),
            drawing_color_preview: HWND::default(),
            text_color_preview: HWND::default(),
            hotkey_label: HWND::default(),
            config_path_label: HWND::default(),
            ocr_language_label: HWND::default(),
            hotkey_edit: HWND::default(),
            config_path_edit: HWND::default(),
            config_path_browse_button: HWND::default(),
            ocr_language_combo: HWND::default(),
            ok_button: HWND::default(),
            cancel_button: HWND::default(),
            font: HFONT::default(),
            drawing_color_brush: HBRUSH::default(),
            text_color_brush: HBRUSH::default(),
        }
    }

    pub fn is_open() -> bool {
        let hwnd_value = super::SETTINGS_WINDOW.load(Ordering::Acquire);
        if hwnd_value != 0 {
            let hwnd = HWND(hwnd_value as *mut _);
            unsafe { IsWindow(Some(hwnd)).as_bool() }
        } else {
            false
        }
    }

    /// Show settings window. Returns `Ok(true)` if the user saved successfully.
    pub fn show(parent_window: WindowId) -> Result<bool> {
        unsafe {
            // If already open, just bring to front.
            let existing_hwnd_value = super::SETTINGS_WINDOW.load(Ordering::Acquire);
            if existing_hwnd_value != 0 {
                let existing_hwnd = HWND(existing_hwnd_value as *mut _);
                if IsWindow(Some(existing_hwnd)).as_bool() {
                    let platform = WindowsHostPlatform::new();
                    let window_id = to_window_id(existing_hwnd);
                    let _ = platform.restore_window(window_id);
                    let _ = platform.bring_window_to_top(window_id);
                    return Ok(false);
                } else {
                    super::SETTINGS_WINDOW.store(0, Ordering::Release);
                }
            }

            super::SETTINGS_SAVED.store(false, Ordering::Release);

            // Enable modern common controls.
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

            // Register class (best-effort).
            let window_class = WNDCLASSW {
                lpfnWndProc: Some(Self::window_proc),
                hInstance: instance.into(),
                lpszClassName: PCWSTR(class_name.as_ptr()),
                hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as *mut _),
                hCursor: LoadCursorW(None, IDC_ARROW)?,
                style: CS_HREDRAW | CS_VREDRAW,
                ..Default::default()
            };
            let _ = RegisterClassW(&window_class);

            let parent_hwnd = if parent_window.is_valid() {
                Some(to_hwnd(parent_window))
            } else {
                None
            };

            // Create window.
            let hwnd = CreateWindowExW(
                WS_EX_DLGMODALFRAME | WS_EX_CONTROLPARENT,
                PCWSTR(class_name.as_ptr()),
                PCWSTR(to_wide_chars("🎨 截图工具 - 设置").as_ptr()),
                WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                WINDOW_DEFAULT_WIDTH,
                WINDOW_DEFAULT_HEIGHT,
                parent_hwnd,
                None,
                Some(instance.into()),
                None,
            )?;

            // Center.
            let mut rect = RECT::default();
            let _ = GetWindowRect(hwnd, &mut rect);
            let width = rect.right - rect.left;
            let height = rect.bottom - rect.top;
            let screen_width = GetSystemMetrics(SM_CXSCREEN);
            let screen_height = GetSystemMetrics(SM_CYSCREEN);
            let x = (screen_width - width) / 2;
            let y = (screen_height - height) / 2;
            let _ = SetWindowPos(hwnd, None, x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER);

            super::SETTINGS_WINDOW.store(hwnd.0 as isize, Ordering::Release);

            let platform = WindowsHostPlatform::new();
            let window_id = to_window_id(hwnd);
            let _ = platform.show_window(window_id);
            let _ = platform.update_window(window_id);

            // Modal loop: process messages until window destroyed.
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);

                if !IsWindow(Some(hwnd)).as_bool() {
                    break;
                }
            }

            Ok(super::SETTINGS_SAVED.swap(false, Ordering::AcqRel))
        }
    }

    pub(super) fn create_controls(&mut self) {
        unsafe {
            let instance: HINSTANCE = GetModuleHandleW(None).unwrap_or_default().into();

            self.font = HFONT(GetStockObject(DEFAULT_GUI_FONT).0);

            let mut client_rect = RECT::default();
            let _ = GetClientRect(self.hwnd, &mut client_rect);
            let window_width = client_rect.right - client_rect.left;
            let window_height = client_rect.bottom - client_rect.top;

            let tabs_height = window_height - BUTTON_HEIGHT - MARGIN * 3;
            let tabs = TabsContainer::builder()
                .position(MARGIN, MARGIN)
                .size(window_width - MARGIN * 2, tabs_height)
                .parent(self.hwnd)
                .build()
                .unwrap();
            tabs.set_font(&Font { handle: self.font });
            self.tabs_container = tabs.handle;

            let tab_drawing = Tab::builder()
                .text("绘图设置")
                .parent(self.hwnd)
                .build(&tabs)
                .unwrap();
            self.tab_drawing = tab_drawing.handle;

            let tab_system = Tab::builder()
                .text("系统设置")
                .parent(self.hwnd)
                .build(&tabs)
                .unwrap();
            self.tab_system = tab_system.handle;

            // Drawing tab.
            self.line_thickness_label = self.create_label("线条粗细:", self.tab_drawing, instance);

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

            self.font_label = self.create_label("字体设置:", self.tab_drawing, instance);

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

            self.drawing_color_label = self.create_label("绘图颜色:", self.tab_drawing, instance);

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

            // System tab.
            self.hotkey_label = self.create_label("截图热键:", self.tab_system, instance);

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

            #[allow(clippy::fn_to_numeric_cast)]
            let original_proc = SetWindowLongPtrW(
                self.hotkey_edit,
                GWLP_WNDPROC,
                Self::hotkey_edit_proc as isize,
            );
            SetWindowLongPtrW(self.hotkey_edit, GWLP_USERDATA, original_proc);

            self.config_path_label = self.create_label("保存路径:", self.tab_system, instance);

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

            self.ocr_language_label = self.create_label("OCR语言", self.tab_system, instance);

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

            self.load_ocr_languages();

            // Bottom buttons.
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

            self.layout_controls();
        }
    }

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

    fn load_ocr_languages(&self) {
        unsafe {
            let available_languages = get_available_languages(std::path::Path::new("models"));

            if available_languages.is_empty() {
                let text = to_wide_chars("未找到 OCR 模型");
                SendMessageW(
                    self.ocr_language_combo,
                    0x0143, // CB_ADDSTRING
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
                        0x0143, // CB_ADDSTRING
                        Some(WPARAM(0)),
                        Some(LPARAM(text.as_ptr() as isize)),
                    );

                    let value_text = to_wide_chars(&lang.id);
                    let value_box = Box::new(value_text);
                    let value_ptr = Box::into_raw(value_box);
                    SendMessageW(
                        self.ocr_language_combo,
                        0x0151, // CB_SETITEMDATA
                        Some(WPARAM(index.0 as usize)),
                        Some(LPARAM(value_ptr as isize)),
                    );
                }
            }
        }
    }

    pub(super) fn handle_tab_change(&self) {
        unsafe {
            if self.tabs_container.is_invalid() {
                return;
            }

            let index = SendMessageW(
                self.tabs_container,
                TCM_GETCURSEL,
                Some(WPARAM(0)),
                Some(LPARAM(0)),
            )
            .0 as i32;

            if !self.tab_drawing.is_invalid() {
                let _ = ShowWindow(self.tab_drawing, if index == 0 { SW_SHOW } else { SW_HIDE });
            }
            if !self.tab_system.is_invalid() {
                let _ = ShowWindow(self.tab_system, if index == 1 { SW_SHOW } else { SW_HIDE });
            }
        }
    }

    pub(super) fn load_values(&mut self) {
        unsafe {
            let thickness_text = to_wide_chars(&self.settings.line_thickness.to_string());
            let _ = SetWindowTextW(self.line_thickness_edit, PCWSTR(thickness_text.as_ptr()));

            let hotkey_text = to_wide_chars(&self.settings.get_hotkey_string());
            let _ = SetWindowTextW(self.hotkey_edit, PCWSTR(hotkey_text.as_ptr()));

            let config_path_text = to_wide_chars(&self.settings.config_path);
            let _ = SetWindowTextW(self.config_path_edit, PCWSTR(config_path_text.as_ptr()));

            // OCR language.
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

            self.update_color_brushes();
            self.update_color_preview();
        }
    }

    pub(super) fn handle_edit_change(&mut self, _control_id: i32) {}

    pub(super) fn handle_command(&mut self, command_id: i32) {
        match command_id {
            ID_OK => {
                self.save_settings();

                if let Err(e) = self.settings.save() {
                    self.show_error(&format!("保存设置失败: {e}"));
                    return;
                }

                super::SETTINGS_SAVED.store(true, Ordering::Release);
                let _ = WindowsHostPlatform::new().request_close(to_window_id(self.hwnd));
            }

            ID_CANCEL => {
                let _ = WindowsHostPlatform::new().request_close(to_window_id(self.hwnd));
            }

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

    fn show_error(&self, message: &str) {
        WindowsHostPlatform::new().show_error_message(to_window_id(self.hwnd), "设置", message);
    }

    fn show_font_dialog(&mut self) {
        if let Some(selection) = file_dialog::show_font_dialog(
            self.hwnd,
            self.settings.font_size,
            self.settings.font_weight,
            self.settings.font_italic,
            self.settings.font_underline,
            self.settings.font_strikeout,
            &self.settings.font_name,
            self.settings.font_color,
        ) {
            self.settings.font_size = selection.font_size;
            self.settings.font_weight = selection.font_weight;
            self.settings.font_italic = selection.font_italic;
            self.settings.font_underline = selection.font_underline;
            self.settings.font_strikeout = selection.font_strikeout;
            self.settings.font_name = selection.font_name;
            self.settings.font_color = selection.font_color;
            self.load_values();
        }
    }

    fn show_drawing_color_dialog(&mut self) {
        let initial = (
            self.settings.drawing_color_red,
            self.settings.drawing_color_green,
            self.settings.drawing_color_blue,
        );

        if let Some((r, g, b)) = file_dialog::show_color_dialog(self.hwnd, initial) {
            self.settings.drawing_color_red = r;
            self.settings.drawing_color_green = g;
            self.settings.drawing_color_blue = b;
            self.update_color_brushes();
            self.update_color_preview();
        }
    }

    fn update_color_brushes(&mut self) {
        unsafe {
            if !self.drawing_color_brush.0.is_null() {
                let _ = DeleteObject(self.drawing_color_brush.into());
            }
            if !self.text_color_brush.0.is_null() {
                let _ = DeleteObject(self.text_color_brush.into());
            }

            let drawing_color = (self.settings.drawing_color_red as u32)
                | ((self.settings.drawing_color_green as u32) << 8)
                | ((self.settings.drawing_color_blue as u32) << 16);
            self.drawing_color_brush = CreateSolidBrush(COLORREF(drawing_color));

            let text_color = (self.settings.text_color_red as u32)
                | ((self.settings.text_color_green as u32) << 8)
                | ((self.settings.text_color_blue as u32) << 16);
            self.text_color_brush = CreateSolidBrush(COLORREF(text_color));
        }
    }

    fn update_color_preview(&self) {
        let platform = WindowsHostPlatform::new();
        let _ = platform.request_redraw_erase(to_window_id(self.drawing_color_preview));
        let _ = platform.request_redraw_erase(to_window_id(self.text_color_preview));
    }

    fn save_settings(&mut self) {
        unsafe {
            let mut buffer = [0u16; 32];

            if GetWindowTextW(self.line_thickness_edit, &mut buffer) > 0 {
                let text = String::from_utf16_lossy(&buffer);
                let text = text.trim_end_matches('\0');
                if let Ok(value) = text.parse::<f32>() {
                    self.settings.line_thickness = value.clamp(1.0, 20.0);
                }
            }

            let mut hotkey_buffer = [0u16; 64];
            if GetWindowTextW(self.hotkey_edit, &mut hotkey_buffer) > 0 {
                let hotkey_text = String::from_utf16_lossy(&hotkey_buffer);
                let hotkey_text = hotkey_text.trim_end_matches('\0');
                let _ = self.settings.parse_hotkey_string(hotkey_text);
            }

            let mut config_path_buffer = [0u16; 260];
            if GetWindowTextW(self.config_path_edit, &mut config_path_buffer) > 0 {
                let config_path_text = String::from_utf16_lossy(&config_path_buffer);
                let config_path_text = config_path_text.trim_end_matches('\0');
                if !config_path_text.is_empty() {
                    self.settings.config_path = config_path_text.to_string();
                }
            }

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

    fn show_folder_browser_dialog(&mut self) {
        match file_dialog::show_folder_picker_dialog(self.hwnd, "选择配置文件保存路径") {
            file_dialog::FolderPickerOutcome::Selected(path_str) => unsafe {
                let path_wide = to_wide_chars(&path_str);
                let _ = SetWindowTextW(self.config_path_edit, PCWSTR(path_wide.as_ptr()));
            },
            file_dialog::FolderPickerOutcome::NotSelected => {}
            file_dialog::FolderPickerOutcome::Unavailable => {
                self.show_simple_path_input();
            }
        }
    }

    fn show_simple_path_input(&mut self) {
        let current_path = unsafe {
            let mut buffer = [0u16; 260];
            GetWindowTextW(self.config_path_edit, &mut buffer);
            String::from_utf16_lossy(&buffer)
        };
        let current_path = current_path.trim_end_matches('\0');

        let message = format!("当前路径: {current_path}\n\n请手动在输入框中修改路径");
        WindowsHostPlatform::new().show_info_message(to_window_id(self.hwnd), "配置路径", &message);
    }

    pub(super) fn cleanup(&mut self) {
        unsafe {
            // Free combo item data.
            let item_count = SendMessageW(self.ocr_language_combo, 0x0146, None, None).0; // CB_GETCOUNT
            if item_count > 0 {
                for i in 0..item_count {
                    let data_ptr = SendMessageW(
                        self.ocr_language_combo,
                        0x0150, // CB_GETITEMDATA
                        Some(WPARAM(i as usize)),
                        None,
                    );
                    if data_ptr.0 != 0 {
                        let value_ptr = data_ptr.0 as *mut Vec<u16>;
                        if !value_ptr.is_null() {
                            let _ = Box::from_raw(value_ptr);
                        }
                        // Best-effort: clear item data.
                        let _ = SendMessageW(
                            self.ocr_language_combo,
                            0x0151, // CB_SETITEMDATA
                            Some(WPARAM(i as usize)),
                            Some(LPARAM(0)),
                        );
                    }
                }
            }

            // Free stored original text prop if still present.
            let prop_name = to_wide_chars("OriginalText");
            let text_handle = GetPropW(self.hotkey_edit, PCWSTR(prop_name.as_ptr()));
            if !text_handle.is_invalid() {
                let text_ptr = text_handle.0 as *mut Vec<u16>;
                if !text_ptr.is_null() {
                    let _ = Box::from_raw(text_ptr);
                }
                let _ = RemovePropW(self.hotkey_edit, PCWSTR(prop_name.as_ptr()));
            }

            // Free brushes.
            if !self.drawing_color_brush.0.is_null() {
                let _ = DeleteObject(self.drawing_color_brush.into());
                self.drawing_color_brush = HBRUSH::default();
            }
            if !self.text_color_brush.0.is_null() {
                let _ = DeleteObject(self.text_color_brush.into());
                self.text_color_brush = HBRUSH::default();
            }
        }
    }
}
