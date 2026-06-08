use std::ffi::c_void;

use sc_platform_windows::win_api::to_wide_chars;
use windows::Win32::Foundation::*;
use windows::Win32::UI::Controls::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::PCWSTR;

use super::window::SettingsWindowState;

const ORIGINAL_TEXT_PROP: &str = "SC_HotkeyOriginalText";
const ORIGINAL_PROC_PROP: &str = "SC_HotkeyOriginalProc";
const HOTKEY_PLACEHOLDER: &str = "按下快捷键";

impl SettingsWindowState {
    pub(super) unsafe fn set_modern_theme(hwnd: HWND) {
        let theme_name = to_wide_chars("Explorer");
        let _ = unsafe { SetWindowTheme(hwnd, PCWSTR(theme_name.as_ptr()), PCWSTR::null()) };
    }

    pub(super) unsafe fn subclass_hotkey_edit(hwnd: HWND) -> windows::core::Result<()> {
        unsafe {
            let original_proc = SetWindowLongPtrW(
                hwnd,
                GWLP_WNDPROC,
                Self::hotkey_edit_proc as *const () as isize,
            );

            if original_proc == 0 {
                return Err(Self::win32_error("Failed to subclass hotkey edit"));
            }

            let prop_name = to_wide_chars(ORIGINAL_PROC_PROP);
            if let Err(e) = SetPropW(
                hwnd,
                PCWSTR(prop_name.as_ptr()),
                Some(HANDLE(original_proc as *mut c_void)),
            ) {
                let _ = SetWindowLongPtrW(hwnd, GWLP_WNDPROC, original_proc);
                return Err(e);
            }

            Ok(())
        }
    }

    pub(super) unsafe fn unsubclass_hotkey_edit(hwnd: HWND) {
        unsafe {
            if hwnd.is_invalid() {
                return;
            }

            let prop_name = to_wide_chars(ORIGINAL_PROC_PROP);
            if let Ok(proc_handle) = RemovePropW(hwnd, PCWSTR(prop_name.as_ptr()))
                && !proc_handle.is_invalid()
            {
                let _ = SetWindowLongPtrW(hwnd, GWLP_WNDPROC, proc_handle.0 as isize);
            }

            Self::clear_original_hotkey_text(hwnd);
        }
    }

    pub(super) unsafe fn clear_original_hotkey_text(hwnd: HWND) {
        unsafe {
            let _ = Self::take_original_hotkey_text(hwnd);
        }
    }

    unsafe fn take_original_hotkey_text(hwnd: HWND) -> Option<Vec<u16>> {
        unsafe {
            let prop_name = to_wide_chars(ORIGINAL_TEXT_PROP);
            let text_handle = RemovePropW(hwnd, PCWSTR(prop_name.as_ptr())).ok()?;
            if text_handle.is_invalid() {
                return None;
            }

            let text_ptr = text_handle.0 as *mut Vec<u16>;
            if text_ptr.is_null() {
                return None;
            }

            Some(*Box::from_raw(text_ptr))
        }
    }

    unsafe fn call_original_hotkey_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        unsafe {
            let prop_name = to_wide_chars(ORIGINAL_PROC_PROP);
            let proc_handle = GetPropW(hwnd, PCWSTR(prop_name.as_ptr()));
            if proc_handle.is_invalid() {
                return DefWindowProcW(hwnd, msg, wparam, lparam);
            }

            let wndproc: WNDPROC = std::mem::transmute(proc_handle.0);
            CallWindowProcW(wndproc, hwnd, msg, wparam, lparam)
        }
    }

    pub(super) unsafe extern "system" fn hotkey_edit_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        unsafe {
            match msg {
                WM_LBUTTONDOWN => {
                    let mut buffer = [0u16; 64];
                    let len = GetWindowTextW(hwnd, &mut buffer);
                    if len > 0 {
                        Self::clear_original_hotkey_text(hwnd);

                        let current_text = String::from_utf16_lossy(&buffer[..len as usize]);
                        let text_wide = to_wide_chars(&current_text);
                        let text_box = Box::new(text_wide);
                        let text_ptr = Box::into_raw(text_box);
                        let prop_name = to_wide_chars(ORIGINAL_TEXT_PROP);
                        if SetPropW(
                            hwnd,
                            PCWSTR(prop_name.as_ptr()),
                            Some(HANDLE(text_ptr as *mut c_void)),
                        )
                        .is_err()
                        {
                            let _ = Box::from_raw(text_ptr);
                        }
                    }

                    let placeholder_text = to_wide_chars(HOTKEY_PLACEHOLDER);
                    let _ = SetWindowTextW(hwnd, PCWSTR(placeholder_text.as_ptr()));
                    let _ = SetFocus(Some(hwnd));
                    return LRESULT(0);
                }

                WM_KILLFOCUS => {
                    let mut buffer = [0u16; 64];
                    let len = GetWindowTextW(hwnd, &mut buffer);
                    let current_text = if len > 0 {
                        String::from_utf16_lossy(&buffer[..len as usize])
                    } else {
                        String::new()
                    };

                    if current_text.trim() == HOTKEY_PLACEHOLDER || current_text.trim().is_empty() {
                        if let Some(text_box) = Self::take_original_hotkey_text(hwnd) {
                            let _ = SetWindowTextW(hwnd, PCWSTR(text_box.as_ptr()));
                        }
                    } else {
                        Self::clear_original_hotkey_text(hwnd);
                    }

                    return Self::call_original_hotkey_proc(hwnd, msg, wparam, lparam);
                }

                WM_KEYDOWN | WM_SYSKEYDOWN => {
                    let mut modifiers = 0u32;
                    if GetKeyState(VK_CONTROL.0 as i32) < 0 {
                        modifiers |= MOD_CONTROL.0;
                    }
                    if GetKeyState(VK_MENU.0 as i32) < 0 {
                        modifiers |= MOD_ALT.0;
                    }
                    if GetKeyState(VK_SHIFT.0 as i32) < 0 {
                        modifiers |= MOD_SHIFT.0;
                    }

                    let key = wparam.0 as u32;

                    if ((b'A' as u32..=b'Z' as u32).contains(&key)
                        || (b'0' as u32..=b'9' as u32).contains(&key))
                        && modifiers != 0
                    {
                        let mut parts = Vec::new();
                        if modifiers & MOD_CONTROL.0 != 0 {
                            parts.push("Ctrl".to_string());
                        }
                        if modifiers & MOD_ALT.0 != 0 {
                            parts.push("Alt".to_string());
                        }
                        if modifiers & MOD_SHIFT.0 != 0 {
                            parts.push("Shift".to_string());
                        }
                        let key_char = char::from_u32(key).unwrap_or('?');
                        parts.push(key_char.to_string());

                        let hotkey_string = parts.join("+");
                        let hotkey_wide = to_wide_chars(&hotkey_string);
                        let _ = SetWindowTextW(hwnd, PCWSTR(hotkey_wide.as_ptr()));
                    }

                    return LRESULT(0);
                }

                WM_CHAR => return LRESULT(0),

                _ => {}
            }

            Self::call_original_hotkey_proc(hwnd, msg, wparam, lparam)
        }
    }
}
