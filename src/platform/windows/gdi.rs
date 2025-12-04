//! GDI 屏幕捕获模块
//!
//! 提供基于 GDI 的屏幕捕获功能。

use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, GetDC, HBITMAP, ReleaseDC,
    SRCCOPY, SelectObject,
};

use super::resources::ManagedDC;

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

    // SAFETY: 以下 GDI 调用均为 Windows API 的标准用法：
    // 1. GetDC(NULL) 获取屏幕 DC，始终有效
    // 2. CreateCompatibleDC/Bitmap 创建兼容的内存 DC 和位图
    // 3. SelectObject 将位图选入 DC，返回旧对象以便恢复
    // 4. BitBlt 复制屏幕内容到内存位图
    let screen_dc = unsafe { GetDC(Some(HWND(std::ptr::null_mut()))) };
    // 使用 RAII 封装管理 mem_dc，离开作用域时自动调用 DeleteDC
    let mem_dc = ManagedDC::new(unsafe { CreateCompatibleDC(Some(screen_dc)) });
    let bitmap = unsafe { CreateCompatibleBitmap(screen_dc, width, height) };
    let old_bitmap = unsafe { SelectObject(mem_dc.handle(), bitmap.into()) };

    // SAFETY: BitBlt 从屏幕 DC 复制到内存 DC，两个 DC 都有效。
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

    // 清理资源：
    // 1. 恢复原始位图选择
    // 2. 释放屏幕 DC（使用 ReleaseDC 而非 DeleteDC）
    // 注：mem_dc 使用 RAII 封装，会在离开作用域时自动 DeleteDC
    unsafe {
        SelectObject(mem_dc.handle(), old_bitmap);
        let _ = ReleaseDC(Some(HWND(std::ptr::null_mut())), screen_dc);
    }

    Ok(bitmap)
}
