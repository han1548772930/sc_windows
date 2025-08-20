// 保存和导出功能
//
// 提供截图保存到文件或复制到剪贴板的独立函数

use super::{ScreenshotData, ScreenshotError};

/// 保存截图到文件（BMP格式）
pub fn save_to_file(screenshot: &ScreenshotData, path: &str) -> Result<(), ScreenshotError> {
    use std::fs::File;
    use std::io::Write;
    use windows::Win32::Graphics::Gdi::{BI_RGB, BITMAPFILEHEADER, BITMAPINFOHEADER};

    // 验证数据
    if screenshot.data.is_empty() || screenshot.width == 0 || screenshot.height == 0 {
        return Err(ScreenshotError::SaveError(
            "Invalid screenshot data".to_string(),
        ));
    }

    // 计算行字节数（必须是4的倍数）
    let bytes_per_pixel = 3; // RGB 24位
    let row_size = ((screenshot.width * bytes_per_pixel as u32 + 3) / 4) * 4;
    let image_size = row_size * screenshot.height;

    // 创建BMP文件头
    let file_header = BITMAPFILEHEADER {
        bfType: 0x4D42, // "BM"
        bfSize: (std::mem::size_of::<BITMAPFILEHEADER>()
            + std::mem::size_of::<BITMAPINFOHEADER>()
            + image_size as usize) as u32,
        bfReserved1: 0,
        bfReserved2: 0,
        bfOffBits: (std::mem::size_of::<BITMAPFILEHEADER>()
            + std::mem::size_of::<BITMAPINFOHEADER>()) as u32,
    };

    // 创建BMP信息头
    let info_header = BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: screenshot.width as i32,
        biHeight: screenshot.height as i32, // 正值表示自下而上的位图
        biPlanes: 1,
        biBitCount: 24, // 24位RGB
        biCompression: BI_RGB.0,
        biSizeImage: image_size,
        biXPelsPerMeter: 0,
        biYPelsPerMeter: 0,
        biClrUsed: 0,
        biClrImportant: 0,
    };

    // 转换BGRA到RGB并翻转行（BMP是自下而上存储）
    let mut rgb_data = Vec::with_capacity(image_size as usize);
    for y in (0..screenshot.height).rev() {
        let mut row_data = Vec::with_capacity(row_size as usize);
        for x in 0..screenshot.width {
            let pixel_index = ((y * screenshot.width + x) * 4) as usize;
            if pixel_index + 3 < screenshot.data.len() {
                // BGRA -> RGB
                row_data.push(screenshot.data[pixel_index + 2]); // R
                row_data.push(screenshot.data[pixel_index + 1]); // G
                row_data.push(screenshot.data[pixel_index + 0]); // B
            }
        }
        // 填充到4字节边界
        while row_data.len() < row_size as usize {
            row_data.push(0);
        }
        rgb_data.extend(row_data);
    }

    // 写入文件
    let mut file = File::create(path)
        .map_err(|e| ScreenshotError::SaveError(format!("Failed to create file: {}", e)))?;

    // 写入文件头
    let file_header_bytes = unsafe {
        std::slice::from_raw_parts(
            &file_header as *const _ as *const u8,
            std::mem::size_of::<BITMAPFILEHEADER>(),
        )
    };
    file.write_all(file_header_bytes)
        .map_err(|e| ScreenshotError::SaveError(format!("Failed to write file header: {}", e)))?;

    // 写入信息头
    let info_header_bytes = unsafe {
        std::slice::from_raw_parts(
            &info_header as *const _ as *const u8,
            std::mem::size_of::<BITMAPINFOHEADER>(),
        )
    };
    file.write_all(info_header_bytes)
        .map_err(|e| ScreenshotError::SaveError(format!("Failed to write info header: {}", e)))?;

    // 写入位图数据
    file.write_all(&rgb_data)
        .map_err(|e| ScreenshotError::SaveError(format!("Failed to write image data: {}", e)))?;

    Ok(())
}

