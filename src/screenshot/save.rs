// 保存和导出功能
//
// 提供截图保存到文件或复制到剪贴板的独立函数

use super::ScreenshotError;

// 注意：save_to_file 和 copy_to_clipboard 函数已被移除
// 这些函数操作 ScreenshotData 结构体，但应用程序实际使用的是 HBITMAP 版本
// 请使用 save_hbitmap_to_file 和 copy_hbitmap_to_clipboard 函数代替

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
            .map_err(|e| ScreenshotError::SaveError(format!("Failed to create file: {e}")))?;

        // 写入文件头
        let file_header_bytes = std::slice::from_raw_parts(
            &file_header as *const _ as *const u8,
            std::mem::size_of::<BITMAPFILEHEADER>(),
        );
        file.write_all(file_header_bytes)
            .map_err(|e| ScreenshotError::SaveError(format!("Failed to write file header: {e}")))?;

        // 写入信息头
        let info_header_bytes = std::slice::from_raw_parts(
            &bmi.bmiHeader as *const _ as *const u8,
            std::mem::size_of::<BITMAPINFOHEADER>(),
        );
        file.write_all(info_header_bytes)
            .map_err(|e| ScreenshotError::SaveError(format!("Failed to write info header: {e}")))?;

        // 写入像素数据（从底部开始，因为BMP是倒置的）
        for y in (0..height).rev() {
            let row_start = (y * width * 3) as usize;
            let row_end = row_start + (width * 3) as usize;
            if row_end <= bgr_data.len() {
                file.write_all(&bgr_data[row_start..row_end]).map_err(|e| {
                    ScreenshotError::SaveError(format!("Failed to write pixel data: {e}"))
                })?;

                // 写入行填充
                if padding > 0 {
                    let padding_bytes = vec![0u8; padding as usize];
                    file.write_all(&padding_bytes).map_err(|e| {
                        ScreenshotError::SaveError(format!("Failed to write padding: {e}"))
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

/// 复制文本到剪贴板
pub fn copy_text_to_clipboard(text: &str) -> Result<(), ScreenshotError> {
    use std::ffi::c_void;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};

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

        // 转换文本为 UTF-16 (Windows 使用 UTF-16 格式)
        let mut wide_text: Vec<u16> = text.encode_utf16().collect();
        wide_text.push(0); // 添加空终止符
        let data_size = wide_text.len() * std::mem::size_of::<u16>();

        // 分配全局内存
        let h_mem = GlobalAlloc(GMEM_MOVEABLE, data_size);
        let h_mem = match h_mem {
            Ok(mem) => mem,
            Err(_) => {
                let _ = CloseClipboard();
                return Err(ScreenshotError::SaveError(
                    "Failed to allocate global memory".to_string(),
                ));
            }
        };

        // 锁定内存并复制数据
        let mem_ptr = GlobalLock(h_mem);
        if mem_ptr.is_null() {
            let _ = CloseClipboard();
            return Err(ScreenshotError::SaveError(
                "Failed to lock global memory".to_string(),
            ));
        }

        std::ptr::copy_nonoverlapping(wide_text.as_ptr() as *const c_void, mem_ptr, data_size);

        let _ = GlobalUnlock(h_mem);

        // 设置剪贴板数据 (CF_UNICODETEXT = 13)
        if SetClipboardData(13u32, Some(windows::Win32::Foundation::HANDLE(h_mem.0))).is_err() {
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
