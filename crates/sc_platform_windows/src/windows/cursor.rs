use sc_platform::CursorIcon;
use windows::Win32::Foundation::HINSTANCE;
use windows::Win32::UI::WindowsAndMessaging::{
    IDC_ARROW, IDC_CROSS, IDC_HAND, IDC_IBEAM, IDC_NO, IDC_SIZEALL, IDC_SIZENESW, IDC_SIZENS,
    IDC_SIZENWSE, IDC_SIZEWE, LoadCursorW, SetCursor,
};
use windows::core::PCWSTR;

pub fn cursor_id(cursor: CursorIcon) -> PCWSTR {
    match cursor {
        CursorIcon::Arrow => IDC_ARROW,
        CursorIcon::Hand => IDC_HAND,
        CursorIcon::IBeam => IDC_IBEAM,
        CursorIcon::Crosshair => IDC_CROSS,
        CursorIcon::SizeAll => IDC_SIZEALL,
        CursorIcon::SizeNWSE => IDC_SIZENWSE,
        CursorIcon::SizeNESW => IDC_SIZENESW,
        CursorIcon::SizeNS => IDC_SIZENS,
        CursorIcon::SizeWE => IDC_SIZEWE,
        CursorIcon::NotAllowed => IDC_NO,
    }
}

pub fn set_cursor(cursor: CursorIcon) {
    let cursor_id = cursor_id(cursor);

    unsafe {
        if let Ok(cursor) = LoadCursorW(Some(HINSTANCE(std::ptr::null_mut())), cursor_id) {
            let _ = SetCursor(Some(cursor));
        }
    }
}
