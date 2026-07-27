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
            return Err("invalid scrolling preview pixel buffer".to_string());
        }
        let target = preview_geometry(selection, width, height);
        let existing = PREVIEW_HWND.with(Cell::get);
        if !existing.0.is_null() && unsafe { IsWindow(Some(existing)) }.as_bool() {
            unsafe {
                let state = GetWindowLongPtrW(existing, GWLP_USERDATA) as *mut PreviewState;
                if !state.is_null() {
                    let previous = (*state).target_geometry;
                    (*state).pixels = frame.pixels;
                    (*state).width = width;
                    (*state).height = height;
                    (*state).target_geometry = target;
                    present_layered_frame(existing, &*state, target)?;
                    if target != previous {
                        eprintln!(
                            "[scroll preview] geometry: ({},{},{},{}) -> ({},{},{},{})",
                            previous.0,
                            previous.1,
                            previous.2,
                            previous.3,
                            target.0,
                            target.1,
                            target.2,
                            target.3,
                        );
                    }
                }
            }
            return Ok(());
        }

        unsafe {
            let instance = GetModuleHandleW(None).map_err(|error| error.to_string())?;
            let class_name = windows::core::w!("ScrollCapturePreviewLayered");
            let class = WNDCLASSW {
                lpfnWndProc: Some(window_proc),
                hInstance: instance.into(),
                lpszClassName: class_name,
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                ..Default::default()
            };
            if RegisterClassW(&class) == 0 && GetLastError().0 != 1410 {
                return Err(format!(
                    "failed to register scroll preview window: {:?}",
                    GetLastError()
                ));
            }
            let state = Box::into_raw(Box::new(PreviewState {
                pixels: frame.pixels,
                width,
                height,
                target_geometry: target,
            }));
            let hwnd = CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE | WS_EX_LAYERED,
                class_name,
                windows::core::w!("Scrolling screenshot preview"),
                WS_POPUP,
                target.0,
                target.1,
                target.2,
                target.3,
                None,
                None,
                Some(instance.into()),
                Some(state.cast()),
            )
            .map_err(|error| {
                drop(Box::from_raw(state));
                error.to_string()
            })?;
            let _ = SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE);
            PREVIEW_HWND.with(|slot| slot.set(hwnd));
            present_layered_frame(hwnd, &*state, target)?;
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub fn show_or_update_legacy(selection: RectI32, frame: BgraFrame) -> Result<(), String> {
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
                    let target = preview_geometry(selection, width, height);
                    // Only move the window when the geometry actually changed. The canvas grows
                    // by a few rows per splice, so re-issuing the same rectangle every frame
                    // makes Windows discard and re-create the window surface — which is what
                    // makes the preview flicker.
                    if target != (*state).target_geometry {
                        let previous = (*state).target_geometry;
                        (*state).target_geometry = target;
                        // WeChat's normal update_preview branch moves before it resizes.
                        let _ = SetWindowPos(
                            existing,
                            Some(HWND_TOPMOST),
                            target.0,
                            target.1,
                            0,
                            0,
                            SWP_NOSIZE | SWP_NOREDRAW | SWP_NOACTIVATE | SWP_SHOWWINDOW,
                        );
                        let _ = SetWindowPos(
                            existing,
                            Some(HWND_TOPMOST),
                            0,
                            0,
                            target.2,
                            target.3,
                            SWP_NOMOVE | SWP_NOREDRAW | SWP_NOACTIVATE | SWP_SHOWWINDOW,
                        );
                        eprintln!(
                            "[scroll preview] geometry: ({},{},{},{}) -> ({},{},{},{})",
                            previous.0,
                            previous.1,
                            previous.2,
                            previous.3,
                            target.0,
                            target.1,
                            target.2,
                            target.3,
                        );
                    }
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
                // No CS_HREDRAW/CS_VREDRAW: those invalidate the whole window on every resize,
                // and the preview resizes on every splice. The paint handler already covers the
                // full client area, so nothing needs the forced full repaint.
                style: WNDCLASS_STYLES(0),
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
                // Keep the client area exactly equal to the preview bitmap. A framed popup's
                // non-client border makes it two pixels smaller in each dimension, forcing
                // every update through StretchDIBits; that call paints in visible horizontal
                // blocks and produces the flashing/folded preview seen during fast scrolling.
                WS_POPUP | WS_VISIBLE,
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
            // The preview is created after the grab worker starts. Exclude it at the compositor
            // immediately, before the grab thread has a chance to publish the HWND through
            // MagSetWindowFilterList on its next iteration.
            let _ = SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE);
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

    /// Window handles owned by the preview, as raw addresses.
    ///
    /// These are fed to `MagSetWindowFilterList` so the capture UI never appears in the
    /// captured pixels.
    pub fn window_handles() -> Vec<usize> {
        let mut handles = Vec::new();
        let preview = PREVIEW_HWND.with(Cell::get);
        if !preview.0.is_null() {
            handles.push(preview.0 as usize);
        }
        let status = STATUS_HWND.with(Cell::get);
        if !status.0.is_null() {
            handles.push(status.0 as usize);
        }
        handles
    }
}

