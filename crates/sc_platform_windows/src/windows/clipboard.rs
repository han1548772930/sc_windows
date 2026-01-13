use std::ffi::c_void;
use std::fmt;

use windows::Win32::Foundation::{HANDLE, HWND};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};

#[derive(Debug, Clone)]
pub enum ClipboardError {
    BmpDataTooSmall,
    OpenClipboardFailed,
    EmptyClipboardFailed,
    AllocateGlobalMemoryFailed,
    LockGlobalMemoryFailed,
    SetClipboardDataFailed,
    CloseClipboardFailed,
}

impl fmt::Display for ClipboardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClipboardError::BmpDataTooSmall => write!(f, "BMP data too small"),
            ClipboardError::OpenClipboardFailed => write!(f, "Failed to open clipboard"),
            ClipboardError::EmptyClipboardFailed => write!(f, "Failed to empty clipboard"),
            ClipboardError::AllocateGlobalMemoryFailed => {
                write!(f, "Failed to allocate global memory")
            }
            ClipboardError::LockGlobalMemoryFailed => write!(f, "Failed to lock global memory"),
            ClipboardError::SetClipboardDataFailed => write!(f, "Failed to set clipboard data"),
            ClipboardError::CloseClipboardFailed => write!(f, "Failed to close clipboard"),
        }
    }
}

impl std::error::Error for ClipboardError {}

/// 将 BMP 数据复制到剪贴板（使用 CF_DIB 格式）
pub fn copy_bmp_data_to_clipboard(bmp_data: &[u8]) -> Result<(), ClipboardError> {
    // BMP 文件头是 14 字节，CF_DIB 格式不需要文件头，只需要 BITMAPINFO + 像素数据
    if bmp_data.len() < 54 {
        return Err(ClipboardError::BmpDataTooSmall);
    }

    // 跳过 BMP 文件头 (14 字节)，获取 DIB 数据
    let dib_data = &bmp_data[14..];
    let data_size = dib_data.len();

    unsafe {
        // 打开剪贴板
        if OpenClipboard(Some(HWND(std::ptr::null_mut()))).is_err() {
            return Err(ClipboardError::OpenClipboardFailed);
        }

        // 清空剪贴板
        if EmptyClipboard().is_err() {
            let _ = CloseClipboard();
            return Err(ClipboardError::EmptyClipboardFailed);
        }

        // 分配全局内存
        let h_mem = match GlobalAlloc(GMEM_MOVEABLE, data_size) {
            Ok(mem) => mem,
            Err(_) => {
                let _ = CloseClipboard();
                return Err(ClipboardError::AllocateGlobalMemoryFailed);
            }
        };

        // 锁定内存并复制数据
        let mem_ptr = GlobalLock(h_mem);
        if mem_ptr.is_null() {
            let _ = CloseClipboard();
            return Err(ClipboardError::LockGlobalMemoryFailed);
        }

        std::ptr::copy_nonoverlapping(dib_data.as_ptr() as *const c_void, mem_ptr, data_size);

        let _ = GlobalUnlock(h_mem);

        // 设置剪贴板数据 (CF_DIB = 8)
        if SetClipboardData(8u32, Some(HANDLE(h_mem.0))).is_err() {
            let _ = CloseClipboard();
            return Err(ClipboardError::SetClipboardDataFailed);
        }

        // 关闭剪贴板
        if CloseClipboard().is_err() {
            return Err(ClipboardError::CloseClipboardFailed);
        }

        Ok(())
    }
}

/// 复制文本到剪贴板
pub fn copy_text_to_clipboard(text: &str) -> Result<(), ClipboardError> {
    unsafe {
        // 打开剪贴板
        if OpenClipboard(Some(HWND(std::ptr::null_mut()))).is_err() {
            return Err(ClipboardError::OpenClipboardFailed);
        }

        // 清空剪贴板
        if EmptyClipboard().is_err() {
            let _ = CloseClipboard();
            return Err(ClipboardError::EmptyClipboardFailed);
        }

        // 转换文本为 UTF-16 (Windows 使用 UTF-16 格式)
        let mut wide_text: Vec<u16> = text.encode_utf16().collect();
        wide_text.push(0); // 添加空终止符
        let data_size = wide_text.len() * std::mem::size_of::<u16>();

        // 分配全局内存
        let h_mem = match GlobalAlloc(GMEM_MOVEABLE, data_size) {
            Ok(mem) => mem,
            Err(_) => {
                let _ = CloseClipboard();
                return Err(ClipboardError::AllocateGlobalMemoryFailed);
            }
        };

        // 锁定内存并复制数据
        let mem_ptr = GlobalLock(h_mem);
        if mem_ptr.is_null() {
            let _ = CloseClipboard();
            return Err(ClipboardError::LockGlobalMemoryFailed);
        }

        std::ptr::copy_nonoverlapping(wide_text.as_ptr() as *const c_void, mem_ptr, data_size);

        let _ = GlobalUnlock(h_mem);

        // 设置剪贴板数据 (CF_UNICODETEXT = 13)
        if SetClipboardData(13u32, Some(HANDLE(h_mem.0))).is_err() {
            let _ = CloseClipboard();
            return Err(ClipboardError::SetClipboardDataFailed);
        }

        // 关闭剪贴板
        if CloseClipboard().is_err() {
            return Err(ClipboardError::CloseClipboardFailed);
        }

        Ok(())
    }
}
