use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Graphics::Gdi::{DeleteDC, DeleteObject, HBITMAP, HDC};

/// RAII wrapper for owned `HBITMAP` handles.
///
/// Do not use this after transferring the bitmap to the clipboard because the clipboard takes
/// ownership of the handle.
#[derive(Debug)]
pub struct ManagedBitmap(HBITMAP);

impl ManagedBitmap {
    /// Wrap an owned bitmap handle.
    ///
    /// # Safety
    ///
    /// The caller must own `bitmap`, and it must be valid for `DeleteObject` unless ownership is
    /// later transferred with `into_inner`.
    pub fn new(bitmap: HBITMAP) -> Self {
        Self(bitmap)
    }

    pub fn handle(&self) -> HBITMAP {
        self.0
    }

    /// Consume the wrapper and return the raw handle without deleting it.
    pub fn into_inner(self) -> HBITMAP {
        let bitmap = self.0;
        std::mem::forget(self);
        bitmap
    }

    pub fn is_valid(&self) -> bool {
        !self.0.is_invalid()
    }
}

impl Drop for ManagedBitmap {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
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

/// RAII wrapper for `CreateCompatibleDC`-style device contexts.
#[derive(Debug)]
pub struct ManagedDC(HDC);

impl ManagedDC {
    /// Wrap an owned device context handle.
    ///
    /// # Safety
    ///
    /// The caller must ensure `dc` was created by `CreateCompatibleDC` or an equivalent API that
    /// should be released with `DeleteDC`. Do not wrap `GetDC` handles here.
    pub fn new(dc: HDC) -> Self {
        Self(dc)
    }

    pub fn handle(&self) -> HDC {
        self.0
    }

    /// Consume the wrapper and return the raw handle without deleting it.
    pub fn into_inner(self) -> HDC {
        let dc = self.0;
        std::mem::forget(self);
        dc
    }

    pub fn is_valid(&self) -> bool {
        !self.0.is_invalid()
    }
}

impl Drop for ManagedDC {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
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

/// RAII wrapper for owned Win32 `HANDLE` values.
#[derive(Debug)]
pub struct ManagedHandle(HANDLE);

impl ManagedHandle {
    pub fn new(handle: HANDLE) -> Self {
        Self(handle)
    }

    pub fn handle(&self) -> HANDLE {
        self.0
    }

    /// Consume the wrapper and return the raw handle without closing it.
    pub fn into_inner(self) -> HANDLE {
        let handle = self.0;
        std::mem::forget(self);
        handle
    }

    pub fn is_valid(&self) -> bool {
        !self.0.is_invalid()
    }
}

impl Drop for ManagedHandle {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}

impl From<HANDLE> for ManagedHandle {
    fn from(handle: HANDLE) -> Self {
        Self::new(handle)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_managed_bitmap_into_inner() {
        let invalid_bitmap = super::HBITMAP::default();
        let managed = super::ManagedBitmap::new(invalid_bitmap);

        let raw = managed.into_inner();
        assert_eq!(raw, invalid_bitmap);
    }

    #[test]
    fn test_managed_bitmap_is_valid() {
        let invalid_bitmap = super::HBITMAP::default();
        let managed = super::ManagedBitmap::new(invalid_bitmap);

        assert!(!managed.is_valid() || managed.handle().is_invalid());
    }

    #[test]
    fn test_managed_dc_into_inner() {
        let invalid_dc = super::HDC::default();
        let managed = super::ManagedDC::new(invalid_dc);

        let raw = managed.into_inner();
        assert_eq!(raw, invalid_dc);
    }
}
