// Windows system helpers
//
// Centralize common system queries used across the app.

use windows::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, SM_CXSCREEN, SM_CYBORDER, SM_CYCAPTION, SM_CYFRAME, SM_CYSCREEN,
};

pub fn get_screen_size() -> (i32, i32) {
    let w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let h = unsafe { GetSystemMetrics(SM_CYSCREEN) };
    (w, h)
}

pub fn get_caption_height() -> i32 {
    unsafe { GetSystemMetrics(SM_CYCAPTION) }
}

pub fn get_border_height() -> i32 {
    unsafe { GetSystemMetrics(SM_CYBORDER) }
}

pub fn get_frame_height() -> i32 {
    unsafe { GetSystemMetrics(SM_CYFRAME) }
}
