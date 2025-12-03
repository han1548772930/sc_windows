#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]
use sc_windows::platform::windows::Direct2DRenderer;
use sc_windows::settings::Settings;
use sc_windows::utils::{to_wide_chars, win_api};
use sc_windows::{App, Command, CommandExecutor, WINDOW_CLASS_NAME};

use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{PROCESS_PER_MONITOR_DPI_AWARE, SetProcessDpiAwareness};
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::*;

unsafe fn get_app_state(hwnd: HWND) -> Option<&'static mut App> {
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App;
    if ptr.is_null() { None } else { Some(&mut *ptr) }
}

/// 处理命令的辅助函数
unsafe fn handle_commands(app: &mut App, commands: Vec<Command>, hwnd: HWND) {
    app.execute_command_chain(commands, hwnd);
}

/// 执行截图并显示窗口
unsafe fn perform_capture_and_show(hwnd: HWND, app: &mut App) {
    sc_windows::ocr::PaddleOcrEngine::start_ocr_engine_async();
    app.start_async_ocr_check(hwnd);
    app.reset_to_initial_state();
    let (screen_width, screen_height) = win_api::get_screen_size();

    if app.capture_screen_direct().is_ok() {
        let _ = app.create_d2d_bitmap_from_gdi();
        let _ = win_api::show_window(hwnd);
        let _ = win_api::set_window_topmost(
            hwnd,
            0,
            0,
            screen_width,
            screen_height,
        );
        let _ = win_api::request_redraw(hwnd);
        let _ = win_api::update_window(hwnd);
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

        let _ = win_api::hide_window(hwnd);
        let _ = win_api::update_window(hwnd);
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
                        if renderer
                            .initialize(hwnd, screen_width, screen_height)
                            .is_err()
                        {
                            return LRESULT(-1);
                        }

                        match App::new(Box::new(renderer)) {
                            Ok(mut app) => {
                                let _ = app.init_system_tray(hwnd);

                                app.start_async_ocr_check(hwnd);
                                let settings = Settings::load();
                                let hotkey_id = 1001;
                                let _ = RegisterHotKey(
                                    Some(hwnd),
                                    hotkey_id,
                                    HOT_KEY_MODIFIERS(settings.hotkey_modifiers),
                                    settings.hotkey_key,
                                );

                                let app_box = Box::new(app);
                                SetWindowLongPtrW(
                                    hwnd,
                                    GWLP_USERDATA,
                                    Box::into_raw(app_box) as isize,
                                );
                                LRESULT(0)
                            }
                            Err(_) => LRESULT(-1),
                        }
                    }
                    Err(_) => LRESULT(-1),
                }
            }

            WM_CLOSE => {
                let is_visible = win_api::is_window_visible(hwnd);

                if !is_visible {
                    let _ = win_api::destroy_window(hwnd);
                } else if let Some(app) = get_app_state(hwnd) {
                    app.reset_to_initial_state();
                    let _ = win_api::hide_window(hwnd);
                } else {
                    let _ = win_api::destroy_window(hwnd);
                }
                LRESULT(0)
            }

            WM_DESTROY => {
                let _ = UnregisterHotKey(Some(hwnd), 1001);
                sc_windows::ocr::PaddleOcrEngine::cleanup_global_engine();

                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App;
                if !ptr.is_null() {
                    let _ = Box::from_raw(ptr);
                }
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);

                win_api::quit_message_loop(0);
                LRESULT(0)
            }

            WM_PAINT => {
                if let Some(app) = get_app_state(hwnd) {
                    let _ = app.paint(hwnd);
                }
                LRESULT(0)
            }

            WM_MOUSEMOVE => {
                if let Some(app) = get_app_state(hwnd) {
                    let (x, y) = sc_windows::utils::extract_mouse_coords(lparam);
                    let commands = app.handle_mouse_move(x, y);
                    handle_commands(app, commands, hwnd);
                }
                LRESULT(0)
            }

            WM_LBUTTONDOWN => {
                if let Some(app) = get_app_state(hwnd) {
                    let (x, y) = sc_windows::utils::extract_mouse_coords(lparam);
                    let commands = app.handle_mouse_down(x, y);
                    handle_commands(app, commands, hwnd);
                }
                LRESULT(0)
            }

            WM_LBUTTONUP => {
                if let Some(app) = get_app_state(hwnd) {
                    let (x, y) = sc_windows::utils::extract_mouse_coords(lparam);
                    let commands = app.handle_mouse_up(x, y);
                    handle_commands(app, commands, hwnd);
                }
                LRESULT(0)
            }

            WM_LBUTTONDBLCLK => {
                if let Some(app) = get_app_state(hwnd) {
                    let (x, y) = sc_windows::utils::extract_mouse_coords(lparam);
                    let commands = app.handle_double_click(x, y);
                    handle_commands(app, commands, hwnd);
                }
                LRESULT(0)
            }

            WM_CHAR => {
                if let Some(app) = get_app_state(hwnd) {
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
                if let Some(app) = get_app_state(hwnd) {
                    let commands = app.handle_tray_message(wparam.0 as u32, lparam.0 as u32);
                    handle_commands(app, commands, hwnd);
                }
                LRESULT(0)
            }

            val if val == WM_USER + 2 => {
                if let Some(app) = get_app_state(hwnd) {
                    app.stop_ocr_engine_async();
                    let _ = win_api::hide_window(hwnd);
                }
                LRESULT(0)
            }

            val if val == WM_USER + 3 => {
                if let Some(app) = get_app_state(hwnd) {
                    let commands = app.reload_settings();
                    handle_commands(app, commands, hwnd);
                    let _ = app.reregister_hotkey(hwnd);
                }
                LRESULT(0)
            }

            val if val == WM_USER + 10 => {
                if let Some(app) = get_app_state(hwnd) {
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
            val if val == WM_USER + 11 => {
                if let Some(app) = get_app_state(hwnd) {
                    let data_ptr = lparam.0 as *mut sc_windows::system::ocr::OcrCompletionData;
                    if !data_ptr.is_null() {
                        let data = Box::from_raw(data_ptr);
                        
                        // Check if we have results
                        let has_results = !data.ocr_results.is_empty();
                        let is_ocr_failed = data.ocr_results.len() == 1 && data.ocr_results[0].text == "OCR识别失败";

                        // Show OCR Result Window
                        if let Err(e) = sc_windows::ocr_result_window::OcrResultWindow::show(
                            data.image_data,
                            data.ocr_results.clone(),
                            data.selection_rect,
                        ) {
                             eprintln!("Failed to show OCR result window: {:?}", e);
                        }

                        // Copy to clipboard
                        if has_results {
                            let text: String = data.ocr_results
                                .iter()
                                .map(|r| r.text.clone())
                                .collect::<Vec<_>>()
                                .join("\n");

                            if let Err(e) = sc_windows::screenshot::save::copy_text_to_clipboard(&text) {
                                eprintln!("Failed to copy OCR text to clipboard: {:?}", e);
                            }
                        }

                        // Message Box if no text
                        if !has_results || is_ocr_failed {
                             let message = "未识别到文本内容。\n\n请确保选择区域包含清晰的文字。";
                             let message_w: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();
                             let title_w: Vec<u16> = "OCR结果".encode_utf16().chain(std::iter::once(0)).collect();
                             
                             let _ = MessageBoxW(
                                Some(hwnd),
                                PCWSTR(message_w.as_ptr()),
                                PCWSTR(title_w.as_ptr()),
                                MB_OK | MB_ICONINFORMATION,
                            );
                        }
                        
                        // Cleanup and ensure main window is hidden
                        app.stop_ocr_engine_async();
                        let _ = win_api::hide_window(hwnd);
                    }
                }
                LRESULT(0)
            }
            WM_HOTKEY => {
                if wparam.0 == 1001 {
                    if let Some(app) = get_app_state(hwnd) {
                        if win_api::is_window_visible(hwnd) {
                            let _ = win_api::hide_window(hwnd);
                            // 使用定时器替代线程休眠，避免阻塞主线程
                            let _ = SetTimer(Some(hwnd), 2001, 50, None);
                        } else {
                            perform_capture_and_show(hwnd, app);
                        }
                    }
                }
                LRESULT(0)
            }

            WM_KEYDOWN => {
                if let Some(app) = get_app_state(hwnd) {
                    let commands = app.handle_key_input(wparam.0 as u32);
                    handle_commands(app, commands, hwnd);
                }
                LRESULT(0)
            }

            WM_TIMER => {
                if wparam.0 == 2001 {
                    // 截图延迟定时器触发
                    let _ = KillTimer(Some(hwnd), 2001);
                    if let Some(app) = get_app_state(hwnd) {
                        perform_capture_and_show(hwnd, app);
                    }
                    LRESULT(0)
                } else {
                    if let Some(app) = get_app_state(hwnd) {
                        let commands = app.handle_cursor_timer(wparam.0 as u32);
                        handle_commands(app, commands, hwnd);
                    }
                    LRESULT(0)
                }
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}
