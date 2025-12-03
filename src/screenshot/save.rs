use super::ScreenshotError;

/// 将 BMP 数据复制到剪贴板（使用 CF_DIB 格式）
pub fn copy_bmp_data_to_clipboard(bmp_data: &[u8]) -> Result<(), ScreenshotError> {
    use std::ffi::c_void;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};

    // BMP 文件头是 14 字节，CF_DIB 格式不需要文件头，只需要 BITMAPINFO + 像素数据
    if bmp_data.len() < 54 {
        return Err(ScreenshotError::SaveError(
            "BMP data too small".to_string(),
        ));
    }

    // 跳过 BMP 文件头 (14 字节)，获取 DIB 数据
    let dib_data = &bmp_data[14..];
    let data_size = dib_data.len();

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

        std::ptr::copy_nonoverlapping(dib_data.as_ptr() as *const c_void, mem_ptr, data_size);

        let _ = GlobalUnlock(h_mem);

        // 设置剪贴板数据 (CF_DIB = 8)
        if SetClipboardData(8u32, Some(windows::Win32::Foundation::HANDLE(h_mem.0))).is_err() {
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
