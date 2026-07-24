use std::cell::Cell;

use sc_app::selection::RectI32;
use sc_platform_windows::windows::graphics_capture::BgraFrame;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;

thread_local! {
    static PREVIEW_HWND: Cell<HWND> = const { Cell::new(HWND(std::ptr::null_mut())) };
    static STATUS_HWND: Cell<HWND> = const { Cell::new(HWND(std::ptr::null_mut())) };
}

struct PreviewState {
    pixels: Vec<u8>,
    width: i32,
    height: i32,
    target_geometry: (i32, i32, i32, i32),
}

pub struct ScrollPreviewWindow;

impl ScrollPreviewWindow {
    pub fn set_status(selection: RectI32, status: Option<&str>) {
        let existing = STATUS_HWND.with(Cell::get);
        if status.is_none() {
            if !existing.0.is_null() {
                unsafe {
                    let _ = DestroyWindow(existing);
                }
            }
            return;
        }

        let selection_width = (selection.right - selection.left).max(1);
        let selection_height = (selection.bottom - selection.top).max(1);
        let width = selection_width.min(280);
        let height = selection_height.min(52);
        let x = selection.left + (selection_width - width) / 2;
        let y = selection.top + (selection_height - height) / 2;

        if !existing.0.is_null() && unsafe { IsWindow(Some(existing)) }.as_bool() {
            unsafe {
                let state = GetWindowLongPtrW(existing, GWLP_USERDATA) as *mut String;
                if !state.is_null() {
                    *state = status.expect("status checked above").to_owned();
                }
                let _ = SetWindowPos(
                    existing,
                    Some(HWND_TOPMOST),
                    x,
                    y,
                    width,
                    height,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
                let _ = InvalidateRect(Some(existing), None, false);
            }
            return;
        }

        unsafe {
            let instance = match GetModuleHandleW(None) {
                Ok(instance) => instance,
                Err(_) => return,
            };
            let class_name = windows::core::w!("ScrollCaptureStatus");
            let class = WNDCLASSW {
                lpfnWndProc: Some(status_window_proc),
                hInstance: instance.into(),
                lpszClassName: class_name,
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                style: CS_HREDRAW | CS_VREDRAW,
                ..Default::default()
            };
            if RegisterClassW(&class) == 0 && GetLastError().0 != 1410 {
                return;
            }
            let state = Box::into_raw(Box::new(status.expect("status checked above").to_owned()));
            let hwnd = match CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT,
                class_name,
                windows::core::w!("滚动截图状态"),
                WS_POPUP,
                x,
                y,
                width,
                height,
                None,
                None,
                Some(instance.into()),
                Some(state.cast()),
            ) {
                Ok(hwnd) => hwnd,
                Err(_) => {
                    drop(Box::from_raw(state));
                    return;
                }
            };
            let _ = SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE);
            STATUS_HWND.with(|slot| slot.set(hwnd));
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        }
    }

    pub fn show_or_update(selection: RectI32, frame: BgraFrame) -> Result<(), String> {
        let width = frame.width as i32;
        let height = frame.height as i32;
        if width <= 0 || height <= 0 || frame.pixels.len() != width as usize * height as usize * 4 {
            return Err("滚动预览像素缓冲区无效".to_string());
        }
        let existing = PREVIEW_HWND.with(Cell::get);
        if !existing.0.is_null() && unsafe { IsWindow(Some(existing)) }.as_bool() {
            unsafe {
                let state = GetWindowLongPtrW(existing, GWLP_USERDATA) as *mut PreviewState;
                if !state.is_null() {
                    (*state).pixels = frame.pixels;
                    (*state).width = width;
                    (*state).height = height;
                    (*state).target_geometry = preview_geometry(selection, width, height);
                    let target = (*state).target_geometry;
                    let _ = SetWindowPos(
                        existing,
                        Some(HWND_TOPMOST),
                        target.0,
                        target.1,
                        target.2,
                        target.3,
                        SWP_NOACTIVATE | SWP_SHOWWINDOW,
                    );
                    let _ = InvalidateRect(Some(existing), None, false);
                }
            }
            return Ok(());
        }

        unsafe {
            let instance = GetModuleHandleW(None).map_err(|e| e.to_string())?;
            let class_name = windows::core::w!("ScrollCapturePreview");
            let class = WNDCLASSW {
                lpfnWndProc: Some(window_proc),
                hInstance: instance.into(),
                lpszClassName: class_name,
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                hbrBackground: HBRUSH::default(),
                style: CS_HREDRAW | CS_VREDRAW,
                ..Default::default()
            };
            if RegisterClassW(&class) == 0 && GetLastError().0 != 1410 {
                return Err(format!("注册滚动预览窗口失败: {:?}", GetLastError()));
            }

            let (x, y, preview_width, preview_height) = preview_geometry(selection, width, height);
            let state = Box::new(PreviewState {
                pixels: frame.pixels,
                width,
                height,
                target_geometry: (x, y, preview_width, preview_height),
            });
            let hwnd = CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE,
                class_name,
                windows::core::w!("滚动截图预览"),
                WS_POPUP | WS_BORDER | WS_VISIBLE,
                x,
                y,
                preview_width,
                preview_height,
                None,
                None,
                Some(instance.into()),
                Some(Box::into_raw(state).cast()),
            )
            .map_err(|e| e.to_string())?;
            PREVIEW_HWND.with(|slot| slot.set(hwnd));
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        }
        Ok(())
    }

    pub fn close() {
        let status_hwnd = STATUS_HWND.with(|slot| slot.replace(HWND(std::ptr::null_mut())));
        if !status_hwnd.0.is_null() {
            unsafe {
                let _ = DestroyWindow(status_hwnd);
            }
        }
        let hwnd = PREVIEW_HWND.with(|slot| slot.replace(HWND(std::ptr::null_mut())));
        if !hwnd.0.is_null() {
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
        }
    }
}

