#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::ffi::OsStr;
use std::iter::once;
use std::os::windows::ffi::OsStrExt;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::LibraryLoader::*;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, VK_ESCAPE, VK_RETURN,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::*;

const WINDOW_CLASS_NAME: &str = "ScreenshotWindow";
const MIN_BOX_SIZE: i32 = 50;
macro_rules! RGB {
    ($r:expr, $g:expr, $b:expr) => {
        COLORREF(($r as u32) | (($g as u32) << 8) | (($b as u32) << 16))
    };
}
// 辅助函数：将字符串转换为宽字符
fn to_wide_chars(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(once(0)).collect()
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum DragMode {
    None,
    Drawing, // 新增：正在画框
    Moving,
    ResizingTopLeft,
    ResizingTopCenter,
    ResizingTopRight,
    ResizingMiddleRight,
    ResizingBottomRight,
    ResizingBottomCenter,
    ResizingBottomLeft,
    ResizingMiddleLeft,
}

#[derive(Debug)]
struct WindowState {
    // 截图相关
    screenshot_dc: HDC,
    screenshot_bitmap: HBITMAP,
    screen_width: i32,
    screen_height: i32,

    // 双缓冲相关
    back_buffer_dc: HDC,
    back_buffer_bitmap: HBITMAP,

    // 选择框
    selection_rect: RECT,
    has_selection: bool,

    // 拖拽状态
    drag_mode: DragMode,
    mouse_pressed: bool,
    drag_start_pos: POINT,
    drag_start_rect: RECT,

    // 绘图相关
    border_pen: HPEN,
    handle_brush: HBRUSH,
    mask_brush: HBRUSH,
}

impl WindowState {
    fn new() -> Result<Self> {
        unsafe {
            let screen_width = GetSystemMetrics(SM_CXSCREEN);
            let screen_height = GetSystemMetrics(SM_CYSCREEN);

            // 创建屏幕DC和兼容DC
            let screen_dc = GetDC(HWND(std::ptr::null_mut()));
            let screenshot_dc = CreateCompatibleDC(screen_dc);
            let screenshot_bitmap = CreateCompatibleBitmap(screen_dc, screen_width, screen_height);

            if screenshot_dc.is_invalid() || screenshot_bitmap.is_invalid() {
                ReleaseDC(HWND(std::ptr::null_mut()), screen_dc);
                return Err(Error::from_win32());
            }

            SelectObject(screenshot_dc, screenshot_bitmap);

            // 捕获屏幕
            BitBlt(
                screenshot_dc,
                0,
                0,
                screen_width,
                screen_height,
                screen_dc,
                0,
                0,
                SRCCOPY,
            );

            // 创建双缓冲DC和位图
            let back_buffer_dc = CreateCompatibleDC(screen_dc);
            let back_buffer_bitmap = CreateCompatibleBitmap(screen_dc, screen_width, screen_height);

            if back_buffer_dc.is_invalid() || back_buffer_bitmap.is_invalid() {
                ReleaseDC(HWND(std::ptr::null_mut()), screen_dc);
                return Err(Error::from_win32());
            }

            SelectObject(back_buffer_dc, back_buffer_bitmap);

            ReleaseDC(HWND(std::ptr::null_mut()), screen_dc);

            // 创建绘图对象
            let border_pen = CreatePen(PS_SOLID, 2, RGB!(0, 120, 215)); // Windows蓝色边框
            let handle_brush = CreateSolidBrush(RGB!(0, 120, 215)); // Windows蓝色手柄

            // 创建半透明遮罩刷子
            let mask_brush = CreateSolidBrush(RGB!(0, 0, 0));

            if border_pen.is_invalid() || handle_brush.is_invalid() || mask_brush.is_invalid() {
                return Err(Error::from_win32());
            }

            // 初始无选择框
            let selection_rect = RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };

            Ok(WindowState {
                screenshot_dc,
                screenshot_bitmap,
                screen_width,
                screen_height,
                back_buffer_dc,
                back_buffer_bitmap,
                selection_rect,
                has_selection: false, // 初始无选择框
                drag_mode: DragMode::None,
                mouse_pressed: false,
                drag_start_pos: POINT { x: 0, y: 0 },
                drag_start_rect: RECT {
                    left: 0,
                    top: 0,
                    right: 0,
                    bottom: 0,
                },
                border_pen,
                handle_brush,
                mask_brush,
            })
        }
    }
    fn get_handle_at_position(&self, x: i32, y: i32) -> DragMode {
        if !self.has_selection {
            return DragMode::None;
        }

        let rect = &self.selection_rect;
        let handle_size = 12;
        let half_handle = handle_size / 2;

        let center_x = (rect.left + rect.right) / 2;
        let center_y = (rect.top + rect.bottom) / 2;

        let handles = [
            (rect.left, rect.top, DragMode::ResizingTopLeft),
            (center_x, rect.top, DragMode::ResizingTopCenter),
            (rect.right, rect.top, DragMode::ResizingTopRight),
            (rect.right, center_y, DragMode::ResizingMiddleRight),
            (rect.right, rect.bottom, DragMode::ResizingBottomRight),
            (center_x, rect.bottom, DragMode::ResizingBottomCenter),
            (rect.left, rect.bottom, DragMode::ResizingBottomLeft),
            (rect.left, center_y, DragMode::ResizingMiddleLeft),
        ];

        // 检查手柄
        for (hx, hy, mode) in handles.iter() {
            if x >= hx - half_handle
                && x <= hx + half_handle
                && y >= hy - half_handle
                && y <= hy + half_handle
            {
                return *mode;
            }
        }

        // 检查是否在选择框内（排除边框区域）
        let border_width = 8; // 边框宽度
        if x >= rect.left + border_width
            && x <= rect.right - border_width
            && y >= rect.top + border_width
            && y <= rect.bottom - border_width
        {
            return DragMode::Moving;
        }

        DragMode::None
    }

    fn get_cursor_for_drag_mode(&self, mode: DragMode) -> PCWSTR {
        match mode {
            DragMode::ResizingTopLeft | DragMode::ResizingBottomRight => IDC_SIZENWSE,
            DragMode::ResizingTopRight | DragMode::ResizingBottomLeft => IDC_SIZENESW,
            DragMode::ResizingTopCenter | DragMode::ResizingBottomCenter => IDC_SIZENS,
            DragMode::ResizingMiddleLeft | DragMode::ResizingMiddleRight => IDC_SIZEWE,
            DragMode::Moving => IDC_SIZEALL,
            DragMode::Drawing => IDC_CROSS, // 新增：画框时的十字光标
            DragMode::None => IDC_CROSS,    // 修改：默认十字光标
        }
    }

    fn start_drag(&mut self, x: i32, y: i32) {
        let handle_mode = self.get_handle_at_position(x, y);

        if handle_mode != DragMode::None {
            // 在现有选择框上操作
            self.drag_mode = handle_mode;
            self.mouse_pressed = true;
            self.drag_start_pos = POINT { x, y };
            self.drag_start_rect = self.selection_rect;
        } else {
            // 开始画新的框
            self.drag_mode = DragMode::Drawing;
            self.mouse_pressed = true;
            self.drag_start_pos = POINT { x, y };
            self.selection_rect = RECT {
                left: x,
                top: y,
                right: x,
                bottom: y,
            };
            self.has_selection = true;
        }
    }

    fn update_drag(&mut self, x: i32, y: i32) {
        if !self.mouse_pressed {
            return;
        }

        match self.drag_mode {
            DragMode::Drawing => {
                // 画框：从起始点到当前点
                let left = self.drag_start_pos.x.min(x);
                let right = self.drag_start_pos.x.max(x);
                let top = self.drag_start_pos.y.min(y);
                let bottom = self.drag_start_pos.y.max(y);

                self.selection_rect = RECT {
                    left: left.max(0),
                    top: top.max(0),
                    right: right.min(self.screen_width),
                    bottom: bottom.min(self.screen_height),
                };
            }

            DragMode::Moving => {
                let dx = x - self.drag_start_pos.x;
                let dy = y - self.drag_start_pos.y;
                let start_rect = self.drag_start_rect;
                let width = start_rect.right - start_rect.left;
                let height = start_rect.bottom - start_rect.top;

                let new_left = (start_rect.left + dx).max(0).min(self.screen_width - width);
                let new_top = (start_rect.top + dy)
                    .max(0)
                    .min(self.screen_height - height);

                self.selection_rect = RECT {
                    left: new_left,
                    top: new_top,
                    right: new_left + width,
                    bottom: new_top + height,
                };
            }

            DragMode::ResizingTopLeft => {
                let start_rect = self.drag_start_rect;
                let dx = x - self.drag_start_pos.x;
                let dy = y - self.drag_start_pos.y;

                self.selection_rect.left = (start_rect.left + dx)
                    .max(0)
                    .min(start_rect.right - MIN_BOX_SIZE);
                self.selection_rect.top = (start_rect.top + dy)
                    .max(0)
                    .min(start_rect.bottom - MIN_BOX_SIZE);
            }

            DragMode::ResizingTopCenter => {
                let start_rect = self.drag_start_rect;
                let dy = y - self.drag_start_pos.y;

                self.selection_rect.top = (start_rect.top + dy)
                    .max(0)
                    .min(start_rect.bottom - MIN_BOX_SIZE);
            }

            DragMode::ResizingTopRight => {
                let start_rect = self.drag_start_rect;
                let dx = x - self.drag_start_pos.x;
                let dy = y - self.drag_start_pos.y;

                self.selection_rect.right = (start_rect.right + dx)
                    .min(self.screen_width)
                    .max(start_rect.left + MIN_BOX_SIZE);
                self.selection_rect.top = (start_rect.top + dy)
                    .max(0)
                    .min(start_rect.bottom - MIN_BOX_SIZE);
            }

            DragMode::ResizingMiddleRight => {
                let start_rect = self.drag_start_rect;
                let dx = x - self.drag_start_pos.x;

                self.selection_rect.right = (start_rect.right + dx)
                    .min(self.screen_width)
                    .max(start_rect.left + MIN_BOX_SIZE);
            }

            DragMode::ResizingBottomRight => {
                let start_rect = self.drag_start_rect;
                let dx = x - self.drag_start_pos.x;
                let dy = y - self.drag_start_pos.y;

                self.selection_rect.right = (start_rect.right + dx)
                    .min(self.screen_width)
                    .max(start_rect.left + MIN_BOX_SIZE);
                self.selection_rect.bottom = (start_rect.bottom + dy)
                    .min(self.screen_height)
                    .max(start_rect.top + MIN_BOX_SIZE);
            }

            DragMode::ResizingBottomCenter => {
                let start_rect = self.drag_start_rect;
                let dy = y - self.drag_start_pos.y;

                self.selection_rect.bottom = (start_rect.bottom + dy)
                    .min(self.screen_height)
                    .max(start_rect.top + MIN_BOX_SIZE);
            }

            DragMode::ResizingBottomLeft => {
                let start_rect = self.drag_start_rect;
                let dx = x - self.drag_start_pos.x;
                let dy = y - self.drag_start_pos.y;

                self.selection_rect.left = (start_rect.left + dx)
                    .max(0)
                    .min(start_rect.right - MIN_BOX_SIZE);
                self.selection_rect.bottom = (start_rect.bottom + dy)
                    .min(self.screen_height)
                    .max(start_rect.top + MIN_BOX_SIZE);
            }

            DragMode::ResizingMiddleLeft => {
                let start_rect = self.drag_start_rect;
                let dx = x - self.drag_start_pos.x;

                self.selection_rect.left = (start_rect.left + dx)
                    .max(0)
                    .min(start_rect.right - MIN_BOX_SIZE);
            }

            DragMode::None => {}
        }
    }

    fn end_drag(&mut self) {
        if self.drag_mode == DragMode::Drawing {
            // 画框结束，检查框的大小
            let width = self.selection_rect.right - self.selection_rect.left;
            let height = self.selection_rect.bottom - self.selection_rect.top;

            if width < MIN_BOX_SIZE || height < MIN_BOX_SIZE {
                // 框太小，取消选择
                self.has_selection = false;
            }
        }

        self.mouse_pressed = false;
        self.drag_mode = DragMode::None;
    }

    fn paint(&self, hwnd: HWND) {
        unsafe {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);

            // 在后台缓冲区绘制所有内容
            self.render_to_buffer();

            // 一次性将后台缓冲区内容复制到前台
            BitBlt(
                hdc,
                0,
                0,
                self.screen_width,
                self.screen_height,
                self.back_buffer_dc,
                0,
                0,
                SRCCOPY,
            );

            EndPaint(hwnd, &ps);
        }
    }
    // 新增：在后台缓冲区中渲染所有内容
    fn render_to_buffer(&self) {
        unsafe {
            let hdc = self.back_buffer_dc;

            // 1. 先绘制原始截图作为背景
            BitBlt(
                hdc,
                0,
                0,
                self.screen_width,
                self.screen_height,
                self.screenshot_dc,
                0,
                0,
                SRCCOPY,
            );
            if !self.has_selection {
                self.draw_full_screen_overlay(hdc);
            } else {
                // 3. 绘制半透明黑色遮罩（除了选中区域）
                self.draw_dimmed_overlay(hdc);

                // 4. 绘制选择框边框
                self.draw_selection_border(hdc);

                // 5. 绘制拖拽手柄
                self.draw_handles(hdc);
            }
        }
    }
    // 新增：绘制全屏遮罩
    fn draw_full_screen_overlay(&self, hdc: HDC) {
        unsafe {
            // 创建半透明遮罩DC
            let mask_dc = CreateCompatibleDC(hdc);
            let mask_bitmap = CreateCompatibleBitmap(hdc, self.screen_width, self.screen_height);
            let old_bitmap = SelectObject(mask_dc, mask_bitmap);

            // 用半透明黑色填充遮罩DC
            let black_brush = CreateSolidBrush(RGB!(1, 1, 1));
            let full_rect = RECT {
                left: 0,
                top: 0,
                right: self.screen_width,
                bottom: self.screen_height,
            };
            FillRect(mask_dc, &full_rect, black_brush);

            // 使用AlphaBlend绘制半透明遮罩
            let blend_func = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 160, // 更浅的遮罩，便于看清要截取的区域
                AlphaFormat: 0,
            };

            AlphaBlend(
                hdc,
                0,
                0,
                self.screen_width,
                self.screen_height,
                mask_dc,
                0,
                0,
                self.screen_width,
                self.screen_height,
                blend_func,
            );

            // 清理资源
            SelectObject(mask_dc, old_bitmap);
            DeleteObject(mask_bitmap);
            DeleteDC(mask_dc);
            DeleteObject(black_brush);
        }
    }
    fn draw_dimmed_overlay(&self, hdc: HDC) {
        unsafe {
            // 使用Region方法，类似你提供的C++代码

            // 创建整个屏幕区域
            let full_region = CreateRectRgn(0, 0, self.screen_width, self.screen_height);

            // 创建选择框区域
            let selection_region = CreateRectRgn(
                self.selection_rect.left,
                self.selection_rect.top,
                self.selection_rect.right,
                self.selection_rect.bottom,
            );

            // 从整个屏幕区域中排除选择框区域
            CombineRgn(full_region, full_region, selection_region, RGN_DIFF);

            // 选择区域到DC
            SelectClipRgn(hdc, full_region);

            // 创建半透明遮罩DC
            let mask_dc = CreateCompatibleDC(hdc);
            let mask_bitmap = CreateCompatibleBitmap(hdc, self.screen_width, self.screen_height);
            let old_bitmap = SelectObject(mask_dc, mask_bitmap);

            // 用半透明黑色填充遮罩DC
            let black_brush = CreateSolidBrush(RGB!(1, 1, 1));
            let full_rect = RECT {
                left: 0,
                top: 0,
                right: self.screen_width,
                bottom: self.screen_height,
            };
            FillRect(mask_dc, &full_rect, black_brush);

            // 使用AlphaBlend绘制半透明遮罩（只在裁剪区域内）
            let blend_func = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 160, // 半透明度
                AlphaFormat: 0,
            };

            AlphaBlend(
                hdc,
                0,
                0,
                self.screen_width,
                self.screen_height,
                mask_dc,
                0,
                0,
                self.screen_width,
                self.screen_height,
                blend_func,
            );

            // 恢复裁剪区域
            SelectClipRgn(hdc, HRGN(std::ptr::null_mut()));

            // 清理资源
            SelectObject(mask_dc, old_bitmap);
            DeleteObject(mask_bitmap);
            DeleteDC(mask_dc);
            DeleteObject(black_brush);
            DeleteObject(full_region);
            DeleteObject(selection_region);
        }
    }

    fn draw_selection_border(&self, hdc: HDC) {
        unsafe {
            let old_pen = SelectObject(hdc, self.border_pen);
            let old_brush = SelectObject(hdc, GetStockObject(NULL_BRUSH));

            Rectangle(
                hdc,
                self.selection_rect.left,
                self.selection_rect.top,
                self.selection_rect.right,
                self.selection_rect.bottom,
            );

            SelectObject(hdc, old_pen);
            SelectObject(hdc, old_brush);
        }
    }

    fn draw_handles(&self, hdc: HDC) {
        unsafe {
            let old_brush = SelectObject(hdc, self.handle_brush);
            let old_pen = SelectObject(hdc, GetStockObject(NULL_PEN));

            let center_x = (self.selection_rect.left + self.selection_rect.right) / 2;
            let center_y = (self.selection_rect.top + self.selection_rect.bottom) / 2;
            let handle_size = 8;
            let half_handle = handle_size / 2;

            let handles = [
                (self.selection_rect.left, self.selection_rect.top),
                (center_x, self.selection_rect.top),
                (self.selection_rect.right, self.selection_rect.top),
                (self.selection_rect.right, center_y),
                (self.selection_rect.right, self.selection_rect.bottom),
                (center_x, self.selection_rect.bottom),
                (self.selection_rect.left, self.selection_rect.bottom),
                (self.selection_rect.left, center_y),
            ];

            for (hx, hy) in handles.iter() {
                Rectangle(
                    hdc,
                    hx - half_handle,
                    hy - half_handle,
                    hx + half_handle,
                    hy + half_handle,
                );
            }

            SelectObject(hdc, old_brush);
            SelectObject(hdc, old_pen);
        }
    }

    // 保存选中区域的截图
    fn save_selection(&self) -> Result<()> {
        unsafe {
            let width = self.selection_rect.right - self.selection_rect.left;
            let height = self.selection_rect.bottom - self.selection_rect.top;

            if width <= 0 || height <= 0 {
                return Ok(());
            }

            // 创建选中区域的位图
            let screen_dc = GetDC(HWND(std::ptr::null_mut()));
            let mem_dc = CreateCompatibleDC(screen_dc);
            let bitmap = CreateCompatibleBitmap(screen_dc, width, height);

            SelectObject(mem_dc, bitmap);

            // 复制选中区域
            BitBlt(
                mem_dc,
                0,
                0,
                width,
                height,
                self.screenshot_dc,
                self.selection_rect.left,
                self.selection_rect.top,
                SRCCOPY,
            );

            // 复制到剪贴板
            if OpenClipboard(HWND(std::ptr::null_mut())).is_ok() {
                let _ = EmptyClipboard();
                let _ = SetClipboardData(2, HANDLE(bitmap.0 as *mut std::ffi::c_void));
                let _ = CloseClipboard();
            } else {
                // 如果剪贴板操作失败，删除bitmap避免内存泄漏
                DeleteObject(bitmap);
            }

            ReleaseDC(HWND(std::ptr::null_mut()), screen_dc);
            DeleteDC(mem_dc);

            Ok(())
        }
    }
}

