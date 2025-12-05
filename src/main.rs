#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]
use sc_windows::constants::*;
use sc_windows::platform::windows::Direct2DRenderer;
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
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App;
        if ptr.is_null() { None } else { Some(&mut *ptr) }
    }
}

/// 处理命令的辅助函数
unsafe fn handle_commands(app: &mut App, commands: Vec<Command>, hwnd: HWND) {
    app.execute_command_chain(commands, hwnd);
}

/// 执行截图并显示窗口
unsafe fn perform_capture_and_show(hwnd: HWND, app: &mut App) {
    // 使用带通知的版本，引擎启动完成后会发送状态更新消息
    sc_windows::ocr::PaddleOcrEngine::start_ocr_engine_async_with_hwnd(hwnd);
    app.reset_to_initial_state();

    // 使用 App 中缓存的屏幕尺寸，避免重复调用系统API
    let (screen_width, screen_height) = app.get_screen_size();

    if app.capture_screen_direct().is_ok() {
        let _ = app.create_d2d_bitmap_from_gdi();
        let _ = win_api::show_window(hwnd);
        let _ = win_api::set_window_topmost(hwnd, 0, 0, screen_width, screen_height);
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
            // WM_CREATE 和 WM_DESTROY 需要特殊处理（创建/销毁 App 本身）
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
                            // 显示错误提示而不是静默失败
                            let msg: Vec<u16> = "图形引擎初始化失败，请检查显卡驱动是否正常。"
                                .encode_utf16().chain(std::iter::once(0)).collect();
                            let title: Vec<u16> = "启动错误"
                                .encode_utf16().chain(std::iter::once(0)).collect();
                            let _ = MessageBoxW(
                                Some(hwnd),
                                PCWSTR(msg.as_ptr()),
                                PCWSTR(title.as_ptr()),
                                MB_OK | MB_ICONERROR,
                            );
                            return LRESULT(-1);
                        }

                        match App::new(renderer) {
                            Ok(mut app) => {
                                let _ = app.init_system_tray(hwnd);
                                app.start_async_ocr_check(hwnd);
                                // 从 ConfigManager 获取热键配置
                                let (hotkey_modifiers, hotkey_key) = app.config().hotkey();
                                let _ = RegisterHotKey(
                                    Some(hwnd),
                                    HOTKEY_SCREENSHOT_ID,
                                    HOT_KEY_MODIFIERS(hotkey_modifiers),
                                    hotkey_key,
                                );

                                let app_box = Box::new(app);
                                SetWindowLongPtrW(
                                    hwnd,
                                    GWLP_USERDATA,
                                    Box::into_raw(app_box) as isize,
                                );
                                LRESULT(0)
                            }
                            Err(e) => {
                                // 显示应用初始化错误
                                let msg: Vec<u16> = format!("应用初始化失败: {:?}", e)
                                    .encode_utf16().chain(std::iter::once(0)).collect();
                                let title: Vec<u16> = "启动错误"
                                    .encode_utf16().chain(std::iter::once(0)).collect();
                                let _ = MessageBoxW(
                                    Some(hwnd),
                                    PCWSTR(msg.as_ptr()),
                                    PCWSTR(title.as_ptr()),
                                    MB_OK | MB_ICONERROR,
                                );
                                LRESULT(-1)
                            }
                        }
                    }
                    Err(e) => {
                        // 显示 D2D 创建错误
                        let msg: Vec<u16> = format!("图形引擎创建失败: {:?}\n\n请检查显卡驱动是否正常安装。", e)
                            .encode_utf16().chain(std::iter::once(0)).collect();
                        let title: Vec<u16> = "启动错误"
                            .encode_utf16().chain(std::iter::once(0)).collect();
                        let _ = MessageBoxW(
                            Some(hwnd),
                            PCWSTR(msg.as_ptr()),
                            PCWSTR(title.as_ptr()),
                            MB_OK | MB_ICONERROR,
                        );
                        LRESULT(-1)
                    }
                }
            }

            WM_DESTROY => {
                let _ = UnregisterHotKey(Some(hwnd), HOTKEY_SCREENSHOT_ID);
                sc_windows::ocr::PaddleOcrEngine::cleanup_global_engine();

                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App;
                if !ptr.is_null() {
                    let _ = Box::from_raw(ptr);
                }
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);

                win_api::quit_message_loop(0);
                LRESULT(0)
            }

            // OCR 完成、热键、定时器等需要特殊处理的消息
            val if val == WM_OCR_COMPLETED => {
                if let Some(app) = get_app_state(hwnd) {
                    let data_ptr = lparam.0 as *mut sc_windows::ocr::OcrCompletionData;
                    if !data_ptr.is_null() {
                        let data = Box::from_raw(data_ptr);

                        let (has_results, is_ocr_failed, text) = app.handle_ocr_completed(
                            data.ocr_results,
                            data.image_data,
                            data.selection_rect,
                        );

                        if has_results
                            && let Err(e) =
                                sc_windows::screenshot::save::copy_text_to_clipboard(&text)
                        {
                            eprintln!("Failed to copy OCR text to clipboard: {:?}", e);
                        }

                        if !has_results || is_ocr_failed {
                            let message = "未识别到文本内容。\n\n请确保选择区域包含清晰的文字。";
                            let message_w: Vec<u16> =
                                message.encode_utf16().chain(std::iter::once(0)).collect();
                            let title_w: Vec<u16> =
                                "OCR结果".encode_utf16().chain(std::iter::once(0)).collect();
                            let _ = MessageBoxW(
                                Some(hwnd),
                                PCWSTR(message_w.as_ptr()),
                                PCWSTR(title_w.as_ptr()),
                                MB_OK | MB_ICONINFORMATION,
                            );
                        }

                        app.stop_ocr_engine_async();
                        let _ = win_api::hide_window(hwnd);
                    }
                }
                LRESULT(0)
            }

            WM_HOTKEY => {
                if wparam.0 == HOTKEY_SCREENSHOT_ID as usize
                    && let Some(app) = get_app_state(hwnd)
                {
                    if win_api::is_window_visible(hwnd) {
                        let _ = win_api::hide_window(hwnd);
                        let _ = SetTimer(
                            Some(hwnd),
                            TIMER_CAPTURE_DELAY_ID,
                            TIMER_CAPTURE_DELAY_MS,
                            None,
                        );
                    } else {
                        perform_capture_and_show(hwnd, app);
                    }
                }
                LRESULT(0)
            }

            WM_TIMER => {
                if wparam.0 == TIMER_CAPTURE_DELAY_ID {
                    let _ = KillTimer(Some(hwnd), TIMER_CAPTURE_DELAY_ID);
                    if let Some(app) = get_app_state(hwnd) {
                        perform_capture_and_show(hwnd, app);
                    }
                } else if let Some(app) = get_app_state(hwnd) {
                    let commands = app.handle_cursor_timer(wparam.0 as u32);
                    handle_commands(app, commands, hwnd);
                }
                LRESULT(0)
            }

            // 其他消息委托给 App.handle_window_message()
            _ => {
                if let Some(app) = get_app_state(hwnd) {
                    if let Some(result) = app.handle_window_message(hwnd, msg, wparam, lparam) {
                        return result;
                    }
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
        }
    }
}
