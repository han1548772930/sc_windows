// Thread-safe wrapper for Windows handles
//
// Windows handles are raw pointers that don't implement Send/Sync by default.
// This wrapper makes them thread-safe for our use case.

use std::sync::atomic::{AtomicIsize, Ordering};
use windows::Win32::Foundation::HWND;

/// Thread-safe wrapper for HWND
#[derive(Debug)]
pub struct SafeHwnd {
    handle: AtomicIsize,
}

impl SafeHwnd {
    /// Create a new SafeHwnd from an HWND
    pub fn new(hwnd: Option<HWND>) -> Self {
        Self {
            handle: AtomicIsize::new(hwnd.map_or(0, |h| h.0 as isize)),
        }
    }

    /// Get the HWND value
    pub fn get(&self) -> Option<HWND> {
        let value = self.handle.load(Ordering::Relaxed);
        if value == 0 {
            None
        } else {
            Some(HWND(value as *mut std::ffi::c_void))
        }
    }

    /// Set the HWND value
    pub fn set(&self, hwnd: Option<HWND>) {
        self.handle
            .store(hwnd.map_or(0, |h| h.0 as isize), Ordering::Relaxed);
    }
}

// Safe to send between threads (we're just storing an integer)
unsafe impl Send for SafeHwnd {}
unsafe impl Sync for SafeHwnd {}

impl Clone for SafeHwnd {
    fn clone(&self) -> Self {
        Self {
            handle: AtomicIsize::new(self.handle.load(Ordering::Relaxed)),
        }
    }
}

impl Default for SafeHwnd {
    fn default() -> Self {
        Self::new(None)
    }
}
