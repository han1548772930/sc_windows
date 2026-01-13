use windows::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, SM_CXSCREEN, SM_CYBORDER, SM_CYCAPTION, SM_CYFRAME, SM_CYSCREEN,
};

/// 获取屏幕尺寸（宽度，高度）
pub fn get_screen_size() -> (i32, i32) {
    // SAFETY: GetSystemMetrics 是线程安全的只读 API，
    // SM_CXSCREEN/SM_CYSCREEN 是有效的系统度量标识符。
    let w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let h = unsafe { GetSystemMetrics(SM_CYSCREEN) };
    (w, h)
}

/// 获取窗口标题栏高度
pub fn get_caption_height() -> i32 {
    // SAFETY: GetSystemMetrics 是线程安全的只读 API。
    unsafe { GetSystemMetrics(SM_CYCAPTION) }
}

/// 获取窗口边框高度
pub fn get_border_height() -> i32 {
    // SAFETY: GetSystemMetrics 是线程安全的只读 API。
    unsafe { GetSystemMetrics(SM_CYBORDER) }
}

/// 获取窗口框架高度
pub fn get_frame_height() -> i32 {
    // SAFETY: GetSystemMetrics 是线程安全的只读 API。
    unsafe { GetSystemMetrics(SM_CYFRAME) }
}
