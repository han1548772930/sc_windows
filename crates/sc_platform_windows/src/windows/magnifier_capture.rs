//! Magnification-API screen grabbing for long screenshots.
//!
//! The Magnification API is used instead of `BitBlt`: its scaling callback hands over the
//! source scan lines directly, so a frame costs one `memcpy` per row with no `GetDIBits`,
//! encode, or colour conversion on the path. The source is already 32bpp BGRA.
//!
//! Excluding the capture UI from the frame is done with
//! `MagSetWindowFilterList(MW_FILTERMODE_EXCLUDE)` rather than by punching a hole in the
//! overlay's window region.

use std::cell::RefCell;

use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{HRGN, UpdateWindow};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Magnification::{
    MAGIMAGEHEADER, MW_FILTERMODE_EXCLUDE, MagInitialize, MagSetImageScalingCallback,
    MagSetWindowFilterList, MagSetWindowSource, MagUninitialize, WC_MAGNIFIER,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, HMENU, LWA_ALPHA, MSG,
    PM_REMOVE, PeekMessageW, RegisterClassExW, SW_SHOWNOACTIVATE, SetLayeredWindowAttributes,
    ShowWindow, TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE, WNDCLASSEXW, WS_EX_LAYERED,
    WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
};
use windows::core::{BOOL, PCWSTR, w};

use sc_drawing::Rect;

/// Window class hosting the magnifier control.
const HOST_CLASS: PCWSTR = w!("MagnifierHostClass");
/// Title of the host window.
const HOST_TITLE: PCWSTR = w!("MagnifierHost");

/// The host is layered so it can be made fully transparent, click-through, and off the taskbar.
const HOST_EX_STYLE: WINDOW_EX_STYLE =
    WINDOW_EX_STYLE(WS_EX_LAYERED.0 | WS_EX_TRANSPARENT.0 | WS_EX_TOOLWINDOW.0);
/// `WS_POPUP`.
const HOST_STYLE: WINDOW_STYLE = WINDOW_STYLE(0x8000_0000);
/// `WS_CHILD | WS_VISIBLE`.
const MAGNIFIER_STYLE: WINDOW_STYLE = WINDOW_STYLE(0x5000_0000);

/// Pump interval and iteration cap used while waiting for the scaling callback to deliver.
const PUMP_SLEEP: std::time::Duration = std::time::Duration::from_millis(10);
const PUMP_MAX_ITERATIONS: u32 = 9;

thread_local! {
    /// Destination for the scaling callback. The Magnification API invokes the callback on the
    /// thread that owns the magnifier window, so thread-local storage is sufficient and avoids
    /// locking on the capture path.
    static CALLBACK_TARGET: RefCell<CallbackTarget> = const { RefCell::new(CallbackTarget::new()) };
}

/// Pixels captured by the most recent callback invocation.
struct CallbackTarget {
    pixels: Vec<u8>,
    width: u32,
    height: u32,
    ready: bool,
}

impl CallbackTarget {
    const fn new() -> Self {
        Self {
            pixels: Vec::new(),
            width: 0,
            height: 0,
            ready: false,
        }
    }
}

/// Copy the source scan lines into our own buffer and flag the frame as ready. The source is
/// 32bpp BGRA, so no conversion is required.
unsafe extern "system" fn image_scaling_callback(
    _hwnd: HWND,
    srcdata: *mut core::ffi::c_void,
    srcheader: MAGIMAGEHEADER,
    _destdata: *mut core::ffi::c_void,
    _destheader: MAGIMAGEHEADER,
    _unclipped: RECT,
    _clipped: RECT,
    _dirty: HRGN,
) -> BOOL {
    if srcdata.is_null() {
        return BOOL(0);
    }
    CALLBACK_TARGET.with(|target| {
        let Ok(mut target) = target.try_borrow_mut() else {
            return BOOL(0);
        };
        let width = srcheader.width;
        let height = srcheader.height;
        let row_bytes = width as usize * 4;
        if row_bytes == 0 || height == 0 {
            return BOOL(0);
        }
        target.pixels.resize(row_bytes * height as usize, 0);
        // Copy row by row: the source stride may exceed the packed row width, so the buffer
        // cannot be assumed contiguous.
        let stride = srcheader.stride as usize;
        for row in 0..height as usize {
            let source = unsafe { (srcdata as *const u8).add(row * stride) };
            let destination = target.pixels[row * row_bytes..].as_mut_ptr();
            unsafe { std::ptr::copy_nonoverlapping(source, destination, row_bytes) };
        }
        target.width = width;
        target.height = height;
        target.ready = true;
        BOOL(1)
    })
}

/// The host window only needs default handling; it exists to parent the magnifier control.
unsafe extern "system" fn host_window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}

/// A magnifier host + magnifier child window pair, used as the capture source.
pub struct MagnifierCapture {
    host: HWND,
    magnifier: HWND,
}

