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
use windows::core::{Error, HRESULT, PCWSTR};

use super::{
    BUTTON_HEIGHT, ID_CANCEL, ID_CONFIG_PATH_BROWSE, ID_CONFIG_PATH_EDIT, ID_DRAWING_COLOR_BUTTON,
    ID_FONT_CHOOSE_BUTTON, ID_HOTKEY_EDIT, ID_LINE_THICKNESS, ID_OCR_LANGUAGE_COMBO, ID_OK, MARGIN,
    WINDOW_DEFAULT_HEIGHT, WINDOW_DEFAULT_WIDTH,
};

/// Settings window.
pub struct SettingsWindow;

#[derive(Debug, Default)]
pub(super) struct OwnedBrush {
    handle: HBRUSH,
}

impl OwnedBrush {
    pub(super) fn handle(&self) -> HBRUSH {
        self.handle
    }

    pub(super) fn is_valid(&self) -> bool {
        !self.handle.0.is_null()
    }

    pub(super) fn reset(&mut self, handle: HBRUSH) {
        self.clear();
        self.handle = handle;
    }

    pub(super) fn clear(&mut self) {
        if self.is_valid() {
            unsafe {
                let _ = DeleteObject(self.handle.into());
            }
            self.handle = HBRUSH::default();
        }
    }
}

impl Drop for OwnedBrush {
    fn drop(&mut self) {
        self.clear();
    }
}

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
    pub(super) drawing_color_brush: OwnedBrush,
    pub(super) text_color_brush: OwnedBrush,
}

