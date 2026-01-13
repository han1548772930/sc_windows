use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    HOT_KEY_MODIFIERS, RegisterHotKey, UnregisterHotKey,
};

/// Register a global hotkey.
///
/// `modifiers` is a Win32 HOT_KEY_MODIFIERS bitmask.
pub fn register_hotkey(
    hwnd: HWND,
    hotkey_id: i32,
    modifiers: u32,
    key: u32,
) -> windows::core::Result<()> {
    // SAFETY: RegisterHotKey is an OS API. Caller provides the target HWND and key/modifier values.
    unsafe { RegisterHotKey(Some(hwnd), hotkey_id, HOT_KEY_MODIFIERS(modifiers), key) }
}

/// Unregister a global hotkey.
///
/// This matches the legacy behavior where `hWnd` is passed as NULL.
pub fn unregister_hotkey(hotkey_id: i32) -> windows::core::Result<()> {
    // SAFETY: UnregisterHotKey is an OS API.
    unsafe { UnregisterHotKey(None, hotkey_id) }
}

/// Unregister a global hotkey associated with a window.
///
/// This matches the legacy behavior in the Win32 message pump (WM_DESTROY).
pub fn unregister_hotkey_for_window(hwnd: HWND, hotkey_id: i32) -> windows::core::Result<()> {
    // SAFETY: UnregisterHotKey is an OS API.
    unsafe { UnregisterHotKey(Some(hwnd), hotkey_id) }
}
