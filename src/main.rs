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
use windows::Win32::UI::HiDpi::{
    GetDpiForSystem, PROCESS_DPI_UNAWARE, PROCESS_PER_MONITOR_DPI_AWARE, SetProcessDpiAwareness,
};
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
enum ToolbarButton {
    Save,
    Copy,
    Cancel,
    None,
}

#[derive(Debug)]
struct Toolbar {
    rect: RECT,
    visible: bool,
    buttons: Vec<(RECT, ToolbarButton, &'static str)>,
    hovered_button: ToolbarButton,
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
    // DPI相关 - 新增
    // dpi_scale: f32,
    // logical_width: i32,
    // logical_height: i32,
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

    // 添加工具栏
    toolbar: Toolbar,

    // 添加工具栏相关画刷
    toolbar_brush: HBRUSH,
    toolbar_border_pen: HPEN,
    button_brush: HBRUSH,
    button_hover_brush: HBRUSH,
}
impl Toolbar {
    fn new() -> Self {
        Self {
            rect: RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            visible: false,
            buttons: Vec::new(),
            hovered_button: ToolbarButton::None,
        }
    }

    fn update_position(&mut self, selection_rect: &RECT, screen_width: i32, screen_height: i32) {
        const TOOLBAR_HEIGHT: i32 = 40;
        const BUTTON_WIDTH: i32 = 60;
        const BUTTON_HEIGHT: i32 = 30;
        const BUTTON_SPACING: i32 = 10;
        const TOOLBAR_PADDING: i32 = 5;

        let toolbar_width = BUTTON_WIDTH * 3 + BUTTON_SPACING * 2 + TOOLBAR_PADDING * 2;

        let mut toolbar_x =
            selection_rect.left + (selection_rect.right - selection_rect.left - toolbar_width) / 2;
        let mut toolbar_y = selection_rect.bottom + 10;

        if toolbar_y + TOOLBAR_HEIGHT > screen_height {
            toolbar_y = selection_rect.top - TOOLBAR_HEIGHT - 10;
        }

        toolbar_x = toolbar_x.max(0).min(screen_width - toolbar_width);
        toolbar_y = toolbar_y.max(0).min(screen_height - TOOLBAR_HEIGHT);

        self.rect = RECT {
            left: toolbar_x,
            top: toolbar_y,
            right: toolbar_x + toolbar_width,
            bottom: toolbar_y + TOOLBAR_HEIGHT,
        };

        self.buttons.clear();
        let button_y = toolbar_y + TOOLBAR_PADDING;
        let mut button_x = toolbar_x + TOOLBAR_PADDING;

        let buttons_data = [
            (ToolbarButton::Save, "保存"),
            (ToolbarButton::Copy, "复制"),
            (ToolbarButton::Cancel, "取消"),
        ];

        for (button_type, text) in buttons_data.iter() {
            let button_rect = RECT {
                left: button_x,
                top: button_y,
                right: button_x + BUTTON_WIDTH,
                bottom: button_y + BUTTON_HEIGHT,
            };
            self.buttons.push((button_rect, *button_type, text));
            button_x += BUTTON_WIDTH + BUTTON_SPACING;
        }

        self.visible = true;
    }

    fn get_button_at_position(&self, x: i32, y: i32) -> ToolbarButton {
        if !self.visible {
            return ToolbarButton::None;
        }

        for (rect, button_type, _) in &self.buttons {
            if x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom {
                return *button_type;
            }
        }
        ToolbarButton::None
    }

    fn set_hovered_button(&mut self, button: ToolbarButton) {
        self.hovered_button = button;
    }

