#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]
use sc_windows::platform::windows::Direct2DRenderer;
use sc_windows::settings::Settings;
use sc_windows::utils::to_wide_chars;
use sc_windows::{App, Command, WINDOW_CLASS_NAME};

use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{PROCESS_PER_MONITOR_DPI_AWARE, SetProcessDpiAwareness};
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::*;

static mut APP: Option<App> = None;

/// 处理命令的辅助函数
unsafe fn handle_commands(app: &mut App, commands: Vec<Command>, hwnd: HWND) {
    let mut pending_commands = commands;

    // 处理所有命令，包括新产生的命令
    while !pending_commands.is_empty() {
        let mut new_commands = Vec::new();

        for command in pending_commands {
            let result_commands = app.execute_command(command, hwnd);
            new_commands.extend(result_commands);
        }

        pending_commands = new_commands;
    }
}

fn main() -> Result<()> {
    unsafe {
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

        let (screen_width, screen_height) =
            sc_windows::platform::windows::system::get_screen_size();
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

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match msg {
            WM_CREATE => {
                let _ = windows::Win32::System::Com::CoInitialize(None);

                let (screen_width, screen_height) =
                    sc_windows::platform::windows::system::get_screen_size();
                match Direct2DRenderer::new() {
                    Ok(mut renderer) => {
                        if let Err(e) = renderer.initialize(hwnd, screen_width, screen_height) {
                            eprintln!("Failed to initialize renderer: {}", e);
                            return LRESULT(-1);
                        }

                        match App::new(Box::new(renderer)) {
                            Ok(mut app) => {
                                if let Err(e) = app.init_system_tray(hwnd) {
                                    eprintln!("Failed to initialize system tray: {}", e);
                                }

                                app.start_async_ocr_check(hwnd);
                                let settings = Settings::load();
                                let hotkey_id = 1001;
                                let _ = RegisterHotKey(
                                    Some(hwnd),
                                    hotkey_id,
                                    HOT_KEY_MODIFIERS(settings.hotkey_modifiers),
                                    settings.hotkey_key,
                                );

                                APP = Some(app);
                                LRESULT(0)
                            }
                            Err(e) => {
                                eprintln!("Failed to create app: {}", e);
                                LRESULT(-1)
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to create renderer: {}", e);
                        LRESULT(-1)
                    }
                }
            }

            WM_CLOSE => {
                let is_visible = IsWindowVisible(hwnd).as_bool();

                if !is_visible {
                    let _ = DestroyWindow(hwnd);
                } else {
                    if let Some(ref mut app) = APP {
                        app.reset_to_initial_state();
                        let _ = ShowWindow(hwnd, SW_HIDE);
                    } else {
                        let _ = DestroyWindow(hwnd);
                    }
                }
                LRESULT(0)
            }

            WM_DESTROY => {
                let _ = UnregisterHotKey(Some(hwnd), 1001);
                sc_windows::ocr::PaddleOcrEngine::cleanup_global_engine();
                APP = None;
                PostQuitMessage(0);
                LRESULT(0)
            }

            WM_PAINT => {
                if let Some(ref mut app) = APP {
                    let _ = app.paint(hwnd);
                }
                LRESULT(0)
            }

            WM_MOUSEMOVE => {
                if let Some(ref mut app) = APP {
                    let x = (lparam.0 & 0xFFFF) as i16 as i32;
                    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
                    let commands = app.handle_mouse_move(x, y);
                    handle_commands(app, commands, hwnd);
                }
                LRESULT(0)
            }

            WM_LBUTTONDOWN => {
                if let Some(ref mut app) = APP {
                    let x = (lparam.0 & 0xFFFF) as i16 as i32;
                    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
                    let commands = app.handle_mouse_down(x, y);
                    handle_commands(app, commands, hwnd);
                }
                LRESULT(0)
            }

            WM_LBUTTONUP => {
                if let Some(ref mut app) = APP {
                    let x = (lparam.0 & 0xFFFF) as i16 as i32;
                    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
                    let commands = app.handle_mouse_up(x, y);
                    handle_commands(app, commands, hwnd);
                }
                LRESULT(0)
            }

            WM_LBUTTONDBLCLK => {
                if let Some(ref mut app) = APP {
                    let x = (lparam.0 & 0xFFFF) as i16 as i32;
                    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
                    let commands = app.handle_double_click(x, y);
                    handle_commands(app, commands, hwnd);
                }
                LRESULT(0)
            }

            WM_CHAR => {
                if let Some(ref mut app) = APP {
                    if let Some(character) = char::from_u32(wparam.0 as u32) {
                        if !character.is_control() || character == ' ' || character == '\t' {
                            let commands = app.handle_text_input(character);
                            handle_commands(app, commands, hwnd);
                        }
                    }
                }
                LRESULT(0)
            }

            WM_SETCURSOR => LRESULT(1),

            val if val == WM_USER + 1 => {
                if let Some(ref mut app) = APP {
                    let commands = app.handle_tray_message(wparam.0 as u32, lparam.0 as u32);
                    handle_commands(app, commands, hwnd);
                }
                LRESULT(0)
            }

            val if val == WM_USER + 2 => {
                if let Some(ref mut app) = APP {
                    app.stop_ocr_engine_async();
                    let _ = ShowWindow(hwnd, SW_HIDE);
                }
                LRESULT(0)
            }

            val if val == WM_USER + 3 => {
                if let Some(ref mut app) = APP {
                    let commands = app.reload_settings();
                    handle_commands(app, commands, hwnd);
                    let _ = app.reregister_hotkey(hwnd);
                }
                LRESULT(0)
            }

            val if val == WM_USER + 10 => {
                if let Some(ref mut app) = APP {
                    let available = wparam.0 != 0;
                    app.update_ocr_engine_status(available, hwnd);
                    let commands = vec![
                        sc_windows::message::Command::UpdateToolbar,
                        sc_windows::message::Command::RequestRedraw,
                    ];
                    handle_commands(app, commands, hwnd);
                }
                LRESULT(0)
            }
            WM_HOTKEY => {
                if wparam.0 == 1001 {
                    if let Some(ref mut app) = APP {
                        if IsWindowVisible(hwnd).as_bool() {
                            let _ = ShowWindow(hwnd, SW_HIDE);
                            std::thread::sleep(std::time::Duration::from_millis(50));
                        }

                        sc_windows::ocr::PaddleOcrEngine::start_ocr_engine_async();
                        app.start_async_ocr_check(hwnd);
                        app.reset_to_initial_state();
                        let screen_width = GetSystemMetrics(SM_CXSCREEN);
                        let screen_height = GetSystemMetrics(SM_CYSCREEN);

                        if app.capture_screen_direct().is_ok() {
                            if let Err(e) = app.create_d2d_bitmap_from_gdi() {
                                eprintln!("Failed to create D2D bitmap: {:?}", e);
                            }
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
                        } else {
                            eprintln!("Failed to capture screen");
                        }
                    }
                }
                LRESULT(0)
            }

            WM_KEYDOWN => {
                if let Some(ref mut app) = APP {
                    let commands = app.handle_key_input(wparam.0 as u32);
                    handle_commands(app, commands, hwnd);
                }
                LRESULT(0)
            }

            WM_TIMER => {
                if let Some(ref mut app) = APP {
                    let commands = app.handle_cursor_timer(wparam.0 as u32);
                    handle_commands(app, commands, hwnd);
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}
