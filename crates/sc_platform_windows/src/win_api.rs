use std::{ffi::OsStr, ffi::c_void, iter::once, os::windows::ffi::OsStrExt};

use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::Com::CoInitialize;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::HiDpi::{PROCESS_PER_MONITOR_DPI_AWARE, SetProcessDpiAwareness};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{BOOL, PCWSTR};

#[inline]
pub fn set_process_per_monitor_dpi_aware() -> windows::core::Result<()> {
    unsafe { SetProcessDpiAwareness(PROCESS_PER_MONITOR_DPI_AWARE) }
}

#[inline]
pub fn co_initialize() -> windows::core::HRESULT {
    unsafe { CoInitialize(None) }
}

#[inline]
pub fn get_window_user_data(hwnd: HWND) -> isize {
    unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) }
}

#[inline]
pub fn set_window_user_data(hwnd: HWND, data: isize) -> isize {
    unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, data) }
}

#[inline]
pub fn def_window_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

#[inline]
pub fn get_module_handle() -> windows::core::Result<HMODULE> {
    unsafe { GetModuleHandleW(None) }
}

#[inline]
pub fn load_cursor_arrow() -> windows::core::Result<HCURSOR> {
    unsafe { LoadCursorW(Some(HINSTANCE(std::ptr::null_mut())), IDC_ARROW) }
}

#[inline]
pub fn register_class(window_class: &WNDCLASSW) -> u16 {
    unsafe { RegisterClassW(window_class) }
}

#[inline]
pub fn create_toolwindow_popup(
    instance: HMODULE,
    class_name: PCWSTR,
    width: i32,
    height: i32,
) -> windows::core::Result<HWND> {
    create_toolwindow_popup_with_params(instance, class_name, width, height, None)
}

#[inline]
pub fn create_toolwindow_popup_with_params(
    instance: HMODULE,
    class_name: PCWSTR,
    width: i32,
    height: i32,
    create_params: Option<*const c_void>,
) -> windows::core::Result<HWND> {
    unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW,
            class_name,
            PCWSTR::null(),
            WS_POPUP,
            0,
            0,
            width,
            height,
            Some(HWND(std::ptr::null_mut())),
            Some(HMENU(std::ptr::null_mut())),
            Some(instance.into()),
            create_params,
        )
    }
}

pub fn create_hidden_toolwindow(
    window_class_name: &str,
    window_proc: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT,
    width: i32,
    height: i32,
    class_style: WNDCLASS_STYLES,
) -> windows::core::Result<HWND> {
    create_hidden_toolwindow_with_params(
        window_class_name,
        window_proc,
        width,
        height,
        class_style,
        None,
    )
}

pub fn create_hidden_toolwindow_with_params(
    window_class_name: &str,
    window_proc: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT,
    width: i32,
    height: i32,
    class_style: WNDCLASS_STYLES,
    create_params: Option<*const c_void>,
) -> windows::core::Result<HWND> {
    let instance = get_module_handle()?;
    let class_name = to_wide_chars(window_class_name);

    let window_class = WNDCLASSW {
        lpfnWndProc: Some(window_proc),
        hInstance: instance.into(),
        lpszClassName: PCWSTR(class_name.as_ptr()),
        hbrBackground: HBRUSH(std::ptr::null_mut()),
        hCursor: load_cursor_arrow()?,
        style: class_style,
        ..Default::default()
    };

    register_class(&window_class);

    let hwnd = create_toolwindow_popup_with_params(
        instance,
        PCWSTR(class_name.as_ptr()),
        width,
        height,
        create_params,
    )?;
    let _ = hide_window(hwnd);
    let _ = update_window(hwnd);
    Ok(hwnd)
}

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

#[inline]
pub fn minimize_window(hwnd: HWND) -> windows::core::Result<()> {
    unsafe {
        let _ = ShowWindow(hwnd, SW_MINIMIZE);
    }
    Ok(())
}

#[inline]
pub fn maximize_window(hwnd: HWND) -> windows::core::Result<()> {
    unsafe {
        let _ = ShowWindow(hwnd, SW_MAXIMIZE);
    }
    Ok(())
}

#[inline]
pub fn restore_window(hwnd: HWND) -> windows::core::Result<()> {
    unsafe {
        let _ = ShowWindow(hwnd, SW_RESTORE);
        let _ = SetForegroundWindow(hwnd);
    }
    Ok(())
}

