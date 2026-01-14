use std::sync::atomic::Ordering;

use sc_platform::HostPlatform;
use sc_platform_windows::windows::{WindowsHostPlatform, window_id as to_window_id};
use sc_settings::Settings;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::Controls::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use super::window::SettingsWindowState;

impl SettingsWindowState {
    pub(super) unsafe extern "system" fn window_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        unsafe {
            match msg {
                WM_CREATE => {
                    let settings = Settings::load();
                    let mut window = SettingsWindowState::new(hwnd, settings);
                    window.create_controls();
                    window.load_values();

                    let window_box = Box::new(window);
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(window_box) as isize);

                    return LRESULT(0);
                }

                WM_CLOSE => {
                    let window_ptr =
                        GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SettingsWindowState;
                    if !window_ptr.is_null() {
                        let mut window = Box::from_raw(window_ptr);
                        window.cleanup();
                        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                    }

                    super::SETTINGS_WINDOW.store(0, Ordering::Release);
                    let _ = DestroyWindow(hwnd);
                    return LRESULT(0);
                }

                WM_DESTROY => {
                    super::SETTINGS_WINDOW.store(0, Ordering::Release);
                    return LRESULT(0);
                }

                _ => {}
            }

            let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SettingsWindowState;
            if window_ptr.is_null() {
                return DefWindowProcW(hwnd, msg, wparam, lparam);
            }

            let window = &mut *window_ptr;
            if let Some(result) = window.handle_msg(msg, wparam, lparam) {
                return result;
            }

            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
    }

    unsafe fn handle_msg(&mut self, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
        match msg {
            WM_NOTIFY => unsafe {
                let nmhdr = &*(lparam.0 as *const NMHDR);
                // TCN_SELCHANGE = TCN_FIRST - 1
                if nmhdr.code == 0xFFFF_FDD9_u32 {
                    self.handle_tab_change();
                }
                Some(LRESULT(0))
            },

            WM_COMMAND => {
                let command_id = (wparam.0 & 0xFFFF) as i32;
                let notification = ((wparam.0 >> 16) & 0xFFFF) as i32;

                // EN_CHANGE
                if notification == 0x0300 {
                    self.handle_edit_change(command_id);
                } else {
                    self.handle_command(command_id);
                }

                Some(LRESULT(0))
            }

            WM_CTLCOLORSTATIC => unsafe {
                let hdc = HDC(wparam.0 as *mut _);
                let control_hwnd = HWND(lparam.0 as *mut _);

                if control_hwnd == self.drawing_color_preview
                    && !self.drawing_color_brush.0.is_null()
                {
                    let color = (self.settings.drawing_color_red as u32)
                        | ((self.settings.drawing_color_green as u32) << 8)
                        | ((self.settings.drawing_color_blue as u32) << 16);
                    SetBkColor(hdc, COLORREF(color));
                    return Some(LRESULT(self.drawing_color_brush.0 as isize));
                }

                if control_hwnd == self.text_color_preview && !self.text_color_brush.0.is_null() {
                    let color = (self.settings.text_color_red as u32)
                        | ((self.settings.text_color_green as u32) << 8)
                        | ((self.settings.text_color_blue as u32) << 16);
                    SetBkColor(hdc, COLORREF(color));
                    return Some(LRESULT(self.text_color_brush.0 as isize));
                }

                SetBkMode(hdc, TRANSPARENT);
                SetTextColor(hdc, COLORREF(0x000000));
                Some(LRESULT(GetStockObject(WHITE_BRUSH).0 as isize))
            },

            WM_CTLCOLOREDIT => unsafe {
                let hdc = HDC(wparam.0 as *mut _);
                SetBkColor(hdc, COLORREF(0xFFFFFF));
                SetTextColor(hdc, COLORREF(0x000000));
                SetBkMode(hdc, OPAQUE);
                Some(LRESULT(GetStockObject(WHITE_BRUSH).0 as isize))
            },

            WM_CTLCOLORBTN => unsafe {
                let hdc = HDC(wparam.0 as *mut _);
                SetBkMode(hdc, TRANSPARENT);
                Some(LRESULT(GetStockObject(NULL_BRUSH).0 as isize))
            },

            WM_ERASEBKGND => unsafe {
                let hdc = HDC(wparam.0 as *mut _);
                let mut rect = RECT::default();
                let _ = GetClientRect(self.hwnd, &mut rect);
                let bg_brush = GetSysColorBrush(COLOR_BTNFACE);
                FillRect(hdc, &rect, bg_brush);
                Some(LRESULT(1))
            },

            WM_PAINT => unsafe {
                let platform = WindowsHostPlatform::new();
                let _ = platform.request_redraw_erase(to_window_id(self.hotkey_edit));
                let _ = platform.update_window(to_window_id(self.hotkey_edit));

                Some(DefWindowProcW(self.hwnd, msg, wparam, lparam))
            },

            _ => None,
        }
    }
}
