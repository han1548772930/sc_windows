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
use windows::Win32::Foundation::{LPARAM, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::ScreenToClient;
use windows::Win32::Graphics::Gdi::{
    CombineRgn, CreateRectRgn, DeleteObject, RGN_DIFF, SetWindowRgn,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYEVENTF_KEYUP, MOUSEEVENTF_WHEEL,
    MOUSEINPUT, SendInput, VIRTUAL_KEY, VK_NEXT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CWP_SKIPDISABLED, CWP_SKIPINVISIBLE, CWP_SKIPTRANSPARENT, ChildWindowFromPointEx, GW_HWNDNEXT,
    GetCursorPos, GetWindow, GetWindowRect, IsWindowVisible, SMTO_ABORTIFHUNG, SendMessageTimeoutW,
    SetCursorPos, WM_MOUSEWHEEL, WindowFromPoint,
};

use sc_drawing::Rect;
use sc_platform::WindowId;
use std::sync::atomic::{AtomicI32, AtomicI64, AtomicU32, AtomicU64, Ordering};
use windows::Win32::Foundation::LRESULT;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, MSG, MSLLHOOKSTRUCT, PostThreadMessageW, SetWindowsHookExW,
    UnhookWindowsHookEx, WH_MOUSE_LL, WM_QUIT,
};

static HOOK_THREAD_ID: AtomicU32 = AtomicU32::new(0);
static WHEEL_SEQUENCE: AtomicU64 = AtomicU64::new(0);
static WHEEL_DELTA_TOTAL: AtomicI64 = AtomicI64::new(0);
static HOOK_LEFT: AtomicI32 = AtomicI32::new(0);
static HOOK_TOP: AtomicI32 = AtomicI32::new(0);
static HOOK_RIGHT: AtomicI32 = AtomicI32::new(0);
static HOOK_BOTTOM: AtomicI32 = AtomicI32::new(0);

unsafe extern "system" fn scroll_mouse_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 && wparam.0 as u32 == WM_MOUSEWHEEL {
        let info = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
        if info.pt.x >= HOOK_LEFT.load(Ordering::Relaxed)
            && info.pt.x < HOOK_RIGHT.load(Ordering::Relaxed)
            && info.pt.y >= HOOK_TOP.load(Ordering::Relaxed)
            && info.pt.y < HOOK_BOTTOM.load(Ordering::Relaxed)
        {
            WHEEL_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let delta = (info.mouseData >> 16) as u16 as i16 as i64;
            WHEEL_DELTA_TOTAL.fetch_add(delta, Ordering::Relaxed);
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

pub fn start_scroll_wheel_hook(rect: Rect) -> Result<u64, String> {
    stop_scroll_wheel_hook();
    HOOK_LEFT.store(rect.left, Ordering::Relaxed);
    HOOK_TOP.store(rect.top, Ordering::Relaxed);
    HOOK_RIGHT.store(rect.right, Ordering::Relaxed);
    HOOK_BOTTOM.store(rect.bottom, Ordering::Relaxed);
    let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let thread_id = unsafe { GetCurrentThreadId() };
        HOOK_THREAD_ID.store(thread_id, Ordering::Release);
        let hook = match unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(scroll_mouse_hook), None, 0) }
        {
            Ok(hook) => hook,
            Err(error) => {
                HOOK_THREAD_ID.store(0, Ordering::Release);
                let _ = ready_tx.send(Err(format!("SetWindowsHookExW failed: {error}")));
                return;
            }
        };
        let _ = ready_tx.send(Ok(()));
        let mut message = MSG::default();
        while unsafe { GetMessageW(&mut message, None, 0, 0) }.as_bool() {}
        let _ = unsafe { UnhookWindowsHookEx(hook) };
        HOOK_THREAD_ID
            .compare_exchange(thread_id, 0, Ordering::AcqRel, Ordering::Relaxed)
            .ok();
    });
    ready_rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .map_err(|e| format!("scroll hook thread failed: {e}"))??;
    Ok(WHEEL_SEQUENCE.load(Ordering::Relaxed))
}

pub fn stop_scroll_wheel_hook() {
    let thread_id = HOOK_THREAD_ID.swap(0, Ordering::AcqRel);
    if thread_id != 0 {
        let _ = unsafe { PostThreadMessageW(thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) };
    }
}

pub fn scroll_wheel_sequence() -> u64 {
    WHEEL_SEQUENCE.load(Ordering::Relaxed)
}

pub fn scroll_wheel_delta_total() -> i64 {
    WHEEL_DELTA_TOTAL.load(Ordering::Relaxed)
}

