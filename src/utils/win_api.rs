// Windows API Helper Functions
//
// Centralized Windows API wrappers to reduce code duplication

use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::WindowsAndMessaging::*;

/// 安全地隐藏窗口
#[inline]
pub fn hide_window(hwnd: HWND) -> windows::core::Result<()> {
    unsafe {
        let _ = ShowWindow(hwnd, SW_HIDE);
    }
    Ok(())
}

/// 安全地显示窗口
#[inline]
pub fn show_window(hwnd: HWND) -> windows::core::Result<()> {
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
    }
    Ok(())
}

/// 请求窗口重绘
#[inline]
pub fn request_redraw(hwnd: HWND) -> windows::core::Result<()> {
    unsafe {
        let _ = InvalidateRect(Some(hwnd), None, FALSE.into());
    }
    Ok(())
}

/// 更新窗口
#[inline]
pub fn update_window(hwnd: HWND) -> windows::core::Result<()> {
    unsafe {
        let _ = UpdateWindow(hwnd);
    }
    Ok(())
}

/// 设置窗口为最顶层
#[inline]
pub fn set_window_topmost(
    hwnd: HWND,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> windows::core::Result<()> {
    unsafe {
        SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            x,
            y,
            width,
            height,
            SWP_SHOWWINDOW,
        )?;
    }
    Ok(())
}

/// 启动定时器
#[inline]
pub fn start_timer(hwnd: HWND, timer_id: u32, interval_ms: u32) -> windows::core::Result<()> {
    unsafe {
        SetTimer(Some(hwnd), timer_id as usize, interval_ms, None);
    }
    Ok(())
}

/// 停止定时器
#[inline]
pub fn stop_timer(hwnd: HWND, timer_id: u32) -> windows::core::Result<()> {
    unsafe {
        KillTimer(Some(hwnd), timer_id as usize)?;
    }
    Ok(())
}

/// 销毁窗口
#[inline]
pub fn destroy_window(hwnd: HWND) -> windows::core::Result<()> {
    unsafe {
        DestroyWindow(hwnd)?;
    }
    Ok(())
}

/// 退出消息循环
#[inline]
pub fn quit_message_loop(exit_code: i32) {
    unsafe {
        PostQuitMessage(exit_code);
    }
}

/// 检查窗口是否可见
#[inline]
pub fn is_window_visible(hwnd: HWND) -> bool {
    unsafe { IsWindowVisible(hwnd).as_bool() }
}

/// 获取屏幕尺寸 - 重新导出platform模块的实现
#[inline]
pub fn get_screen_size() -> (i32, i32) {
    crate::platform::windows::system::get_screen_size()
}
