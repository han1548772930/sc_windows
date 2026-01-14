use sc_platform::WindowEvent;

use crate::win32::{
    LPARAM, RECT, SIZE_MINIMIZED, WM_DISPLAYCHANGE, WM_DPICHANGED, WM_SIZE, WPARAM,
};

pub struct WindowEventConverter;

impl WindowEventConverter {
    pub fn convert(msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<WindowEvent> {
        match msg {
            WM_SIZE => {
                if wparam.0 as u32 == SIZE_MINIMIZED {
                    return None;
                }

                let lp = lparam.0 as u32;
                let width = ((lp & 0xFFFF) as i32).max(1);
                let height = (((lp >> 16) & 0xFFFF) as i32).max(1);
                Some(WindowEvent::Resized { width, height })
            }

            WM_DPICHANGED => {
                let wp = wparam.0 as u32;
                let dpi_x = wp & 0xFFFF;
                let dpi_y = (wp >> 16) & 0xFFFF;

                let rect_ptr = lparam.0 as *const RECT;
                if rect_ptr.is_null() {
                    return None;
                }
                let rect = unsafe { *rect_ptr };

                Some(WindowEvent::DpiChanged {
                    dpi_x,
                    dpi_y,
                    suggested_rect: (rect.left, rect.top, rect.right, rect.bottom),
                })
            }

            WM_DISPLAYCHANGE => {
                let bits_per_pixel = wparam.0 as u32;
                let lp = lparam.0 as u32;
                let width = (lp & 0xFFFF) as i32;
                let height = ((lp >> 16) & 0xFFFF) as i32;

                Some(WindowEvent::DisplayChanged {
                    bits_per_pixel,
                    width,
                    height,
                })
            }

            _ => None,
        }
    }
}
