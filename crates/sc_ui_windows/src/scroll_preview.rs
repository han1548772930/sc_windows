use std::cell::Cell;

use sc_app::selection::RectI32;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;

thread_local! {
    static PREVIEW_HWND: Cell<HWND> = const { Cell::new(HWND(std::ptr::null_mut())) };
}

struct PreviewState {
    bmp: Vec<u8>,
    width: i32,
    height: i32,
    target_geometry: (i32, i32, i32, i32),
}

pub struct ScrollPreviewWindow;

impl ScrollPreviewWindow {
    pub fn show_or_update(selection: RectI32, bmp: Vec<u8>) -> Result<(), String> {
        let (width, height) = bmp_dimensions(&bmp)?;
        let existing = PREVIEW_HWND.with(Cell::get);
        if !existing.0.is_null() && unsafe { IsWindow(Some(existing)) }.as_bool() {
            unsafe {
                let state = GetWindowLongPtrW(existing, GWLP_USERDATA) as *mut PreviewState;
                if !state.is_null() {
                    (*state).bmp = bmp;
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
                bmp,
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

fn bmp_dimensions(bmp: &[u8]) -> Result<(i32, i32), String> {
    if bmp.len() < 54 || &bmp[..2] != b"BM" {
        return Err("滚动预览数据不是有效 BMP".to_string());
    }
    let width = i32::from_le_bytes(bmp[18..22].try_into().unwrap()).abs();
    let height = i32::from_le_bytes(bmp[22..26].try_into().unwrap()).abs();
    Ok((width, height))
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
                let offset = u32::from_le_bytes(state.bmp[10..14].try_into().unwrap()) as usize;
                let available_w = (client.right - client.left - 16).max(1);
                let available_h = (client.bottom - client.top - 16).max(1);
                let scale = (available_w as f32 / state.width as f32)
                    .min(available_h as f32 / state.height as f32);
                let draw_w = (state.width as f32 * scale) as i32;
                let draw_h = (state.height as f32 * scale) as i32;
                let x = (client.right - draw_w) / 2;
                let y = (client.bottom - draw_h) / 2;
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
                        Some(state.bmp[offset..].as_ptr().cast()),
                        state.bmp[14..].as_ptr().cast(),
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
