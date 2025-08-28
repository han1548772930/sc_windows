#![windows_subsystem = "windows"]

use windows::{
    Win32::{
        Foundation::*,
        Graphics::{
            Dwm::{
                DWM_WINDOW_CORNER_PREFERENCE, DWMWA_CAPTION_BUTTON_BOUNDS,
                DWMWA_TRANSITIONS_FORCEDISABLED, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
                DwmDefWindowProc, DwmGetWindowAttribute, DwmSetWindowAttribute,
            },
            Gdi::*,
        },
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            Controls::*,
            Input::KeyboardAndMouse::{TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent},
            WindowsAndMessaging::*,
        },
    },
    core::*,
};

// --- 常量定义 ---
const TITLE_BAR_HEIGHT: i32 = 40;
const BUTTON_WIDTH: i32 = 46;
const BUTTON_HEIGHT: i32 = TITLE_BAR_HEIGHT;
const INPUT_WIDTH: i32 = 300;
const INPUT_HEIGHT: i32 = 24;
const INPUT_ID: isize = 101;
const PANEL_ID: isize = 100;

// --- 按钮状态枚举 ---
#[derive(Copy, Clone, PartialEq, Debug)]
enum ButtonState {
    Normal,
    Hover,
    Pressed,
}

// --- 应用程序全局状态 ---
struct AppState {
    main_hwnd: HWND,
    panel_hwnd: HWND,
    input_hwnd: HWND,
    theme: HTHEME,
    min_button_state: ButtonState,
    max_button_state: ButtonState,
    close_button_state: ButtonState,
    min_button_rect: RECT,
    max_button_rect: RECT,
    close_button_rect: RECT,
    mouse_on_panel: bool,
    // ★★★ 新增：跟踪窗口的最大化状态 ★★★
    is_maximized: bool,
}

// --- Main 函数 ---
fn main() -> Result<()> {
    unsafe {
        let icc = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_STANDARD_CLASSES,
        };
        InitCommonControlsEx(&icc);

        let instance = GetModuleHandleW(None)?;
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW | CS_DROPSHADOW,
            lpfnWndProc: Some(main_wndproc),
            hInstance: instance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            hbrBackground: HBRUSH(std::ptr::null_mut()),
            lpszClassName: w!("RustNativeTitlebarFinal"),
            ..Default::default()
        };
        RegisterClassW(&wc);

        // 使用无框自绘标题栏：保留系统动画所需的最小化/最大化/系统菜单样式
        let style = WS_POPUP
            | WS_THICKFRAME
            | WS_MINIMIZEBOX
            | WS_MAXIMIZEBOX
            | WS_SYSMENU
            | WS_VISIBLE
            | WS_CLIPCHILDREN;

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            wc.lpszClassName,
            w!("Rust Native Titlebar"),
            style, // 使用修正后的样式
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            900,
            600,
            None,
            None,
            Some(instance.into()),
            None,
        )?;

        ShowWindow(hwnd, SW_SHOW);

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    Ok(())
}