/// 复制截图到剪贴板
pub fn copy_to_clipboard(screenshot: &ScreenshotData) -> Result<(), ScreenshotError> {
    use windows::Win32::Foundation::{HANDLE, HWND};
    use windows::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleBitmap, DIB_RGB_COLORS, GetDC,
        ReleaseDC, SetDIBits,
    };
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };

    // 验证数据
    if screenshot.data.is_empty() || screenshot.width == 0 || screenshot.height == 0 {
        return Err(ScreenshotError::SaveError(
            "Invalid screenshot data".to_string(),
        ));
    }

    unsafe {
        // 获取屏幕DC
        let screen_dc = GetDC(Some(HWND(std::ptr::null_mut())));
        if screen_dc.is_invalid() {
            return Err(ScreenshotError::SaveError(
                "Failed to get screen DC".to_string(),
            ));
        }

        // 创建兼容位图
        let bitmap =
            CreateCompatibleBitmap(screen_dc, screenshot.width as i32, screenshot.height as i32);
        if bitmap.is_invalid() {
            ReleaseDC(Some(HWND(std::ptr::null_mut())), screen_dc);
            return Err(ScreenshotError::SaveError(
                "Failed to create bitmap".to_string(),
            ));
        }

        // 设置位图信息
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: screenshot.width as i32,
                biHeight: -(screenshot.height as i32), // 负值表示自顶向下的位图
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

        // 将像素数据设置到位图
        let result = SetDIBits(
            Some(screen_dc),
            bitmap,
            0,
            screenshot.height,
            screenshot.data.as_ptr() as *const std::ffi::c_void,
            &bmi,
            DIB_RGB_COLORS,
        );

        ReleaseDC(Some(HWND(std::ptr::null_mut())), screen_dc);

        if result == 0 {
            return Err(ScreenshotError::SaveError(
                "Failed to set bitmap data".to_string(),
            ));
        }

        // 打开剪贴板
        if OpenClipboard(Some(HWND(std::ptr::null_mut()))).is_err() {
            return Err(ScreenshotError::SaveError(
                "Failed to open clipboard".to_string(),
            ));
        }

        // 清空剪贴板
        if EmptyClipboard().is_err() {
            let _ = CloseClipboard();
            return Err(ScreenshotError::SaveError(
                "Failed to empty clipboard".to_string(),
            ));
        }

        // 设置剪贴板数据（CF_BITMAP = 2）
        if SetClipboardData(2u32, Some(HANDLE(bitmap.0 as *mut std::ffi::c_void))).is_err() {
            let _ = CloseClipboard();
            return Err(ScreenshotError::SaveError(
                "Failed to set clipboard data".to_string(),
            ));
        }

        // 关闭剪贴板
        if CloseClipboard().is_err() {
            return Err(ScreenshotError::SaveError(
                "Failed to close clipboard".to_string(),
            ));
        }

        Ok(())
    }
}

