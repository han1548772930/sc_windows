// Windows平台实现
//
// 提供Windows平台特定的渲染实现

pub mod d2d;
pub mod gdi;
pub mod system;

pub use d2d::Direct2DRenderer;

use windows::Win32::Foundation::HWND;

/// 安全的窗口句柄包装
#[derive(Debug, Default, Clone, Copy)]
pub struct SafeHwnd {
    hwnd: Option<HWND>,
}

impl SafeHwnd {
    pub fn new(hwnd: HWND) -> Self {
        Self { hwnd: Some(hwnd) }
    }

    pub fn set(&mut self, hwnd: Option<HWND>) {
        self.hwnd = hwnd;
    }

    pub fn get(&self) -> Option<HWND> {
        self.hwnd
    }
}
