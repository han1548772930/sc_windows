use sc_platform::{InputEvent, KeyCode, Modifiers, MouseButton};
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CONTROL, VK_MENU, VK_SHIFT};
use windows::Win32::UI::WindowsAndMessaging::*;

#[inline]
fn current_modifiers() -> Modifiers {
    unsafe {
        Modifiers {
            ctrl: (GetKeyState(VK_CONTROL.0 as i32) as u16 & 0x8000) != 0,
            shift: (GetKeyState(VK_SHIFT.0 as i32) as u16 & 0x8000) != 0,
            alt: (GetKeyState(VK_MENU.0 as i32) as u16 & 0x8000) != 0,
        }
    }
}

/// Extract mouse coordinates from Win32 `LPARAM`.
#[inline]
fn extract_mouse_coords(lparam: LPARAM) -> (i32, i32) {
    let x = (lparam.0 as i16) as i32;
    let y = ((lparam.0 >> 16) as i16) as i32;
    (x, y)
}

/// Convert Win32 window messages to platform-agnostic [`InputEvent`].
pub struct EventConverter;

impl EventConverter {
    /// Returns `None` if the message is not an input event.
    pub fn convert(msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<InputEvent> {
        match msg {
            WM_MOUSEMOVE => {
                let (x, y) = extract_mouse_coords(lparam);
                Some(InputEvent::MouseMove { x, y })
            }

            WM_LBUTTONDOWN => {
                let (x, y) = extract_mouse_coords(lparam);
                Some(InputEvent::MouseDown {
                    x,
                    y,
                    button: MouseButton::Left,
                })
            }

            WM_LBUTTONUP => {
                let (x, y) = extract_mouse_coords(lparam);
                Some(InputEvent::MouseUp {
                    x,
                    y,
                    button: MouseButton::Left,
                })
            }

            WM_RBUTTONDOWN => {
                let (x, y) = extract_mouse_coords(lparam);
                Some(InputEvent::MouseDown {
                    x,
                    y,
                    button: MouseButton::Right,
                })
            }

            WM_RBUTTONUP => {
                let (x, y) = extract_mouse_coords(lparam);
                Some(InputEvent::MouseUp {
                    x,
                    y,
                    button: MouseButton::Right,
                })
            }

            WM_MBUTTONDOWN => {
                let (x, y) = extract_mouse_coords(lparam);
                Some(InputEvent::MouseDown {
                    x,
                    y,
                    button: MouseButton::Middle,
                })
            }

            WM_MBUTTONUP => {
                let (x, y) = extract_mouse_coords(lparam);
                Some(InputEvent::MouseUp {
                    x,
                    y,
                    button: MouseButton::Middle,
                })
            }

            WM_LBUTTONDBLCLK => {
                let (x, y) = extract_mouse_coords(lparam);
                Some(InputEvent::DoubleClick {
                    x,
                    y,
                    button: MouseButton::Left,
                })
            }

            WM_RBUTTONDBLCLK => {
                let (x, y) = extract_mouse_coords(lparam);
                Some(InputEvent::DoubleClick {
                    x,
                    y,
                    button: MouseButton::Right,
                })
            }

            WM_KEYDOWN => {
                let key = KeyCode(wparam.0 as u32);
                let modifiers = current_modifiers();
                Some(InputEvent::KeyDown { key, modifiers })
            }

            WM_KEYUP => {
                let key = KeyCode(wparam.0 as u32);
                let modifiers = current_modifiers();
                Some(InputEvent::KeyUp { key, modifiers })
            }

            WM_CHAR => char::from_u32(wparam.0 as u32).and_then(|character| {
                // Only handle printable characters and space/tab.
                if !character.is_control() || character == ' ' || character == '\t' {
                    Some(InputEvent::TextInput { character })
                } else {
                    None
                }
            }),

            WM_HOTKEY => Some(InputEvent::Hotkey {
                id: wparam.0 as u32,
            }),

            WM_TIMER => Some(InputEvent::Timer {
                id: wparam.0 as u32,
            }),

            WM_MOUSEWHEEL => {
                let (x, y) = extract_mouse_coords(lparam);
                let delta = ((wparam.0 >> 16) as i16) as i32;
                Some(InputEvent::MouseWheel { x, y, delta })
            }

            _ => None,
        }
    }
}
