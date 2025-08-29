// SC Windows - Improved Main Entry Point
//
// Uses safe state management instead of unsafe global static
#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use sc_windows::platform::windows::Direct2DRenderer;
use sc_windows::settings::Settings;
use sc_windows::state::{initialize_app, with_app};
use sc_windows::utils::to_wide_chars;
use sc_windows::{App, Command, WINDOW_CLASS_NAME};
use sc_windows::error::AppResult;

use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{SetProcessDpiAwareness, PROCESS_PER_MONITOR_DPI_AWARE};
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::*;

fn main() -> AppResult<()> {
    unsafe {
        // Set DPI awareness (continue even if it fails)
        let _ = SetProcessDpiAwareness(PROCESS_PER_MONITOR_DPI_AWARE);

        let instance = GetModuleHandleW(None)?;
        let class_name = to_wide_chars(WINDOW_CLASS_NAME);

        // Register window class
        let window_class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance.into(),
            lpszClassName: PCWSTR(class_name.as_ptr()),
            hbrBackground: HBRUSH(std::ptr::null_mut()),
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            style: CS_DBLCLKS | CS_OWNDC | CS_HREDRAW,
            ..Default::default()
        };

        RegisterClassW(&window_class);

        // Get screen dimensions
        let (screen_width, screen_height) = 
            sc_windows::platform::windows::system::get_screen_size();

        // Create main window
        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW,
            PCWSTR(class_name.as_ptr()),
            PCWSTR::null(),
            WS_POPUP,
            0,
            0,
            screen_width,
            screen_height,
            None,
            None,
            Some(instance.into()),
            None,
        )?;

        // Initially hide the window
        let _ = ShowWindow(hwnd, SW_HIDE);
        let _ = UpdateWindow(hwnd);

        // Message loop
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        Ok(())
    }
}

/// Safe command handler that uses the state management system
fn handle_commands_safe(commands: Vec<Command>, hwnd: HWND) -> AppResult<()> {
    let mut pending_commands = commands;
    
    while !pending_commands.is_empty() {
        let mut new_commands = Vec::new();
        
        for command in pending_commands {
            let result_commands = with_app(|app| app.execute_command(command, hwnd))?;
            new_commands.extend(result_commands);
        }
        
        pending_commands = new_commands;
    }
    
    Ok(())
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => {
            // Initialize COM
            let _ = windows::Win32::System::Com::CoInitialize(None);
            
            // Get screen dimensions
            let (screen_width, screen_height) = 
                sc_windows::platform::windows::system::get_screen_size();
            
            // Create and initialize renderer
            match Direct2DRenderer::new() {
                Ok(mut renderer) => {
                    if let Err(e) = renderer.initialize(hwnd, screen_width, screen_height) {
                        eprintln!("Failed to initialize renderer: {}", e);
                        return LRESULT(-1);
                    }
                    
                    // Create application instance
                    match App::new(Box::new(renderer)) {
                        Ok(mut app) => {
                            // Initialize system tray
                            if let Err(e) = app.init_system_tray(hwnd) {
                                eprintln!("Failed to initialize system tray: {}", e);
                            }
                            
                            // Start async OCR engine check
                            app.start_async_ocr_check(hwnd);
                            
                            // Register hotkey
                            let settings = Settings::load();
                            let hotkey_id = 1001;
                            let _ = RegisterHotKey(
                                Some(hwnd),
                                hotkey_id,
                                HOT_KEY_MODIFIERS(settings.hotkey_modifiers),
                                settings.hotkey_key,
                            );
                            
                            // Initialize the global app state
                            if let Err(e) = initialize_app(app) {
                                eprintln!("Failed to initialize app state: {}", e);
                                return LRESULT(-1);
                            }
                            
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
                // Window not visible = tray exit request
                let _ = DestroyWindow(hwnd);
            } else {
                // Window visible = user clicked X, just hide
                let _ = with_app(|app| {
                    app.reset_to_initial_state();
                });
                let _ = ShowWindow(hwnd, SW_HIDE);
            }
            LRESULT(0)
        }
        
        WM_DESTROY => {
            // Unregister hotkey
            let _ = UnregisterHotKey(Some(hwnd), 1001);
            
            // Cleanup OCR engine
            sc_windows::ocr::PaddleOcrEngine::cleanup_global_engine();
            
            // State will be cleaned up automatically when the process exits
            PostQuitMessage(0);
            LRESULT(0)
        }
        
        WM_PAINT => {
            let _ = with_app(|app| app.paint(hwnd));
            LRESULT(0)
        }
        
        WM_HOTKEY => {
            if wparam.0 as i32 == 1001 {
                // Handle hotkey press
                let _ = handle_commands_safe(vec![Command::TakeScreenshot], hwnd);
            }
            LRESULT(0)
        }
        
        WM_LBUTTONDOWN => {
            let x = (lparam.0 & 0xFFFF) as i32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as i32;
            
            let _ = with_app(|app| {
                let commands = app.handle_left_button_down(x, y);
                handle_commands_safe(commands, hwnd)
            });
            LRESULT(0)
        }
        
        WM_LBUTTONUP => {
            let x = (lparam.0 & 0xFFFF) as i32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as i32;
            
            let _ = with_app(|app| {
                let commands = app.handle_left_button_up(x, y);
                handle_commands_safe(commands, hwnd)
            });
            LRESULT(0)
        }
        
        WM_MOUSEMOVE => {
            let x = (lparam.0 & 0xFFFF) as i32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as i32;
            
            let _ = with_app(|app| {
                let commands = app.handle_mouse_move(x, y);
                handle_commands_safe(commands, hwnd)
            });
            LRESULT(0)
        }
        
        WM_RBUTTONDOWN => {
            let x = (lparam.0 & 0xFFFF) as i32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as i32;
            
            let _ = with_app(|app| {
                let commands = app.handle_right_button_down(x, y);
                handle_commands_safe(commands, hwnd)
            });
            LRESULT(0)
        }
        
        WM_KEYDOWN => {
            let key_code = wparam.0 as u32;
            
            let _ = with_app(|app| {
                let commands = app.handle_key_down(key_code);
                handle_commands_safe(commands, hwnd)
            });
            LRESULT(0)
        }
        
        WM_CHAR => {
            let char_code = wparam.0 as u32;
            
            if char_code >= 32 && char_code != 127 {
                if let Some(ch) = char::from_u32(char_code) {
                    let _ = with_app(|app| {
                        let commands = app.handle_char_input(ch);
                        handle_commands_safe(commands, hwnd)
                    });
                }
            }
            LRESULT(0)
        }
        
        WM_TIMER => {
            let timer_id = wparam.0 as u32;
            
            let _ = with_app(|app| {
                let commands = app.handle_timer(timer_id);
                handle_commands_safe(commands, hwnd)
            });
            LRESULT(0)
        }
        
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
