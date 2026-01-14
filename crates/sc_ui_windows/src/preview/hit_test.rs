use windows::Win32::Foundation::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use super::types::SvgIcon;
use super::window::PreviewWindowState;
use crate::constants::TITLE_BAR_HEIGHT;

#[inline]
pub(super) fn icon_contains_hover_point(icon: &SvgIcon, x: i32, y: i32) -> bool {
    sc_ui::preview_hit_test::icon_contains_hover_point(icon.rect, x, y)
}

#[inline]
pub(super) fn icon_contains_click_point(icon: &SvgIcon, x: i32, y: i32) -> bool {
    sc_ui::preview_hit_test::icon_contains_click_point(icon.rect, icon.is_title_bar_button, x, y)
}

pub(super) fn update_icon_hover_states(icons: &mut [SvgIcon], x: i32, y: i32) -> bool {
    let mut changed = false;

    if (0..=TITLE_BAR_HEIGHT).contains(&y) {
        for icon in icons {
            let hovered = icon_contains_hover_point(icon, x, y);
            if icon.hovered != hovered {
                icon.hovered = hovered;
                changed = true;
            }
        }
    } else {
        for icon in icons {
            if icon.hovered {
                icon.hovered = false;
                changed = true;
            }
        }
    }

    changed
}

impl PreviewWindowState {
    pub(super) fn hit_test_nca(hwnd: HWND, lparam: LPARAM) -> LRESULT {
        unsafe {
            let pt_mouse_x = (lparam.0 as i16) as i32;
            let pt_mouse_y = ((lparam.0 >> 16) as i16) as i32;

            let mut rc_window = RECT::default();
            let _ = GetWindowRect(hwnd, &mut rc_window);

            let client_x = pt_mouse_x - rc_window.left;
            let client_y = pt_mouse_y - rc_window.top;

            let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut PreviewWindowState;
            if !window_ptr.is_null() {
                let window = &*window_ptr;
                if (0..TITLE_BAR_HEIGHT).contains(&client_y) {
                    for icon in &window.svg_icons {
                        if icon_contains_click_point(icon, client_x, client_y) {
                            return LRESULT(HTCLIENT as isize);
                        }
                    }
                }
            }

            let mut rc_frame = RECT::default();
            let _ = AdjustWindowRectEx(
                &mut rc_frame,
                WS_OVERLAPPEDWINDOW & !WS_CAPTION,
                false,
                WS_EX_OVERLAPPEDWINDOW,
            );

            let mut u_row = 1;
            let mut u_col = 1;
            let mut f_on_resize_border = false;

            if pt_mouse_y >= rc_window.top && pt_mouse_y < rc_window.top + TITLE_BAR_HEIGHT {
                f_on_resize_border = pt_mouse_y < (rc_window.top - rc_frame.top);
                u_row = 0;
            } else if pt_mouse_y < rc_window.bottom && pt_mouse_y >= rc_window.bottom - 5 {
                u_row = 2;
            }

            if pt_mouse_x >= rc_window.left && pt_mouse_x < rc_window.left + 5 {
                u_col = 0;
            } else if pt_mouse_x < rc_window.right && pt_mouse_x >= rc_window.right - 5 {
                u_col = 2;
            }

            let hit_tests = [
                [
                    HTTOPLEFT,
                    if f_on_resize_border { HTTOP } else { HTCAPTION },
                    HTTOPRIGHT,
                ],
                [HTLEFT, HTCLIENT, HTRIGHT],
                [HTBOTTOMLEFT, HTBOTTOM, HTBOTTOMRIGHT],
            ];

            LRESULT(hit_tests[u_row][u_col] as isize)
        }
    }
}
