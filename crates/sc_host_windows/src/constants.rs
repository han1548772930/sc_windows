pub const WINDOW_CLASS_NAME: &str = "sc_windows_main";

/// Selection handle hit radius (host-side selection interaction).
///
/// Drawing element handle hit testing lives in `sc_drawing` / `sc_drawing_host`.
pub const HANDLE_DETECTION_RADIUS: f32 = 10.0;

// ==================== Hotkey and timers ====================

pub const HOTKEY_SCREENSHOT_ID: i32 = 1001;

pub const TIMER_CAPTURE_DELAY_ID: usize = 2001;
pub const TIMER_CAPTURE_DELAY_MS: u32 = 50;
