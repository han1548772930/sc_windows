// 屏幕捕获功能
//
// 提供屏幕捕获的独立函数

use super::{ScreenshotData, ScreenshotError};

/// 捕获整个屏幕（从原始WindowState迁移）
pub fn capture_screen() -> Result<ScreenshotData, ScreenshotError> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC,
        DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, GetDIBits, ReleaseDC, SRCCOPY, SelectObject,
    };
    use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};

    unsafe {
        // 获取当前屏幕尺寸
        let screen_width = GetSystemMetrics(SM_CXSCREEN);
        let screen_height = GetSystemMetrics(SM_CYSCREEN);

        // 获取屏幕DC
        let screen_dc = GetDC(Some(HWND(std::ptr::null_mut())));
        if screen_dc.is_invalid() {
            return Err(ScreenshotError::CaptureError(
                "Failed to get screen DC".to_string(),
            ));
        }

        // 创建兼容的内存DC
        let mem_dc = CreateCompatibleDC(Some(screen_dc));
        if mem_dc.is_invalid() {
            ReleaseDC(Some(HWND(std::ptr::null_mut())), screen_dc);
            return Err(ScreenshotError::CaptureError(
                "Failed to create memory DC".to_string(),
            ));
        }

        // 创建兼容的位图
        let bitmap = CreateCompatibleBitmap(screen_dc, screen_width, screen_height);
        if bitmap.is_invalid() {
            let _ = DeleteDC(mem_dc);
            ReleaseDC(Some(HWND(std::ptr::null_mut())), screen_dc);
            return Err(ScreenshotError::CaptureError(
                "Failed to create bitmap".to_string(),
            ));
        }

        // 选择位图到内存DC
        let old_bitmap = SelectObject(mem_dc, bitmap.into());

        // 将屏幕内容复制到位图
        let result = BitBlt(
            mem_dc,
            0,
            0,
            screen_width,
            screen_height,
            Some(screen_dc),
            0,
            0,
            SRCCOPY,
        );

        // 恢复原始位图
        SelectObject(mem_dc, old_bitmap);

        if result.is_ok() {
            // 提取位图像素数据
            let mut bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: screen_width,
                    biHeight: -screen_height, // 负值表示自顶向下的位图
                    biPlanes: 1,
                    biBitCount: 32, // 32位BGRA格式
                    biCompression: BI_RGB.0,
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [Default::default(); 1],
            };

            // 计算图像数据大小（4字节每像素，BGRA格式）
            let data_size = (screen_width * screen_height * 4) as usize;
            let mut pixel_data = vec![0u8; data_size];

            // 从位图提取像素数据
            let lines_copied = GetDIBits(
                screen_dc,
                bitmap,
                0,
                screen_height as u32,
                Some(pixel_data.as_mut_ptr() as *mut std::ffi::c_void),
                &mut bmi,
                DIB_RGB_COLORS,
            );

            // 清理资源
            let _ = DeleteDC(mem_dc);
            ReleaseDC(Some(HWND(std::ptr::null_mut())), screen_dc);
            let _ = DeleteObject(bitmap.into());

            if lines_copied > 0 {
                // 创建截图数据，包含实际的像素数据
                let screenshot_data = ScreenshotData {
                    width: screen_width as u32,
                    height: screen_height as u32,
                    data: pixel_data,
                };

                Ok(screenshot_data)
            } else {
                Err(ScreenshotError::CaptureError(
                    "Failed to extract pixel data from bitmap".to_string(),
                ))
            }
        } else {
            // 清理资源
            let _ = DeleteDC(mem_dc);
            ReleaseDC(Some(HWND(std::ptr::null_mut())), screen_dc);
            let _ = DeleteObject(bitmap.into());
            Err(ScreenshotError::CaptureError(
                "Failed to capture screen".to_string(),
            ))
        }
    }
}

/// 捕获指定区域
pub fn capture_region(
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Result<ScreenshotData, ScreenshotError> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC,
        DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, GetDIBits, ReleaseDC, SRCCOPY, SelectObject,
    };

    // 验证参数
    if width == 0 || height == 0 {
        return Err(ScreenshotError::CaptureError(
            "Invalid region dimensions".to_string(),
        ));
    }

    unsafe {
        // 获取屏幕DC
        let screen_dc = GetDC(Some(HWND(std::ptr::null_mut())));
        if screen_dc.is_invalid() {
            return Err(ScreenshotError::CaptureError(
                "Failed to get screen DC".to_string(),
            ));
        }

        // 创建兼容的内存DC
        let mem_dc = CreateCompatibleDC(Some(screen_dc));
        if mem_dc.is_invalid() {
            ReleaseDC(Some(HWND(std::ptr::null_mut())), screen_dc);
            return Err(ScreenshotError::CaptureError(
                "Failed to create memory DC".to_string(),
            ));
        }

        // 创建兼容的位图
        let bitmap = CreateCompatibleBitmap(screen_dc, width as i32, height as i32);
        if bitmap.is_invalid() {
            let _ = DeleteDC(mem_dc);
            ReleaseDC(Some(HWND(std::ptr::null_mut())), screen_dc);
            return Err(ScreenshotError::CaptureError(
                "Failed to create bitmap".to_string(),
            ));
        }

        // 选择位图到内存DC
        let old_bitmap = SelectObject(mem_dc, bitmap.into());

        // 将指定区域的屏幕内容复制到位图
        let result = BitBlt(
            mem_dc,
            0,
            0,
            width as i32,
            height as i32,
            Some(screen_dc),
            x,
            y,
            SRCCOPY,
        );

        // 恢复原始位图
        SelectObject(mem_dc, old_bitmap);

        if result.is_ok() {
            // 提取位图像素数据
            let mut bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width as i32,
                    biHeight: -(height as i32), // 负值表示自顶向下的位图
                    biPlanes: 1,
                    biBitCount: 32, // 32位BGRA格式
                    biCompression: BI_RGB.0,
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [Default::default(); 1],
            };

            // 计算图像数据大小（4字节每像素，BGRA格式）
            let data_size = (width * height * 4) as usize;
            let mut pixel_data = vec![0u8; data_size];

            // 从位图提取像素数据
            let lines_copied = GetDIBits(
                screen_dc,
                bitmap,
                0,
                height,
                Some(pixel_data.as_mut_ptr() as *mut std::ffi::c_void),
                &mut bmi,
                DIB_RGB_COLORS,
            );

            // 清理资源
            let _ = DeleteDC(mem_dc);
            ReleaseDC(Some(HWND(std::ptr::null_mut())), screen_dc);
            let _ = DeleteObject(bitmap.into());

            if lines_copied > 0 {
                // 创建截图数据，包含实际的像素数据
                let screenshot_data = ScreenshotData {
                    width,
                    height,
                    data: pixel_data,
                };

                Ok(screenshot_data)
            } else {
                Err(ScreenshotError::CaptureError(
                    "Failed to extract pixel data from bitmap".to_string(),
                ))
            }
        } else {
            // 清理资源
            let _ = DeleteDC(mem_dc);
            ReleaseDC(Some(HWND(std::ptr::null_mut())), screen_dc);
            let _ = DeleteObject(bitmap.into());
            Err(ScreenshotError::CaptureError(
                "Failed to capture region".to_string(),
            ))
        }
    }
}
