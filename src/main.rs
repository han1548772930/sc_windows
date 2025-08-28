// SC Windows - 新架构主程序
//
// 使用重构后的模块化架构，严格按照原始代码逻辑
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

// 全局应用实例（按照原始代码的WindowState模式）
static mut APP: Option<App> = None;

/// 处理命令的辅助函数（简化版，使用App::execute_command）
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
        // 尝试设置DPI感知，如果失败也继续运行（与原始程序一致）
        let _ = SetProcessDpiAwareness(PROCESS_PER_MONITOR_DPI_AWARE);

        let instance = GetModuleHandleW(None)?;
        let class_name = to_wide_chars(WINDOW_CLASS_NAME);

        // 注册窗口类（与原始程序完全一致）
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

        // 获取屏幕尺寸（集中到platform::windows::system）
        let (screen_width, screen_height) =
            sc_windows::platform::windows::system::get_screen_size();

        // 创建窗口（与原始程序完全一致）
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

        // 启动时隐藏窗口，等待热键触发（与原始程序一致）
        let _ = ShowWindow(hwnd, SW_HIDE);
        let _ = UpdateWindow(hwnd);

        // 消息循环（与原始程序一致）
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
            // WM_CREATE - 初始化应用程序（按照原始代码逻辑）
            WM_CREATE => {
                // 初始化COM
                let _ = windows::Win32::System::Com::CoInitialize(None);

                // 获取屏幕尺寸（集中到platform::windows::system）
                let (screen_width, screen_height) =
                    sc_windows::platform::windows::system::get_screen_size();

                // 创建并初始化渲染器
                match Direct2DRenderer::new() {
                    Ok(mut renderer) => {
                        // 初始化Direct2D资源
                        if let Err(e) = renderer.initialize(hwnd, screen_width, screen_height) {
                            eprintln!("Failed to initialize renderer: {}", e);
                            return LRESULT(-1);
                        }

                        // 创建应用程序实例
                        match App::new(Box::new(renderer)) {
                            Ok(mut app) => {
                                // 初始化系统托盘（从原始代码迁移）
                                if let Err(e) = app.init_system_tray(hwnd) {
                                    eprintln!("Failed to initialize system tray: {}", e);
                                    // 继续运行，不退出程序
                                }

                                // 启动异步OCR引擎状态检查（从原始代码迁移）
                                app.start_async_ocr_check(hwnd);

                                // 从设置中读取热键配置并注册全局热键
                                let settings = Settings::load();
                                let hotkey_id = 1001;
                                let _ = RegisterHotKey(
                                    Some(hwnd),
                                    hotkey_id,
                                    HOT_KEY_MODIFIERS(settings.hotkey_modifiers),
                                    settings.hotkey_key,
                                );

                                // 存储应用程序实例
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
                // 处理窗口关闭请求
                // 检查是否是托盘退出请求（通过检查窗口是否可见来判断）
                let is_visible = IsWindowVisible(hwnd).as_bool();

                if !is_visible {
                    // 窗口不可见，说明是托盘退出请求，真正退出程序
                    let _ = DestroyWindow(hwnd);
                } else {
                    // 窗口可见，说明是用户点击X按钮，只隐藏窗口
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
                // 注销全局热键
                let _ = UnregisterHotKey(Some(hwnd), 1001);

                // 清理OCR引擎（从原始代码迁移）
                sc_windows::ocr::PaddleOcrEngine::cleanup_global_engine();

                // 清理应用程序实例
                APP = None;
                PostQuitMessage(0);
                LRESULT(0)
            }

            // WM_PAINT - 渲染（按照原始代码逻辑）
            WM_PAINT => {
                if let Some(ref mut app) = APP {
                    let _ = app.paint(hwnd);
                }
                LRESULT(0)
            }

            // 鼠标事件处理（从原始代码迁移）
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
                    // 双击处理：按照原始代码逻辑，双击可能用于确认选择或进入编辑模式
                    let commands = app.handle_double_click(x, y);
                    handle_commands(app, commands, hwnd);
                }
                LRESULT(0)
            }

            WM_CHAR => {
                if let Some(ref mut app) = APP {
                    // 正确处理Unicode字符，支持中文输入（按照原始代码逻辑）
                    if let Some(character) = char::from_u32(wparam.0 as u32) {
                        // 允许所有可打印字符，包括中文和其他Unicode字符
                        // 排除控制字符（除了空格和制表符）
                        if !character.is_control() || character == ' ' || character == '\t' {
                            let commands = app.handle_text_input(character);
                            handle_commands(app, commands, hwnd);
                        }
                    }
                }
                LRESULT(0)
            }

            WM_SETCURSOR => {
                // 让我们自己处理光标
                LRESULT(1)
            }

            // 处理托盘消息（从原始代码迁移）
            val if val == WM_USER + 1 => {
                if let Some(ref mut app) = APP {
                    let commands = app.handle_tray_message(wparam.0 as u32, lparam.0 as u32);
                    handle_commands(app, commands, hwnd);
                }
                LRESULT(0)
            }

            // 处理 OCR 完成后关闭截图的消息（从原始代码迁移）
            val if val == WM_USER + 2 => {
                if let Some(ref mut app) = APP {
                    // 异步停止OCR引擎
                    app.stop_ocr_engine_async();

                    // 隐藏截图窗口
                    let _ = ShowWindow(hwnd, SW_HIDE);
                }
                LRESULT(0)
            }

            // 处理设置更改消息（从原始代码迁移）
            val if val == WM_USER + 3 => {
                if let Some(ref mut app) = APP {
                    // 重新加载设置并处理返回的命令
                    let commands = app.reload_settings();
                    handle_commands(app, commands, hwnd);

                    // 重新注册热键
                    let _ = app.reregister_hotkey(hwnd);
                }
                LRESULT(0)
            }

            // 处理OCR引擎状态更新消息（从原始代码迁移）
            val if val == WM_USER + 10 => {
                if let Some(ref mut app) = APP {
                    let available = wparam.0 != 0; // wparam为1表示可用，0表示不可用
                    app.update_ocr_engine_status(available, hwnd);
                    // 引擎状态变化时同步刷新工具栏，使 OCR 按钮即时启用/禁用
                    let commands = vec![
                        sc_windows::message::Command::UpdateToolbar,
                        sc_windows::message::Command::RequestRedraw,
                    ];
                    handle_commands(app, commands, hwnd);
                }
                LRESULT(0)
            }
            // 处理全局热键消息（完全按照原始程序逻辑）
            WM_HOTKEY => {
                if wparam.0 == 1001 {
                    if let Some(ref mut app) = APP {
                        // 如果窗口当前可见，先隐藏它（与原始代码一致）
                        if IsWindowVisible(hwnd).as_bool() {
                            let _ = ShowWindow(hwnd, SW_HIDE);
                            std::thread::sleep(std::time::Duration::from_millis(50));
                        }

                        // 异步启动OCR引擎（不阻塞截图启动）
                        // 使用原始代码的异步启动逻辑
                        sc_windows::ocr::PaddleOcrEngine::start_ocr_engine_async();

                        // 重新检查OCR引擎状态
                        app.start_async_ocr_check(hwnd);

                        // 重置状态并截取屏幕（与原始代码一致）
                        app.reset_to_initial_state();

                        // 确保窗口恢复到全屏状态
                        let screen_width = GetSystemMetrics(SM_CXSCREEN);
                        let screen_height = GetSystemMetrics(SM_CYSCREEN);

                        // 直接调用截图功能（与原始代码一致）
                        if app.capture_screen_direct().is_ok() {
                            // 关键修复：立即创建D2D位图用于渲染
                            // 这确保了截图背景能够正确显示
                            if let Err(e) = app.create_d2d_bitmap_from_gdi() {
                                eprintln!("Failed to create D2D bitmap: {:?}", e);
                                // 即使D2D位图创建失败，也继续显示窗口
                                // 这样至少用户可以看到透明窗口进行操作
                            }

                            // 显示窗口并设置为全屏和最顶层（与原始代码完全一致）
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

                            // 强制重绘窗口以显示截图背景
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
                    // 处理光标闪烁定时器（从原始代码迁移）
                    let commands = app.handle_cursor_timer(wparam.0 as u32);
                    handle_commands(app, commands, hwnd);
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}