const LINE_THICKNESS_TEXT_LIMIT: usize = 31;
const HOTKEY_TEXT_LIMIT: usize = 63;
const CONFIG_PATH_TEXT_LIMIT: usize = 259;
const CONFIG_PATH_TEXT_BUFFER: usize = CONFIG_PATH_TEXT_LIMIT + 1;

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
            drawing_color_brush: OwnedBrush::default(),
            text_color_brush: OwnedBrush::default(),
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
                if IsDialogMessageW(hwnd, &msg).as_bool() {
                    if !IsWindow(Some(hwnd)).as_bool() {
                        break;
                    }
                    continue;
                }

                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);

                if !IsWindow(Some(hwnd)).as_bool() {
                    break;
                }
            }

            Ok(super::SETTINGS_SAVED.swap(false, Ordering::AcqRel))
        }
    }

    pub(super) fn create_controls(&mut self) -> windows::core::Result<()> {
        unsafe {
            let instance: HINSTANCE = GetModuleHandleW(None)?.into();

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
                .build()?;
            tabs.set_font(&Font { handle: self.font });
            self.tabs_container = tabs.handle;

            let tab_drawing = Tab::builder()
                .text("绘图设置")
                .parent(self.hwnd)
                .build(&tabs)?;
            self.tab_drawing = tab_drawing.handle;

            let tab_system = Tab::builder()
                .text("系统设置")
                .parent(self.hwnd)
                .build(&tabs)?;
            self.tab_system = tab_system.handle;

            // Drawing tab.
            self.line_thickness_label =
                self.create_label("线条粗细:", self.tab_drawing, instance)?;

            self.line_thickness_edit =
                self.create_edit(self.tab_drawing, ID_LINE_THICKNESS, instance)?;
            self.set_edit_text_limit(self.line_thickness_edit, LINE_THICKNESS_TEXT_LIMIT);

            self.font_label = self.create_label("字体设置:", self.tab_drawing, instance)?;

            self.font_choose_button = self.create_button(
                "选择字体...",
                self.tab_drawing,
                ID_FONT_CHOOSE_BUTTON,
                instance,
            )?;

            self.drawing_color_label =
                self.create_label("绘图颜色:", self.tab_drawing, instance)?;

            self.drawing_color_preview = self.create_static_preview(self.tab_drawing, instance)?;

            self.drawing_color_button = self.create_button(
                "选择颜色...",
                self.tab_drawing,
                ID_DRAWING_COLOR_BUTTON,
                instance,
            )?;

            // System tab.
            self.hotkey_label = self.create_label("截图热键:", self.tab_system, instance)?;

            self.hotkey_edit = self.create_edit(self.tab_system, ID_HOTKEY_EDIT, instance)?;
            self.set_edit_text_limit(self.hotkey_edit, HOTKEY_TEXT_LIMIT);
            Self::set_modern_theme(self.hotkey_edit);

            Self::subclass_hotkey_edit(self.hotkey_edit)?;

            self.config_path_label = self.create_label("保存路径:", self.tab_system, instance)?;

            self.config_path_edit =
                self.create_edit(self.tab_system, ID_CONFIG_PATH_EDIT, instance)?;
            self.set_edit_text_limit(self.config_path_edit, CONFIG_PATH_TEXT_LIMIT);
            Self::set_modern_theme(self.config_path_edit);

            self.config_path_browse_button =
                self.create_button("浏览...", self.tab_system, ID_CONFIG_PATH_BROWSE, instance)?;

            self.ocr_language_label = self.create_label("OCR语言", self.tab_system, instance)?;

            self.ocr_language_combo = self.create_child_control(
                "COMBOBOX",
                None,
                WS_EX_CLIENTEDGE,
                WINDOW_STYLE(WS_VISIBLE.0 | WS_CHILD.0 | WS_TABSTOP.0 | 0x0003),
                self.tab_system,
                Some(ID_OCR_LANGUAGE_COMBO),
                instance,
                true,
            )?;
            Self::set_modern_theme(self.ocr_language_combo);

            self.load_ocr_languages();

            // Bottom buttons.
            self.ok_button = self.create_button("确定", self.hwnd, ID_OK, instance)?;
            self.cancel_button = self.create_button("取消", self.hwnd, ID_CANCEL, instance)?;

            self.layout_controls();
            Ok(())
        }
    }

    fn create_edit(
        &self,
        parent: HWND,
        control_id: i32,
        instance: HINSTANCE,
    ) -> windows::core::Result<HWND> {
        self.create_child_control(
            "EDIT",
            None,
            WS_EX_CLIENTEDGE,
            WS_VISIBLE | WS_CHILD | WS_TABSTOP,
            parent,
            Some(control_id),
            instance,
            true,
        )
    }

    fn create_button(
        &self,
        text: &str,
        parent: HWND,
        control_id: i32,
        instance: HINSTANCE,
    ) -> windows::core::Result<HWND> {
        self.create_child_control(
            "BUTTON",
            Some(text),
            WINDOW_EX_STYLE::default(),
            WS_VISIBLE | WS_CHILD | WS_TABSTOP,
            parent,
            Some(control_id),
            instance,
            true,
        )
    }

    fn create_static_preview(
        &self,
        parent: HWND,
        instance: HINSTANCE,
    ) -> windows::core::Result<HWND> {
        self.create_child_control(
            "STATIC",
            None,
            WS_EX_CLIENTEDGE,
            WS_VISIBLE | WS_CHILD,
            parent,
            None,
            instance,
            false,
        )
    }

    fn create_label(
        &self,
        text: &str,
        parent: HWND,
        instance: HINSTANCE,
    ) -> windows::core::Result<HWND> {
        self.create_child_control(
            "STATIC",
            Some(text),
            WINDOW_EX_STYLE::default(),
            WS_VISIBLE | WS_CHILD,
            parent,
            None,
            instance,
            true,
        )
    }

    fn create_child_control(
        &self,
        class_name: &str,
        text: Option<&str>,
        ex_style: WINDOW_EX_STYLE,
        style: WINDOW_STYLE,
        parent: HWND,
        control_id: Option<i32>,
        instance: HINSTANCE,
        apply_font: bool,
    ) -> windows::core::Result<HWND> {
        unsafe {
            let class_name = to_wide_chars(class_name);
            let text = text.map(to_wide_chars);
            let text_ptr = text
                .as_ref()
                .map_or(PCWSTR::null(), |value| PCWSTR(value.as_ptr()));
            let menu = control_id.map(|id| HMENU(id as *mut _));

            let hwnd = CreateWindowExW(
                ex_style,
                PCWSTR(class_name.as_ptr()),
                text_ptr,
                style,
                0,
                0,
                0,
                0,
                Some(parent),
                menu,
                Some(instance),
                None,
            )?;

            if apply_font {
                self.set_control_font(hwnd);
            }

            Ok(hwnd)
        }
    }

    fn set_control_font(&self, hwnd: HWND) {
        unsafe {
            SendMessageW(hwnd, WM_SETFONT, Some(WPARAM(self.font.0 as usize)), None);
        }
    }

    fn set_edit_text_limit(&self, hwnd: HWND, max_chars: usize) {
        unsafe {
            SendMessageW(hwnd, EM_LIMITTEXT, Some(WPARAM(max_chars)), None);
        }
    }

    fn read_window_text<const N: usize>(hwnd: HWND) -> Option<String> {
        unsafe {
            let mut buffer = [0u16; N];
            if GetWindowTextW(hwnd, &mut buffer) <= 0 {
                return None;
            }

            Some(
                String::from_utf16_lossy(&buffer)
                    .trim_end_matches('\0')
                    .to_string(),
            )
        }
    }

    fn add_ocr_language_item(&self, display: &str, language_id: Option<&str>) {
        unsafe {
            let text = to_wide_chars(display);
            let index = SendMessageW(
                self.ocr_language_combo,
                CB_ADDSTRING,
                Some(WPARAM(0)),
                Some(LPARAM(text.as_ptr() as isize)),
            )
            .0;

            if index == CB_ERR as isize || index == CB_ERRSPACE as isize {
                return;
            }

            if let Some(language_id) = language_id {
                let value_ptr = Box::into_raw(Box::new(to_wide_chars(language_id)));
                let result = SendMessageW(
                    self.ocr_language_combo,
                    CB_SETITEMDATA,
                    Some(WPARAM(index as usize)),
                    Some(LPARAM(value_ptr as isize)),
                );
                if result.0 == CB_ERR as isize {
                    let _ = Box::from_raw(value_ptr);
                }
            }
        }
    }

    fn ocr_language_item_count(&self) -> usize {
        unsafe {
            let count = SendMessageW(self.ocr_language_combo, CB_GETCOUNT, None, None).0;
            if count <= 0 || count == CB_ERR as isize {
                0
            } else {
                count as usize
            }
        }
    }

    fn ocr_language_value_at(&self, index: usize) -> Option<String> {
        unsafe {
            let data = SendMessageW(
                self.ocr_language_combo,
                CB_GETITEMDATA,
                Some(WPARAM(index)),
                None,
            )
            .0;
            if data == 0 || data == CB_ERR as isize {
                return None;
            }

            let value_ptr = data as *const Vec<u16>;
            if value_ptr.is_null() {
                return None;
            }

            Some(
                String::from_utf16_lossy(&*value_ptr)
                    .trim_end_matches('\0')
                    .to_string(),
            )
        }
    }

    fn clear_ocr_language_items(&self) {
        unsafe {
            for i in 0..self.ocr_language_item_count() {
                let data = SendMessageW(
                    self.ocr_language_combo,
                    CB_GETITEMDATA,
                    Some(WPARAM(i)),
                    None,
                )
                .0;
                if data == 0 || data == CB_ERR as isize {
                    continue;
                }

                let value_ptr = data as *mut Vec<u16>;
                let _ = SendMessageW(
                    self.ocr_language_combo,
                    CB_SETITEMDATA,
                    Some(WPARAM(i)),
                    Some(LPARAM(0)),
                );

                if !value_ptr.is_null() {
                    let _ = Box::from_raw(value_ptr);
                }
            }
        }
    }

    fn load_ocr_languages(&self) {
        let available_languages =
            get_available_languages(std::path::Path::new(sc_ocr::DEFAULT_MODELS_DIR));

        if available_languages.is_empty() {
            self.add_ocr_language_item("未找到 OCR 模型", None);
        } else {
            for (i, lang) in available_languages.iter().enumerate() {
                let display = if i == 0 {
                    format!("{} (默认)", lang.display_name)
                } else {
                    lang.display_name.clone()
                };

                self.add_ocr_language_item(&display, Some(&lang.id));
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
            for i in 0..self.ocr_language_item_count() {
                if let Some(value) = self.ocr_language_value_at(i) {
                    if value == self.settings.ocr_language {
                        SendMessageW(self.ocr_language_combo, CB_SETCURSEL, Some(WPARAM(i)), None);
                        break;
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
            let drawing_color = (self.settings.drawing_color_red as u32)
                | ((self.settings.drawing_color_green as u32) << 8)
                | ((self.settings.drawing_color_blue as u32) << 16);
            self.drawing_color_brush
                .reset(CreateSolidBrush(COLORREF(drawing_color)));

            let text_color = (self.settings.text_color_red as u32)
                | ((self.settings.text_color_green as u32) << 8)
                | ((self.settings.text_color_blue as u32) << 16);
            self.text_color_brush
                .reset(CreateSolidBrush(COLORREF(text_color)));
        }
    }

    fn update_color_preview(&self) {
        let platform = WindowsHostPlatform::new();
        let _ = platform.request_redraw_erase(to_window_id(self.drawing_color_preview));
        let _ = platform.request_redraw_erase(to_window_id(self.text_color_preview));
    }

    fn save_settings(&mut self) {
        unsafe {
            if let Some(text) = Self::read_window_text::<{ LINE_THICKNESS_TEXT_LIMIT + 1 }>(
                self.line_thickness_edit,
            ) {
                if let Ok(value) = text.parse::<f32>() {
                    self.settings.line_thickness = value.clamp(1.0, 20.0);
                }
            }

            if let Some(hotkey_text) =
                Self::read_window_text::<{ HOTKEY_TEXT_LIMIT + 1 }>(self.hotkey_edit)
            {
                let _ = self.settings.parse_hotkey_string(&hotkey_text);
            }

            if let Some(config_path_text) =
                Self::read_window_text::<CONFIG_PATH_TEXT_BUFFER>(self.config_path_edit)
            {
                if !config_path_text.is_empty() {
                    self.settings.config_path = config_path_text;
                }
            }

            let selected_index = SendMessageW(self.ocr_language_combo, CB_GETCURSEL, None, None).0;
            if selected_index != CB_ERR as isize {
                if let Some(value) = self.ocr_language_value_at(selected_index as usize) {
                    self.settings.ocr_language = value;
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
        let current_path = Self::read_window_text::<CONFIG_PATH_TEXT_BUFFER>(self.config_path_edit)
            .unwrap_or_default();

        let message = format!("当前路径: {current_path}\n\n请手动在输入框中修改路径");
        WindowsHostPlatform::new().show_info_message(to_window_id(self.hwnd), "配置路径", &message);
    }

    pub(super) fn cleanup(&mut self) {
        unsafe {
            self.clear_ocr_language_items();
            Self::unsubclass_hotkey_edit(self.hotkey_edit);
            self.drawing_color_brush.clear();
            self.text_color_brush.clear();
        }
    }

    pub(super) fn win32_error(message: &str) -> Error {
        Error::new(HRESULT(-1), message)
    }
}
