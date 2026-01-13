pub use windows::core::{Error, Result};

pub use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};

pub use windows::Win32::UI::WindowsAndMessaging::{
    CS_DBLCLKS, CS_HREDRAW, CS_OWNDC, WM_CLOSE, WM_CREATE, WM_DESTROY, WM_HOTKEY, WM_PAINT,
    WM_SETCURSOR, WM_TIMER,
};