/// Exclude a rectangle from a window's visible and hit-test region.
pub fn set_window_region_hole(
    window: WindowId,
    screen_size: (i32, i32),
    hole: Option<Rect>,
) -> Result<(), String> {
    unsafe {
        let hwnd = super::hwnd(window);
        let Some(hole) = hole else {
            SetWindowRgn(hwnd, None, true);
            return Ok(());
        };
        let region = CreateRectRgn(0, 0, screen_size.0, screen_size.1);
        let excluded = CreateRectRgn(hole.left, hole.top, hole.right, hole.bottom);
        if region.is_invalid() || excluded.is_invalid() {
            return Err("CreateRectRgn failed".to_string());
        }
        CombineRgn(Some(region), Some(region), Some(excluded), RGN_DIFF);
        let _ = DeleteObject(excluded.into());
        if SetWindowRgn(hwnd, Some(region), true) == 0 {
            let _ = DeleteObject(region.into());
            return Err("SetWindowRgn failed".to_string());
        }
        Ok(())
    }
}

/// Find the actual window or child control below a screen point.
pub fn window_at_screen_point(x: i32, y: i32) -> Option<WindowId> {
    let child = unsafe { WindowFromPoint(POINT { x, y }) };
    if child.0.is_null() {
        return None;
    }
    Some(super::window_id(child))
}

/// Find the first visible top-level window below `exclude` in Z order that contains the point.
pub fn window_below_at_screen_point(exclude: WindowId, x: i32, y: i32) -> Option<WindowId> {
    let mut candidate = unsafe { GetWindow(super::hwnd(exclude), GW_HWNDNEXT) };
    while let Ok(hwnd) = candidate {
        if hwnd.0.is_null() {
            break;
        }
        let mut rect = RECT::default();
        if unsafe { IsWindowVisible(hwnd) }.as_bool()
            && unsafe { GetWindowRect(hwnd, &mut rect) }.is_ok()
            && x >= rect.left
            && x < rect.right
            && y >= rect.top
            && y < rect.bottom
        {
            let mut target = hwnd;
            loop {
                let mut client_point = POINT { x, y };
                if !unsafe { ScreenToClient(target, &mut client_point) }.as_bool() {
                    break;
                }
                let child = unsafe {
                    ChildWindowFromPointEx(
                        target,
                        client_point,
                        CWP_SKIPINVISIBLE | CWP_SKIPDISABLED | CWP_SKIPTRANSPARENT,
                    )
                };
                if child.0.is_null() || child == target {
                    break;
                }
                target = child;
            }
            return Some(super::window_id(target));
        }
        candidate = unsafe { GetWindow(hwnd, GW_HWNDNEXT) };
    }
    None
}

/// Post a wheel-down message directly to a window, independent of keyboard focus.
pub fn post_wheel_down(window: WindowId, x: i32, y: i32) -> Result<(), String> {
    let delta = (-120i16 as u16 as usize) << 16;
    let position = (x as u16 as usize) | ((y as u16 as usize) << 16);
    for _ in 0..5 {
        unsafe {
            SendMessageTimeoutW(
                super::hwnd(window),
                WM_MOUSEWHEEL,
                WPARAM(delta),
                LPARAM(position as isize),
                SMTO_ABORTIFHUNG,
                200,
                None,
            )
        };
    }
    Ok(())
}

/// Send a real wheel event through hit testing at the given screen point.
pub fn send_real_wheel_down(x: i32, y: i32) -> Result<(), String> {
    let mut original = POINT::default();
    unsafe { GetCursorPos(&mut original) }.map_err(|e| e.to_string())?;
    unsafe { SetCursorPos(x, y) }.map_err(|e| e.to_string())?;
    let input = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                mouseData: (-600i32) as u32,
                dwFlags: MOUSEEVENTF_WHEEL,
                ..Default::default()
            },
        },
    };
    let sent = unsafe { SendInput(&[input], std::mem::size_of::<INPUT>() as i32) };
    std::thread::sleep(std::time::Duration::from_millis(20));
    let _ = unsafe { SetCursorPos(original.x, original.y) };
    if sent == 1 {
        Ok(())
    } else {
        Err("SendInput(mouse wheel) failed".to_string())
    }
}

/// Send one PageDown key press to the foreground window.
pub fn send_page_down() -> Result<(), String> {
    let key = |flags| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(VK_NEXT.0),
                dwFlags: flags,
                ..Default::default()
            },
        },
    };
    let inputs = [key(Default::default()), key(KEYEVENTF_KEYUP)];
    let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    if sent == inputs.len() as u32 {
        Ok(())
    } else {
        Err("SendInput(PageDown) failed".to_string())
    }
}
