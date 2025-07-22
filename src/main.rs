#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use sc_windows::utils::to_wide_chars;
use sc_windows::*;
use windows::Win32::Foundation::*;

use windows::Win32::Graphics::Gdi::*;

use windows::Win32::System::LibraryLoader::*;
use windows::Win32::UI::HiDpi::{PROCESS_PER_MONITOR_DPI_AWARE, SetProcessDpiAwareness};

use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::*;

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => match WindowState::new(hwnd) {
            Ok(state) => {
                let state_box = Box::new(state);
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state_box) as isize);
                LRESULT(0)
            }
            Err(_) => LRESULT(-1),
        },

        WM_DESTROY => {
            let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
            if !state_ptr.is_null() {
                let _state = Box::from_raw(state_ptr);
            }
            PostQuitMessage(0);
            LRESULT(0)
        }

        WM_PAINT => {
            let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
            if !state_ptr.is_null() {
                let state = &*state_ptr;
                state.paint(hwnd);
            }
            LRESULT(0)
        }

        WM_MOUSEMOVE => {
            let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
            if !state_ptr.is_null() {
                let state = &mut *state_ptr;
                let x = (lparam.0 & 0xFFFF) as i16 as i32;
                let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
                state.handle_mouse_move(hwnd, x, y);
            }
            LRESULT(0)
        }

        WM_LBUTTONDOWN => {
            let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
            if !state_ptr.is_null() {
                let state = &mut *state_ptr;
                let x = (lparam.0 & 0xFFFF) as i16 as i32;
                let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
                state.handle_left_button_down(hwnd, x, y);
            }
            LRESULT(0)
        }

        WM_LBUTTONUP => {
            let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
            if !state_ptr.is_null() {
                let state = &mut *state_ptr;
                let x = (lparam.0 & 0xFFFF) as i16 as i32;
                let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
                state.handle_left_button_up(hwnd, x, y);
            }
            LRESULT(0)
        }

        WM_LBUTTONDBLCLK => {
            let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
            if !state_ptr.is_null() {
                let state = &mut *state_ptr;
                let x = (lparam.0 & 0xFFFF) as i16 as i32;
                let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
                state.handle_double_click(hwnd, x, y);
            }
            LRESULT(0)
        }

        WM_KEYDOWN => {
            let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
            if !state_ptr.is_null() {
                let state = &mut *state_ptr;
                state.handle_key_down(hwnd, wparam.0 as u32);
            }
            LRESULT(0)
        }

        WM_CHAR => {
            let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
            if !state_ptr.is_null() {
                let state = &mut *state_ptr;
                // 正确处理Unicode字符，支持中文输入
                if let Some(character) = char::from_u32(wparam.0 as u32) {
                    // 允许所有可打印字符，包括中文和其他Unicode字符
                    // 排除控制字符（除了空格和制表符）
                    if !character.is_control() || character == ' ' || character == '\t' {
                        state.handle_text_input(character, hwnd);
                    }
                }
            }
            LRESULT(0)
        }

        WM_TIMER => {
            let state_ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut WindowState;
            if !state_ptr.is_null() {
                let state = unsafe { &mut *state_ptr };
                // 处理光标闪烁定时器
                if wparam.0 == state.cursor_timer_id {
                    state.handle_cursor_timer(hwnd);
                }
            }
            LRESULT(0)
        }

        WM_SETCURSOR => {
            // 让我们自己处理光标
            LRESULT(1)
        }

        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn main() -> Result<()> {
    unsafe {
        SetProcessDpiAwareness(PROCESS_PER_MONITOR_DPI_AWARE)?;
        let instance = GetModuleHandleW(None)?;
        let class_name = to_wide_chars(WINDOW_CLASS_NAME);

        let window_class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance.into(),
            lpszClassName: PCWSTR(class_name.as_ptr()),
            hbrBackground: HBRUSH(std::ptr::null_mut()),
            hCursor: LoadCursorW(Some(HINSTANCE(std::ptr::null_mut())), IDC_ARROW)?,
            style: CS_DBLCLKS | CS_OWNDC | CS_HREDRAW,
            ..Default::default()
        };

        RegisterClassW(&window_class);

        let screen_width = GetSystemMetrics(SM_CXSCREEN);
        let screen_height = GetSystemMetrics(SM_CYSCREEN);

        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            PCWSTR(class_name.as_ptr()),
            PCWSTR::null(),
            WS_POPUP,
            0,
            0,
            screen_width,
            screen_height,
            Some(HWND(std::ptr::null_mut())),
            Some(HMENU(std::ptr::null_mut())),
            Some(instance.into()),
            None,
        )?;

        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = UpdateWindow(hwnd);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, Some(HWND(std::ptr::null_mut())), 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        Ok(())
    }
}
