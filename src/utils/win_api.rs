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

/// 发送自定义消息到窗口
#[inline]
pub fn post_message(hwnd: HWND, msg: u32, wparam: usize, lparam: isize) -> windows::core::Result<()> {
    unsafe {
        PostMessageW(Some(hwnd), msg, WPARAM(wparam), LPARAM(lparam))?;
    }
    Ok(())
}

/// 发送同步消息到窗口
#[inline]
pub fn send_message(hwnd: HWND, msg: u32, wparam: usize, lparam: isize) -> LRESULT {
    unsafe {
        SendMessageW(hwnd, msg, Some(WPARAM(wparam)), Some(LPARAM(lparam)))
    }
}

/// 获取窗口矩形
#[inline]
pub fn get_window_rect(hwnd: HWND) -> windows::core::Result<RECT> {
    let mut rect = RECT::default();
    unsafe {
        GetWindowRect(hwnd, &mut rect)?;
    }
    Ok(rect)
}

/// 获取客户区矩形
#[inline]
pub fn get_client_rect(hwnd: HWND) -> windows::core::Result<RECT> {
    let mut rect = RECT::default();
    unsafe {
        GetClientRect(hwnd, &mut rect)?;
    }
    Ok(rect)
}

/// 设置窗口位置和尺寸
#[inline]
pub fn set_window_pos(
    hwnd: HWND,
    hwnd_insert_after: Option<HWND>,
    x: i32,
    y: i32,
    cx: i32,
    cy: i32,
    flags: SET_WINDOW_POS_FLAGS,
) -> windows::core::Result<()> {
    unsafe {
        SetWindowPos(hwnd, hwnd_insert_after, x, y, cx, cy, flags)?;
    }
    Ok(())
}

/// 将窗口设置为前台
#[inline]
pub fn set_foreground_window(hwnd: HWND) -> bool {
    unsafe { SetForegroundWindow(hwnd).as_bool() }
}

// ==================== 单元测试 ====================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_screen_size() {
        let (width, height) = get_screen_size();
        assert!(width > 0, "屏幕宽度应该大于0");
        assert!(height > 0, "屏幕高度应该大于0");
    }
}
