#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::ffi::OsStr;
use std::iter::once;
use std::os::windows::ffi::OsStrExt;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWriteCreateFactory, IDWriteFactory,
};
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::Media::{timeBeginPeriod, timeEndPeriod};
use windows::Win32::System::Com::CoInitializeEx;
use windows::Win32::System::Com::*;
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::LibraryLoader::*;
use windows::Win32::System::Threading::{
    GetCurrentProcess, GetCurrentThread, HIGH_PRIORITY_CLASS, NORMAL_PRIORITY_CLASS,
    REALTIME_PRIORITY_CLASS, SetPriorityClass, SetThreadPriority, THREAD_PRIORITY_ABOVE_NORMAL,
    THREAD_PRIORITY_NORMAL, THREAD_PRIORITY_TIME_CRITICAL,
};
use windows::Win32::UI::HiDpi::{
    GetDpiForSystem, PROCESS_DPI_UNAWARE, PROCESS_PER_MONITOR_DPI_AWARE, SetProcessDpiAwareness,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, ReleaseCapture, SetCapture, VK_CONTROL, VK_ESCAPE, VK_RETURN, VK_Z,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::*;

const WINDOW_CLASS_NAME: &str = "ScreenshotWindow";
const MIN_BOX_SIZE: i32 = 50;
// 添加绘制元素句柄的常量
const HANDLE_SIZE: i32 = 8;
macro_rules! RGB {
    ($r:expr, $g:expr, $b:expr) => {
        COLORREF(($r as u32) | (($g as u32) << 8) | (($b as u32) << 16))
    };
}

const SAVE_ICON: &str = "💾"; // 保存
const COPY_ICON: &str = "📋"; // 复制  
const RECT_ICON: &str = "⬜"; // 矩形
const CIRCLE_ICON: &str = "⭕"; // 圆形
const ARROW_ICON: &str = "➡"; // 箭头
const PEN_ICON: &str = "✏"; // 画笔
const TEXT_ICON: &str = "T"; // 文字
const UNDO_ICON: &str = "↶"; // 撤销
const CONFIRM_ICON: &str = "✓"; // 确认
const CANCEL_ICON: &str = "✕"; // 取消
#[derive(Debug, Clone)]
struct IconData {
    text: String,
}
impl IconData {
    fn from_text(text: &str) -> Self {
        IconData {
            text: text.to_string(),
        }
    }
}
// 辅助函数：将字符串转换为宽字符
fn to_wide_chars(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(once(0)).collect()
}
#[derive(Debug, Clone, Copy, PartialEq)]
enum ToolbarButton {
    Save,
    Copy,
    Rectangle,
    Circle,
    Arrow,
    Pen,
    Text,
    Undo,
    Confirm,
    Cancel,
    None,
}
#[derive(Debug, Clone, Copy, PartialEq)]
enum DrawingTool {
    None,
    Rectangle,
    Circle,
    Arrow,
    Pen,
    Text,
}
#[derive(Debug, Clone, PartialEq)]
struct DrawingElement {
    tool: DrawingTool,
    points: Vec<POINT>,
    rect: RECT,
    color: COLORREF,
    thickness: i32,
    text: String,
    selected: bool,
}
#[derive(Debug)]
struct Toolbar {
    rect: RECT,
    visible: bool,
    buttons: Vec<(RECT, ToolbarButton, IconData)>, // 简化为只包含SVG路径
    hovered_button: ToolbarButton,
    clicked_button: ToolbarButton,
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
    DrawingShape,    // 正在绘制形状
    MovingElement,   // 移动绘图元素
    ResizingElement, // 调整绘图元素大小
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

    // 绘图功能
    current_tool: DrawingTool,
    drawing_elements: Vec<DrawingElement>,
    current_element: Option<DrawingElement>,
    selected_element: Option<usize>,
    drawing_color: COLORREF,
    drawing_thickness: i32,

    // 历史记录用于撤销
    history: Vec<Vec<DrawingElement>>,
}
impl DrawingElement {
    fn new(tool: DrawingTool) -> Self {
        Self {
            tool,
            points: Vec::new(),
            rect: RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            color: RGB!(255, 0, 0), // 默认红色
            thickness: 3,
            text: String::new(),
            selected: false,
        }
    }

    fn get_bounding_rect(&self) -> RECT {
        match self.tool {
            DrawingTool::Rectangle
            | DrawingTool::Circle
            | DrawingTool::Arrow
            | DrawingTool::Text => self.rect,
            DrawingTool::Pen => {
                if self.points.is_empty() {
                    return RECT {
                        left: 0,
                        top: 0,
                        right: 0,
                        bottom: 0,
                    };
                }
                let mut min_x = self.points[0].x;
                let mut max_x = self.points[0].x;
                let mut min_y = self.points[0].y;
                let mut max_y = self.points[0].y;

                for point in &self.points {
                    min_x = min_x.min(point.x);
                    max_x = max_x.max(point.x);
                    min_y = min_y.min(point.y);
                    max_y = max_y.max(point.y);
                }

                RECT {
                    left: min_x,
                    top: min_y,
                    right: max_x,
                    bottom: max_y,
                }
            }
            _ => RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
        }
    }

    fn contains_point(&self, x: i32, y: i32) -> bool {
        let rect = self.get_bounding_rect();
        x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom
    }
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
            clicked_button: ToolbarButton::None,
        }
    }

    fn update_position(&mut self, selection_rect: &RECT, screen_width: i32, screen_height: i32) {
        const TOOLBAR_HEIGHT: i32 = 50;
        const BUTTON_WIDTH: i32 = 40;
        const BUTTON_HEIGHT: i32 = 35;
        const BUTTON_SPACING: i32 = 4;
        const TOOLBAR_PADDING: i32 = 8;

        let button_count = 10; // 增加到10个按钮
        let toolbar_width =
            BUTTON_WIDTH * button_count + BUTTON_SPACING * (button_count - 1) + TOOLBAR_PADDING * 2;

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
            (ToolbarButton::Rectangle, IconData::from_text(RECT_ICON)),
            (ToolbarButton::Circle, IconData::from_text(CIRCLE_ICON)),
            (ToolbarButton::Arrow, IconData::from_text(ARROW_ICON)),
            (ToolbarButton::Pen, IconData::from_text(PEN_ICON)),
            (ToolbarButton::Text, IconData::from_text(TEXT_ICON)),
            (ToolbarButton::Undo, IconData::from_text(UNDO_ICON)),
            (ToolbarButton::Save, IconData::from_text(SAVE_ICON)),
            (ToolbarButton::Copy, IconData::from_text(COPY_ICON)),
            (ToolbarButton::Confirm, IconData::from_text(CONFIRM_ICON)),
            (ToolbarButton::Cancel, IconData::from_text(CANCEL_ICON)),
        ];

        for (button_type, icon_data) in buttons_data.iter() {
            let button_rect = RECT {
                left: button_x,
                top: button_y,
                right: button_x + BUTTON_WIDTH,
                bottom: button_y + BUTTON_HEIGHT,
            };
            self.buttons
                .push((button_rect, *button_type, icon_data.clone()));
            button_x += BUTTON_WIDTH + BUTTON_SPACING;
        }

        self.visible = true;
    }

    fn set_clicked_button(&mut self, button: ToolbarButton) {
        self.clicked_button = button;
    }

    fn clear_clicked_button(&mut self) {
        self.clicked_button = ToolbarButton::None;
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

            // 工具栏相关画刷 - 添加更多颜色
            let toolbar_brush = CreateSolidBrush(RGB!(255, 255, 255)); // 白色背景
            let toolbar_border_pen = CreatePen(PS_SOLID, 1, RGB!(200, 200, 200)); // 浅灰边框
            let button_brush = CreateSolidBrush(RGB!(255, 255, 255)); // 按钮默认白色
            let button_hover_brush = CreateSolidBrush(RGB!(240, 240, 240)); // 悬停浅灰
            let mut toolbar = Toolbar::new();

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
                toolbar,
                toolbar_brush,
                toolbar_border_pen,
                button_brush,
                button_hover_brush,
                current_tool: DrawingTool::None,
                drawing_elements: Vec::new(),
                current_element: None,
                selected_element: None,
                drawing_color: RGB!(255, 0, 0),
                drawing_thickness: 3,
                history: Vec::new(),
            })
        }
    }
    fn save_history(&mut self) {
        // 只有当状态真正改变时才保存
        if self.history.is_empty() || self.history.last() != Some(&self.drawing_elements) {
            self.history.push(self.drawing_elements.clone());
            // 限制历史记录数量，避免内存过度使用
            if self.history.len() > 50 {
                self.history.remove(0);
            }
        }
    }
    fn can_undo(&self) -> bool {
        !self.history.is_empty()
    }
    fn undo(&mut self) -> bool {
        if self.can_undo() {
            if let Some(previous_state) = self.history.pop() {
                self.drawing_elements = previous_state;
                self.selected_element = None;
                self.current_element = None; // 清除当前正在绘制的元素
                return true;
            }
        }
        false
    }
    fn get_element_at_position(&self, x: i32, y: i32) -> Option<usize> {
        // 从后往前检查，因为后面的元素在上层
        for (index, element) in self.drawing_elements.iter().enumerate().rev() {
            if element.contains_point(x, y) {
                return Some(index);
            }
        }
        None
    }
    fn get_element_handle_at_position(
        &self,
        x: i32,
        y: i32,
        element: &DrawingElement,
    ) -> Option<usize> {
        let rect = element.get_bounding_rect();
        let handles = [
            (rect.left, rect.top),                       // 0: 左上
            ((rect.left + rect.right) / 2, rect.top),    // 1: 上中
            (rect.right, rect.top),                      // 2: 右上
            (rect.right, (rect.top + rect.bottom) / 2),  // 3: 右中
            (rect.right, rect.bottom),                   // 4: 右下
            ((rect.left + rect.right) / 2, rect.bottom), // 5: 下中
            (rect.left, rect.bottom),                    // 6: 左下
            (rect.left, (rect.top + rect.bottom) / 2),   // 7: 左中
        ];

        const HANDLE_DETECTION_SIZE: i32 = 12; // 增大检测区域
        for (index, (hx, hy)) in handles.iter().enumerate() {
            if x >= hx - HANDLE_DETECTION_SIZE
                && x <= hx + HANDLE_DETECTION_SIZE
                && y >= hy - HANDLE_DETECTION_SIZE
                && y <= hy + HANDLE_DETECTION_SIZE
            {
                return Some(index);
            }
        }
        None
    }
    fn get_handle_at_position(&self, x: i32, y: i32) -> DragMode {
        // 首先检查是否在工具栏区域
        if self.toolbar.visible
            && x >= self.toolbar.rect.left
            && x <= self.toolbar.rect.right
            && y >= self.toolbar.rect.top
            && y <= self.toolbar.rect.bottom
        {
            return DragMode::None;
        }

        // **修复：优先检查绘图元素的调整手柄**
        if let Some(element_index) = self.selected_element {
            if element_index < self.drawing_elements.len() {
                let element = &self.drawing_elements[element_index];
                if let Some(_handle_index) = self.get_element_handle_at_position(x, y, element) {
                    return DragMode::ResizingElement;
                }
                // 检查是否在元素内部（移动）
                if element.contains_point(x, y) {
                    return DragMode::MovingElement;
                }
            }
        }

        // **修复：如果选择了绘图工具，仍然允许在选择框外操作选择框**
        if self.current_tool != DrawingTool::None {
            // 检查是否在选择框的调整手柄上
            if self.has_selection {
                let rect = &self.selection_rect;
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

                let detection_radius = 10;
                for (hx, hy, mode) in handles.iter() {
                    if x >= hx - detection_radius
                        && x <= hx + detection_radius
                        && y >= hy - detection_radius
                        && y <= hy + detection_radius
                    {
                        return *mode;
                    }
                }

                // 检查是否在选择框边框上（移动）
                let border_margin = 5;
                if x >= rect.left + border_margin
                    && x <= rect.right - border_margin
                    && y >= rect.top + border_margin
                    && y <= rect.bottom - border_margin
                {
                    return DragMode::Moving;
                }
            }

            // 检查是否点击了其他绘图元素
            if let Some(_element_index) = self.get_element_at_position(x, y) {
                return DragMode::MovingElement;
            }
            return DragMode::None; // 在选择框内的其他区域用于绘图
        }

        // 检查是否在绘图元素上
        if let Some(_element_index) = self.get_element_at_position(x, y) {
            return DragMode::MovingElement;
        }

        // 检查选择框的调整手柄
        if !self.has_selection {
            return DragMode::None;
        }

        let rect = &self.selection_rect;
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

        let detection_radius = 10;
        for (hx, hy, mode) in handles.iter() {
            if x >= hx - detection_radius
                && x <= hx + detection_radius
                && y >= hy - detection_radius
                && y <= hy + detection_radius
            {
                return *mode;
            }
        }

        let border_margin = 5;
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
            DragMode::Moving | DragMode::MovingElement => IDC_SIZEALL,
            DragMode::Drawing | DragMode::DrawingShape => IDC_CROSS,
            DragMode::ResizingElement => IDC_SIZENWSE,
            DragMode::None => match self.current_tool {
                DrawingTool::Pen => IDC_CROSS,
                _ => IDC_ARROW,
            },
        }
    }

    fn start_drag(&mut self, x: i32, y: i32) {
        let handle_mode = self.get_handle_at_position(x, y);

        if handle_mode == DragMode::ResizingElement {
            // 开始调整元素大小
            self.drag_mode = DragMode::ResizingElement;
            self.mouse_pressed = true;
            self.drag_start_pos = POINT { x, y };
            if let Some(element_index) = self.selected_element {
                if element_index < self.drawing_elements.len() {
                    self.drag_start_rect = self.drawing_elements[element_index].get_bounding_rect();
                }
            }
        } else if handle_mode == DragMode::MovingElement {
            // 选择并开始移动绘图元素
            if self.selected_element.is_none() {
                if let Some(element_index) = self.get_element_at_position(x, y) {
                    self.selected_element = Some(element_index);
                }
            }
            self.drag_mode = DragMode::MovingElement;
            self.mouse_pressed = true;
            self.drag_start_pos = POINT { x, y };
        } else if matches!(
            handle_mode,
            DragMode::Moving
                | DragMode::ResizingTopLeft
                | DragMode::ResizingTopCenter
                | DragMode::ResizingTopRight
                | DragMode::ResizingMiddleRight
                | DragMode::ResizingBottomRight
                | DragMode::ResizingBottomCenter
                | DragMode::ResizingBottomLeft
                | DragMode::ResizingMiddleLeft
        ) {
            // **修复：无论是否选择了绘图工具，都允许操作选择框**
            self.drag_mode = handle_mode;
            self.mouse_pressed = true;
            self.drag_start_pos = POINT { x, y };
            self.drag_start_rect = self.selection_rect;
        } else if self.current_tool != DrawingTool::None {
            // 当选择了绘图工具时，只允许在选择框内绘图
            if x >= self.selection_rect.left
                && x <= self.selection_rect.right
                && y >= self.selection_rect.top
                && y <= self.selection_rect.bottom
            {
                self.save_history();
                self.drag_mode = DragMode::DrawingShape;
                self.mouse_pressed = true;
                self.drag_start_pos = POINT { x, y };

                let mut new_element = DrawingElement::new(self.current_tool);
                new_element.color = self.drawing_color;
                new_element.thickness = self.drawing_thickness;

                match self.current_tool {
                    DrawingTool::Pen => {
                        new_element.points.push(POINT { x, y });
                    }
                    DrawingTool::Rectangle | DrawingTool::Circle | DrawingTool::Arrow => {
                        new_element.rect = RECT {
                            left: x,
                            top: y,
                            right: x,
                            bottom: y,
                        };
                    }
                    DrawingTool::Text => {
                        new_element.rect = RECT {
                            left: x,
                            top: y,
                            right: x + 100,
                            bottom: y + 30,
                        };
                        new_element.text = "文字".to_string();
                    }
                    _ => {}
                }

                self.current_element = Some(new_element);
            }
        } else {
            // 创建新的选择框
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
            self.toolbar.hide();
            self.selected_element = None; // 清除元素选择
        }
    }

    fn update_drag(&mut self, x: i32, y: i32) -> bool {
        if !self.mouse_pressed {
            return false;
        }

        let min_box_size = MIN_BOX_SIZE;
        let old_rect = self.selection_rect; // 保存旧矩形用于变化检测

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
            }
            DragMode::DrawingShape => {
                if let Some(ref mut element) = self.current_element {
                    match element.tool {
                        DrawingTool::Pen => {
                            // **修复：先提取选择框坐标，避免借用冲突**
                            let selection_left = self.selection_rect.left;
                            let selection_right = self.selection_rect.right;
                            let selection_top = self.selection_rect.top;
                            let selection_bottom = self.selection_rect.bottom;

                            let clamped_x = x.max(selection_left).min(selection_right);
                            let clamped_y = y.max(selection_top).min(selection_bottom);
                            element.points.push(POINT {
                                x: clamped_x,
                                y: clamped_y,
                            });
                        }
                        DrawingTool::Rectangle | DrawingTool::Circle | DrawingTool::Arrow => {
                            // **修复：先提取选择框坐标**
                            let selection_left = self.selection_rect.left;
                            let selection_right = self.selection_rect.right;
                            let selection_top = self.selection_rect.top;
                            let selection_bottom = self.selection_rect.bottom;

                            let clamped_x = x.max(selection_left).min(selection_right);
                            let clamped_y = y.max(selection_top).min(selection_bottom);
                            let left = self.drag_start_pos.x.min(clamped_x);
                            let right = self.drag_start_pos.x.max(clamped_x);
                            let top = self.drag_start_pos.y.min(clamped_y);
                            let bottom = self.drag_start_pos.y.max(clamped_y);

                            element.rect = RECT {
                                left: left.max(selection_left).min(selection_right),
                                top: top.max(selection_top).min(selection_bottom),
                                right: right.max(selection_left).min(selection_right),
                                bottom: bottom.max(selection_top).min(selection_bottom),
                            };
                        }
                        _ => {}
                    }
                }
                return true;
            }
            DragMode::MovingElement => {
                if let Some(element_index) = self.selected_element {
                    if element_index < self.drawing_elements.len() {
                        let dx = x - self.drag_start_pos.x;
                        let dy = y - self.drag_start_pos.y;

                        // **修复：先提取选择框坐标**
                        let selection_left = self.selection_rect.left;
                        let selection_right = self.selection_rect.right;
                        let selection_top = self.selection_rect.top;
                        let selection_bottom = self.selection_rect.bottom;

                        let element = &mut self.drawing_elements[element_index];

                        match element.tool {
                            DrawingTool::Rectangle
                            | DrawingTool::Circle
                            | DrawingTool::Arrow
                            | DrawingTool::Text => {
                                let new_left = element.rect.left + dx;
                                let new_top = element.rect.top + dy;
                                let new_right = element.rect.right + dx;
                                let new_bottom = element.rect.bottom + dy;

                                // **修复：改为更宽松的边界检查，允许元素接触边界**
                                if new_left >= selection_left - 10
                                    && new_right <= selection_right + 10
                                    && new_top >= selection_top - 10
                                    && new_bottom <= selection_bottom + 10
                                {
                                    // 进一步限制，确保元素主体仍在选择框内
                                    let final_left =
                                        new_left.max(selection_left).min(selection_right - 10);
                                    let final_top =
                                        new_top.max(selection_top).min(selection_bottom - 10);
                                    let width = element.rect.right - element.rect.left;
                                    let height = element.rect.bottom - element.rect.top;

                                    element.rect.left = final_left;
                                    element.rect.top = final_top;
                                    element.rect.right = (final_left + width).min(selection_right);
                                    element.rect.bottom =
                                        (final_top + height).min(selection_bottom);

                                    self.drag_start_pos = POINT { x, y };
                                }
                            }
                            DrawingTool::Pen => {
                                // **修复：对画笔工具也使用更宽松的边界检查**
                                let mut can_move = true;
                                let new_points: Vec<POINT> = element
                                    .points
                                    .iter()
                                    .map(|point| {
                                        let new_x = point.x + dx;
                                        let new_y = point.y + dy;
                                        if new_x < selection_left - 10
                                            || new_x > selection_right + 10
                                            || new_y < selection_top - 10
                                            || new_y > selection_bottom + 10
                                        {
                                            can_move = false;
                                        }
                                        POINT { x: new_x, y: new_y }
                                    })
                                    .collect();

                                if can_move {
                                    // 限制点在边界内
                                    for (i, new_point) in new_points.iter().enumerate() {
                                        element.points[i].x =
                                            new_point.x.max(selection_left).min(selection_right);
                                        element.points[i].y =
                                            new_point.y.max(selection_top).min(selection_bottom);
                                    }
                                    self.drag_start_pos = POINT { x, y };
                                }
                            }
                            _ => {}
                        }
                        return true;
                    }
                }
            }
            DragMode::ResizingElement => {
                if let Some(element_index) = self.selected_element {
                    if element_index < self.drawing_elements.len() {
                        let dx = x - self.drag_start_pos.x;
                        let dy = y - self.drag_start_pos.y;

                        // **修复：先提取选择框坐标**
                        let selection_right = self.selection_rect.right;
                        let selection_bottom = self.selection_rect.bottom;

                        let element = &mut self.drawing_elements[element_index];

                        match element.tool {
                            DrawingTool::Rectangle
                            | DrawingTool::Circle
                            | DrawingTool::Arrow
                            | DrawingTool::Text => {
                                let new_right = (element.rect.right + dx).min(selection_right);
                                let new_bottom = (element.rect.bottom + dy).min(selection_bottom);

                                // 确保最小尺寸
                                if new_right - element.rect.left >= 10
                                    && new_bottom - element.rect.top >= 10
                                {
                                    element.rect.right = new_right;
                                    element.rect.bottom = new_bottom;
                                    self.drag_start_pos = POINT { x, y };
                                }
                            }
                            _ => {}
                        }
                        return true;
                    }
                }
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
                    .min(start_rect.right - min_box_size);
                self.selection_rect.top = (start_rect.top + dy)
                    .max(0)
                    .min(start_rect.bottom - min_box_size);
            }
            DragMode::ResizingTopCenter => {
                let start_rect = self.drag_start_rect;
                let dy = y - self.drag_start_pos.y;

                self.selection_rect.top = (start_rect.top + dy)
                    .max(0)
                    .min(start_rect.bottom - min_box_size);
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
            }
            DragMode::ResizingMiddleRight => {
                let start_rect = self.drag_start_rect;
                let dx = x - self.drag_start_pos.x;

                self.selection_rect.right = (start_rect.right + dx)
                    .min(self.screen_width)
                    .max(start_rect.left + min_box_size);
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
            }
            DragMode::ResizingBottomCenter => {
                let start_rect = self.drag_start_rect;
                let dy = y - self.drag_start_pos.y;

                self.selection_rect.bottom = (start_rect.bottom + dy)
                    .min(self.screen_height)
                    .max(start_rect.top + min_box_size);
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
            }
            DragMode::ResizingMiddleLeft => {
                let start_rect = self.drag_start_rect;
                let dx = x - self.drag_start_pos.x;

                self.selection_rect.left = (start_rect.left + dx)
                    .max(0)
                    .min(start_rect.right - min_box_size);
            }
            DragMode::None => {
                return false;
            }
        }

        let has_changed = old_rect.left != self.selection_rect.left
            || old_rect.top != self.selection_rect.top
            || old_rect.right != self.selection_rect.right
            || old_rect.bottom != self.selection_rect.bottom;

        // 修复工具栏位置更新
        if has_changed && self.toolbar.visible {
            // **修复：分离借用操作**
            let selection_rect = self.selection_rect;
            let screen_width = self.screen_width;
            let screen_height = self.screen_height;

            self.toolbar
                .update_position(&selection_rect, screen_width, screen_height);
        }

        has_changed
    }
    // 优化2：超高效的双缓冲渲染
    fn render_to_buffer_fast(&self) {
        unsafe {
            let hdc = self.back_buffer_dc;

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

            if self.has_selection {
                self.draw_fast_dimmed_overlay_simple(hdc);
                self.draw_selection_border_fast(hdc);
                self.draw_handles_fast(hdc);

                if self.toolbar.visible && self.is_selection_valid() {
                    self.draw_toolbar(hdc);
                }
            }

            // 绘制所有绘图元素
            self.draw_all_elements(hdc);

            // 绘制当前正在绘制的元素
            if let Some(ref element) = self.current_element {
                self.draw_element(hdc, element);
            }

            // 绘制选中元素的调整手柄
            if let Some(element_index) = self.selected_element {
                if element_index < self.drawing_elements.len() {
                    self.draw_element_handles(hdc, &self.drawing_elements[element_index]);
                }
            }
        }
    }
    fn draw_all_elements(&self, hdc: HDC) {
        for element in &self.drawing_elements {
            self.draw_element(hdc, element);
        }
    }

    fn draw_element(&self, hdc: HDC, element: &DrawingElement) {
        unsafe {
            let pen = CreatePen(PS_SOLID, element.thickness, element.color);
            let old_pen = SelectObject(hdc, pen);

            match element.tool {
                DrawingTool::Rectangle => {
                    // **只绘制边框，不填充**
                    SelectObject(hdc, GetStockObject(NULL_BRUSH)); // 透明填充
                    Rectangle(
                        hdc,
                        element.rect.left,
                        element.rect.top,
                        element.rect.right,
                        element.rect.bottom,
                    );
                }
                DrawingTool::Circle => {
                    // **只绘制边框，不填充**
                    SelectObject(hdc, GetStockObject(NULL_BRUSH)); // 透明填充
                    Ellipse(
                        hdc,
                        element.rect.left,
                        element.rect.top,
                        element.rect.right,
                        element.rect.bottom,
                    );
                }
                DrawingTool::Arrow => {
                    // **修复箭头绘制**
                    self.draw_arrow_fixed(hdc, &element.rect, element.thickness);
                }
                DrawingTool::Pen => {
                    if element.points.len() > 1 {
                        for i in 1..element.points.len() {
                            MoveToEx(hdc, element.points[i - 1].x, element.points[i - 1].y, None);
                            LineTo(hdc, element.points[i].x, element.points[i].y);
                        }
                    }
                }
                DrawingTool::Text => {
                    SetTextColor(hdc, element.color);
                    SetBkMode(hdc, TRANSPARENT);
                    let font = CreateFontW(
                        24,
                        0,
                        0,
                        0,
                        FW_NORMAL.0 as i32,
                        0,
                        0,
                        0,
                        DEFAULT_CHARSET.0 as u32,
                        OUT_DEFAULT_PRECIS.0 as u32,
                        CLIP_DEFAULT_PRECIS.0 as u32,
                        CLEARTYPE_QUALITY.0 as u32,
                        (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
                        PCWSTR(std::ptr::null()),
                    );
                    let old_font = SelectObject(hdc, font);

                    let mut text_wide = to_wide_chars(&element.text);
                    let mut text_rect = element.rect;
                    DrawTextW(
                        hdc,
                        &mut text_wide,
                        &mut text_rect as *mut RECT,
                        DT_LEFT | DT_TOP | DT_SINGLELINE,
                    );

                    SelectObject(hdc, old_font);
                    DeleteObject(font);
                }
                _ => {}
            }

            SelectObject(hdc, old_pen);
            DeleteObject(pen);
        }
    }
    fn draw_arrow_fixed(&self, hdc: HDC, rect: &RECT, thickness: i32) {
        unsafe {
            let dx = rect.right - rect.left;
            let dy = rect.bottom - rect.top;
            let length = ((dx * dx + dy * dy) as f64).sqrt();

            if length < 5.0 {
                return; // 太短不绘制
            }

            // 绘制箭头主线
            MoveToEx(hdc, rect.left, rect.top, None);
            LineTo(hdc, rect.right, rect.bottom);

            // **修复箭头头部计算**
            let arrow_length = (thickness as f64 * 4.0).max(10.0).min(length * 0.4);
            let arrow_angle = 0.5f64; // 箭头角度

            let cos_main = dx as f64 / length;
            let sin_main = dy as f64 / length;

            // 计算箭头两个分支的端点
            let end_x = rect.right as f64 - arrow_length * cos_main;
            let end_y = rect.bottom as f64 - arrow_length * sin_main;

            // 第一个分支
            let cos1 = cos_main * arrow_angle.cos() + sin_main * arrow_angle.sin();
            let sin1 = sin_main * arrow_angle.cos() - cos_main * arrow_angle.sin();
            let arrow_x1 = end_x + arrow_length * cos1;
            let arrow_y1 = end_y + arrow_length * sin1;

            // 第二个分支
            let cos2 = cos_main * arrow_angle.cos() - sin_main * arrow_angle.sin();
            let sin2 = sin_main * arrow_angle.cos() + cos_main * arrow_angle.sin();
            let arrow_x2 = end_x + arrow_length * cos2;
            let arrow_y2 = end_y + arrow_length * sin2;

            // 绘制箭头头部
            MoveToEx(hdc, rect.right, rect.bottom, None);
            LineTo(hdc, arrow_x1 as i32, arrow_y1 as i32);

            MoveToEx(hdc, rect.right, rect.bottom, None);
            LineTo(hdc, arrow_x2 as i32, arrow_y2 as i32);
        }
    }

    fn draw_arrow(&self, hdc: HDC, rect: &RECT) {
        unsafe {
            // 绘制箭头主线
            MoveToEx(hdc, rect.left, rect.top, None);
            LineTo(hdc, rect.right, rect.bottom);

            // 计算箭头头部
            let dx = rect.right - rect.left;
            let dy = rect.bottom - rect.top;
            let length = ((dx * dx + dy * dy) as f64).sqrt();

            if length > 0.0 {
                let arrow_length = 20.0f64;
                let arrow_angle = 0.5f64;

                let cos_main = dx as f64 / length;
                let sin_main = dy as f64 / length;

                let end_x = rect.right as f64 - arrow_length * cos_main;
                let end_y = rect.bottom as f64 - arrow_length * sin_main;

                let cos_arrow = (cos_main * arrow_angle.cos() - sin_main * arrow_angle.sin());
                let sin_arrow = (sin_main * arrow_angle.cos() + cos_main * arrow_angle.sin());

                let arrow_x1 = end_x + arrow_length * cos_arrow;
                let arrow_y1 = end_y + arrow_length * sin_arrow;

                let cos_arrow2 = (cos_main * arrow_angle.cos() + sin_main * arrow_angle.sin());
                let sin_arrow2 = (sin_main * arrow_angle.cos() - cos_main * arrow_angle.sin());

                let arrow_x2 = end_x + arrow_length * cos_arrow2;
                let arrow_y2 = end_y + arrow_length * sin_arrow2;

                // 绘制箭头头部
                MoveToEx(hdc, rect.right, rect.bottom, None);
                LineTo(hdc, arrow_x1 as i32, arrow_y1 as i32);
                MoveToEx(hdc, rect.right, rect.bottom, None);
                LineTo(hdc, arrow_x2 as i32, arrow_y2 as i32);
            }
        }
    }
    fn invalidate_selection_area(&self, hwnd: HWND) {
        unsafe {
            if self.has_selection {
                // 扩展一点边距以包含边框和手柄
                let margin = 20;
                let invalid_rect = RECT {
                    left: (self.selection_rect.left - margin).max(0),
                    top: (self.selection_rect.top - margin).max(0),
                    right: (self.selection_rect.right + margin).min(self.screen_width),
                    bottom: (self.selection_rect.bottom + margin).min(self.screen_height),
                };

                InvalidateRect(hwnd, Some(&invalid_rect), FALSE);
            } else {
                InvalidateRect(hwnd, None, FALSE);
            }
            UpdateWindow(hwnd);
        }
    }
    fn draw_element_handles(&self, hdc: HDC, element: &DrawingElement) {
        unsafe {
            let handle_brush = CreateSolidBrush(RGB!(255, 255, 255));
            let border_pen = CreatePen(PS_SOLID, 2, RGB!(0, 120, 215)); // 蓝色边框
            let old_brush = SelectObject(hdc, handle_brush);
            let old_pen = SelectObject(hdc, border_pen);

            let rect = element.get_bounding_rect();
            const HANDLE_SIZE: i32 = 8;
            let handles = [
                (rect.left, rect.top),
                ((rect.left + rect.right) / 2, rect.top),
                (rect.right, rect.top),
                (rect.right, (rect.top + rect.bottom) / 2),
                (rect.right, rect.bottom),
                ((rect.left + rect.right) / 2, rect.bottom),
                (rect.left, rect.bottom),
                (rect.left, (rect.top + rect.bottom) / 2),
            ];

            for (x, y) in handles.iter() {
                Rectangle(
                    hdc,
                    x - HANDLE_SIZE / 2,
                    y - HANDLE_SIZE / 2,
                    x + HANDLE_SIZE / 2,
                    y + HANDLE_SIZE / 2,
                );
            }

            SelectObject(hdc, old_brush);
            SelectObject(hdc, old_pen);
            DeleteObject(handle_brush);
            DeleteObject(border_pen);
        }
    }

    fn save_to_file(&self) -> Result<()> {
        // 这里可以实现保存到文件的逻辑
        // 目前先保存到剪贴板
        self.save_selection()
    }
    fn draw_fast_dimmed_overlay_simple(&self, hdc: HDC) {
        unsafe {
            let rects = [
                RECT {
                    left: 0,
                    top: 0,
                    right: self.screen_width,
                    bottom: self.selection_rect.top,
                },
                RECT {
                    left: 0,
                    top: self.selection_rect.bottom,
                    right: self.screen_width,
                    bottom: self.screen_height,
                },
                RECT {
                    left: 0,
                    top: self.selection_rect.top,
                    right: self.selection_rect.left,
                    bottom: self.selection_rect.bottom,
                },
                RECT {
                    left: self.selection_rect.right,
                    top: self.selection_rect.top,
                    right: self.screen_width,
                    bottom: self.selection_rect.bottom,
                },
            ];

            for rect in rects.iter() {
                let width = rect.right - rect.left;
                let height = rect.bottom - rect.top;

                if width > 0 && height > 0 {
                    // 使用简化的AlphaBlend - 这才是正确的半透明方法
                    let temp_dc = CreateCompatibleDC(hdc);
                    let temp_bitmap = CreateCompatibleBitmap(hdc, width, height);
                    let old_bitmap = SelectObject(temp_dc, temp_bitmap);

                    // 半透明混合
                    let blend_func = BLENDFUNCTION {
                        BlendOp: AC_SRC_OVER as u8,
                        BlendFlags: 0,
                        SourceConstantAlpha: 120, // 半透明效果
                        AlphaFormat: 0,
                    };

                    AlphaBlend(
                        hdc, rect.left, rect.top, width, height, temp_dc, 0, 0, width, height,
                        blend_func,
                    );

                    SelectObject(temp_dc, old_bitmap);
                    DeleteObject(temp_bitmap);
                    DeleteDC(temp_dc);
                }
            }
        }
    }

    fn draw_selection_border_fast(&self, hdc: HDC) {
        unsafe {
            let old_pen = SelectObject(hdc, self.border_pen);

            // 使用MoveToEx和LineTo绘制四条边，比Rectangle快
            MoveToEx(
                hdc,
                self.selection_rect.left,
                self.selection_rect.top,
                Some(std::ptr::null_mut()),
            );
            LineTo(hdc, self.selection_rect.right, self.selection_rect.top);
            LineTo(hdc, self.selection_rect.right, self.selection_rect.bottom);
            LineTo(hdc, self.selection_rect.left, self.selection_rect.bottom);
            LineTo(hdc, self.selection_rect.left, self.selection_rect.top);

            SelectObject(hdc, old_pen);
        }
    }

    fn draw_handles_fast(&self, hdc: HDC) {
        // **修复：同时显示选择框手柄和绘图工具提示**
        unsafe {
            let old_brush = SelectObject(hdc, self.handle_brush);
            let old_pen = SelectObject(hdc, GetStockObject(NULL_PEN));

            let center_x = (self.selection_rect.left + self.selection_rect.right) / 2;
            let center_y = (self.selection_rect.top + self.selection_rect.bottom) / 2;
            let handle_size = 12;
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
                Ellipse(
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

    // 优化7：立即重绘方法
    fn invalidate_immediately(&self, hwnd: HWND) {
        unsafe {
            // 使用最快的重绘方式
            InvalidateRect(hwnd, None, FALSE);
            UpdateWindow(hwnd); // 立即强制重绘
        }
    }

    fn end_drag(&mut self) {
        if self.drag_mode == DragMode::DrawingShape {
            if let Some(element) = self.current_element.take() {
                // 只有当形状足够大时才添加
                let rect = element.get_bounding_rect();
                let width = rect.right - rect.left;
                let height = rect.bottom - rect.top;

                if width > 5 && height > 5 || element.tool == DrawingTool::Pen {
                    self.drawing_elements.push(element);
                }
            }
        } else if self.drag_mode == DragMode::Drawing {
            let width = self.selection_rect.right - self.selection_rect.left;
            let height = self.selection_rect.bottom - self.selection_rect.top;

            if width < MIN_BOX_SIZE || height < MIN_BOX_SIZE {
                self.has_selection = false;
                self.toolbar.hide();
            } else {
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
    fn is_selection_valid(&self) -> bool {
        if !self.has_selection {
            return false;
        }

        let width = self.selection_rect.right - self.selection_rect.left;
        let height = self.selection_rect.bottom - self.selection_rect.top;

        width >= MIN_BOX_SIZE && height >= MIN_BOX_SIZE
    }
    fn handle_toolbar_click(&mut self, button: ToolbarButton) -> bool {
        self.toolbar.set_clicked_button(button);

        match button {
            ToolbarButton::Rectangle => {
                self.current_tool = DrawingTool::Rectangle;
                self.selected_element = None;
                false
            }
            ToolbarButton::Circle => {
                self.current_tool = DrawingTool::Circle;
                self.selected_element = None;
                false
            }
            ToolbarButton::Arrow => {
                self.current_tool = DrawingTool::Arrow;
                self.selected_element = None;
                false
            }
            ToolbarButton::Pen => {
                self.current_tool = DrawingTool::Pen;
                self.selected_element = None;
                false
            }
            ToolbarButton::Text => {
                self.current_tool = DrawingTool::Text;
                self.selected_element = None;
                false
            }
            ToolbarButton::Undo => {
                // **只有能撤销时才执行撤销**
                if self.can_undo() {
                    self.undo();
                }
                false // 撤销不退出程序
            }
            ToolbarButton::Save => {
                let _ = self.save_to_file();
                false
            }
            ToolbarButton::Copy => {
                let _ = self.save_selection();
                false
            }
            ToolbarButton::Confirm => {
                let _ = self.save_selection();
                true // 确认后退出
            }
            ToolbarButton::Cancel => {
                // **取消时重置工具选择**
                self.current_tool = DrawingTool::None;
                self.selected_element = None;
                self.current_element = None;
                true // 取消后退出
            }
            ToolbarButton::None => false,
        }
    }
    fn draw_toolbar(&self, hdc: HDC) {
        unsafe {
            // 绘制工具栏背景（白色）
            let old_brush = SelectObject(hdc, self.toolbar_brush);
            let old_pen = SelectObject(hdc, self.toolbar_border_pen);

            // 绘制圆角矩形背景
            RoundRect(
                hdc,
                self.toolbar.rect.left,
                self.toolbar.rect.top,
                self.toolbar.rect.right,
                self.toolbar.rect.bottom,
                10,
                10,
            );

            // 绘制按钮
            for (rect, button_type, icon_data) in &self.toolbar.buttons {
                // 确定按钮状态和颜色
                let (button_brush, icon_color, needs_cleanup) = if *button_type
                    == self.toolbar.clicked_button
                {
                    // 点击状态：绿色背景，白色图标
                    let green_brush = CreateSolidBrush(RGB!(34, 197, 94));
                    (green_brush, RGB!(255, 255, 255), true)
                } else if *button_type == self.toolbar.hovered_button {
                    // 悬停状态：浅灰背景，深灰图标
                    (self.button_hover_brush, RGB!(64, 64, 64), false)
                } else {
                    // **检查特殊状态**
                    let is_current_tool = match button_type {
                        ToolbarButton::Rectangle => self.current_tool == DrawingTool::Rectangle,
                        ToolbarButton::Circle => self.current_tool == DrawingTool::Circle,
                        ToolbarButton::Arrow => self.current_tool == DrawingTool::Arrow,
                        ToolbarButton::Pen => self.current_tool == DrawingTool::Pen,
                        ToolbarButton::Text => self.current_tool == DrawingTool::Text,
                        _ => false,
                    };

                    // **撤销按钮禁用状态**
                    let is_undo_disabled = *button_type == ToolbarButton::Undo && !self.can_undo();

                    if is_current_tool {
                        // 当前选中的工具：蓝色背景
                        let active_brush = CreateSolidBrush(RGB!(200, 230, 255));
                        (active_brush, RGB!(0, 120, 215), true)
                    } else if is_undo_disabled {
                        // 禁用的撤销按钮：灰色
                        (self.button_brush, RGB!(180, 180, 180), false)
                    } else {
                        // 默认状态：白色背景，深灰图标
                        (self.button_brush, RGB!(64, 64, 64), false)
                    }
                };

                // 绘制按钮背景
                SelectObject(hdc, button_brush);
                SelectObject(hdc, GetStockObject(NULL_PEN));

                RoundRect(hdc, rect.left, rect.top, rect.right, rect.bottom, 6, 6);

                // 绘制字符图标
                self.draw_text_icon(hdc, rect, icon_data, icon_color);

                // 如果创建了临时画刷，需要删除
                if needs_cleanup {
                    DeleteObject(button_brush);
                }
            }

            SelectObject(hdc, old_brush);
            SelectObject(hdc, old_pen);
        }
    }
    // 新增：绘制文字图标
    fn draw_text_icon(&self, hdc: HDC, rect: &RECT, icon_data: &IconData, color: COLORREF) {
        unsafe {
            // 设置文本颜色和背景
            SetTextColor(hdc, color);
            SetBkMode(hdc, TRANSPARENT);

            // 创建字体 - 调整字体大小以适应按钮
            let font = CreateFontW(
                20, // 调整字体大小
                0,
                0,
                0,
                FW_NORMAL.0 as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET.0 as u32,
                OUT_DEFAULT_PRECIS.0 as u32,
                CLIP_DEFAULT_PRECIS.0 as u32,
                CLEARTYPE_QUALITY.0 as u32,
                (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
                PCWSTR(std::ptr::null()),
            );

            let old_font = SelectObject(hdc, font);

            // 绘制文本 - 使用正确的 DrawTextW 调用方式
            let mut text_wide = to_wide_chars(&icon_data.text);

            let mut text_rect = *rect;

            // 绘制居中文本
            DrawTextW(
                hdc,
                &mut text_wide,
                &mut text_rect as *mut RECT,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );

            // 恢复字体
            SelectObject(hdc, old_font);
            DeleteObject(font);
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
        WM_CREATE => {
            // 启动时的优化设置
            match WindowState::new() {
                Ok(state) => {
                    let state_box = Box::new(state);
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state_box) as isize);

                    // 设置窗口为最高优先级
                    SetWindowPos(
                        hwnd,
                        HWND_TOPMOST,
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                    );

                    LRESULT(0)
                }
                Err(_) => LRESULT(-1),
            }
        }

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

                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);

                // 使用优化的渲染方法
                state.render_to_buffer_fast();

                // 一次性复制到屏幕
                BitBlt(
                    hdc,
                    0,
                    0,
                    state.screen_width,
                    state.screen_height,
                    state.back_buffer_dc,
                    0,
                    0,
                    SRCCOPY,
                );

                EndPaint(hwnd, &ps);
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
                    if state.update_drag(x, y) {
                        state.invalidate_immediately(hwnd);
                    }
                } else {
                    let toolbar_button = state.toolbar.get_button_at_position(x, y);
                    if toolbar_button != state.toolbar.hovered_button {
                        state.toolbar.set_hovered_button(toolbar_button);

                        if state.toolbar.visible {
                            InvalidateRect(hwnd, Some(&state.toolbar.rect), FALSE);
                        }
                    }

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

                let toolbar_button = state.toolbar.get_button_at_position(x, y);
                if toolbar_button != ToolbarButton::None {
                    state.toolbar.set_clicked_button(toolbar_button);
                    InvalidateRect(hwnd, Some(&state.toolbar.rect), FALSE);
                } else {
                    state.toolbar.clear_clicked_button();
                    state.start_drag(x, y);
                    if state.mouse_pressed {
                        SetCapture(hwnd);

                        // 关键优化：提升线程和进程优先级
                        SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_TIME_CRITICAL);
                        SetPriorityClass(GetCurrentProcess(), REALTIME_PRIORITY_CLASS);
                    }
                }
            }
            LRESULT(0)
        }

        WM_LBUTTONUP => {
            let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
            if !state_ptr.is_null() {
                let state = &mut *state_ptr;

                // 恢复正常优先级
                SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_NORMAL);
                SetPriorityClass(GetCurrentProcess(), NORMAL_PRIORITY_CLASS);

                let x = (lparam.0 & 0xFFFF) as i16 as i32;
                let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
                let toolbar_button = state.toolbar.get_button_at_position(x, y);

                if toolbar_button != ToolbarButton::None
                    && toolbar_button == state.toolbar.clicked_button
                {
                    if state.handle_toolbar_click(toolbar_button) {
                        PostQuitMessage(0);
                        return LRESULT(0);
                    }
                } else {
                    state.toolbar.clear_clicked_button();
                    state.end_drag();
                    ReleaseCapture();
                }

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
            let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
            if !state_ptr.is_null() {
                let state = &mut *state_ptr;

                match wparam.0 as u32 {
                    key if key == VK_ESCAPE.0.into() => {
                        PostQuitMessage(0);
                    }
                    key if key == VK_RETURN.0.into() => {
                        let _ = state.save_selection();
                        PostQuitMessage(0);
                    }
                    key if key == VK_Z.0.into() => {
                        // Ctrl+Z 撤销
                        if (GetKeyState(VK_CONTROL.0 as i32) & 0x8000u16 as i16) != 0 {
                            if state.undo() {
                                InvalidateRect(hwnd, None, FALSE);
                            }
                        }
                    }
                    _ => {}
                }
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
        timeBeginPeriod(1);
        SetProcessDpiAwareness(PROCESS_PER_MONITOR_DPI_AWARE)?;
        let instance = GetModuleHandleW(None)?;

        let class_name = to_wide_chars(WINDOW_CLASS_NAME);

        let window_class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance.into(),
            lpszClassName: PCWSTR(class_name.as_ptr()),
            hbrBackground: HBRUSH(std::ptr::null_mut()), // 透明背景
            hCursor: LoadCursorW(HINSTANCE(std::ptr::null_mut()), IDC_ARROW)?,
            style: CS_DBLCLKS | CS_OWNDC, // 添加CS_OWNDC优化绘制
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
        timeEndPeriod(1);
        Ok(())
    }
}