#[inline]
pub fn bring_window_to_top(hwnd: HWND) -> windows::core::Result<()> {
    unsafe {
        let _ = BringWindowToTop(hwnd);
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

/// 请求窗口重绘（并擦除背景）
///
/// Some UI code paths rely on background erase to avoid stale artifacts.
#[inline]
pub fn request_redraw_erase(hwnd: HWND) -> windows::core::Result<()> {
    unsafe {
        let _ = InvalidateRect(Some(hwnd), None, TRUE.into());
    }
    Ok(())
}

/// 请求窗口局部重绘
/// 只重绘指定的矩形区域，减少不必要的渲染开销
#[inline]
pub fn request_redraw_rect(hwnd: HWND, rect: &RECT) -> windows::core::Result<()> {
    // 验证矩形有效性
    if rect.right <= rect.left || rect.bottom <= rect.top {
        return Ok(()); // 无效矩形，不重绘
    }
    unsafe {
        let _ = InvalidateRect(Some(hwnd), Some(rect), FALSE.into());
    }
    Ok(())
}

/// 请求窗口局部重绘（并擦除背景）
#[inline]
pub fn request_redraw_rect_erase(hwnd: HWND, rect: &RECT) -> windows::core::Result<()> {
    if rect.right <= rect.left || rect.bottom <= rect.top {
        return Ok(());
    }
    unsafe {
        let _ = InvalidateRect(Some(hwnd), Some(rect), TRUE.into());
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

/// Begin a WM_PAINT cycle and return the PAINTSTRUCT.
///
/// The returned PAINTSTRUCT must be passed to [`end_paint`].
#[inline]
pub fn begin_paint(hwnd: HWND) -> PAINTSTRUCT {
    let mut ps = PAINTSTRUCT::default();
    unsafe {
        BeginPaint(hwnd, &mut ps);
    }
    ps
}

/// End a WM_PAINT cycle started by [`begin_paint`].
#[inline]
pub fn end_paint(hwnd: HWND, ps: &PAINTSTRUCT) {
    unsafe {
        let _ = EndPaint(hwnd, ps);
    }
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

/// Toggle a window's topmost flag without moving/resizing.
#[inline]
pub fn set_window_topmost_flag(hwnd: HWND, topmost: bool) -> windows::core::Result<()> {
    unsafe {
        let insert_after = if topmost {
            Some(HWND_TOPMOST)
        } else {
            Some(HWND_NOTOPMOST)
        };

        SetWindowPos(
            hwnd,
            insert_after,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
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

#[inline]
pub fn run_message_loop() {
    unsafe {
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, Some(HWND(std::ptr::null_mut())), 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

/// 检查窗口是否可见
#[inline]
pub fn is_window_visible(hwnd: HWND) -> bool {
    unsafe { IsWindowVisible(hwnd).as_bool() }
}

/// 发送自定义消息到窗口
#[inline]
pub fn post_message(
    hwnd: HWND,
    msg: u32,
    wparam: usize,
    lparam: isize,
) -> windows::core::Result<()> {
    unsafe {
        PostMessageW(Some(hwnd), msg, WPARAM(wparam), LPARAM(lparam))?;
    }
    Ok(())
}

/// 发送同步消息到窗口
#[inline]
pub fn send_message(hwnd: HWND, msg: u32, wparam: usize, lparam: isize) -> LRESULT {
    unsafe { SendMessageW(hwnd, msg, Some(WPARAM(wparam)), Some(LPARAM(lparam))) }
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

/// Convert a Rust string to a NUL-terminated UTF-16 buffer for Win32 APIs.
#[inline]
pub fn to_wide_chars(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(once(0)).collect()
}

/// Find a window by its registered class name.
///
/// Returns an HWND which may be null if no window matched.
pub fn find_window_by_class_name(class_name: &str) -> windows::core::Result<HWND> {
    let class_name = to_wide_chars(class_name);
    unsafe { FindWindowW(PCWSTR(class_name.as_ptr()), PCWSTR::null()) }
}

/// 优雅地关闭当前进程的所有窗口
pub fn close_all_app_windows() {
    unsafe {
        let pid = GetCurrentProcessId();
        // 枚举所有顶级窗口
        let _ = EnumWindows(Some(enum_window_callback), LPARAM(pid as isize));
    }
}

/// EnumWindows 的回调函数
unsafe extern "system" fn enum_window_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    unsafe {
        let target_pid = lparam.0 as u32;
        let mut window_pid = 0;

        // 获取当前枚举到的窗口的进程ID
        GetWindowThreadProcessId(hwnd, Some(&mut window_pid));

        // 如果该窗口属于当前进程
        if window_pid == target_pid {
            // 发送关闭消息。使用 PostMessage 而不是 SendMessage，避免阻塞。
            // WM_CLOSE 会让窗口有机会执行清理（处理 WM_DESTROY）。
            let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
        }

        // 返回 TRUE 继续枚举下一个窗口
        BOOL::from(true)
    }
}
