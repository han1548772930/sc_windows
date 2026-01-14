use std::ffi::c_void;
use std::fmt;
use std::sync::mpsc;

use sc_platform::{InputEvent, WindowId, WindowMessageHandler};
use windows::Win32::UI::WindowsAndMessaging::{CREATESTRUCTW, WM_APP, WNDCLASS_STYLES};

use crate::EventConverter;
use crate::win_api;
use crate::win32::{
    HWND, LPARAM, LRESULT, RECT, SWP_NOACTIVATE, SWP_NOZORDER, WM_CLOSE, WM_CREATE, WM_DESTROY,
    WM_DPICHANGED, WM_PAINT, WM_SETCURSOR, WPARAM,
};

use super::message_box;
use super::system::get_screen_size;
use super::tray_manager::{TRAY_CALLBACK_MESSAGE, tray_event_from_callback};
use super::window_event_converter::WindowEventConverter;

const USER_EVENT_MESSAGE: u32 = WM_APP + 42;

pub struct UserEventSender<E> {
    /// Opaque window id (avoids `HWND` being !Send while still allowing PostMessage from any thread).
    window: WindowId,
    sender: mpsc::Sender<E>,
}

impl<E> Clone for UserEventSender<E> {
    fn clone(&self) -> Self {
        Self {
            window: self.window,
            sender: self.sender.clone(),
        }
    }
}

impl<E> UserEventSender<E> {
    pub fn send(&self, event: E) -> Result<(), mpsc::SendError<E>> {
        self.sender.send(event)?;
        let hwnd = super::hwnd(self.window);
        let _ = win_api::post_message(hwnd, USER_EVENT_MESSAGE, 0, 0);
        Ok(())
    }
}

struct CreateParams<F, E> {
    factory: Option<F>,
    user_events: Option<mpsc::Receiver<E>>,
    user_event_sender: mpsc::Sender<E>,
}

struct AppState<A, E> {
    app: A,
    user_events: mpsc::Receiver<E>,
}

pub fn run_fullscreen_toolwindow_app<A, F, E>(
    window_class_name: &str,
    class_style: WNDCLASS_STYLES,
    create_app: F,
) -> windows::core::Result<()>
where
    A: WindowMessageHandler<WindowHandle = WindowId> + 'static,
    F: FnOnce(WindowId, (i32, i32), UserEventSender<A::UserEvent>) -> std::result::Result<A, E>,
    E: fmt::Display,
{
    let (width, height) = get_screen_size();

    run_toolwindow_app(
        window_class_name,
        width,
        height,
        class_style,
        |window, events| create_app(window, (width, height), events),
    )
}

pub fn run_toolwindow_app<A, F, E>(
    window_class_name: &str,
    width: i32,
    height: i32,
    class_style: WNDCLASS_STYLES,
    create_app: F,
) -> windows::core::Result<()>
where
    A: WindowMessageHandler<WindowHandle = WindowId> + 'static,
    F: FnOnce(WindowId, UserEventSender<A::UserEvent>) -> std::result::Result<A, E>,
    E: fmt::Display,
{
    let _ = win_api::set_process_per_monitor_dpi_aware();

    let (tx, rx) = mpsc::channel::<A::UserEvent>();

    let mut create_params = CreateParams {
        factory: Some(create_app),
        user_events: Some(rx),
        user_event_sender: tx,
    };

    let _hwnd = win_api::create_hidden_toolwindow_with_params(
        window_class_name,
        window_proc::<A, F, E>,
        width,
        height,
        class_style,
        Some(
            (&mut create_params as *mut CreateParams<F, A::UserEvent>).cast::<c_void>()
                as *const c_void,
        ),
    )?;

    win_api::run_message_loop();
    Ok(())
}

