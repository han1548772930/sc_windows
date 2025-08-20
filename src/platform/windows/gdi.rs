// Windows GDI utility helpers
//
// Provide small, reusable helpers to keep platform-specific GDI operations
// out of higher-level modules without changing behavior.

use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, GetDC, HBITMAP, ReleaseDC,
    SRCCOPY, SelectObject,
};

/// Capture a screen region to an owned HBITMAP using GDI BitBlt.
///
/// Safety: This function performs raw Win32 calls. The returned HBITMAP is owned by the caller.
/// The caller is responsible for freeing it via DeleteObject unless it is handed to the clipboard.
pub unsafe fn capture_screen_region_to_hbitmap(
    selection_rect: RECT,
) -> windows::core::Result<HBITMAP> {
    let width = selection_rect.right - selection_rect.left;
    let height = selection_rect.bottom - selection_rect.top;

    // Assume caller validated width/height. Keep behavior aligned with existing code.
    let screen_dc = unsafe { GetDC(Some(HWND(std::ptr::null_mut()))) };
    let mem_dc = unsafe { CreateCompatibleDC(Some(screen_dc)) };
    let bitmap = unsafe { CreateCompatibleBitmap(screen_dc, width, height) };
    let old_bitmap = unsafe { SelectObject(mem_dc, bitmap.into()) };

    // Copy from screen into the memory DC-backed bitmap
    let _ = unsafe {
        BitBlt(
            mem_dc,
            0,
            0,
            width,
            height,
            Some(screen_dc),
            selection_rect.left,
            selection_rect.top,
            SRCCOPY,
        )
    };

    // Detach and cleanup DCs; return a standalone HBITMAP to the caller
    unsafe {
        SelectObject(mem_dc, old_bitmap);
        let _ = DeleteDC(mem_dc);
        let _ = ReleaseDC(Some(HWND(std::ptr::null_mut())), screen_dc);
    }

    Ok(bitmap)
}