fn preview_geometry(
    selection: RectI32,
    image_width: i32,
    image_height: i32,
) -> (i32, i32, i32, i32) {
    let screen_width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let screen_height = unsafe { GetSystemMetrics(SM_CYSCREEN) };
    let available_right = (screen_width - selection.right - 12).max(120);
    let width = 280.min(available_right);
    let selection_height = selection.bottom - selection.top;
    let proportional_height = if image_width > 0 {
        (width - 16) * image_height / image_width + 16
    } else {
        selection_height
    };
    let max_height = (screen_height * 9 / 10).min(screen_height - 24).max(180);
    let height = proportional_height
        .max(selection_height)
        .clamp(180, max_height);
    let x = (selection.right + 12).min(screen_width - width);
    let selection_center_y = selection.top + selection_height / 2;
    let y = (selection_center_y - height / 2).clamp(12, (screen_height - height - 12).max(12));
    (x, y, width, height)
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => {
            let create = lparam.0 as *const CREATESTRUCTW;
            unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, (*create).lpCreateParams as isize) };
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_PAINT => {
            let mut paint = PAINTSTRUCT::default();
            let dc = unsafe { BeginPaint(hwnd, &mut paint) };
            let mut client = RECT::default();
            let _ = unsafe { GetClientRect(hwnd, &mut client) };
            let client_width = (client.right - client.left).max(1);
            let client_height = (client.bottom - client.top).max(1);
            let memory_dc = unsafe { CreateCompatibleDC(Some(dc)) };
            let memory_bitmap = unsafe { CreateCompatibleBitmap(dc, client_width, client_height) };
            let old_bitmap = unsafe { SelectObject(memory_dc, memory_bitmap.into()) };
            let brush = HBRUSH(unsafe { GetStockObject(WHITE_BRUSH) }.0);
            let _ = unsafe { FillRect(memory_dc, &client, brush) };
            let state = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const PreviewState };
            if !state.is_null() {
                let state = unsafe { &*state };
                let available_w = (client.right - client.left - 16).max(1);
                let available_h = (client.bottom - client.top - 16).max(1);
                let scale = (available_w as f32 / state.width as f32)
                    .min(available_h as f32 / state.height as f32);
                let draw_w = (state.width as f32 * scale) as i32;
                let draw_h = (state.height as f32 * scale) as i32;
                let x = (client.right - draw_w) / 2;
                let y = (client.bottom - draw_h) / 2;
                let bitmap_info = BITMAPINFO {
                    bmiHeader: BITMAPINFOHEADER {
                        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                        biWidth: state.width,
                        biHeight: -state.height,
                        biPlanes: 1,
                        biBitCount: 32,
                        biCompression: BI_RGB.0,
                        ..Default::default()
                    },
                    bmiColors: [RGBQUAD::default(); 1],
                };
                unsafe {
                    SetStretchBltMode(memory_dc, HALFTONE);
                    StretchDIBits(
                        memory_dc,
                        x,
                        y,
                        draw_w,
                        draw_h,
                        0,
                        0,
                        state.width,
                        state.height,
                        Some(state.pixels.as_ptr().cast()),
                        &bitmap_info,
                        DIB_RGB_COLORS,
                        SRCCOPY,
                    );
                }
            }
            unsafe {
                let _ = BitBlt(
                    dc,
                    0,
                    0,
                    client_width,
                    client_height,
                    Some(memory_dc),
                    0,
                    0,
                    SRCCOPY,
                );
                SelectObject(memory_dc, old_bitmap);
                let _ = DeleteObject(memory_bitmap.into());
                let _ = DeleteDC(memory_dc);
            }
            let _ = unsafe { EndPaint(hwnd, &paint) };
            LRESULT(0)
        }
        WM_DESTROY => {
            let state = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut PreviewState };
            if !state.is_null() {
                drop(unsafe { Box::from_raw(state) });
                unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) };
            }
            PREVIEW_HWND.with(|slot| slot.set(HWND(std::ptr::null_mut())));
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

