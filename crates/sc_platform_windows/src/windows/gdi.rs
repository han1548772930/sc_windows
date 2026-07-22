use std::ptr::null_mut;

use sc_drawing::Rect;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, GetDC, GetWindowDC, HBITMAP, ReleaseDC,
    SRCCOPY, SelectObject,
};
use windows::Win32::Storage::Xps::{PRINT_WINDOW_FLAGS, PrintWindow};
use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;
use windows::core::Result as WinResult;

use super::resources::{ManagedBitmap, ManagedDC};
use sc_platform::WindowId;

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

/// Capture a screen rectangle directly to top-down 32-bit BMP bytes.
pub fn capture_screen_region_to_bmp(selection_rect: Rect) -> Result<Vec<u8>, String> {
    let width = selection_rect.right - selection_rect.left;
    let height = selection_rect.bottom - selection_rect.top;
    if width <= 0 || height <= 0 {
        return Err("capture region is empty".to_string());
    }
    unsafe {
        let desktop = HWND(null_mut());
        let screen_dc = GetDC(Some(desktop));
        if screen_dc.is_invalid() {
            return Err("GetDC failed".to_string());
        }
        let memory_dc = ManagedDC::new(CreateCompatibleDC(Some(screen_dc)));
        let bitmap = ManagedBitmap::new(CreateCompatibleBitmap(screen_dc, width, height));
        let old_bitmap = SelectObject(memory_dc.handle(), bitmap.handle().into());
        BitBlt(
            memory_dc.handle(),
            0,
            0,
            width,
            height,
            Some(screen_dc),
            selection_rect.left,
            selection_rect.top,
            SRCCOPY,
        )
        .map_err(|e| format!("BitBlt failed: {e}"))?;
        SelectObject(memory_dc.handle(), old_bitmap);
        let result =
            super::bmp::bitmap_to_bmp_data(memory_dc.handle(), bitmap.handle(), width, height)
                .map_err(|e| e.to_string());
        let _ = ReleaseDC(Some(desktop), screen_dc);
        result
    }
}

/// Capture a screen-coordinate region directly from a window, even when another window covers it.
pub fn capture_window_screen_region_to_bmp(
    window: WindowId,
    selection_rect: Rect,
) -> Result<Vec<u8>, String> {
    unsafe {
        let hwnd = super::hwnd(window);
        let mut window_rect = RECT::default();
        GetWindowRect(hwnd, &mut window_rect).map_err(|e| e.to_string())?;
        let width = window_rect.right - window_rect.left;
        let height = window_rect.bottom - window_rect.top;
        if width <= 0 || height <= 0 {
            return Err("目标窗口尺寸无效".to_string());
        }

        let window_dc = GetWindowDC(Some(hwnd));
        if window_dc.is_invalid() {
            return Err("GetWindowDC failed".to_string());
        }
        let memory_dc = ManagedDC::new(CreateCompatibleDC(Some(window_dc)));
        let bitmap = ManagedBitmap::new(CreateCompatibleBitmap(window_dc, width, height));
        let old_bitmap = SelectObject(memory_dc.handle(), bitmap.handle().into());
        let printed = PrintWindow(hwnd, memory_dc.handle(), PRINT_WINDOW_FLAGS(2)).as_bool();
        SelectObject(memory_dc.handle(), old_bitmap);
        let _ = ReleaseDC(Some(hwnd), window_dc);
        if !printed {
            return Err("PrintWindow failed".to_string());
        }

        let bmp =
            super::bmp::bitmap_to_bmp_data(memory_dc.handle(), bitmap.handle(), width, height)
                .map_err(|e| e.to_string())?;
        let local = Rect {
            left: selection_rect.left - window_rect.left,
            top: selection_rect.top - window_rect.top,
            right: selection_rect.right - window_rect.left,
            bottom: selection_rect.bottom - window_rect.top,
        };
        super::bmp::crop_bmp(&bmp, &local).map_err(|e| e.to_string())
    }
}
