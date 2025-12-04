//! GDI 屏幕捕获模块
//!
//! 提供基于 GDI 的屏幕捕获功能。

use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, GetDC, HBITMAP, ReleaseDC,
    SRCCOPY, SelectObject,
};

/// 捕获屏幕区域到 HBITMAP
///
/// 使用 GDI BitBlt 捕获指定屏幕区域的图像。
///
/// # 参数
/// * `selection_rect` - 要捕获的屏幕区域矩形
///
/// # 返回值
/// 成功返回包含捕获图像的 HBITMAP，失败返回错误
///
/// # Safety
/// 该函数执行原始 Win32 API 调用，存在以下安全要求：
/// - `selection_rect` 的坐标必须在有效屏幕范围内
/// - `selection_rect.right > selection_rect.left` 且 `selection_rect.bottom > selection_rect.top`
/// - 返回的 HBITMAP 由调用者拥有，调用者负责通过 `DeleteObject` 释放
/// - 如果 HBITMAP 被传递给剪贴板，则不应调用 `DeleteObject`，因为剪贴板会接管所有权
///
/// # 示例
/// ```no_run
/// use windows::Win32::Foundation::RECT;
/// use sc_windows::platform::windows::gdi::capture_screen_region_to_hbitmap;
///
/// let rect = RECT { left: 0, top: 0, right: 100, bottom: 100 };
/// let bitmap = unsafe { capture_screen_region_to_hbitmap(rect) };
/// // 使用完毕后记得释放
/// ```
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