unsafe extern "system" fn status_window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => {
            let create = lparam.0 as *const CREATESTRUCTW;
            unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, (*create).lpCreateParams as isize) };
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_PAINT => {
            let mut paint = PAINTSTRUCT::default();
            let dc = unsafe { BeginPaint(hwnd, &mut paint) };
            let mut client = RECT::default();
            let _ = unsafe { GetClientRect(hwnd, &mut client) };
            let brush = unsafe { CreateSolidBrush(COLORREF(0x00969696)) };
            let _ = unsafe { FillRect(dc, &client, brush) };
            let _ = unsafe { DeleteObject(brush.into()) };

            let font_name: Vec<u16> = "Microsoft YaHei\0".encode_utf16().collect();
            let font = unsafe {
                CreateFontW(
                    -15,
                    0,
                    0,
                    0,
                    FW_NORMAL.0 as i32,
                    0,
                    0,
                    0,
                    DEFAULT_CHARSET,
                    OUT_DEFAULT_PRECIS,
                    CLIP_DEFAULT_PRECIS,
                    CLEARTYPE_QUALITY,
                    (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
                    windows::core::PCWSTR(font_name.as_ptr()),
                )
            };
            let old_font = (!font.is_invalid()).then(|| unsafe { SelectObject(dc, font.into()) });
            let state = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const String };
            if !state.is_null() {
                let mut text: Vec<u16> = unsafe { &*state }.encode_utf16().collect();
                client.left += 8;
                client.right -= 8;
                unsafe {
                    SetBkMode(dc, TRANSPARENT);
                    SetTextColor(dc, COLORREF(0x00ffffff));
                    let _ = DrawTextW(
                        dc,
                        &mut text,
                        &mut client,
                        DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
                    );
                }
            }
            if let Some(old_font) = old_font {
                unsafe {
                    SelectObject(dc, old_font);
                    let _ = DeleteObject(font.into());
                }
            }
            let _ = unsafe { EndPaint(hwnd, &paint) };
            LRESULT(0)
        }
        WM_DESTROY => {
            let state = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut String };
            if !state.is_null() {
                drop(unsafe { Box::from_raw(state) });
                unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) };
            }
            STATUS_HWND.with(|slot| slot.set(HWND(std::ptr::null_mut())));
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
