use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    MB_ICONERROR, MB_ICONINFORMATION, MB_OK, MESSAGEBOX_STYLE, MessageBoxW,
};
use windows::core::PCWSTR;

use crate::win_api::to_wide_chars;

#[inline]
pub fn show_error(hwnd: HWND, title: &str, message: &str) {
    show(hwnd, title, message, MB_OK | MB_ICONERROR);
}

#[inline]
pub fn show_info(hwnd: HWND, title: &str, message: &str) {
    show(hwnd, title, message, MB_OK | MB_ICONINFORMATION);
}

#[inline]
fn show(hwnd: HWND, title: &str, message: &str, style: MESSAGEBOX_STYLE) {
    let title_w = to_wide_chars(title);
    let message_w = to_wide_chars(message);

    unsafe {
        let _ = MessageBoxW(
            Some(hwnd),
            PCWSTR(message_w.as_ptr()),
            PCWSTR(title_w.as_ptr()),
            style,
        );
    }
}