// --- 主窗口过程 ---
extern "system" fn main_wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_CREATE => {
            unsafe {
                let preference = DWMWCP_ROUND;
                DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_WINDOW_CORNER_PREFERENCE,
                    &preference as *const _ as *const _,
                    std::mem::size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
                )
                .unwrap();

                // 启用窗口过渡动画（确保最大化/还原有系统动画）。FALSE 表示不禁用（启用）
                let enable_transitions = BOOL(0);
                let _ = DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_TRANSITIONS_FORCEDISABLED,
                    &enable_transitions as *const _ as *const _,
                    std::mem::size_of::<BOOL>() as u32,
                );
            }

            let instance = unsafe { GetModuleHandleW(None).unwrap() };
            let mut rect = RECT::default();
            unsafe { GetClientRect(hwnd, &mut rect) };
            let width = rect.right - rect.left;

            let panel_hwnd = unsafe {
                CreateWindowExW(
                    WINDOW_EX_STYLE::default(),
                    w!("STATIC"),
                    w!(""),
                    WS_CHILD | WS_VISIBLE,
                    0,
                    0,
                    width,
                    TITLE_BAR_HEIGHT,
                    Some(hwnd),
                    Some(HMENU(PANEL_ID as *mut _)),
                    Some(instance.into()),
                    None,
                )
                .expect("Failed to create panel")
            };

            let input_x = (width - INPUT_WIDTH) / 2;
            let input_y = (TITLE_BAR_HEIGHT - INPUT_HEIGHT) / 2;
            let edit_style =
                WINDOW_STYLE((WS_CHILD | WS_VISIBLE | WS_BORDER).0 | (ES_CENTER) as u32);
            let input_hwnd = unsafe {
                CreateWindowExW(
                    WINDOW_EX_STYLE::default(),
                    w!("EDIT"),
                    w!("Type here..."),
                    edit_style,
                    input_x,
                    input_y,
                    INPUT_WIDTH,
                    INPUT_HEIGHT,
                    Some(panel_hwnd),
                    Some(HMENU(INPUT_ID as *mut _)),
                    Some(instance.into()),
                    None,
                )
                .expect("Failed to create edit control")
            };

            let app_state = Box::new(AppState {
                main_hwnd: hwnd,
                panel_hwnd,
                input_hwnd,
                theme: unsafe { OpenThemeData(Some(hwnd), w!("Window")) },
                min_button_state: ButtonState::Normal,
                max_button_state: ButtonState::Normal,
                close_button_state: ButtonState::Normal,
                min_button_rect: RECT::default(),
                max_button_rect: RECT::default(),
                close_button_rect: RECT::default(),
                mouse_on_panel: false,
                is_maximized: false, // 初始状态为非最大化
            });

            unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(app_state) as isize) };
            unsafe { SetWindowLongPtrW(panel_hwnd, GWLP_WNDPROC, panel_wndproc as isize) };
            unsafe { SendMessageW(hwnd, WM_SIZE, Some(WPARAM(0)), Some(lparam)) };
            LRESULT(0)
        }
        // 自绘无框：告诉系统整个区域是客户区，从而隐藏原生标题栏与边框
        WM_NCCALCSIZE => {
            if wparam.0 == 1 {
                return LRESULT(0);
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        // ★★★ 关键修复 3：处理 WM_GETMINMAXINFO 来实现正确的最大化行为 ★★★
        WM_GETMINMAXINFO => {
            unsafe {
                let mmi = &mut *(lparam.0 as *mut MINMAXINFO);
                let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
                let mut monitor_info = MONITORINFO {
                    cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                    ..Default::default()
                };
                GetMonitorInfoW(monitor, &mut monitor_info);

                // 设置最大化时的位置和大小为监视器的工作区 (不包括任务栏)
                mmi.ptMaxPosition = POINT {
                    x: monitor_info.rcWork.left,
                    y: monitor_info.rcWork.top,
                };
                mmi.ptMaxSize.x = monitor_info.rcWork.right - monitor_info.rcWork.left;
                mmi.ptMaxSize.y = monitor_info.rcWork.bottom - monitor_info.rcWork.top;
            }
            LRESULT(0)
        }
        // 自绘无框：拦截非客户区绘制/激活，避免系统重绘
        WM_NCPAINT => LRESULT(0),
        WM_NCACTIVATE => LRESULT(1),
        WM_SIZE => {
            let app_state_ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppState };
            if let Some(app) = unsafe { app_state_ptr.as_mut() } {
                // ★★★ 新增：根据 wparam 更新最大化状态 ★★★
                app.is_maximized = wparam.0 == SIZE_MAXIMIZED as usize;

                let width = LOWORD(lparam.0 as u32) as i32;
                unsafe {
                    SetWindowPos(
                        app.panel_hwnd,
                        None,
                        0,
                        0,
                        width,
                        TITLE_BAR_HEIGHT,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    )
                    .ok();
                }
                let input_x = (width - INPUT_WIDTH) / 2;
                let input_y = (TITLE_BAR_HEIGHT - INPUT_HEIGHT) / 2;
                unsafe {
                    SetWindowPos(
                        app.input_hwnd,
                        None,
                        input_x,
                        input_y,
                        INPUT_WIDTH,
                        INPUT_HEIGHT,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    )
                    .ok();
                }
                app.close_button_rect = RECT {
                    left: width - BUTTON_WIDTH,
                    top: 0,
                    right: width,
                    bottom: BUTTON_HEIGHT,
                };
                app.max_button_rect = RECT {
                    left: width - 2 * BUTTON_WIDTH,
                    top: 0,
                    right: width - BUTTON_WIDTH,
                    bottom: BUTTON_HEIGHT,
                };
                app.min_button_rect = RECT {
                    left: width - 3 * BUTTON_WIDTH,
                    top: 0,
                    right: width - 2 * BUTTON_WIDTH,
                    bottom: BUTTON_HEIGHT,
                };
            }
            LRESULT(0)
        }
        WM_NCHITTEST => {
            let mut result = LRESULT(0);
            if unsafe { DwmDefWindowProc(hwnd, msg, wparam, lparam, &mut result) }.as_bool() {
                if result.0 != 0 {
                    return result;
                }
            }

            let x = GET_X_LPARAM(lparam);
            let y = GET_Y_LPARAM(lparam);
            let mut window_rect = RECT::default();
            unsafe { GetWindowRect(hwnd, &mut window_rect) };

            let border_width = 8;
            if x >= window_rect.left && x < window_rect.left + border_width {
                if y >= window_rect.top && y < window_rect.top + border_width {
                    return LRESULT(HTTOPLEFT as isize);
                }
                if y < window_rect.bottom && y >= window_rect.bottom - border_width {
                    return LRESULT(HTBOTTOMLEFT as isize);
                }
                return LRESULT(HTLEFT as isize);
            }
            if x < window_rect.right && x >= window_rect.right - border_width {
                if y >= window_rect.top && y < window_rect.top + border_width {
                    return LRESULT(HTTOPRIGHT as isize);
                }
                if y < window_rect.bottom && y >= window_rect.bottom - border_width {
                    return LRESULT(HTBOTTOMRIGHT as isize);
                }
                return LRESULT(HTRIGHT as isize);
            }
            if y >= window_rect.top && y < window_rect.top + border_width {
                return LRESULT(HTTOP as isize);
            }
            if y < window_rect.bottom && y >= window_rect.bottom - border_width {
                return LRESULT(HTBOTTOM as isize);
            }

            if y - window_rect.top > 0 && y - window_rect.top < TITLE_BAR_HEIGHT {
                let app_state_ptr =
                    unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppState };
                if let Some(app) = unsafe { app_state_ptr.as_ref() } {
                    let hit_input = unsafe {
                        let mut input_rect = RECT::default();
                        GetWindowRect(app.input_hwnd, &mut input_rect);
                        PtInRect(&input_rect, POINT { x, y }).as_bool()
                    };
                    if !hit_input {
                        return LRESULT(HTCAPTION as isize);
                    }
                }
            }
            LRESULT(HTCLIENT as isize)
        }
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            unsafe {
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rect = RECT::default();
                GetClientRect(hwnd, &mut rect);
                let main_bg_brush = CreateSolidBrush(COLORREF(0x00FFFFFF));
                FillRect(hdc, &rect, main_bg_brush);
                DeleteObject(main_bg_brush.into());
                EndPaint(hwnd, &ps);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            let app_state_ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppState };
            if let Some(app_state) = unsafe { (app_state_ptr).as_mut() } {
                unsafe { CloseThemeData(app_state.theme) };
                let _: Box<AppState> = unsafe { Box::from_raw(app_state) };
            }
            unsafe {
                PostQuitMessage(0);
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

// --- Panel Subclass Procedure ---
extern "system" fn panel_wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let main_hwnd = unsafe { GetParent(hwnd).unwrap() };
    let app_state_ptr = unsafe { GetWindowLongPtrW(main_hwnd, GWLP_USERDATA) as *mut AppState };
    if app_state_ptr.is_null() {
        return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
    }
    let app = unsafe { &mut *app_state_ptr };

    match msg {
        WM_NCHITTEST => {
            // 自定义标题栏（panel）命中测试：
            // - 输入框/按钮区域：返回 HTCLIENT，供控件交互
            // - 其他区域：返回 HTTRANSPARENT，让父窗口处理（父窗口会返回 HTCAPTION），从而拖拽移动整个窗口
            let x = GET_X_LPARAM(lparam);
            let y = GET_Y_LPARAM(lparam);

            // 1) 如果在输入框上，保持为客户端区
            unsafe {
                let mut input_rect = RECT::default();
                GetWindowRect(app.input_hwnd, &mut input_rect);
                if PtInRect(&input_rect, POINT { x, y }).as_bool() {
                    return LRESULT(HTCLIENT as isize);
                }
            }

            // 2) 如果在三大按钮上，保持为客户端区（供按钮点击使用）
            // 将屏幕坐标转换为 panel 客户端坐标后再判断
            let mut pt_panel = POINT { x, y };
            unsafe {
                ScreenToClient(hwnd, &mut pt_panel);
            }
            let on_button = unsafe {
                PtInRect(&app.min_button_rect, pt_panel).as_bool()
                    || PtInRect(&app.max_button_rect, pt_panel).as_bool()
                    || PtInRect(&app.close_button_rect, pt_panel).as_bool()
            };
            if on_button {
                return LRESULT(HTCLIENT as isize);
            }

            // 3) 其他 panel 区域透传给父窗口
            return LRESULT(HTTRANSPARENT as isize);
        }
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            unsafe {
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rect = RECT::default();
                GetClientRect(hwnd, &mut rect);
                let bg_brush = CreateSolidBrush(COLORREF(0x00302D2D));
                FillRect(hdc, &rect, bg_brush);
                DeleteObject(bg_brush.into());

                // ★★★ 新增：根据窗口状态选择正确的最大化/还原按钮 ★★★
                let max_button_part = if app.is_maximized {
                    WP_RESTOREBUTTON
                } else {
                    WP_MAXBUTTON
                };

                // 使用系统主题的状态枚举，分别映射不同按钮
                let min_state_id = min_state(app.min_button_state);
                let max_state_id = if app.is_maximized {
                    restore_state(app.max_button_state)
                } else {
                    max_state(app.max_button_state)
                };
                let close_state_id = close_state(app.close_button_state);

                DrawThemeBackground(
                    app.theme,
                    hdc,
                    WP_MINBUTTON.0,
                    min_state_id,
                    &app.min_button_rect,
                    None,
                );
                DrawThemeBackground(
                    app.theme,
                    hdc,
                    max_button_part.0,
                    max_state_id,
                    &app.max_button_rect,
                    None,
                );
                DrawThemeBackground(
                    app.theme,
                    hdc,
                    WP_CLOSEBUTTON.0,
                    close_state_id,
                    &app.close_button_rect,
                    None,
                );

                SetBkMode(hdc, TRANSPARENT);
                SetTextColor(hdc, COLORREF(0x00FFFFFF));
                let mut text_rect = rect;
                text_rect.left += 10;
                let mut text_buffer: Vec<u16> = w!("Rust Native Titlebar").as_wide().to_vec();
                DrawTextW(
                    hdc,
                    &mut text_buffer,
                    &mut text_rect,
                    DT_VCENTER | DT_SINGLELINE,
                );
                EndPaint(hwnd, &ps);
            }
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            let point = POINT {
                x: GET_X_LPARAM(lparam),
                y: GET_Y_LPARAM(lparam),
            };
            let old_states = (
                app.min_button_state,
                app.max_button_state,
                app.close_button_state,
            );
            unsafe {
                app.min_button_state = if PtInRect(&app.min_button_rect, point).as_bool() {
                    ButtonState::Hover
                } else {
                    ButtonState::Normal
                };
                app.max_button_state = if PtInRect(&app.max_button_rect, point).as_bool() {
                    ButtonState::Hover
                } else {
                    ButtonState::Normal
                };
                app.close_button_state = if PtInRect(&app.close_button_rect, point).as_bool() {
                    ButtonState::Hover
                } else {
                    ButtonState::Normal
                };
            }
            if old_states
                != (
                    app.min_button_state,
                    app.max_button_state,
                    app.close_button_state,
                )
            {
                unsafe { InvalidateRect(Some(hwnd), None, true) };
            }
            if !app.mouse_on_panel {
                app.mouse_on_panel = true;
                let mut tme = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: HOVER_DEFAULT,
                };
                unsafe { TrackMouseEvent(&mut tme).ok() };
            }
            LRESULT(0)
        }
        WM_MOUSELEAVE => {
            app.min_button_state = ButtonState::Normal;
            app.max_button_state = ButtonState::Normal;
            app.close_button_state = ButtonState::Normal;
            app.mouse_on_panel = false;
            unsafe { InvalidateRect(Some(hwnd), None, true) };
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let point = POINT {
                x: GET_X_LPARAM(lparam),
                y: GET_Y_LPARAM(lparam),
            };
            unsafe {
                if PtInRect(&app.min_button_rect, point).as_bool() {
                    app.min_button_state = ButtonState::Pressed;
                }
                if PtInRect(&app.max_button_rect, point).as_bool() {
                    app.max_button_state = ButtonState::Pressed;
                }
                if PtInRect(&app.close_button_rect, point).as_bool() {
                    app.close_button_state = ButtonState::Pressed;
                }
            }
            unsafe { InvalidateRect(Some(hwnd), None, true) };
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            let point = POINT {
                x: GET_X_LPARAM(lparam),
                y: GET_Y_LPARAM(lparam),
            };
            unsafe {
                if app.min_button_state == ButtonState::Pressed
                    && PtInRect(&app.min_button_rect, point).as_bool()
                {
                    // 使用系统原生最小化动画（发送到父窗口）
                    let _ = SendMessageW(
                        app.main_hwnd,
                        WM_SYSCOMMAND,
                        Some(WPARAM(SC_MINIMIZE as usize)),
                        Some(LPARAM(0)),
                    );
                }
                // ★★★ 新增：根据窗口状态发送正确的最大化/还原命令 ★★★
                if app.max_button_state == ButtonState::Pressed
                    && PtInRect(&app.max_button_rect, point).as_bool()
                {
                    let command = if app.is_maximized {
                        SC_RESTORE
                    } else {
                        SC_MAXIMIZE
                    };
                    let _ = SendMessageW(
                        app.main_hwnd,
                        WM_SYSCOMMAND,
                        Some(WPARAM(command as usize)),
                        Some(LPARAM(0)),
                    );
                }
                if app.close_button_state == ButtonState::Pressed
                    && PtInRect(&app.close_button_rect, point).as_bool()
                {
                    let _ = SendMessageW(
                        app.main_hwnd,
                        WM_SYSCOMMAND,
                        Some(WPARAM(SC_CLOSE as usize)),
                        Some(LPARAM(0)),
                    );
                }
            }
            app.min_button_state = ButtonState::Normal;

            app.max_button_state = ButtonState::Normal;
            app.close_button_state = ButtonState::Normal;
            unsafe { InvalidateRect(Some(hwnd), None, true) };
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
// --- Helpers ---
impl From<ButtonState> for i32 {
    fn from(val: ButtonState) -> Self {
        match val {
            ButtonState::Normal => CBS_NORMAL.0,
            ButtonState::Hover => CBS_HOT.0,
            ButtonState::Pressed => CBS_PUSHED.0,
        }
    }
}
#[allow(non_snake_case)]
fn GET_X_LPARAM(lp: LPARAM) -> i32 {
    (lp.0 & 0xffff) as i16 as i32
}

// --- Theme state helpers (global) ---
fn min_state(s: ButtonState) -> i32 {
    match s {
        ButtonState::Normal => MINBS_NORMAL.0,
        ButtonState::Hover => MINBS_HOT.0,
        ButtonState::Pressed => MINBS_PUSHED.0,
    }
}
fn max_state(s: ButtonState) -> i32 {
    match s {
        ButtonState::Normal => MAXBS_NORMAL.0,
        ButtonState::Hover => MAXBS_HOT.0,
        ButtonState::Pressed => MAXBS_PUSHED.0,
    }
}
fn restore_state(s: ButtonState) -> i32 {
    match s {
        ButtonState::Normal => RBS_NORMAL.0,
        ButtonState::Hover => RBS_HOT.0,
        ButtonState::Pressed => RBS_PUSHED.0,
    }
}
fn close_state(s: ButtonState) -> i32 {
    match s {
        ButtonState::Normal => CBS_NORMAL.0,
        ButtonState::Hover => CBS_HOT.0,
        ButtonState::Pressed => CBS_PUSHED.0,
    }
}

#[allow(non_snake_case)]
fn GET_Y_LPARAM(lp: LPARAM) -> i32 {
    (lp.0 >> 16) as i16 as i32
}
#[allow(non_snake_case)]
fn LOWORD(l: u32) -> u16 {
    (l & 0xffff) as u16
}
