pub const WINDOW_CLASS_NAME: &str = "sc_windows_main";

/// Selection handle hit radius (host-side selection interaction).
/// Drawing element handle hit testing lives in `sc_drawing` / `sc_drawing_host`.
pub const HANDLE_DETECTION_RADIUS: f32 = 10.0;

// ==================== Hotkey and timers ====================

pub const HOTKEY_SCREENSHOT_ID: i32 = 1001;

pub const TIMER_CAPTURE_DELAY_ID: usize = 2001;
pub const TIMER_CAPTURE_DELAY_MS: u32 = 50;
pub const TIMER_SCROLL_CAPTURE_ID: usize = 2002;
// Poll faster than a 60 Hz producer so the bounded WGC frame pool is drained
// in FIFO order even when a timer tick is delayed by other UI work.
pub const TIMER_SCROLL_CAPTURE_MS: u32 = 2;
// Three 8ms polls filter wheel/inertia transitions without making the preview
// visibly lag behind a normal mouse-wheel gesture.
pub const SCROLL_SETTLE_TICKS: u8 = 3;