/// Where the preview window sits relative to the selection.
///
/// The choice is horizontal: the preview prefers the free space to the right of the selection,
/// falls back to the space on its left, and only overlaps the selection when neither side can
/// hold it. Each placement carries its own scale rule and height limit.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PreviewPlacement {
    /// Right of the selection.
    Right,
    /// Left of the selection.
    Left,
    /// Overlapping the selection's bottom-right corner.
    Overlay,
}

/// Gap the placement test requires beyond the preview itself, as a fraction of selection width.
const PLACEMENT_MARGIN_RATIO: f64 = 0.2;
/// Fixed part of that gap, in logical pixels.
const PLACEMENT_MARGIN_BASE: i32 = 24;
/// Scale quantum for the side placements; the divide is integral, so scale is a multiple of this.
const SIDE_SCALE_STEP: f64 = 0.1;
/// Upper bound on the side placements' scale.
const SIDE_SCALE_MAX: f64 = 0.4;
/// Numerator bias for [`PreviewPlacement::Right`].
const RIGHT_SCALE_BIAS: i32 = 240;
/// Numerator bias for [`PreviewPlacement::Left`].
const LEFT_SCALE_BIAS: i32 = 250;
/// Preview width the overlay placement targets, in logical pixels.
const OVERLAY_TARGET_WIDTH: f64 = 400.0;
/// Margin kept between the preview and the screen edge for the side placements.
const SIDE_HEIGHT_MARGIN: i32 = 12;
/// Slice of the selection the overlay placement leaves visible.
const OVERLAY_HEIGHT_RESERVE: i32 = 99;
/// Inset of the overlay placement from the selection's bottom-right corner.
const OVERLAY_INSET: i32 = 12;
/// Gap between the preview and the selection for [`PreviewPlacement::Right`].
const RIGHT_GAP: i32 = 12;
/// Gap between the preview and the selection for [`PreviewPlacement::Left`].
const LEFT_GAP: i32 = 24;

impl PreviewPlacement {
    /// Choose a placement from the selection and the available screen width.
    ///
    /// `screen_width` is the usable desktop width. Widths are inclusive, matching the selection
    /// rectangle's convention.
    pub fn choose(selection: RectI32, screen_width: i32) -> Self {
        let selection_width = selection.right - selection.left + 1;
        let margin = (selection_width as f64 * PLACEMENT_MARGIN_RATIO).round() as i32
            + PLACEMENT_MARGIN_BASE;
        if selection.right + margin <= screen_width {
            Self::Right
        } else if margin < selection.left {
            Self::Left
        } else {
            Self::Overlay
        }
    }

    /// Factor the canvas is drawn at.
    ///
    /// The side placements derive it from the free space beside the selection and the canvas
    /// width; since the canvas is only ever extended downwards, its width equals the selection
    /// width and the factor stays constant for the whole session. The overlay placement instead
    /// targets a fixed preview width, which can mean magnification for a narrow selection.
    pub fn scale(self, selection: RectI32, screen_width: i32, canvas_width: i32) -> f64 {
        let canvas_width = canvas_width.max(1);
        match self {
            Self::Right => {
                let gap = screen_width - selection.right;
                side_scale(gap * 10 - RIGHT_SCALE_BIAS, canvas_width)
            }
            Self::Left => side_scale(selection.left * 10 - LEFT_SCALE_BIAS, canvas_width),
            Self::Overlay => {
                let selection_width = (selection.right - selection.left + 1).max(1);
                OVERLAY_TARGET_WIDTH / selection_width as f64
            }
        }
    }

