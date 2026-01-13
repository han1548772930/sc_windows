use std::fmt;

use sc_platform::{TrayEvent, WindowId};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{WM_APP, WM_LBUTTONDBLCLK, WM_RBUTTONUP};

use super::{SafeHwnd, tray};

pub const TRAY_CALLBACK_MESSAGE: u32 = WM_APP + 1;

pub fn tray_event_from_callback(hwnd: HWND, lparam: u32) -> Option<TrayEvent> {
    match lparam {
        WM_RBUTTONUP => Some(TrayEvent::MenuCommand(tray::show_default_context_menu(
            hwnd,
        ))),
        WM_LBUTTONDBLCLK => Some(TrayEvent::DoubleClick),
        _ => None,
    }
}

#[derive(Debug)]
pub enum TrayManagerError {
    Icon(tray::TrayIconError),
    AddFailed,
}

impl fmt::Display for TrayManagerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TrayManagerError::Icon(e) => write!(f, "tray icon error: {e}"),
            TrayManagerError::AddFailed => write!(f, "failed to add tray icon"),
        }
    }
}

impl std::error::Error for TrayManagerError {}

impl From<tray::TrayIconError> for TrayManagerError {
    fn from(value: tray::TrayIconError) -> Self {
        TrayManagerError::Icon(value)
    }
}

pub type Result<T> = std::result::Result<T, TrayManagerError>;

/// Stateful system tray manager.
///
/// This lives in the Windows platform backend (similar to Zed's `gpui::platform::windows`).
#[derive(Debug)]
pub struct TrayManager {
    hwnd: SafeHwnd,
    icon_id: u32,
    is_added: bool,
    callback_message: u32,
}

impl TrayManager {
    pub fn new() -> Self {
        Self {
            hwnd: SafeHwnd::default(),
            icon_id: 1001,
            is_added: false,
            callback_message: 0,
        }
    }

    pub fn initialize(&mut self, window: WindowId, tooltip: &str) -> Result<()> {
        let hwnd = super::hwnd(window);
        self.hwnd.set(Some(hwnd));
        self.callback_message = TRAY_CALLBACK_MESSAGE;

        let icon = tray::create_default_icon()?;

        if tray::add_tray_icon(hwnd, self.icon_id, self.callback_message, tooltip, icon) {
            self.is_added = true;
            Ok(())
        } else {
            Err(TrayManagerError::AddFailed)
        }
    }

    pub fn handle_message(&mut self, _wparam: u32, lparam: u32) -> Option<TrayEvent> {
        let hwnd = self.hwnd.get()?;
        tray_event_from_callback(hwnd, lparam)
    }

    pub fn cleanup(&mut self) {
        if !self.is_added {
            return;
        }

        if let Some(hwnd) = self.hwnd.get() {
            let _ = tray::delete_tray_icon(hwnd, self.icon_id);
        }
        self.is_added = false;
    }

    pub fn reload_settings(&mut self) {
        // Currently no-op. In the future this can refresh tooltip/icon.
    }
}

impl Default for TrayManager {
    fn default() -> Self {
        Self::new()
    }
}
