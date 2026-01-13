use std::fmt;

/// Error returned by host-facing platform side-effect APIs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlatformServicesError {
    Window(String),
    Clipboard(String),
    Dialog(String),
    MessageBox(String),
    Tray(String),
    Hotkey(String),
    Other(String),
}

impl fmt::Display for PlatformServicesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlatformServicesError::Window(msg) => write!(f, "window error: {msg}"),
            PlatformServicesError::Clipboard(msg) => write!(f, "clipboard error: {msg}"),
            PlatformServicesError::Dialog(msg) => write!(f, "dialog error: {msg}"),
            PlatformServicesError::MessageBox(msg) => write!(f, "message box error: {msg}"),
            PlatformServicesError::Tray(msg) => write!(f, "tray error: {msg}"),
            PlatformServicesError::Hotkey(msg) => write!(f, "hotkey error: {msg}"),
            PlatformServicesError::Other(msg) => write!(f, "platform error: {msg}"),
        }
    }
}

impl std::error::Error for PlatformServicesError {}

/// Cursor icon (system cursor).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorIcon {
    Arrow,
    Hand,
    IBeam,
    Crosshair,
    SizeAll,
    SizeNWSE,
    SizeNESW,
    SizeNS,
    SizeWE,
    NotAllowed,
}

/// Minimal host-facing platform API.
///
/// This is the boundary where the host can request platform side effects (show/hide window,
/// timers, clipboard, dialogs, etc) without reaching into a platform backend's internal helpers.
///
/// This is intentionally small and focused on host needs, and can evolve towards a gpui-like
/// `Platform` surface over time.
pub trait HostPlatform {
    type WindowHandle: Copy;

    fn is_window_visible(&self, window: Self::WindowHandle) -> bool;

    fn screen_size(&self) -> (i32, i32);

    fn show_window(&self, window: Self::WindowHandle) -> Result<(), PlatformServicesError>;
    fn hide_window(&self, window: Self::WindowHandle) -> Result<(), PlatformServicesError>;

    fn minimize_window(&self, window: Self::WindowHandle) -> Result<(), PlatformServicesError>;
    fn maximize_window(&self, window: Self::WindowHandle) -> Result<(), PlatformServicesError>;
    fn restore_window(&self, window: Self::WindowHandle) -> Result<(), PlatformServicesError>;

    fn bring_window_to_top(&self, window: Self::WindowHandle) -> Result<(), PlatformServicesError>;

    fn set_cursor(&self, cursor: CursorIcon);

    fn request_redraw(&self, window: Self::WindowHandle) -> Result<(), PlatformServicesError>;

    fn request_redraw_erase(&self, window: Self::WindowHandle)
    -> Result<(), PlatformServicesError>;

    fn request_redraw_rect(
        &self,
        window: Self::WindowHandle,
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    ) -> Result<(), PlatformServicesError>;

    fn request_redraw_rect_erase(
        &self,
        window: Self::WindowHandle,
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    ) -> Result<(), PlatformServicesError>;

    fn update_window(&self, window: Self::WindowHandle) -> Result<(), PlatformServicesError>;

    fn set_window_topmost(
        &self,
        window: Self::WindowHandle,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> Result<(), PlatformServicesError>;

    fn set_window_topmost_flag(
        &self,
        window: Self::WindowHandle,
        topmost: bool,
    ) -> Result<(), PlatformServicesError>;

    fn start_timer(
        &self,
        window: Self::WindowHandle,
        timer_id: u32,
        interval_ms: u32,
    ) -> Result<(), PlatformServicesError>;

    fn stop_timer(
        &self,
        window: Self::WindowHandle,
        timer_id: u32,
    ) -> Result<(), PlatformServicesError>;

    /// Request that the platform closes the window gracefully.
    ///
    /// On Win32 this typically posts/sends a `WM_CLOSE` message so normal teardown can run.
    fn request_close(&self, window: Self::WindowHandle) -> Result<(), PlatformServicesError>;

    fn destroy_window(&self, window: Self::WindowHandle) -> Result<(), PlatformServicesError>;

    fn copy_text_to_clipboard(&self, text: &str) -> Result<(), PlatformServicesError>;
    fn copy_bmp_data_to_clipboard(&self, bmp_data: &[u8]) -> Result<(), PlatformServicesError>;

    fn show_image_save_dialog(
        &self,
        window: Self::WindowHandle,
        default_filename: &str,
    ) -> Result<Option<String>, PlatformServicesError>;

    fn show_info_message(&self, window: Self::WindowHandle, title: &str, message: &str);
    fn show_error_message(&self, window: Self::WindowHandle, title: &str, message: &str);

    /// Initialize the system tray icon (if supported).
    fn init_tray(
        &self,
        window: Self::WindowHandle,
        tooltip: &str,
    ) -> Result<(), PlatformServicesError>;

    /// Cleanup the system tray icon (if supported).
    fn cleanup_tray(&self) -> Result<(), PlatformServicesError>;

    /// Register a global hotkey (if supported).
    fn set_global_hotkey(
        &self,
        window: Self::WindowHandle,
        hotkey_id: i32,
        modifiers: u32,
        key: u32,
    ) -> Result<(), PlatformServicesError>;

    /// Unregister all global hotkeys registered by this process (if supported).
    fn clear_global_hotkeys(&self) -> Result<(), PlatformServicesError>;
}