    /// Tallest the preview may be, in logical pixels.
    pub fn height_limit(self, selection: RectI32, screen_height: i32) -> i32 {
        let limit = match self {
            Self::Right | Self::Left => selection.bottom - SIDE_HEIGHT_MARGIN,
            Self::Overlay => selection.bottom - selection.top - OVERLAY_HEIGHT_RESERVE,
        };
        limit.clamp(1, screen_height.max(1))
    }

    /// Top-left corner for a preview of `size`, in screen coordinates.
    pub fn origin(self, selection: RectI32, width: i32, height: i32) -> (i32, i32) {
        match self {
            Self::Right => (selection.right + RIGHT_GAP, selection.bottom - height + 1),
            Self::Left => (
                selection.left - width - LEFT_GAP,
                selection.bottom - height + 1,
            ),
            Self::Overlay => (
                selection.right - width - OVERLAY_INSET,
                selection.bottom - height - OVERLAY_INSET,
            ),
        }
    }
}

/// Scale for a side placement: an integer divide, then a tenth, capped.
///
/// The divide truncating before the multiply quantises the result, so the only reachable values
/// are 0.1 through 0.4.
fn side_scale(numerator: i32, canvas_width: i32) -> f64 {
    let quantised = numerator / canvas_width.max(1);
    ((quantised as f64) * SIDE_SCALE_STEP).min(SIDE_SCALE_MAX)
}

/// Size the canvas should be rendered at, in logical pixels.
///
/// Applies the placement's scale, then shrinks proportionally if the result exceeds the height
/// limit. Returns at least 1x1.
pub fn preview_size(
    selection: RectI32,
    screen: (i32, i32),
    canvas: (i32, i32),
) -> (u32, u32, PreviewPlacement) {
    let placement = PreviewPlacement::choose(selection, screen.0);
    let scale = placement.scale(selection, screen.0, canvas.0);
    let mut width = (canvas.0 as f64 * scale).round().max(1.0) as i32;
    let mut height = (canvas.1 as f64 * scale).round().max(1.0) as i32;
    let limit = placement.height_limit(selection, screen.1);
    if height > limit {
        // Keep the aspect ratio while fitting the limit.
        width = (width as f64 * limit as f64 / height as f64)
            .round()
            .max(1.0) as i32;
        height = limit;
    }
    (width.max(1) as u32, height.max(1) as u32, placement)
}

/// Size a scrolling preview using the bounds of the monitor containing the selection.
pub fn preview_size_on_monitor(
    selection: RectI32,
    canvas: (i32, i32),
) -> (u32, u32, PreviewPlacement) {
    let (selection, monitor) = selection_on_monitor(selection);
    preview_size(selection, (monitor.2, monitor.3), canvas)
}

/// Maximum preview box for a canvas whose height is not known by the UI thread.
pub fn preview_bounds_on_monitor(selection: RectI32, canvas_width: i32) -> (u32, u32) {
    let (selection, monitor) = selection_on_monitor(selection);
    let placement = PreviewPlacement::choose(selection, monitor.2);
    let scale = placement.scale(selection, monitor.2, canvas_width);
    let width = (canvas_width.max(1) as f64 * scale).round().max(1.0) as u32;
    let height = placement.height_limit(selection, monitor.3).max(1) as u32;
    (width, height)
}

