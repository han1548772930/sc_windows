use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Dwm::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use sc_platform::HostPlatform;
use sc_platform_windows::windows::{WindowsHostPlatform, window_id as to_window_id};

use super::hit_test::{icon_contains_click_point, update_icon_hover_states};
use super::renderer::PreviewRenderArgs;
use super::window::{PreviewWindowState, WM_APP_PREVIEW_OCR_DONE};
use sc_ui::preview_layout;

impl PreviewWindowState {
    pub(super) unsafe extern "system" fn window_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
            if msg == WM_LBUTTONDOWN || msg == WM_LBUTTONUP || msg == WM_MOUSEMOVE {
                let result = Self::custom_caption_proc(hwnd, msg, wparam, lparam);
                if result.0 == 0 && msg == WM_LBUTTONDOWN {
                    return result;
                }
            }

            let dwm_enabled = DwmIsCompositionEnabled().unwrap_or(FALSE);
            if dwm_enabled.as_bool() {
                let mut lret = LRESULT(0);
                let call_dwp = !DwmDefWindowProc(hwnd, msg, wparam, lparam, &mut lret).as_bool();
                if call_dwp {
                    Self::custom_caption_proc(hwnd, msg, wparam, lparam)
                } else {
                    lret
                }
            } else {
                Self::app_window_proc(hwnd, msg, wparam, lparam)
            }
        }));

        match result {
            Ok(lresult) => lresult,
            Err(_) => unsafe {
                eprintln!("Panic in window_proc! msg={}", msg);
                DefWindowProcW(hwnd, msg, wparam, lparam)
            },
        }
    }

    fn custom_caption_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        unsafe {
            match msg {
                WM_ERASEBKGND => LRESULT(1),

                WM_SIZE => {
                    let new_width = (lparam.0 & 0xFFFF) as i32;
                    let new_height = ((lparam.0 >> 16) & 0xFFFF) as i32;
                    let window_ptr =
                        GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut PreviewWindowState;
                    if !window_ptr.is_null() {
                        let window = &mut *window_ptr;
                        let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
                        window.is_maximized = (style & WS_MAXIMIZE.0) != 0;
                        window.window_width = new_width;
                        window.window_height = new_height;
                        window.update_title_bar_buttons();
                        window.recalculate_layout();
                    }
                    LRESULT(0)
                }

                WM_PAINT => {
                    let window_ptr =
                        GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut PreviewWindowState;
                    if !window_ptr.is_null() {
                        let window = &mut *window_ptr;
                        let window_id = window.window_id();
                        if let Some(renderer) = &mut window.renderer {
                            let mut rect = RECT::default();
                            let _ = GetClientRect(hwnd, &mut rect);
                            let width = rect.right - rect.left;
                            let height = rect.bottom - rect.top;

                            if width > 0 && height > 0 {
                                if let Err(e) = renderer.initialize(window_id, width, height) {
                                    eprintln!("PreviewWindow: renderer.initialize failed: {:?}", e);
                                }
                                if let Err(e) = renderer.set_image_from_pixels(
                                    &window.image_pixels,
                                    window.image_width,
                                    window.image_height,
                                ) {
                                    eprintln!(
                                        "PreviewWindow: set_image_from_pixels failed: {:?}",
                                        e
                                    );
                                }

                                let args = PreviewRenderArgs {
                                    text_lines: &window.text_lines,
                                    text_rect: window.text_area_rect,
                                    width,
                                    icons: &window.svg_icons,
                                    is_pinned: window.is_pinned,
                                    scroll_offset: window.scroll_offset,
                                    line_height: window.line_height,
                                    image_width: window.image_width,
                                    image_height: window.image_height,
                                    selection: window
                                        .selection_start
                                        .and_then(|s| window.selection_end.map(|e| (s, e))),
                                    show_text_area: window.show_text_area,
                                    drawing_state: window.drawing_state.as_mut(),
                                };

                                if let Err(e) = renderer.render(args) {
                                    eprintln!("PreviewWindow: render failed: {:?}", e);
                                } else {
                                    let _ = ValidateRect(Some(hwnd), None);
                                }
                            }
                        }
                    }
                    LRESULT(0)
                }

                WM_NCCALCSIZE => {
                    if wparam.0 == 1 {
                        let params = lparam.0 as *mut NCCALCSIZE_PARAMS;
                        if !params.is_null() {
                            let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
                            let is_maximized = (style & WS_MAXIMIZE.0) != 0;
                            if is_maximized {
                                let frame_thickness = Self::get_frame_thickness(hwnd);
                                let rgrc = &mut (*params).rgrc;
                                rgrc[0].top += frame_thickness;
                                rgrc[0].bottom -= frame_thickness;
                                rgrc[0].left += frame_thickness;
                                rgrc[0].right -= frame_thickness;
                            }
                        }
                        return LRESULT(0);
                    }
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }

                WM_NCHITTEST => Self::hit_test_nca(hwnd, lparam),

                _ => Self::app_window_proc(hwnd, msg, wparam, lparam),
            }
        }
    }

    fn app_window_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        unsafe {
            match msg {
                WM_NCCALCSIZE => {
                    if wparam.0 == 1 {
                        return LRESULT(0);
                    }
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }

                WM_NCPAINT => LRESULT(0),

                WM_NCACTIVATE => LRESULT(1),

                WM_GETMINMAXINFO => {
                    let minmax_info = lparam.0 as *mut MINMAXINFO;
                    if !minmax_info.is_null() {
                        let info = &mut *minmax_info;

                        // Minimum width: keep title-bar icons from overlapping.
                        info.ptMinTrackSize.x = PreviewWindowState::min_window_width_for_title_bar();
                        info.ptMinTrackSize.y = 200;
                    }
                    LRESULT(0)
                }

                WM_LBUTTONDOWN => {
                    let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut PreviewWindowState;
                    if !window_ptr.is_null() {
                        let window = &mut *window_ptr;
                        let x = (lparam.0 as i16) as i32;
                        let y = ((lparam.0 >> 16) as i16) as i32;

                        let window_id = window.window_id();
                        let platform = WindowsHostPlatform::new();

                        // 点击图标处理
                        for icon in &mut window.svg_icons {
                            if icon_contains_click_point(icon, x, y) {
                                match icon.name.as_str() {
                                    preview_layout::ICON_WINDOW_MINIMIZE => {
                                        let _ = platform.minimize_window(window_id);
                                        return LRESULT(0);
                                    }
                                    preview_layout::ICON_PIN => {
                                        window.is_pinned = !window.is_pinned;
                                        let _ = platform.set_window_topmost_flag(window_id, window.is_pinned);
                                        let _ = platform.request_redraw(window_id);
                                        return LRESULT(0);
                                    }
                                    preview_layout::ICON_SAVE => {
                                        window.save_image_to_file();
                                        let _ = platform.request_redraw(window_id);
                                        return LRESULT(0);
                                    }
                                    preview_layout::ICON_OCR => {
                                        window.toggle_ocr_text_panel();
                                        let _ = platform.request_redraw(window_id);
                                        return LRESULT(0);
                                    }
                                    preview_layout::ICON_WINDOW_MAXIMIZE => {
                                        let _ = platform.maximize_window(window_id);
                                        return LRESULT(0);
                                    }
                                    preview_layout::ICON_WINDOW_RESTORE => {
                                        let _ = platform.restore_window(window_id);
                                        return LRESULT(0);
                                    }
                                    preview_layout::ICON_WINDOW_CLOSE => {
                                        let _ = platform.request_close(window_id);
                                        return LRESULT(0);
                                    }

                                    // 绘图工具图标
                                    preview_layout::ICON_TOOL_SQUARE => {
                                        window.switch_drawing_tool(sc_drawing_host::DrawingTool::Rectangle);
                                        let _ = platform.request_redraw(window_id);
                                        return LRESULT(0);
                                    }
                                    preview_layout::ICON_TOOL_CIRCLE => {
                                        window.switch_drawing_tool(sc_drawing_host::DrawingTool::Circle);
                                        let _ = platform.request_redraw(window_id);
                                        return LRESULT(0);
                                    }
                                    preview_layout::ICON_TOOL_ARROW => {
                                        window.switch_drawing_tool(sc_drawing_host::DrawingTool::Arrow);
                                        let _ = platform.request_redraw(window_id);
                                        return LRESULT(0);
                                    }
                                    preview_layout::ICON_TOOL_PEN => {
                                        window.switch_drawing_tool(sc_drawing_host::DrawingTool::Pen);
                                        let _ = platform.request_redraw(window_id);
                                        return LRESULT(0);
                                    }
                                    preview_layout::ICON_TOOL_TEXT => {
                                        window.switch_drawing_tool(sc_drawing_host::DrawingTool::Text);
                                        let _ = platform.request_redraw(window_id);
                                        return LRESULT(0);
                                    }
                                    _ => {}
                                }
                            }
                        }

                        // 绘图交互处理
                        if let Some(ds) = window.drawing_state.as_mut() && ds.handle_mouse_down(x, y) {
                            let _ = SetCapture(hwnd);
                            return LRESULT(0);
                        }

                        // 文本选择逻辑 (仅当显示文本区域时)
                        if window.show_text_area {
                            let text_rect = window.text_area_rect;
                            if x >= text_rect.left
                                && x <= text_rect.right
                                && y >= text_rect.top
                                && y <= text_rect.bottom
                            {
                                let relative_y = y - text_rect.top + window.scroll_offset;
                                let line_index = (relative_y / window.line_height) as usize;

                                if line_index < window.text_lines.len() {
                                    let relative_x = (x - text_rect.left) as f32;
                                    let line = &window.text_lines[line_index];
                                    let char_index = if let Some(renderer) = &window.renderer {
                                        renderer.get_text_position_from_point(line, relative_x)
                                    } else {
                                        0
                                    };

                                    window.is_selecting = true;
                                    window.selection_start = Some((line_index, char_index));
                                    window.selection_end = Some((line_index, char_index));
                                    let _ = SetCapture(hwnd);

                                    let _ = platform.request_redraw(window_id);
                                }
                            }
                        }
                    }

                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }

                WM_LBUTTONUP => {
                    let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut PreviewWindowState;
                    if !window_ptr.is_null() {
                        let window = &mut *window_ptr;
                        let x = (lparam.0 as i16) as i32;
                        let y = ((lparam.0 >> 16) as i16) as i32;

                        // 绘图交互处理
                        if let Some(ds) = window.drawing_state.as_mut() && ds.handle_mouse_up(x, y) {
                            let _ = ReleaseCapture();
                            return LRESULT(0);
                        }

                        if window.is_selecting {
                            window.is_selecting = false;
                            let _ = ReleaseCapture();
                        }
                    }

                    let _ = WindowsHostPlatform::new().request_redraw(to_window_id(hwnd));
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }

                WM_MOUSEMOVE => {
                    let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut PreviewWindowState;
                    if !window_ptr.is_null() {
                        let window = &mut *window_ptr;
                        let x = (lparam.0 as i16) as i32;
                        let y = ((lparam.0 >> 16) as i16) as i32;

                        let mut tme = TRACKMOUSEEVENT {
                            cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                            dwFlags: TME_LEAVE,
                            hwndTrack: hwnd,
                            dwHoverTime: 0,
                        };
                        let _ = TrackMouseEvent(&mut tme);

                        let mut needs_repaint = false;
                        if update_icon_hover_states(&mut window.svg_icons, x, y) {
                            needs_repaint = true;
                        }

                        // 绘图交互处理
                        if let Some(ds) = window.drawing_state.as_mut() && ds.handle_mouse_move(x, y) {
                            needs_repaint = true;
                        }

                        // 文本选择移动逻辑
                        if window.is_selecting && window.show_text_area {
                            let text_rect = window.text_area_rect;
                            let clamped_x = x.max(text_rect.left).min(text_rect.right);
                            let clamped_y = y.max(text_rect.top).min(text_rect.bottom);

                            let relative_y = clamped_y - text_rect.top + window.scroll_offset;
                            let line_index = ((relative_y / window.line_height) as usize)
                                .min(window.text_lines.len().saturating_sub(1));

                            let relative_x = (clamped_x - text_rect.left) as f32;
                            let line = &window.text_lines[line_index];
                            let char_index = if let Some(renderer) = &window.renderer {
                                renderer.get_text_position_from_point(line, relative_x)
                            } else {
                                0
                            };

                            window.selection_end = Some((line_index, char_index));
                            needs_repaint = true;
                        }

                        window.update_cursor(x, y);

                        if needs_repaint {
                            let _ = WindowsHostPlatform::new().request_redraw(window.window_id());
                        }
                    }

                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }

                WM_LBUTTONDBLCLK => {
                    let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut PreviewWindowState;
                    if !window_ptr.is_null() {
                        let window = &mut *window_ptr;
                        let x = (lparam.0 as i16) as i32;
                        let y = ((lparam.0 >> 16) as i16) as i32;

                        // 绘图双击处理（用于编辑文本元素）
                        if let Some(ds) = window.drawing_state.as_mut() && ds.handle_double_click(x, y) {
                            return LRESULT(0);
                        }
                    }
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }

                WM_SETCURSOR => {
                    // Only override the cursor for client-area hit tests; let Windows handle non-client
                    // resizing cursors.
                    let hit_test = (lparam.0 & 0xFFFF) as u16;
                    if hit_test as u32 != HTCLIENT {
                        return DefWindowProcW(hwnd, msg, wparam, lparam);
                    }

                    let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut PreviewWindowState;
                    if window_ptr.is_null() {
                        return DefWindowProcW(hwnd, msg, wparam, lparam);
                    }

                    let window = &mut *window_ptr;
                    let mut pt = POINT::default();
                    let _ = GetCursorPos(&mut pt);
                    let _ = ScreenToClient(hwnd, &mut pt);
                    window.update_cursor(pt.x, pt.y);

                    LRESULT(1)
                }

                WM_MOUSEWHEEL => {
                    let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut PreviewWindowState;
                    if !window_ptr.is_null() {
                        let window = &mut *window_ptr;
                        if window.show_text_area {
                            let delta = ((wparam.0 >> 16) as i16) as i32;
                            let scroll_amount = (delta / 120) * window.line_height * 3;
                            window.scroll_offset -= scroll_amount;

                            let max_scroll = (window.text_lines.len() as i32 * window.line_height)
                                - (window.text_area_rect.bottom - window.text_area_rect.top);
                            window.scroll_offset = window.scroll_offset.clamp(0, max_scroll.max(0));

                            let rect = window.text_area_rect;
                            let _ = WindowsHostPlatform::new().request_redraw_rect(
                                window.window_id(),
                                rect.left,
                                rect.top,
                                rect.right,
                                rect.bottom,
                            );
                        }
                    }
                    LRESULT(0)
                }

                0x02A3 /* WM_MOUSELEAVE */ => {
                    let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut PreviewWindowState;
                    if !window_ptr.is_null() {
                        let window = &mut *window_ptr;
                        let mut needs_repaint = false;
                        for icon in &mut window.svg_icons {
                            if icon.hovered {
                                icon.hovered = false;
                                needs_repaint = true;
                            }
                        }
                        if needs_repaint {
                            let _ = WindowsHostPlatform::new().request_redraw(window.window_id());
                        }
                    }
                    LRESULT(0)
                }

                WM_CHAR => {
                    let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut PreviewWindowState;
                    if !window_ptr.is_null() {
                        let window = &mut *window_ptr;
                        let character = char::from_u32(wparam.0 as u32);

                        // 绘图文本输入处理
                        if let Some(ch) = character
                            && let Some(ds) = window.drawing_state.as_mut()
                            && ds.handle_char_input(ch)
                        {
                            return LRESULT(0);
                        }
                    }
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }

                WM_KEYDOWN => {
                    let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut PreviewWindowState;
                    if !window_ptr.is_null() {
                        let window = &mut *window_ptr;
                        let vk = wparam.0 as u32;
                        let ctrl_pressed = (GetKeyState(0x11 /* VK_CONTROL */) as u16 & 0x8000) != 0;

                        // 绘图键盘处理（文本编辑状态下的特殊键：方向键、退格、回车、Escape 等）
                        if let Some(ds) = window.drawing_state.as_mut()
                            && ds.is_text_editing()
                            && ds.handle_key_input(vk)
                        {
                            return LRESULT(0);
                        }

                        if ctrl_pressed && window.show_text_area {
                            match vk {
                                0x41 /* VK_A */ => {
                                    // Ctrl+A: 全选
                                    if !window.text_lines.is_empty() {
                                        window.selection_start = Some((0, 0));
                                        let last_line = window.text_lines.len() - 1;
                                        let last_char = window.text_lines[last_line].chars().count();
                                        window.selection_end = Some((last_line, last_char));
                                        let _ = WindowsHostPlatform::new().request_redraw(window.window_id());
                                    }
                                    return LRESULT(0);
                                }
                                0x43 /* VK_C */ => {
                                    // Ctrl+C: 复制选中文本
                                    if let (Some(start), Some(end)) =
                                        (window.selection_start, window.selection_end)
                                    {
                                        let (start, end) = if start <= end { (start, end) } else { (end, start) };
                                        let mut selected_text = String::new();

                                        for i in start.0..=end.0 {
                                            if i >= window.text_lines.len() {
                                                break;
                                            }
                                            let line = &window.text_lines[i];
                                            let chars: Vec<char> = line.chars().collect();

                                            let start_char = if i == start.0 { start.1 } else { 0 };
                                            let end_char = if i == end.0 { end.1 } else { chars.len() };

                                            if start_char < chars.len() {
                                                let slice: String =
                                                    chars[start_char..end_char.min(chars.len())].iter().collect();
                                                selected_text.push_str(&slice);
                                            }
                                            if i < end.0 {
                                                selected_text.push('\n');
                                            }
                                        }

                                        if !selected_text.is_empty() {
                                            let platform = WindowsHostPlatform::new();
                                            let _ = platform.copy_text_to_clipboard(&selected_text);
                                        }
                                    }
                                    return LRESULT(0);
                                }
                                _ => {}
                            }
                        }
                    }
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }

                WM_APP_PREVIEW_OCR_DONE => {
                    let window_ptr =
                        GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut PreviewWindowState;
                    if !window_ptr.is_null() {
                        let window = &mut *window_ptr;
                        window.handle_ocr_done_message(wparam.0 as u64);
                    }
                    LRESULT(0)
                }

                WM_TIMER => {
                    let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut PreviewWindowState;
                    if !window_ptr.is_null() {
                        let window = &mut *window_ptr;
                        let timer_id = wparam.0 as u32;

                        if let Some(ref mut ds) = window.drawing_state {
                            ds.handle_cursor_timer(timer_id);
                        }
                    }
                    LRESULT(0)
                }

                WM_DESTROY => {

                    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut PreviewWindowState;
                    if !ptr.is_null() {
                        let mut window = Box::from_raw(ptr);
                        window.cleanup_all_resources();
                    }
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                    LRESULT(0)
                }

                _ => DefWindowProcW(hwnd, msg, wparam, lparam),
            }
        }
    }
}
