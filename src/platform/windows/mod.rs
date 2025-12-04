//! Windows 平台特定实现
//!
//! 该模块包含 Windows 平台的各种实现：
//! - `d2d`: Direct2D 渲染器
//! - `dxgi`: DXGI 屏幕捕获
//! - `gdi`: GDI 屏幕捕获
//! - `resources`: RAII 资源封装
//! - `system`: 系统信息查询

pub mod d2d;
pub mod dxgi;
pub mod gdi;
pub mod resources;
pub mod system;

pub use d2d::Direct2DRenderer;
pub use resources::{ManagedBitmap, ManagedDC, ManagedHandle};

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