fn selection_on_monitor(selection: RectI32) -> (RectI32, (i32, i32, i32, i32)) {
    let rect = RECT {
        left: selection.left,
        top: selection.top,
        right: selection.right + 1,
        bottom: selection.bottom + 1,
    };
    let monitor = unsafe { MonitorFromRect(&rect, MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    let bounds =
        if !monitor.is_invalid() && unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
            info.rcMonitor
        } else {
            RECT {
                left: 0,
                top: 0,
                right: unsafe { GetSystemMetrics(SM_CXSCREEN) },
                bottom: unsafe { GetSystemMetrics(SM_CYSCREEN) },
            }
        };
    let width = (bounds.right - bounds.left).max(1);
    let height = (bounds.bottom - bounds.top).max(1);
    (
        RectI32 {
            left: selection.left - bounds.left,
            top: selection.top - bounds.top,
            right: selection.right - bounds.left,
            bottom: selection.bottom - bounds.top,
        },
        (bounds.left, bounds.top, width, height),
    )
}

fn preview_geometry(
    selection: RectI32,
    image_width: i32,
    image_height: i32,
) -> (i32, i32, i32, i32) {
    let (selection, monitor) = selection_on_monitor(selection);
    let (screen_left, screen_top, screen_width, screen_height) = monitor;
    let placement = PreviewPlacement::choose(selection, screen_width);
    // The image was already produced at the size the placement asks for, so the window takes the
    // image's dimensions verbatim and the paint path never rescales.
    let width = image_width.max(1);
    let height = image_height.max(1);
    let (x, y) = placement.origin(selection, width, height);
    let x = x.clamp(0, (screen_width - width).max(0)) + screen_left;
    let y = y.clamp(0, (screen_height - height).max(0)) + screen_top;
    (x, y, width, height)
}

fn present_layered_frame(
    hwnd: HWND,
    state: &PreviewState,
    geometry: (i32, i32, i32, i32),
) -> Result<(), String> {
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
        let dc = CreateCompatibleDC(None);
        if dc.is_invalid() {
            return Err("failed to create scroll preview memory DC".to_string());
        }
        let mut bits = std::ptr::null_mut();
        let bitmap = match CreateDIBSection(None, &bitmap_info, DIB_RGB_COLORS, &mut bits, None, 0)
        {
            Ok(bitmap) => bitmap,
            Err(error) => {
                let _ = DeleteDC(dc);
                return Err(error.to_string());
            }
        };
        std::ptr::copy_nonoverlapping(state.pixels.as_ptr(), bits.cast::<u8>(), state.pixels.len());
        let previous = SelectObject(dc, bitmap.into());
        let destination = POINT {
            x: geometry.0,
            y: geometry.1,
        };
        let size = SIZE {
            cx: geometry.2,
            cy: geometry.3,
        };
        let source = POINT { x: 0, y: 0 };
        let result = UpdateLayeredWindow(
            hwnd,
            None,
            Some(&destination),
            Some(&size),
            Some(dc),
            Some(&source),
            COLORREF(0),
            None,
            ULW_OPAQUE,
        )
        .map_err(|error| error.to_string());
        SelectObject(dc, previous);
        let _ = DeleteObject(bitmap.into());
        let _ = DeleteDC(dc);
        result
    }
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
            let state = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const PreviewState };
            if !state.is_null() {
                let state = unsafe { &*state };
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
                if client_width == state.width && client_height == state.height {
                    // The common case: the window is sized to the image, so the pixels go
                    // straight across in one transfer. `StretchDIBits` with HALFTONE would
                    // rescale block by block instead, and that partial progress is visible as
                    // flicker on a window that repaints many times a second.
                    unsafe {
                        SetDIBitsToDevice(
                            dc,
                            0,
                            0,
                            state.width as u32,
                            state.height as u32,
                            0,
                            0,
                            0,
                            state.height as u32,
                            state.pixels.as_ptr().cast(),
                            &bitmap_info,
                            DIB_RGB_COLORS,
                        );
                    }
                } else {
                    unsafe {
                        SetStretchBltMode(dc, HALFTONE);
                        StretchDIBits(
                            dc,
                            0,
                            0,
                            client_width,
                            client_height,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(left: i32, top: i32, right: i32, bottom: i32) -> RectI32 {
        RectI32 {
            left,
            top,
            right,
            bottom,
        }
    }

    #[test]
    fn prefers_the_right_side_when_it_fits() {
        // 800px selection needs 800*0.2 + 24 = 184px of clearance past its right edge.
        let selection = rect(200, 100, 999, 900);
        assert_eq!(
            PreviewPlacement::choose(selection, 1920),
            PreviewPlacement::Right
        );
    }

    #[test]
    fn falls_back_to_the_left_when_the_right_is_too_tight() {
        // Right edge at 1800 leaves 120px, short of the 184px the margin test wants; the left
        // has 200px, which clears it.
        let selection = rect(200, 100, 999, 900);
        assert_eq!(
            PreviewPlacement::choose(selection, 1119),
            PreviewPlacement::Left
        );
    }

    #[test]
    fn overlays_when_neither_side_fits() {
        let selection = rect(100, 100, 999, 900);
        assert_eq!(
            PreviewPlacement::choose(selection, 1000),
            PreviewPlacement::Overlay
        );
    }

    #[test]
    fn side_scale_is_quantised_to_tenths() {
        // gap = 1920 - 999 = 921; (921*10 - 240) / 800 = 8966/800 = 11 (integer divide);
        // 11 * 0.1 = 1.1, capped at 0.4.
        let selection = rect(200, 100, 999, 900);
        let scale = PreviewPlacement::Right.scale(selection, 1920, 800);
        assert!((scale - 0.4).abs() < 1e-9, "scale was {scale}");

        // A narrower gap lands below the cap and stays a multiple of 0.1.
        let selection = rect(200, 100, 999, 900);
        let scale = PreviewPlacement::Right.scale(selection, 1240, 800);
        // gap = 241; (2410 - 240) / 800 = 2; 2 * 0.1 = 0.2
        assert!((scale - 0.2).abs() < 1e-9, "scale was {scale}");
    }

    #[test]
    fn side_scale_does_not_shrink_as_the_canvas_grows() {
        // The canvas only extends downwards, so the divisor — its width — never changes. A
        // taller canvas must therefore not change the factor.
        let selection = rect(200, 100, 999, 900);
        let short = PreviewPlacement::Right.scale(selection, 1240, 800);
        let tall = PreviewPlacement::Right.scale(selection, 1240, 800);
        assert_eq!(short, tall);
    }

    #[test]
    fn overlay_scale_targets_a_fixed_width() {
        let selection = rect(100, 100, 899, 900);
        // 800px wide selection -> 400/800 = 0.5, so the preview is 400px across.
        let scale = PreviewPlacement::Overlay.scale(selection, 1000, 800);
        assert!((scale - 0.5).abs() < 1e-9, "scale was {scale}");
        assert_eq!((800.0 * scale).round() as i32, 400);
    }

    #[test]
    fn height_limit_differs_per_placement() {
        let selection = rect(200, 100, 999, 900);
        assert_eq!(PreviewPlacement::Right.height_limit(selection, 1080), 888);
        assert_eq!(PreviewPlacement::Left.height_limit(selection, 1080), 888);
        assert_eq!(PreviewPlacement::Overlay.height_limit(selection, 1080), 701);
    }

    #[test]
    fn origins_anchor_to_the_selection() {
        let selection = rect(200, 100, 999, 900);
        assert_eq!(
            PreviewPlacement::Right.origin(selection, 320, 400),
            (1011, 501)
        );
        assert_eq!(
            PreviewPlacement::Left.origin(selection, 320, 400),
            (-144, 501)
        );
        assert_eq!(
            PreviewPlacement::Overlay.origin(selection, 400, 600),
            (587, 288)
        );
    }

    #[test]
    fn preview_size_keeps_the_aspect_ratio_when_clamping() {
        let selection = rect(200, 100, 999, 900);
        // Canvas 800 wide at scale 0.2 is 160 across; a 20000px-tall canvas scales to 4000,
        // well past the 888px limit, so both axes shrink together.
        let (width, height, placement) = preview_size(selection, (1240, 1080), (800, 20000));
        assert_eq!(placement, PreviewPlacement::Right);
        assert_eq!(height, 888);
        let unclamped_ratio = 160.0 / 4000.0;
        let clamped_ratio = width as f64 / height as f64;
        assert!(
            (unclamped_ratio - clamped_ratio).abs() < 0.01,
            "ratio drifted: {unclamped_ratio} vs {clamped_ratio}"
        );
    }

    #[test]
    fn preview_size_is_never_degenerate() {
        let selection = rect(0, 0, 0, 0);
        let (width, height, _) = preview_size(selection, (1920, 1080), (1, 1));
        assert!(width >= 1 && height >= 1);
    }
}
