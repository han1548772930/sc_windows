// 屏幕捕获功能
//
// 提供屏幕捕获的独立函数

use super::ScreenshotError;

// 注意：capture_screen 和 capture_region 函数已被移除
// 这些函数返回 ScreenshotData 结构体，但应用程序实际使用的是 HBITMAP 版本
// 请使用 capture_region_to_hbitmap 函数代替

/// 捕获指定区域到 HBITMAP（为 App 层提供直接的 GDI 位图）
pub fn capture_region_to_hbitmap(
    rect: windows::Win32::Foundation::RECT,
) -> Result<windows::Win32::Graphics::Gdi::HBITMAP, ScreenshotError> {
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;

    // 基本参数校验，避免平台层处理无效尺寸
    if width <= 0 || height <= 0 {
        return Err(ScreenshotError::CaptureError(
            "Invalid region dimensions".to_string(),
        ));
    }

    // 委托平台层统一实现，避免重复的 GDI 代码分散在多处
    unsafe {
        crate::platform::windows::gdi::capture_screen_region_to_hbitmap(rect)
            .map_err(|e| ScreenshotError::CaptureError(format!("GDI capture failed: {e:?}")))
    }
}

// 注意：capture_gdi_common 函数已被移除
// 该函数包含重复的GDI截图逻辑，现在统一使用 platform::windows::gdi::capture_screen_region_to_hbitmap