impl Drop for WindowState {
    fn drop(&mut self) {
        unsafe {
            DeleteObject(self.screenshot_bitmap);
            DeleteDC(self.screenshot_dc);
            DeleteObject(self.back_buffer_bitmap);
            DeleteDC(self.back_buffer_dc);
            DeleteObject(self.border_pen);
            DeleteObject(self.handle_brush);
            DeleteObject(self.mask_brush);
        }
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => match WindowState::new() {
            Ok(state) => {
                let state_box = Box::new(state);
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state_box) as isize);
                LRESULT(0)
            }
            Err(_) => LRESULT(-1),
        },

        WM_DESTROY => {
            let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
            if !state_ptr.is_null() {
                let _state = Box::from_raw(state_ptr);
            }
            PostQuitMessage(0);
            LRESULT(0)
        }

        WM_PAINT => {
            let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
            if !state_ptr.is_null() {
                let state = &*state_ptr;
                state.paint(hwnd);
            }
            LRESULT(0)
        }

        WM_MOUSEMOVE => {
            let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
            if !state_ptr.is_null() {
                let state = &mut *state_ptr;
                let x = (lparam.0 & 0xFFFF) as i16 as i32;
                let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;

                if state.mouse_pressed {
                    state.update_drag(x, y);
                    InvalidateRect(hwnd, None, FALSE);
                } else {
                    // 更新鼠标指针
                    let drag_mode = state.get_handle_at_position(x, y);
                    let cursor_id = state.get_cursor_for_drag_mode(drag_mode);
                    if let Ok(cursor) = LoadCursorW(HINSTANCE(std::ptr::null_mut()), cursor_id) {
                        SetCursor(cursor);
                    }
                }
            }
            LRESULT(0)
        }

        WM_LBUTTONDOWN => {
            let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
            if !state_ptr.is_null() {
                let state = &mut *state_ptr;
                let x = (lparam.0 & 0xFFFF) as i16 as i32;
                let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;

                state.start_drag(x, y);
                if state.mouse_pressed {
                    SetCapture(hwnd);
                }
            }
            LRESULT(0)
        }

        WM_LBUTTONUP => {
            let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
            if !state_ptr.is_null() {
                let state = &mut *state_ptr;
                state.end_drag();
                ReleaseCapture();
                InvalidateRect(hwnd, None, FALSE);
            }
            LRESULT(0)
        }

        WM_LBUTTONDBLCLK => {
            // 双击保存截图并退出
            let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
            if !state_ptr.is_null() {
                let state = &*state_ptr;
                let _ = state.save_selection();
                PostQuitMessage(0);
            }
            LRESULT(0)
        }

        WM_KEYDOWN => {
            match wparam.0 as u32 {
                key if key == VK_ESCAPE.0.into() => {
                    PostQuitMessage(0);
                }
                key if key == VK_RETURN.0.into() => {
                    // Enter键保存截图并退出
                    let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
                    if !state_ptr.is_null() {
                        let state = &*state_ptr;
                        let _ = state.save_selection();
                        PostQuitMessage(0);
                    }
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_SETCURSOR => {
            // 让我们自己处理光标
            LRESULT(1) // TRUE - 我们已经设置了光标
        }

        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn main() -> Result<()> {
    unsafe {
        let instance = GetModuleHandleW(None)?;

        let class_name = to_wide_chars(WINDOW_CLASS_NAME);

        let window_class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance.into(),
            lpszClassName: PCWSTR(class_name.as_ptr()),
            hbrBackground: HBRUSH(std::ptr::null_mut()), // 透明背景
            hCursor: LoadCursorW(HINSTANCE(std::ptr::null_mut()), IDC_ARROW)?,
            style: CS_DBLCLKS, // 启用双击检测
            ..Default::default()
        };

        if RegisterClassW(&window_class) == 0 {
            return Err(Error::from_win32());
        }

        let screen_width = GetSystemMetrics(SM_CXSCREEN);
        let screen_height = GetSystemMetrics(SM_CYSCREEN);

        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            PCWSTR(class_name.as_ptr()),
            PCWSTR::null(),
            WS_POPUP,
            0,
            0,
            screen_width,
            screen_height,
            HWND(std::ptr::null_mut()),
            HMENU(std::ptr::null_mut()),
            instance,
            None,
        )?;

        if hwnd.0 == std::ptr::null_mut() {
            return Err(Error::from_win32());
        }

        ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, HWND(std::ptr::null_mut()), 0, 0).as_bool() {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        Ok(())
    }
}