impl MagnifierCapture {
    /// Build the window pair.
    ///
    /// `selection` sizes the magnifier child; the host is created at 0x0. The host must be
    /// shown and updated before pumping — the magnifier only renders, and therefore only
    /// invokes the callback, once its host window is visible.
    pub fn new(selection: Rect) -> Result<Self, String> {
        let width = (selection.right - selection.left).max(1);
        let height = (selection.bottom - selection.top).max(1);
        unsafe {
            // Initialise per capture session, paired with `MagUninitialize` in `Drop`. Leaving
            // the runtime initialised across sessions makes a later
            // `MagSetImageScalingCallback` fail.
            MagInitialize()
                .ok()
                .map_err(|error| format!("MagInitialize failed: {error}"))?;

            let instance = GetModuleHandleW(None).map_err(|error| {
                let _ = MagUninitialize();
                format!("GetModuleHandleW failed: {error}")
            })?;

            // The class is process-wide, so registering twice is expected to fail benignly.
            let host_class = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(host_window_proc),
                hInstance: instance.into(),
                lpszClassName: HOST_CLASS,
                ..Default::default()
            };
            RegisterClassExW(&host_class);

            let host = CreateWindowExW(
                HOST_EX_STYLE,
                HOST_CLASS,
                HOST_TITLE,
                HOST_STYLE,
                0,
                0,
                0,
                0,
                None,
                None,
                Some(instance.into()),
                None,
            )
            .map_err(|error| {
                let _ = MagUninitialize();
                format!("magnifier host window creation failed: {error}")
            })?;

            // Fully transparent (alpha 0) so the host never paints on screen.
            let _ = SetLayeredWindowAttributes(host, COLORREF(0), 0, LWA_ALPHA);

            let magnifier = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                WC_MAGNIFIER,
                PCWSTR::null(),
                MAGNIFIER_STYLE,
                0,
                0,
                width,
                height,
                Some(host),
                Some(HMENU(std::ptr::null_mut())),
                Some(instance.into()),
                None,
            )
            .map_err(|error| {
                let _ = DestroyWindow(host);
                let _ = MagUninitialize();
                format!("magnifier window creation failed: {error}")
            })?;

            if !MagSetImageScalingCallback(magnifier, Some(image_scaling_callback)).as_bool() {
                let _ = DestroyWindow(host);
                let _ = MagUninitialize();
                return Err("MagSetImageScalingCallback failed".to_string());
            }

            // The magnifier only renders — and therefore only invokes the callback — once its
            // host window is shown.
            let _ = ShowWindow(host, SW_SHOWNOACTIVATE);
            let _ = UpdateWindow(host);

            Ok(Self { host, magnifier })
        }
    }

    /// Exclude windows from the captured image.
    ///
    /// Invalid handles are dropped before the array reaches `MagSetWindowFilterList`.
    pub fn set_excluded_windows(&self, windows: &[HWND]) -> Result<(), String> {
        let mut live: Vec<HWND> = windows
            .iter()
            .copied()
            .filter(|window| unsafe {
                windows::Win32::UI::WindowsAndMessaging::IsWindow(Some(*window)).as_bool()
            })
            .collect();
        let count = live.len() as i32;
        let ok = unsafe {
            MagSetWindowFilterList(
                self.magnifier,
                MW_FILTERMODE_EXCLUDE,
                count,
                live.as_mut_ptr(),
            )
        };
        if ok.as_bool() {
            Ok(())
        } else {
            Err("MagSetWindowFilterList failed".to_string())
        }
    }

    /// Capture `selection` once, returning top-down 32bpp BGRA pixels.
    pub fn capture(&self, selection: Rect) -> Result<(u32, u32, Vec<u8>), String> {
        let width = selection.right - selection.left;
        let height = selection.bottom - selection.top;
        if width <= 0 || height <= 0 {
            return Err("capture region is empty".to_string());
        }

        CALLBACK_TARGET.with(|target| target.borrow_mut().ready = false);

        unsafe {
            if !MagSetWindowSource(
                self.magnifier,
                RECT {
                    left: selection.left,
                    top: selection.top,
                    right: selection.right,
                    bottom: selection.bottom,
                },
            )
            .as_bool()
            {
                return Err("MagSetWindowSource failed".to_string());
            }

            // Test the ready flag before entering the pump: when `MagSetWindowSource` already
            // drove the callback synchronously the whole pump — including its sleep — is
            // skipped. That is the common case, and is why capture is not limited to 100fps.
            if !CALLBACK_TARGET.with(|target| target.borrow().ready) {
                // Bounded pump: drain pending messages, sleep unconditionally, and only
                // afterwards test the ready flag.
                let mut message = MSG::default();
                let mut iterations = 0u32;
                loop {
                    while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                        let _ = TranslateMessage(&message);
                        DispatchMessageW(&message);
                    }
                    std::thread::sleep(PUMP_SLEEP);
                    if iterations >= PUMP_MAX_ITERATIONS {
                        break;
                    }
                    iterations += 1;
                    if CALLBACK_TARGET.with(|target| target.borrow().ready) {
                        break;
                    }
                }
            }
        }

        CALLBACK_TARGET.with(|target| {
            let target = target.borrow();
            if !target.ready {
                return Err("magnifier callback did not deliver a frame".to_string());
            }
            // The matcher requires every frame to have identical dimensions; a frame of a
            // different size would be rejected and would break the stitch chain, losing content.
            if target.width != width as u32 || target.height != height as u32 {
                return Err(format!(
                    "magnifier delivered {}x{}, expected {width}x{height}",
                    target.width, target.height
                ));
            }
            Ok((target.width, target.height, target.pixels.clone()))
        })
    }
}

impl Drop for MagnifierCapture {
    fn drop(&mut self) {
        unsafe {
            let _ = MagSetImageScalingCallback(self.magnifier, None);
            let _ = DestroyWindow(self.host);
            // Paired with the `MagInitialize` in `new`. Leaving the runtime initialised across
            // sessions makes the *second* `MagSetImageScalingCallback` fail, which silently
            // drops the whole capture back to the GDI path.
            let _ = MagUninitialize();
        }
    }
}
