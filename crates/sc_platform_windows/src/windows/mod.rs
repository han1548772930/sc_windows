pub mod app_runner;
pub mod bmp;
pub mod clipboard;
pub mod controls;
pub mod cursor;
pub mod d2d;
pub mod factory;
pub mod file_dialog;
pub mod gdi;
pub mod host_platform;
pub mod hotkey_manager;
pub mod hotkeys;
pub mod message_box;
pub mod resources;
pub mod system;
pub mod tray;
pub mod tray_manager;
mod window_event_converter;

pub use app_runner::{UserEventSender, run_fullscreen_toolwindow_app, run_toolwindow_app};
pub use d2d::Direct2DRenderer;
pub use factory::SharedFactories;
pub use host_platform::WindowsHostPlatform;
pub use hotkey_manager::HotkeyManager;
pub use resources::{ManagedBitmap, ManagedDC, ManagedHandle};
pub use tray_manager::TrayManager;

use std::ffi::c_void;

use sc_platform::WindowId;
use windows::Win32::Foundation::HWND;

#[inline]
pub fn window_id(hwnd: HWND) -> WindowId {
    WindowId::from_raw(hwnd.0 as usize)
}

#[inline]
pub fn hwnd(window: WindowId) -> HWND {
    HWND(window.raw() as *mut c_void)
}

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