unsafe extern "system" fn window_proc<A, F, E>(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT
where
    A: WindowMessageHandler<WindowHandle = WindowId> + 'static,
    F: FnOnce(WindowId, UserEventSender<A::UserEvent>) -> std::result::Result<A, E>,
    E: fmt::Display,
{
    match msg {
        WM_CREATE => {
            let _ = win_api::co_initialize();

            let create_struct = unsafe { &*(lparam.0 as *const CREATESTRUCTW) };
            let create_params = create_struct.lpCreateParams as *mut CreateParams<F, A::UserEvent>;

            if create_params.is_null() {
                return LRESULT(-1);
            }

            let Some(factory) = (unsafe { (&mut *create_params).factory.take() }) else {
                return LRESULT(-1);
            };
            let Some(user_events) = (unsafe { (&mut *create_params).user_events.take() }) else {
                return LRESULT(-1);
            };

            let event_sender = UserEventSender {
                window: super::window_id(hwnd),
                sender: unsafe { (&mut *create_params).user_event_sender.clone() },
            };

            match factory(super::window_id(hwnd), event_sender) {
                Ok(app) => {
                    let state = Box::new(AppState { app, user_events });
                    win_api::set_window_user_data(hwnd, Box::into_raw(state) as isize);
                    LRESULT(0)
                }
                Err(e) => {
                    let msg = format!("应用初始化失败: {e}");
                    message_box::show_error(hwnd, "启动错误", &msg);
                    LRESULT(-1)
                }
            }
        }

        WM_DESTROY => {
            let ptr = win_api::get_window_user_data(hwnd) as *mut AppState<A, A::UserEvent>;
            if !ptr.is_null() {
                let _ = unsafe { Box::from_raw(ptr) };
            }
            win_api::set_window_user_data(hwnd, 0);

            win_api::quit_message_loop(0);
            LRESULT(0)
        }

        val if val == USER_EVENT_MESSAGE => {
            let ptr = win_api::get_window_user_data(hwnd) as *mut AppState<A, A::UserEvent>;
            if ptr.is_null() {
                return LRESULT(0);
            }

            let state = unsafe { &mut *ptr };
            while let Ok(event) = state.user_events.try_recv() {
                let _ = state.app.handle_user_event(super::window_id(hwnd), event);
            }

            LRESULT(0)
        }

        WM_PAINT => {
            let ptr = win_api::get_window_user_data(hwnd) as *mut AppState<A, A::UserEvent>;
            if ptr.is_null() {
                return win_api::def_window_proc(hwnd, msg, wparam, lparam);
            }

            let state = unsafe { &mut *ptr };

            // The runner owns the WM_PAINT cycle.
            let ps = win_api::begin_paint(hwnd);
            let result = state.app.handle_paint(super::window_id(hwnd)).unwrap_or(0);
            win_api::end_paint(hwnd, &ps);

            LRESULT(result)
        }

        WM_CLOSE => {
            let ptr = win_api::get_window_user_data(hwnd) as *mut AppState<A, A::UserEvent>;
            if !ptr.is_null() {
                let state = unsafe { &mut *ptr };
                if let Some(result) = state.app.handle_close_requested(super::window_id(hwnd)) {
                    return LRESULT(result);
                }
            }

            win_api::def_window_proc(hwnd, msg, wparam, lparam)
        }

        WM_DPICHANGED => {
            let ptr = win_api::get_window_user_data(hwnd) as *mut AppState<A, A::UserEvent>;
            if ptr.is_null() {
                return win_api::def_window_proc(hwnd, msg, wparam, lparam);
            }

            let rect_ptr = lparam.0 as *const RECT;
            if rect_ptr.is_null() {
                return win_api::def_window_proc(hwnd, msg, wparam, lparam);
            }

            let rect = unsafe { *rect_ptr };
            let width = rect.right - rect.left;
            let height = rect.bottom - rect.top;

            // Apply the OS suggested window bounds for the new DPI.
            // This will usually trigger WM_SIZE / WM_MOVE.
            let _ = win_api::set_window_pos(
                hwnd,
                None,
                rect.left,
                rect.top,
                width,
                height,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );

            let state = unsafe { &mut *ptr };
            if let Some(event) = WindowEventConverter::convert(msg, wparam, lparam) {
                let _ = state.app.handle_window_event(super::window_id(hwnd), event);
            }

            LRESULT(0)
        }

        WM_SETCURSOR => LRESULT(1),

        val if val == TRAY_CALLBACK_MESSAGE => {
            let Some(tray_event) = tray_event_from_callback(hwnd, lparam.0 as u32) else {
                return LRESULT(0);
            };

            let ptr = win_api::get_window_user_data(hwnd) as *mut AppState<A, A::UserEvent>;
            if !ptr.is_null() {
                let state = unsafe { &mut *ptr };
                if let Some(result) = state
                    .app
                    .handle_input_event(super::window_id(hwnd), InputEvent::Tray(tray_event))
                {
                    return LRESULT(result);
                }
            }

            LRESULT(0)
        }

        _ => {
            let ptr = win_api::get_window_user_data(hwnd) as *mut AppState<A, A::UserEvent>;
            if !ptr.is_null() {
                let state = unsafe { &mut *ptr };
                let window = super::window_id(hwnd);

                if let Some(event) = WindowEventConverter::convert(msg, wparam, lparam)
                    && let Some(result) = state.app.handle_window_event(window, event)
                {
                    return LRESULT(result);
                }

                if let Some(result) = state
                    .app
                    .handle_window_message(window, msg, wparam.0, lparam.0)
                {
                    return LRESULT(result);
                }

                if let Some(event) = EventConverter::convert(msg, wparam, lparam)
                    && let Some(result) = state.app.handle_input_event(window, event)
                {
                    return LRESULT(result);
                }
            }

            win_api::def_window_proc(hwnd, msg, wparam, lparam)
        }
    }
}
