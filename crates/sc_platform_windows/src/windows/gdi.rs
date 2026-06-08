use std::ptr::null_mut;

use sc_drawing::Rect;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, GetDC, HBITMAP, ReleaseDC, SRCCOPY,
    SelectObject,
};
use windows::core::Result as WinResult;

use super::resources::ManagedDC;

/// Capture a screen rectangle into a caller-owned `HBITMAP`.
///
/// # Safety
///
/// The returned bitmap must be deleted with `DeleteObject` or wrapped in `ManagedBitmap`, unless
/// ownership is transferred to another Win32 API.
///
/// ```no_run
/// use sc_drawing::Rect;
/// use sc_platform_windows::windows::gdi::capture_screen_region_to_hbitmap;
///
/// let rect = Rect { left: 0, top: 0, right: 100, bottom: 100 };
/// let bitmap = unsafe { capture_screen_region_to_hbitmap(rect) };
/// ```
pub unsafe fn capture_screen_region_to_hbitmap(selection_rect: Rect) -> WinResult<HBITMAP> {
    let width = selection_rect.right - selection_rect.left;
    let height = selection_rect.bottom - selection_rect.top;

    let desktop = HWND(null_mut());
    let screen_dc = unsafe { GetDC(Some(desktop)) };
    let mem_dc = ManagedDC::new(unsafe { CreateCompatibleDC(Some(screen_dc)) });
    let bitmap = unsafe { CreateCompatibleBitmap(screen_dc, width, height) };
    let old_bitmap = unsafe { SelectObject(mem_dc.handle(), bitmap.into()) };

    let _ = unsafe {
        BitBlt(
            mem_dc.handle(),
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

    unsafe {
        SelectObject(mem_dc.handle(), old_bitmap);
        let _ = ReleaseDC(Some(desktop), screen_dc);
    }

    Ok(bitmap)
}
