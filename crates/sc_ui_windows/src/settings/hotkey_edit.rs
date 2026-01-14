use std::ffi::c_void;

use sc_platform_windows::win_api::to_wide_chars;
use windows::Win32::Foundation::*;
use windows::Win32::UI::Controls::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::PCWSTR;

use super::window::SettingsWindowState;

impl SettingsWindowState {
    pub(super) unsafe fn set_modern_theme(hwnd: HWND) {
        let theme_name = to_wide_chars("Explorer");
        let _ = unsafe { SetWindowTheme(hwnd, PCWSTR(theme_name.as_ptr()), PCWSTR::null()) };
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
                    // Save current text.
                    let mut buffer = [0u16; 64];
                    let len = GetWindowTextW(hwnd, &mut buffer);
                    if len > 0 {
                        let current_text = String::from_utf16_lossy(&buffer[..len as usize]);
                        let text_wide = to_wide_chars(&current_text);
                        let prop_name = to_wide_chars("OriginalText");
                        let text_box = Box::new(text_wide);
                        let text_ptr = Box::into_raw(text_box);
                        let _ = SetPropW(
                            hwnd,
                            PCWSTR(prop_name.as_ptr()),
                            Some(HANDLE(text_ptr as *mut c_void)),
                        );
                    }

                    let placeholder_text = to_wide_chars("按下快捷键");
                    let _ = SetWindowTextW(hwnd, PCWSTR(placeholder_text.as_ptr()));
                    let _ = SetFocus(Some(hwnd));
                    return LRESULT(0);
                }

                WM_KILLFOCUS => {
                    // Restore original text if placeholder/empty.
                    let mut buffer = [0u16; 64];
                    let len = GetWindowTextW(hwnd, &mut buffer);
                    let current_text = if len > 0 {
                        String::from_utf16_lossy(&buffer[..len as usize])
                    } else {
                        String::new()
                    };

                    let prop_name = to_wide_chars("OriginalText");
                    if current_text.trim() == "按下快捷键" || current_text.trim().is_empty() {
                        let text_handle = GetPropW(hwnd, PCWSTR(prop_name.as_ptr()));
                        if !text_handle.is_invalid() {
                            let text_ptr = text_handle.0 as *mut Vec<u16>;
                            if !text_ptr.is_null() {
                                let text_box = Box::from_raw(text_ptr);
                                let _ = SetWindowTextW(hwnd, PCWSTR(text_box.as_ptr()));
                                let _ = RemovePropW(hwnd, PCWSTR(prop_name.as_ptr()));
                            }
                        }
                    } else {
                        // Clean stored original text.
                        let text_handle = GetPropW(hwnd, PCWSTR(prop_name.as_ptr()));
                        if !text_handle.is_invalid() {
                            let text_ptr = text_handle.0 as *mut Vec<u16>;
                            if !text_ptr.is_null() {
                                let _ = Box::from_raw(text_ptr);
                            }
                            let _ = RemovePropW(hwnd, PCWSTR(prop_name.as_ptr()));
                        }
                    }

                    let original_proc = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
                    if original_proc != 0 {
                        let wndproc: WNDPROC = std::mem::transmute(original_proc);
                        return CallWindowProcW(wndproc, hwnd, msg, wparam, lparam);
                    }
                    return LRESULT(0);
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

                    if ((key >= 'A' as u32 && key <= 'Z' as u32)
                        || (key >= '0' as u32 && key <= '9' as u32))
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
                        return LRESULT(0);
                    }

                    return LRESULT(0);
                }

                WM_CHAR => {
                    return LRESULT(0);
                }

                _ => {}
            }

            let original_proc = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
            if original_proc != 0 {
                let wndproc: WNDPROC = std::mem::transmute(original_proc);
                return CallWindowProcW(wndproc, hwnd, msg, wparam, lparam);
            }

            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
    }
}