/// 从 HBITMAP 保存到文件（为 App 层提供直接的 GDI 位图保存）
pub fn save_hbitmap_to_file(
    bitmap: windows::Win32::Graphics::Gdi::HBITMAP,
    path: &str,
    width: i32,
    height: i32,
) -> Result<(), ScreenshotError> {
    use std::fs::File;
    use std::io::Write;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPFILEHEADER, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, GetDC, GetDIBits,
        ReleaseDC,
    };

    // 验证参数
    if width <= 0 || height <= 0 {
        return Err(ScreenshotError::SaveError(
            "Invalid bitmap dimensions".to_string(),
        ));
    }

    unsafe {
        // 获取屏幕设备上下文
        let screen_dc = GetDC(Some(HWND(std::ptr::null_mut())));
        if screen_dc.is_invalid() {
            return Err(ScreenshotError::SaveError(
                "Failed to get screen DC".to_string(),
            ));
        }

        // 准备位图信息结构
        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height, // 负值表示自顶向下的位图
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
            height as u32,
            Some(pixel_data.as_mut_ptr() as *mut std::ffi::c_void),
            &mut bmi,
            DIB_RGB_COLORS,
        );

        ReleaseDC(Some(HWND(std::ptr::null_mut())), screen_dc);

        if lines_copied <= 0 {
            return Err(ScreenshotError::SaveError(
                "Failed to extract pixel data from bitmap".to_string(),
            ));
        }

        // 转换 BGRA 到 BGR（BMP 格式）
        let mut bgr_data = Vec::with_capacity((width * height * 3) as usize);
        for chunk in pixel_data.chunks(4) {
            if chunk.len() >= 4 {
                bgr_data.push(chunk[0]); // B
                bgr_data.push(chunk[1]); // G
                bgr_data.push(chunk[2]); // R
                // 跳过 Alpha 通道
            }
        }

        // 计算行填充（BMP 要求每行字节数为4的倍数）
        let row_size = ((width * 3 + 3) / 4) * 4;
        let padding = row_size - width * 3;

        // 创建文件头
        let file_header = BITMAPFILEHEADER {
            bfType: 0x4D42, // "BM"
            bfSize: (std::mem::size_of::<BITMAPFILEHEADER>()
                + std::mem::size_of::<BITMAPINFOHEADER>()
                + (row_size * height) as usize) as u32,
            bfReserved1: 0,
            bfReserved2: 0,
            bfOffBits: (std::mem::size_of::<BITMAPFILEHEADER>()
                + std::mem::size_of::<BITMAPINFOHEADER>()) as u32,
        };

        // 写入文件
        let mut file = File::create(path)
            .map_err(|e| ScreenshotError::SaveError(format!("Failed to create file: {}", e)))?;

        // 写入文件头
        let file_header_bytes = std::slice::from_raw_parts(
            &file_header as *const _ as *const u8,
            std::mem::size_of::<BITMAPFILEHEADER>(),
        );
        file.write_all(file_header_bytes).map_err(|e| {
            ScreenshotError::SaveError(format!("Failed to write file header: {}", e))
        })?;

        // 写入信息头
        let info_header_bytes = std::slice::from_raw_parts(
            &bmi.bmiHeader as *const _ as *const u8,
            std::mem::size_of::<BITMAPINFOHEADER>(),
        );
        file.write_all(info_header_bytes).map_err(|e| {
            ScreenshotError::SaveError(format!("Failed to write info header: {}", e))
        })?;

        // 写入像素数据（从底部开始，因为BMP是倒置的）
        for y in (0..height).rev() {
            let row_start = (y * width * 3) as usize;
            let row_end = row_start + (width * 3) as usize;
            if row_end <= bgr_data.len() {
                file.write_all(&bgr_data[row_start..row_end]).map_err(|e| {
                    ScreenshotError::SaveError(format!("Failed to write pixel data: {}", e))
                })?;

                // 写入行填充
                if padding > 0 {
                    let padding_bytes = vec![0u8; padding as usize];
                    file.write_all(&padding_bytes).map_err(|e| {
                        ScreenshotError::SaveError(format!("Failed to write padding: {}", e))
                    })?;
                }
            }
        }

        Ok(())
    }
}

/// 从 HBITMAP 复制到剪贴板（为 App 层提供直接的 GDI 位图复制）
pub fn copy_hbitmap_to_clipboard(
    bitmap: windows::Win32::Graphics::Gdi::HBITMAP,
) -> Result<(), ScreenshotError> {
    use windows::Win32::Foundation::{HANDLE, HWND};
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };

    unsafe {
        // 打开剪贴板
        if OpenClipboard(Some(HWND(std::ptr::null_mut()))).is_err() {
            return Err(ScreenshotError::SaveError(
                "Failed to open clipboard".to_string(),
            ));
        }

        // 清空剪贴板
        if EmptyClipboard().is_err() {
            let _ = CloseClipboard();
            return Err(ScreenshotError::SaveError(
                "Failed to empty clipboard".to_string(),
            ));
        }

        // 设置剪贴板数据（CF_BITMAP = 2）
        if SetClipboardData(2u32, Some(HANDLE(bitmap.0 as *mut std::ffi::c_void))).is_err() {
            let _ = CloseClipboard();
            return Err(ScreenshotError::SaveError(
                "Failed to set clipboard data".to_string(),
            ));
        }

        // 关闭剪贴板
        if CloseClipboard().is_err() {
            return Err(ScreenshotError::SaveError(
                "Failed to close clipboard".to_string(),
            ));
        }

        Ok(())
    }
}
