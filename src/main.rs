// 在调试模式下显示控制台，在发布模式下隐藏
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

use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::*;

// 固钉窗口的窗口过程
pub unsafe extern "system" fn pin_window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match msg {
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);

                // 获取窗口客户区大小
                let mut rect = RECT::default();
                let _ = GetClientRect(hwnd, &mut rect);

                // 获取存储的位图句柄
                let bitmap_handle = HBITMAP(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut _);

                if !bitmap_handle.0.is_null() {
                    // 创建兼容DC并选择位图
                    let mem_dc = CreateCompatibleDC(Some(hdc));
                    let old_bitmap = SelectObject(mem_dc, bitmap_handle.into());

                    // 绘制位图到窗口
                    let _ = BitBlt(
                        hdc,
                        0,
                        0,
                        rect.right - rect.left,
                        rect.bottom - rect.top,
                        Some(mem_dc),
                        0,
                        0,
                        SRCCOPY,
                    );

                    // 清理资源
                    SelectObject(mem_dc, old_bitmap);
                    let _ = DeleteDC(mem_dc);
                }

                let _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }

            WM_KEYDOWN => {
                if wparam.0 == VK_ESCAPE.0 as usize {
                    // ESC键关闭固钉窗口
                    let _ = DestroyWindow(hwnd);
                }
                LRESULT(0)
            }

            WM_DESTROY => {
                // 清理存储的位图
                let bitmap_handle = HBITMAP(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut _);
                if !bitmap_handle.0.is_null() {
                    let _ = DeleteObject(bitmap_handle.into());
                }
                LRESULT(0)
            }

            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match msg {
            WM_CREATE => match WindowState::new(hwnd) {
                Ok(mut state) => {
                    // 初始化系统托盘
                    if let Err(_e) = state.init_system_tray(hwnd) {
                        // 继续运行，不退出程序
                    }

                    // 启动异步OCR引擎状态检查
                    state.start_async_ocr_check(hwnd);

                    // 从设置中读取热键配置并注册全局热键
                    let settings = sc_windows::simple_settings::SimpleSettings::load();
                    let hotkey_id = 1001;
                    let _ = RegisterHotKey(
                        Some(hwnd),
                        hotkey_id,
                        HOT_KEY_MODIFIERS(settings.hotkey_modifiers),
                        settings.hotkey_key,
                    );

                    let state_box = Box::new(state);
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state_box) as isize);
                    LRESULT(0)
                }
                Err(_) => LRESULT(-1),
            },

            WM_DESTROY => {
                // 注销全局热键
                let _ = UnregisterHotKey(Some(hwnd), 1001);

                // 清理OCR引擎
                sc_windows::ocr::PaddleOcrEngine::cleanup_global_engine();

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
                let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
                if !state_ptr.is_null() {
                    let state = &mut *state_ptr;
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

            // 处理托盘消息
            val if val == WM_USER + 1 => {
                let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
                if !state_ptr.is_null() {
                    let state = &mut *state_ptr;
                    state.handle_tray_message(hwnd, wparam, lparam);
                }
                LRESULT(0)
            }

            // 处理 OCR 完成后关闭截图的消息
            val if val == WM_USER + 2 => {
                // 异步停止OCR引擎
                sc_windows::ocr::PaddleOcrEngine::stop_ocr_engine_async();

                // 隐藏截图窗口
                let _ = ShowWindow(hwnd, SW_HIDE);
                LRESULT(0)
            }

            // 处理设置更改消息
            val if val == WM_USER + 3 => {
                let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
                if !state_ptr.is_null() {
                    let state = &mut *state_ptr;
                    // 重新加载设置
                    state.reload_settings();
                    // 重新注册热键
                    let _ = state.reregister_hotkey(hwnd);
                }
                LRESULT(0)
            }

            // 处理OCR引擎状态更新消息
            val if val == WM_USER + 10 => {
                let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
                if !state_ptr.is_null() {
                    let state = &mut *state_ptr;
                    let available = wparam.0 != 0; // wparam为1表示可用，0表示不可用
                    state.update_ocr_engine_status(available, hwnd);
                }
                LRESULT(0)
            }

            // 处理全局热键消息
            WM_HOTKEY => {
                if wparam.0 == 1001 {
                    let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
                    if !state_ptr.is_null() {
                        let state = &mut *state_ptr;

                        // 如果窗口当前可见，先隐藏它
                        if IsWindowVisible(hwnd).as_bool() {
                            let _ = ShowWindow(hwnd, SW_HIDE);
                            std::thread::sleep(std::time::Duration::from_millis(50));
                        }

                        // 异步启动OCR引擎（不阻塞截图启动）
                        sc_windows::ocr::PaddleOcrEngine::start_ocr_engine_async();

                        // 重新检查OCR引擎状态
                        state.start_async_ocr_check(hwnd);

                        // 重置状态并截取屏幕
                        state.reset_to_initial_state();

                        // 确保窗口恢复到全屏状态
                        let screen_width = GetSystemMetrics(SM_CXSCREEN);
                        let screen_height = GetSystemMetrics(SM_CYSCREEN);

                        if state.capture_screen().is_ok() {
                            let _ = ShowWindow(hwnd, SW_SHOW);
                            let _ = SetForegroundWindow(hwnd);
                            let _ = SetWindowPos(
                                hwnd,
                                Some(HWND_TOPMOST),
                                0,
                                0,
                                screen_width,
                                screen_height,
                                SWP_SHOWWINDOW,
                            );
                            let _ = InvalidateRect(Some(hwnd), None, FALSE.into());
                            let _ = UpdateWindow(hwnd);
                        }
                    }
                }
                LRESULT(0)
            }

            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

fn main() -> Result<()> {
    unsafe {
        // 尝试设置DPI感知，如果失败也继续运行
        let _ = SetProcessDpiAwareness(PROCESS_PER_MONITOR_DPI_AWARE);
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
            WS_EX_TOOLWINDOW,
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

        // 启动时隐藏窗口，等待热键触发
        let _ = ShowWindow(hwnd, SW_HIDE);
        let _ = UpdateWindow(hwnd);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, Some(HWND(std::ptr::null_mut())), 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        Ok(())
    }
}