    fn hide(&mut self) {
        self.visible = false;
        self.hovered_button = ToolbarButton::None;
    }
}
impl WindowState {
    fn new() -> Result<Self> {
        unsafe {
            // 移除DPI相关代码，直接使用系统坐标
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

            // 创建绘图对象（使用固定大小）
            let border_pen = CreatePen(PS_SOLID, 2, RGB!(0, 120, 215));
            let handle_brush = CreateSolidBrush(RGB!(0, 120, 215));
            let mask_brush = CreateSolidBrush(RGB!(0, 0, 0));
            // 添加工具栏相关画刷
            let toolbar_brush = CreateSolidBrush(RGB!(45, 45, 45));
            let toolbar_border_pen = CreatePen(PS_SOLID, 1, RGB!(120, 120, 120));
            let button_brush = CreateSolidBrush(RGB!(60, 60, 60));
            let button_hover_brush = CreateSolidBrush(RGB!(80, 80, 80));
            if border_pen.is_invalid() || handle_brush.is_invalid() || mask_brush.is_invalid() {
                return Err(Error::from_win32());
            }

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
                has_selection: false,
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
                toolbar: Toolbar::new(),
                toolbar_brush,
                toolbar_border_pen,
                button_brush,
                button_hover_brush,
            })
        }
    }
    // 新增：转换逻辑坐标到物理坐标

    fn get_handle_at_position(&self, x: i32, y: i32) -> DragMode {
        if self.toolbar.visible
            && x >= self.toolbar.rect.left
            && x <= self.toolbar.rect.right
            && y >= self.toolbar.rect.top
            && y <= self.toolbar.rect.bottom
        {
            return DragMode::None;
        }
        if !self.has_selection {
            return DragMode::None;
        }

        let rect = &self.selection_rect;
        let handle_size = 8; // 固定8像素
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

        // 检查手柄 - 使用固定检测区域
        let detection_radius = 10; // 固定10像素检测区域
        for (hx, hy, mode) in handles.iter() {
            if x >= hx - detection_radius
                && x <= hx + detection_radius
                && y >= hy - detection_radius
                && y <= hy + detection_radius
            {
                return *mode;
            }
        }

        // 检查是否在选择框内
        let border_margin = 5; // 固定5像素边距
        if x >= rect.left + border_margin
            && x <= rect.right - border_margin
            && y >= rect.top + border_margin
            && y <= rect.bottom - border_margin
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
            self.drag_mode = handle_mode;
            self.mouse_pressed = true;
            self.drag_start_pos = POINT { x, y };
            self.drag_start_rect = self.selection_rect;
        } else {
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

        let min_box_size = MIN_BOX_SIZE;
        let mut selection_changed = false;

        match self.drag_mode {
            DragMode::Drawing => {
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
                selection_changed = true;
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
                selection_changed = true;
            }

            DragMode::ResizingTopLeft => {
                let start_rect = self.drag_start_rect;
                let dx = x - self.drag_start_pos.x;
                let dy = y - self.drag_start_pos.y;

                self.selection_rect.left = (start_rect.left + dx)
                    .max(0)
                    .min(start_rect.right - min_box_size);
                self.selection_rect.top = (start_rect.top + dy)
                    .max(0)
                    .min(start_rect.bottom - min_box_size);
                selection_changed = true;
            }

            DragMode::ResizingTopCenter => {
                let start_rect = self.drag_start_rect;
                let dy = y - self.drag_start_pos.y;

                self.selection_rect.top = (start_rect.top + dy)
                    .max(0)
                    .min(start_rect.bottom - min_box_size);
                selection_changed = true;
            }

            DragMode::ResizingTopRight => {
                let start_rect = self.drag_start_rect;
                let dx = x - self.drag_start_pos.x;
                let dy = y - self.drag_start_pos.y;

                self.selection_rect.right = (start_rect.right + dx)
                    .min(self.screen_width)
                    .max(start_rect.left + min_box_size);
                self.selection_rect.top = (start_rect.top + dy)
                    .max(0)
                    .min(start_rect.bottom - min_box_size);
                selection_changed = true;
            }

            DragMode::ResizingMiddleRight => {
                let start_rect = self.drag_start_rect;
                let dx = x - self.drag_start_pos.x;

                self.selection_rect.right = (start_rect.right + dx)
                    .min(self.screen_width)
                    .max(start_rect.left + min_box_size);
                selection_changed = true;
            }

            DragMode::ResizingBottomRight => {
                let start_rect = self.drag_start_rect;
                let dx = x - self.drag_start_pos.x;
                let dy = y - self.drag_start_pos.y;

                self.selection_rect.right = (start_rect.right + dx)
                    .min(self.screen_width)
                    .max(start_rect.left + min_box_size);
                self.selection_rect.bottom = (start_rect.bottom + dy)
                    .min(self.screen_height)
                    .max(start_rect.top + min_box_size);
                selection_changed = true;
            }

            DragMode::ResizingBottomCenter => {
                let start_rect = self.drag_start_rect;
                let dy = y - self.drag_start_pos.y;

                self.selection_rect.bottom = (start_rect.bottom + dy)
                    .min(self.screen_height)
                    .max(start_rect.top + min_box_size);
                selection_changed = true;
            }

            DragMode::ResizingBottomLeft => {
                let start_rect = self.drag_start_rect;
                let dx = x - self.drag_start_pos.x;
                let dy = y - self.drag_start_pos.y;

                self.selection_rect.left = (start_rect.left + dx)
                    .max(0)
                    .min(start_rect.right - min_box_size);
                self.selection_rect.bottom = (start_rect.bottom + dy)
                    .min(self.screen_height)
                    .max(start_rect.top + min_box_size);
                selection_changed = true;
            }

            DragMode::ResizingMiddleLeft => {
                let start_rect = self.drag_start_rect;
                let dx = x - self.drag_start_pos.x;

                self.selection_rect.left = (start_rect.left + dx)
                    .max(0)
                    .min(start_rect.right - min_box_size);
                selection_changed = true;
            }

            DragMode::None => {}
        }

        // 只在选择框真正改变时才更新工具栏位置
        if selection_changed && self.toolbar.visible {
            self.toolbar.update_position(
                &self.selection_rect,
                self.screen_width,
                self.screen_height,
            );
        }
    }
    // 优化工具栏位置更新 - 添加防抖动机制
    fn update_toolbar_position_if_needed(&mut self) {
        if self.has_selection {
            self.toolbar.update_position(
                &self.selection_rect,
                self.screen_width,
                self.screen_height,
            );
        }
    }
    fn end_drag(&mut self) {
        if self.drag_mode == DragMode::Drawing {
            let width = self.selection_rect.right - self.selection_rect.left;
            let height = self.selection_rect.bottom - self.selection_rect.top;

            if width < MIN_BOX_SIZE || height < MIN_BOX_SIZE {
                self.has_selection = false;
                self.toolbar.hide();
            } else {
                // 更新工具栏位置
                self.toolbar.update_position(
                    &self.selection_rect,
                    self.screen_width,
                    self.screen_height,
                );
            }
        }

        self.mouse_pressed = false;
        self.drag_mode = DragMode::None;
    }
    fn handle_toolbar_click(&self, button: ToolbarButton) -> bool {
        match button {
            ToolbarButton::Save | ToolbarButton::Copy => {
                let _ = self.save_selection();
                true // 退出程序
            }
            ToolbarButton::Cancel => {
                true // 退出程序
            }
            ToolbarButton::None => false,
        }
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
                // 使用更简单的遮罩方式提高性能
                self.draw_optimized_dimmed_overlay(hdc);
                self.draw_selection_border(hdc);
                self.draw_handles(hdc);

                // 工具栏绘制
                if self.toolbar.visible {
                    self.draw_toolbar(hdc);
                }
            }
        }
    }
    fn draw_optimized_dimmed_overlay(&self, hdc: HDC) {
        unsafe {
            // 直接绘制四个矩形，避免使用复杂的区域操作
            let old_brush = SelectObject(hdc, self.mask_brush);
            let old_pen = SelectObject(hdc, GetStockObject(NULL_PEN));

            // 设置混合模式
            let old_mode = SetROP2(hdc, R2_MASKPEN);

            // 上方矩形
            if self.selection_rect.top > 0 {
                let rect = RECT {
                    left: 0,
                    top: 0,
                    right: self.screen_width,
                    bottom: self.selection_rect.top,
                };
                self.draw_transparent_rect(hdc, &rect);
            }

            // 下方矩形
            if self.selection_rect.bottom < self.screen_height {
                let rect = RECT {
                    left: 0,
                    top: self.selection_rect.bottom,
                    right: self.screen_width,
                    bottom: self.screen_height,
                };
                self.draw_transparent_rect(hdc, &rect);
            }

            // 左侧矩形
            if self.selection_rect.left > 0 {
                let rect = RECT {
                    left: 0,
                    top: self.selection_rect.top,
                    right: self.selection_rect.left,
                    bottom: self.selection_rect.bottom,
                };
                self.draw_transparent_rect(hdc, &rect);
            }

            // 右侧矩形
            if self.selection_rect.right < self.screen_width {
                let rect = RECT {
                    left: self.selection_rect.right,
                    top: self.selection_rect.top,
                    right: self.screen_width,
                    bottom: self.selection_rect.bottom,
                };
                self.draw_transparent_rect(hdc, &rect);
            }

            SetROP2(hdc, R2_MODE(old_mode));
            SelectObject(hdc, old_brush);
            SelectObject(hdc, old_pen);
        }
    }

    // 新增：绘制半透明矩形的辅助方法
    fn draw_transparent_rect(&self, hdc: HDC, rect: &RECT) {
        unsafe {
            // 创建临时DC进行半透明绘制
            let temp_dc = CreateCompatibleDC(hdc);
            let width = rect.right - rect.left;
            let height = rect.bottom - rect.top;
            let temp_bitmap = CreateCompatibleBitmap(hdc, width, height);
            let old_bitmap = SelectObject(temp_dc, temp_bitmap);

            // 填充纯黑色
            let black_brush = CreateSolidBrush(RGB!(0, 0, 0));
            let temp_rect = RECT {
                left: 0,
                top: 0,
                right: width,
                bottom: height,
            };
            FillRect(temp_dc, &temp_rect, black_brush);

            // 使用AlphaBlend绘制半透明效果
            let blend_func = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 120, // 降低透明度，提高性能
                AlphaFormat: 0,
            };

            let _ = AlphaBlend(
                hdc, rect.left, rect.top, width, height, temp_dc, 0, 0, width, height, blend_func,
            );

            // 清理资源
            SelectObject(temp_dc, old_bitmap);
            DeleteObject(temp_bitmap);
            DeleteDC(temp_dc);
            DeleteObject(black_brush);
        }
    }
    fn draw_toolbar(&self, hdc: HDC) {
        unsafe {
            let old_brush = SelectObject(hdc, self.toolbar_brush);
            let old_pen = SelectObject(hdc, self.toolbar_border_pen);

            RoundRect(
                hdc,
                self.toolbar.rect.left,
                self.toolbar.rect.top,
                self.toolbar.rect.right,
                self.toolbar.rect.bottom,
                8,
                8,
            );

            for (rect, button_type, text) in &self.toolbar.buttons {
                let button_brush = if *button_type == self.toolbar.hovered_button {
                    self.button_hover_brush
                } else {
                    self.button_brush
                };

                SelectObject(hdc, button_brush);
                SelectObject(hdc, GetStockObject(NULL_PEN));

                RoundRect(hdc, rect.left, rect.top, rect.right, rect.bottom, 4, 4);

                SetBkMode(hdc, TRANSPARENT);
                SetTextColor(hdc, RGB!(255, 255, 255));

                let mut text_wide = to_wide_chars(text);
                // 修复这一行：创建可变副本并转换为指针
                let mut rect_copy = *rect;
                DrawTextW(
                    hdc,
                    &mut text_wide,
                    &mut rect_copy,
                    DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                );
            }

            SelectObject(hdc, old_brush);
            SelectObject(hdc, old_pen);
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
            let handle_size = 8; // 固定8像素
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
            DeleteObject(self.toolbar_brush);
            DeleteObject(self.toolbar_border_pen);
            DeleteObject(self.button_brush);
            DeleteObject(self.button_hover_brush);
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
                    // 使用异步重绘减少阻塞
                    InvalidateRect(hwnd, None, FALSE);
                } else {
                    // 检查工具栏悬停 - 减少重绘频率
                    let toolbar_button = state.toolbar.get_button_at_position(x, y);
                    if toolbar_button != state.toolbar.hovered_button {
                        state.toolbar.set_hovered_button(toolbar_button);
                        // 只重绘工具栏区域
                        if state.toolbar.visible {
                            InvalidateRect(hwnd, Some(&state.toolbar.rect), FALSE);
                        }
                    }

                    // 更新鼠标指针
                    let cursor_id = if toolbar_button != ToolbarButton::None {
                        IDC_HAND
                    } else {
                        let drag_mode = state.get_handle_at_position(x, y);
                        state.get_cursor_for_drag_mode(drag_mode)
                    };

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

                // 检查是否点击了工具栏按钮
                let x = (lparam.0 & 0xFFFF) as i16 as i32;
                let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
                let toolbar_button = state.toolbar.get_button_at_position(x, y);

                if toolbar_button != ToolbarButton::None {
                    if state.handle_toolbar_click(toolbar_button) {
                        PostQuitMessage(0);
                        return LRESULT(0);
                    }
                }

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
        SetProcessDpiAwareness(PROCESS_PER_MONITOR_DPI_AWARE)?;
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
