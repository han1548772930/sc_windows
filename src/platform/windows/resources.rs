//! Windows GDI 资源的 RAII 封装
//!
//! 该模块提供了 Windows GDI 资源的安全封装，确保资源在离开作用域时自动释放。
//!
//! ## 使用示例
//! ```ignore
//! use sc_windows::platform::windows::resources::ManagedBitmap;
//! use windows::Win32::Graphics::Gdi::HBITMAP;
//!
//! // 假设 hbitmap 是从某处获取的有效位图句柄
//! let hbitmap: HBITMAP = /* ... */;
//! 
//! // 创建位图后，它会在离开作用域时自动释放
//! let bitmap = ManagedBitmap::new(hbitmap);
//! // 使用 bitmap...
//! // 离开作用域时自动调用 DeleteObject
//! ```

use windows::Win32::Foundation::HANDLE;
use windows::Win32::Graphics::Gdi::{DeleteDC, DeleteObject, HBITMAP, HDC};

/// HBITMAP 的 RAII 封装
///
/// 当 `ManagedBitmap` 离开作用域时，会自动调用 `DeleteObject` 释放位图资源。
///
/// # 注意
/// 如果位图已被传递给剪贴板，不应使用此封装，因为剪贴板会接管所有权。
#[derive(Debug)]
pub struct ManagedBitmap(HBITMAP);

impl ManagedBitmap {
    /// 从原始 HBITMAP 创建托管位图
    ///
    /// # Safety
    /// 调用者必须确保 `bitmap` 是有效的 HBITMAP 句柄，且调用者拥有该句柄的所有权。
    pub fn new(bitmap: HBITMAP) -> Self {
        Self(bitmap)
    }

    /// 获取内部的 HBITMAP 句柄（不转移所有权）
    pub fn handle(&self) -> HBITMAP {
        self.0
    }

    /// 消费此封装并返回内部的 HBITMAP（转移所有权）
    ///
    /// 调用此方法后，调用者负责释放返回的 HBITMAP。
    pub fn into_inner(self) -> HBITMAP {
        let bitmap = self.0;
        std::mem::forget(self); // 阻止 Drop 被调用
        bitmap
    }

    /// 检查位图是否有效（非空）
    pub fn is_valid(&self) -> bool {
        !self.0.is_invalid()
    }
}

impl Drop for ManagedBitmap {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            // SAFETY: self.0 是有效的 HBITMAP 句柄（已通过 is_invalid 检查），
            // 且我们拥有其所有权（构造时的合约保证）。
            // DeleteObject 是幂等的，即使重复调用也是安全的。
            unsafe {
                let _ = DeleteObject(self.0.into());
            }
        }
    }
}

impl From<HBITMAP> for ManagedBitmap {
    fn from(bitmap: HBITMAP) -> Self {
        Self::new(bitmap)
    }
}

/// HDC 的 RAII 封装（用于 CreateCompatibleDC 创建的 DC）
///
/// 当 `ManagedDC` 离开作用域时，会自动调用 `DeleteDC` 释放设备上下文。
#[derive(Debug)]
pub struct ManagedDC(HDC);

impl ManagedDC {
    /// 从原始 HDC 创建托管设备上下文
    ///
    /// # Safety
    /// 调用者必须确保 `dc` 是由 `CreateCompatibleDC` 或类似函数创建的有效 HDC。
    /// 不要用于 `GetDC` 返回的 HDC，那些应该使用 `ReleaseDC`。
    pub fn new(dc: HDC) -> Self {
        Self(dc)
    }

    /// 获取内部的 HDC 句柄（不转移所有权）
    pub fn handle(&self) -> HDC {
        self.0
    }

    /// 消费此封装并返回内部的 HDC（转移所有权）
    pub fn into_inner(self) -> HDC {
        let dc = self.0;
        std::mem::forget(self);
        dc
    }

    /// 检查 DC 是否有效（非空）
    pub fn is_valid(&self) -> bool {
        !self.0.is_invalid()
    }
}

impl Drop for ManagedDC {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            // SAFETY: self.0 是有效的 HDC 句柄（已通过 is_invalid 检查），
            // 且是由 CreateCompatibleDC 等函数创建的（构造时的合约保证）。
            // 不应用于 GetDC 返回的 HDC。
            unsafe {
                let _ = DeleteDC(self.0);
            }
        }
    }
}

impl From<HDC> for ManagedDC {
    fn from(dc: HDC) -> Self {
        Self::new(dc)
    }
}

/// 通用 Windows HANDLE 的 RAII 封装
///
/// 用于需要调用 CloseHandle 释放的句柄。
#[derive(Debug)]
pub struct ManagedHandle(HANDLE);

impl ManagedHandle {
    /// 从原始 HANDLE 创建托管句柄
    pub fn new(handle: HANDLE) -> Self {
        Self(handle)
    }

    /// 获取内部的 HANDLE（不转移所有权）
    pub fn handle(&self) -> HANDLE {
        self.0
    }

    /// 消费此封装并返回内部的 HANDLE（转移所有权）
    pub fn into_inner(self) -> HANDLE {
        let handle = self.0;
        std::mem::forget(self);
        handle
    }

    /// 检查句柄是否有效（非空且非 INVALID_HANDLE_VALUE）
    pub fn is_valid(&self) -> bool {
        !self.0.is_invalid()
    }
}

impl Drop for ManagedHandle {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            // SAFETY: self.0 是有效的 HANDLE（已通过 is_invalid 检查），
            // 且我们拥有其所有权。CloseHandle 对无效句柄会返回错误但不会崩溃。
            unsafe {
                let _ = windows::Win32::Foundation::CloseHandle(self.0);
            }
        }
    }
}

impl From<HANDLE> for ManagedHandle {
    fn from(handle: HANDLE) -> Self {
        Self::new(handle)
    }
}

// ==================== 单元测试 ====================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_managed_bitmap_into_inner() {
        // 创建一个无效的位图用于测试（不会实际分配资源）
        let invalid_bitmap = HBITMAP::default();
        let managed = ManagedBitmap::new(invalid_bitmap);
        
        // into_inner 应该返回原始句柄
        let raw = managed.into_inner();
        assert_eq!(raw, invalid_bitmap);
        // 此时 managed 已被消费，Drop 不会被调用
    }

    #[test]
    fn test_managed_bitmap_is_valid() {
        let invalid_bitmap = HBITMAP::default();
        let managed = ManagedBitmap::new(invalid_bitmap);
        
        // 默认的 HBITMAP 是无效的
        assert!(!managed.is_valid() || managed.handle().is_invalid());
    }

    #[test]
    fn test_managed_dc_into_inner() {
        let invalid_dc = HDC::default();
        let managed = ManagedDC::new(invalid_dc);
        
        let raw = managed.into_inner();
        assert_eq!(raw, invalid_dc);
    }
}
