use std::cell::RefCell;

use sc_platform::{CursorIcon, HostPlatform, PlatformServicesError, WindowId};

use crate::win_api;
use crate::win32::{RECT, WM_CLOSE};

use super::{HotkeyManager, TrayManager, clipboard, file_dialog, message_box};

thread_local! {
    static TRAY: RefCell<TrayManager> = RefCell::new(TrayManager::new());
    static HOTKEYS: RefCell<HotkeyManager> = RefCell::new(HotkeyManager::new());
}

/// Windows host-facing platform implementation.
///
/// This wraps common Win32 side effects behind the `sc_platform::HostPlatform` abstraction so the
/// host can avoid calling `win_api::*` directly.
#[derive(Debug, Default, Clone, Copy)]
pub struct WindowsHostPlatform;

impl WindowsHostPlatform {
    pub fn new() -> Self {
        Self
    }

    fn window_err(e: impl std::fmt::Debug) -> PlatformServicesError {
        PlatformServicesError::Window(format!("{e:?}"))
    }
}

impl HostPlatform for WindowsHostPlatform {
    type WindowHandle = WindowId;

    fn is_window_visible(&self, window: WindowId) -> bool {
        win_api::is_window_visible(super::hwnd(window))
    }

    fn screen_size(&self) -> (i32, i32) {
        super::system::get_screen_size()
    }

    fn show_window(&self, window: WindowId) -> Result<(), PlatformServicesError> {
        win_api::show_window(super::hwnd(window)).map_err(Self::window_err)
    }

    fn hide_window(&self, window: WindowId) -> Result<(), PlatformServicesError> {
        win_api::hide_window(super::hwnd(window)).map_err(Self::window_err)
    }

    fn minimize_window(&self, window: WindowId) -> Result<(), PlatformServicesError> {
        win_api::minimize_window(super::hwnd(window)).map_err(Self::window_err)
    }

    fn maximize_window(&self, window: WindowId) -> Result<(), PlatformServicesError> {
        win_api::maximize_window(super::hwnd(window)).map_err(Self::window_err)
    }

    fn restore_window(&self, window: WindowId) -> Result<(), PlatformServicesError> {
        win_api::restore_window(super::hwnd(window)).map_err(Self::window_err)
    }

    fn bring_window_to_top(&self, window: WindowId) -> Result<(), PlatformServicesError> {
        win_api::bring_window_to_top(super::hwnd(window)).map_err(Self::window_err)
    }

    fn set_cursor(&self, cursor: CursorIcon) {
        super::cursor::set_cursor(cursor);
    }

    fn request_redraw(&self, window: WindowId) -> Result<(), PlatformServicesError> {
        win_api::request_redraw(super::hwnd(window)).map_err(Self::window_err)
    }

    fn request_redraw_erase(&self, window: WindowId) -> Result<(), PlatformServicesError> {
        win_api::request_redraw_erase(super::hwnd(window)).map_err(Self::window_err)
    }

    fn request_redraw_rect(
        &self,
        window: WindowId,
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    ) -> Result<(), PlatformServicesError> {
        let rect = RECT {
            left,
            top,
            right,
            bottom,
        };
        win_api::request_redraw_rect(super::hwnd(window), &rect).map_err(Self::window_err)
    }

    fn request_redraw_rect_erase(
        &self,
        window: WindowId,
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    ) -> Result<(), PlatformServicesError> {
        let rect = RECT {
            left,
            top,
            right,
            bottom,
        };
        win_api::request_redraw_rect_erase(super::hwnd(window), &rect).map_err(Self::window_err)
    }

    fn update_window(&self, window: WindowId) -> Result<(), PlatformServicesError> {
        win_api::update_window(super::hwnd(window)).map_err(Self::window_err)
    }

    fn set_window_topmost(
        &self,
        window: WindowId,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> Result<(), PlatformServicesError> {
        win_api::set_window_topmost(super::hwnd(window), x, y, width, height)
            .map_err(Self::window_err)
    }

    fn set_window_topmost_flag(
        &self,
        window: WindowId,
        topmost: bool,
    ) -> Result<(), PlatformServicesError> {
        win_api::set_window_topmost_flag(super::hwnd(window), topmost).map_err(Self::window_err)
    }

    fn start_timer(
        &self,
        window: WindowId,
        timer_id: u32,
        interval_ms: u32,
    ) -> Result<(), PlatformServicesError> {
        win_api::start_timer(super::hwnd(window), timer_id, interval_ms).map_err(Self::window_err)
    }

    fn stop_timer(&self, window: WindowId, timer_id: u32) -> Result<(), PlatformServicesError> {
        win_api::stop_timer(super::hwnd(window), timer_id).map_err(Self::window_err)
    }

    fn request_close(&self, window: WindowId) -> Result<(), PlatformServicesError> {
        win_api::post_message(super::hwnd(window), WM_CLOSE, 0, 0).map_err(Self::window_err)
    }

    fn destroy_window(&self, window: WindowId) -> Result<(), PlatformServicesError> {
        win_api::destroy_window(super::hwnd(window)).map_err(Self::window_err)
    }

    fn copy_text_to_clipboard(&self, text: &str) -> Result<(), PlatformServicesError> {
        clipboard::copy_text_to_clipboard(text)
            .map_err(|e| PlatformServicesError::Clipboard(e.to_string()))
    }

    fn copy_bmp_data_to_clipboard(&self, bmp_data: &[u8]) -> Result<(), PlatformServicesError> {
        clipboard::copy_bmp_data_to_clipboard(bmp_data)
            .map_err(|e| PlatformServicesError::Clipboard(e.to_string()))
    }

    fn show_image_save_dialog(
        &self,
        window: WindowId,
        default_filename: &str,
    ) -> Result<Option<String>, PlatformServicesError> {
        Ok(file_dialog::show_image_save_dialog(
            super::hwnd(window),
            default_filename,
        ))
    }

    fn show_info_message(&self, window: WindowId, title: &str, message: &str) {
        message_box::show_info(super::hwnd(window), title, message);
    }

    fn show_error_message(&self, window: WindowId, title: &str, message: &str) {
        message_box::show_error(super::hwnd(window), title, message);
    }

    fn init_tray(&self, window: WindowId, tooltip: &str) -> Result<(), PlatformServicesError> {
        TRAY.with(|tray| {
            tray.borrow_mut()
                .initialize(window, tooltip)
                .map_err(|e| PlatformServicesError::Tray(e.to_string()))
        })
    }

    fn cleanup_tray(&self) -> Result<(), PlatformServicesError> {
        TRAY.with(|tray| tray.borrow_mut().cleanup());
        Ok(())
    }

    fn set_global_hotkey(
        &self,
        window: WindowId,
        hotkey_id: i32,
        modifiers: u32,
        key: u32,
    ) -> Result<(), PlatformServicesError> {
        HOTKEYS.with(|hotkeys| {
            hotkeys
                .borrow_mut()
                .register_hotkey(window, hotkey_id, modifiers, key)
                .map_err(|e| PlatformServicesError::Hotkey(format!("{e:?}")))
        })
    }

    fn clear_global_hotkeys(&self) -> Result<(), PlatformServicesError> {
        HOTKEYS.with(|hotkeys| hotkeys.borrow_mut().cleanup());
        Ok(())
    }
}
