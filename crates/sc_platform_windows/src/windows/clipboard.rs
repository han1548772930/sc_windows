use std::ffi::c_void;
use std::fmt;

use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND};
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

struct ClipboardSession;

impl ClipboardSession {
    fn open() -> Result<Self, ClipboardError> {
        unsafe {
            OpenClipboard(Some(HWND(std::ptr::null_mut())))
                .map_err(|_| ClipboardError::OpenClipboardFailed)?;
        }
        Ok(Self)
    }

    fn empty(&self) -> Result<(), ClipboardError> {
        unsafe { EmptyClipboard().map_err(|_| ClipboardError::EmptyClipboardFailed) }
    }
}

impl Drop for ClipboardSession {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseClipboard();
        }
    }
}

struct ClipboardMemory {
    handle: HGLOBAL,
}

impl ClipboardMemory {
    fn allocate(size: usize) -> Result<Self, ClipboardError> {
        let handle = unsafe { GlobalAlloc(GMEM_MOVEABLE, size) }
            .map_err(|_| ClipboardError::AllocateGlobalMemoryFailed)?;
        Ok(Self { handle })
    }

    fn copy_from(&self, bytes: &[u8]) -> Result<(), ClipboardError> {
        unsafe {
            let mem_ptr = GlobalLock(self.handle);
            if mem_ptr.is_null() {
                return Err(ClipboardError::LockGlobalMemoryFailed);
            }

            std::ptr::copy_nonoverlapping(bytes.as_ptr() as *const c_void, mem_ptr, bytes.len());
            let _ = GlobalUnlock(self.handle);
        }

        Ok(())
    }

    fn transfer_to_clipboard(mut self, format: u32) -> Result<(), ClipboardError> {
        unsafe {
            SetClipboardData(format, Some(HANDLE(self.handle.0)))
                .map_err(|_| ClipboardError::SetClipboardDataFailed)?;
        }

        self.handle = HGLOBAL::default();
        Ok(())
    }
}

impl Drop for ClipboardMemory {
    fn drop(&mut self) {
        if !self.handle.is_invalid() {
            unsafe {
                let _ = GlobalFree(Some(self.handle));
            }
        }
    }
}

pub fn copy_bmp_data_to_clipboard(bmp_data: &[u8]) -> Result<(), ClipboardError> {
    if bmp_data.len() < 54 {
        return Err(ClipboardError::BmpDataTooSmall);
    }

    let dib_data = &bmp_data[14..];
    copy_bytes_to_clipboard(8, dib_data)
}

pub fn copy_text_to_clipboard(text: &str) -> Result<(), ClipboardError> {
    let mut wide_text: Vec<u16> = text.encode_utf16().collect();
    wide_text.push(0);

    let bytes = unsafe {
        std::slice::from_raw_parts(
            wide_text.as_ptr() as *const u8,
            wide_text.len() * std::mem::size_of::<u16>(),
        )
    };

    copy_bytes_to_clipboard(13, bytes)
}

fn copy_bytes_to_clipboard(format: u32, bytes: &[u8]) -> Result<(), ClipboardError> {
    let session = ClipboardSession::open()?;
    session.empty()?;

    let memory = ClipboardMemory::allocate(bytes.len())?;
    memory.copy_from(bytes)?;
    memory.transfer_to_clipboard(format)
}
