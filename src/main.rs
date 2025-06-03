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
use windows::Win32::UI::HiDpi::{PROCESS_PER_MONITOR_DPI_AWARE, SetProcessDpiAwareness};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, ReleaseCapture, SetCapture, VK_CONTROL, VK_ESCAPE, VK_RETURN, VK_Z,
    GetKeyState, ReleaseCapture, SetCapture, VK_CONTROL, VK_ESCAPE, VK_RETURN, VK_Z,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::*;

macro_rules! RGB {
    ($r:expr, $g:expr, $b:expr) => {
        COLORREF(($r as u32) | (($g as u32) << 8) | (($b as u32) << 16))
    };
}
const WINDOW_CLASS_NAME: &str = "ScreenshotWindow";
const MIN_BOX_SIZE: i32 = 50;

// 颜色常量定义
const COLOR_SELECTION_BORDER: COLORREF = RGB!(0, 120, 215);
const COLOR_SELECTION_DASHED: COLORREF = RGB!(80, 80, 80); // 深灰色虚线
const COLOR_HANDLE_FILL: COLORREF = RGB!(255, 255, 255); // 白色手柄背景
const COLOR_HANDLE_BORDER: COLORREF = RGB!(0, 120, 215); // 手柄边框
const COLOR_ELEMENT_HANDLE_FILL: COLORREF = RGB!(255, 255, 255); // 元素手柄白色背景
const COLOR_ELEMENT_HANDLE_BORDER: COLORREF = RGB!(0, 120, 215); // 元素手柄蓝色边框
const COLOR_SELECTION_HANDLE_FILL: COLORREF = RGB!(255, 255, 255); // 大框手柄白色背景
const COLOR_SELECTION_HANDLE_BORDER: COLORREF = RGB!(0, 120, 215); // 大框手柄蓝色边框
const COLOR_MASK: COLORREF = RGB!(0, 0, 0);
const COLOR_TOOLBAR_BG: COLORREF = RGB!(255, 255, 255);
const COLOR_TOOLBAR_BORDER: COLORREF = RGB!(200, 200, 200);
const COLOR_BUTTON_BG: COLORREF = RGB!(255, 255, 255);
const COLOR_BUTTON_HOVER: COLORREF = RGB!(240, 240, 240);
const COLOR_BUTTON_ACTIVE: COLORREF = RGB!(200, 230, 255);
const COLOR_BUTTON_DISABLED: COLORREF = RGB!(180, 180, 180);
const COLOR_TEXT_NORMAL: COLORREF = RGB!(64, 64, 64);
const COLOR_TEXT_WHITE: COLORREF = RGB!(255, 255, 255);
const COLOR_TEXT_ACTIVE: COLORREF = RGB!(0, 120, 215);

// 工具栏尺寸和距离常量
const TOOLBAR_HEIGHT: i32 = 40; // 工具栏高度
const BUTTON_WIDTH: i32 = 30; // 按钮宽度
const BUTTON_HEIGHT: i32 = 30; // 按钮高度
const BUTTON_SPACING: i32 = 4; // 按钮间距
const TOOLBAR_PADDING: i32 = 8; // 工具栏内边距
const TOOLBAR_MARGIN: i32 = 3; // 工具栏距离选择框的距离
const BUTTON_COUNT: i32 = 10;
// 尺寸常量
const HANDLE_SIZE: i32 = 8; // 手柄绘制尺寸（小一点）
const SELECTION_HANDLE_SIZE: i32 = 8; // 大框手柄尺寸（方块）
const HANDLE_DETECTION_RADIUS: i32 = 10; // 检测范围保持不变

// 工具栏图标
const SAVE_ICON: &str = "💾";
const COPY_ICON: &str = "📋";
const RECT_ICON: &str = "⬜";
const CIRCLE_ICON: &str = "⭕";
const ARROW_ICON: &str = "➡";
const PEN_ICON: &str = "✏";
const TEXT_ICON: &str = "T";
const UNDO_ICON: &str = "↶";
const CONFIRM_ICON: &str = "✓";
const CANCEL_ICON: &str = "✕";

// 辅助函数：将字符串转换为宽字符
fn to_wide_chars(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(once(0)).collect()
}

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
fn point_to_line_distance(px: i32, py: i32, x1: i32, y1: i32, x2: i32, y2: i32) -> f64 {
    let px = px as f64;
    let py = py as f64;
    let x1 = x1 as f64;
    let y1 = y1 as f64;
    let x2 = x2 as f64;
    let y2 = y2 as f64;

    let a = px - x1;
    let b = py - y1;
    let c = x2 - x1;
    let d = y2 - y1;

    let dot = a * c + b * d;
    let len_sq = c * c + d * d;

    if len_sq == 0.0 {
        // 线段退化为点
        return ((px - x1).powi(2) + (py - y1).powi(2)).sqrt();
    }

    let param = dot / len_sq;

    let (xx, yy) = if param < 0.0 {
        (x1, y1)
    } else if param > 1.0 {
        (x2, y2)
    } else {
        (x1 + param * c, y1 + param * d)
    };

    ((px - xx).powi(2) + (py - yy).powi(2)).sqrt()
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
    buttons: Vec<(RECT, ToolbarButton, IconData)>,
    hovered_button: ToolbarButton,
    clicked_button: ToolbarButton,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum DragMode {
    None,
    Drawing,
    Moving,
    ResizingTopLeft,
    ResizingTopCenter,
    ResizingTopRight,
    ResizingMiddleRight,
    ResizingBottomRight,
    ResizingBottomCenter,
    ResizingBottomLeft,
    ResizingMiddleLeft,
    DrawingShape,
    MovingElement,
    ResizingElement,
}

#[derive(Debug)]
struct WindowState {
    // 截图相关
    screenshot_dc: HDC,
    screenshot_bitmap: HBITMAP,
    screen_width: i32,
    screen_height: i32,

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

    // 持久缓冲区（避免每次创建/删除）
    buffer_dc: HDC,
    buffer_bitmap: HBITMAP,
    mask_dc: HDC,
    mask_bitmap: HBITMAP,

    // 新增：工具栏相关
    toolbar: Toolbar,
    toolbar_brush: HBRUSH,
    toolbar_border_pen: HPEN,
    button_brush: HBRUSH,
    button_hover_brush: HBRUSH,

    // 新增：绘图功能
    current_tool: DrawingTool,
    drawing_elements: Vec<DrawingElement>,
    current_element: Option<DrawingElement>,
    selected_element: Option<usize>,
    drawing_color: COLORREF,
    drawing_thickness: i32,
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
            color: RGB!(255, 0, 0),
            thickness: 3,
            text: String::new(),
            selected: false,
        }
    }

    fn update_bounding_rect(&mut self) {
        match self.tool {
            DrawingTool::Rectangle | DrawingTool::Circle | DrawingTool::Arrow => {
                if self.points.len() >= 2 {
                    let start = &self.points[0];
                    let end = &self.points[1];

                    // 对于箭头，需要考虑箭头头部可能超出线段端点
                    if self.tool == DrawingTool::Arrow {
                        // 计算箭头头部的可能范围
                        let dx = end.x - start.x;
                        let dy = end.y - start.y;
                        let length = ((dx * dx + dy * dy) as f64).sqrt();

                        let mut min_x = start.x.min(end.x);
                        let mut max_x = start.x.max(end.x);
                        let mut min_y = start.y.min(end.y);
                        let mut max_y = start.y.max(end.y);

                        if length > 20.0 {
                            let arrow_length = 15.0_f64;
                            let arrow_angle = 0.5_f64;
                            let unit_x = dx as f64 / length;
                            let unit_y = dy as f64 / length;

                            let wing1_x = end.x
                                - (arrow_length
                                    * (unit_x * arrow_angle.cos() + unit_y * arrow_angle.sin()))
                                    as i32;
                            let wing1_y = end.y
                                - (arrow_length
                                    * (unit_y * arrow_angle.cos() - unit_x * arrow_angle.sin()))
                                    as i32;

                            let wing2_x = end.x
                                - (arrow_length
                                    * (unit_x * arrow_angle.cos() - unit_y * arrow_angle.sin()))
                                    as i32;
                            let wing2_y = end.y
                                - (arrow_length
                                    * (unit_y * arrow_angle.cos() + unit_x * arrow_angle.sin()))
                                    as i32;

                            // 扩展边界以包含箭头头部
                            min_x = min_x.min(wing1_x).min(wing2_x);
                            max_x = max_x.max(wing1_x).max(wing2_x);
                            min_y = min_y.min(wing1_y).min(wing2_y);
                            max_y = max_y.max(wing1_y).max(wing2_y);
                        }

                        // 添加一些边距以便于选择
                        let margin = (self.thickness / 2).max(5);
                        self.rect = RECT {
                            left: min_x - margin,
                            top: min_y - margin,
                            right: max_x + margin,
                            bottom: max_y + margin,
                        };
                    } else {
                        // 矩形和圆形的边界计算
                        self.rect = RECT {
                            left: start.x.min(end.x),
                            top: start.y.min(end.y),
                            right: start.x.max(end.x),
                            bottom: start.y.max(end.y),
                        };
                    }
                }
            }
            DrawingTool::Pen => {
                if !self.points.is_empty() {
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

                    // 给画笔添加一些边距
                    let margin = self.thickness / 2 + 10;
                    self.rect = RECT {
                        left: min_x - margin,
                        top: min_y - margin,
                        right: max_x + margin,
                        bottom: max_y + margin,
                    };
                }
            }
            DrawingTool::Text => {
                if !self.points.is_empty() {
                    let text_width = if self.text.is_empty() {
                        50
                    } else {
                        self.text.len() as i32 * self.thickness * 3
                    };
                    let text_height = self.thickness * 6;
                    self.rect = RECT {
                        left: self.points[0].x,
                        top: self.points[0].y,
                        right: self.points[0].x + text_width,
                        bottom: self.points[0].y + text_height,
                    };
                }
            }
            _ => {}
        }
    }

    // 检查点击是否在元素上
    fn contains_point(&self, x: i32, y: i32) -> bool {
        match self.tool {
            DrawingTool::Pen => {
                // 对于画笔，检查是否接近任何线段
                if self.points.len() < 2 {
                    return false;
                }

                for i in 0..self.points.len() - 1 {
                    let p1 = &self.points[i];
                    let p2 = &self.points[i + 1];

                    // 计算点到线段的距离
                    let distance = point_to_line_distance(x, y, p1.x, p1.y, p2.x, p2.y);
                    if distance <= (self.thickness + 5) as f64 {
                        return true;
                    }
                }
                false
            }
            DrawingTool::Rectangle | DrawingTool::Circle => {
                // 对于矩形和圆形，使用实际的点坐标计算边界
                if self.points.len() >= 2 {
                    let start = &self.points[0];
                    let end = &self.points[1];
                    let left = start.x.min(end.x);
                    let right = start.x.max(end.x);
                    let top = start.y.min(end.y);
                    let bottom = start.y.max(end.y);

                    x >= left && x <= right && y >= top && y <= bottom
                } else {
                    false
                }
            }
            DrawingTool::Arrow => {
                // 对于箭头，检查是否接近箭头线段或箭头头部
                if self.points.len() >= 2 {
                    let start = &self.points[0];
                    let end = &self.points[1];

                    // 检查主线段
                    let distance = point_to_line_distance(x, y, start.x, start.y, end.x, end.y);
                    if distance <= (self.thickness + 5) as f64 {
                        return true;
                    }

                    // 检查箭头头部
                    let dx = end.x - start.x;
                    let dy = end.y - start.y;
                    let length = ((dx * dx + dy * dy) as f64).sqrt();

                    if length > 20.0 {
                        let arrow_length = 15.0_f64;
                        let arrow_angle = 0.5_f64;
                        let unit_x = dx as f64 / length;
                        let unit_y = dy as f64 / length;

                        let wing1_x = end.x
                            - (arrow_length
                                * (unit_x * arrow_angle.cos() + unit_y * arrow_angle.sin()))
                                as i32;
                        let wing1_y = end.y
                            - (arrow_length
                                * (unit_y * arrow_angle.cos() - unit_x * arrow_angle.sin()))
                                as i32;

                        let wing2_x = end.x
                            - (arrow_length
                                * (unit_x * arrow_angle.cos() - unit_y * arrow_angle.sin()))
                                as i32;
                        let wing2_y = end.y
                            - (arrow_length
                                * (unit_y * arrow_angle.cos() + unit_x * arrow_angle.sin()))
                                as i32;

                        // 检查两个箭头翼
                        let distance1 =
                            point_to_line_distance(x, y, end.x, end.y, wing1_x, wing1_y);
                        let distance2 =
                            point_to_line_distance(x, y, end.x, end.y, wing2_x, wing2_y);

                        if distance1 <= (self.thickness + 5) as f64
                            || distance2 <= (self.thickness + 5) as f64
                        {
                            return true;
                        }
                    }

                    false
                } else {
                    false
                }
            }
            DrawingTool::Text => {
                // 对于文本，使用点坐标和估算的文本尺寸
                if !self.points.is_empty() {
                    let text_width = if self.text.is_empty() {
                        50
                    } else {
                        self.text.len() as i32 * self.thickness * 3
                    };
                    let text_height = self.thickness * 6;
                    let pos = &self.points[0];

                    x >= pos.x && x <= pos.x + text_width && y >= pos.y && y <= pos.y + text_height
                } else {
                    false
                }
            }
            _ => false,
        }
    }
    // 调整元素大小
    fn resize(&mut self, new_rect: RECT) {
        match self.tool {
            DrawingTool::Rectangle | DrawingTool::Circle => {
                if self.points.len() >= 2 {
                    self.points[0] = POINT {
                        x: new_rect.left,
                        y: new_rect.top,
                    };
                    self.points[1] = POINT {
                        x: new_rect.right,
                        y: new_rect.bottom,
                    };
                }
            }
            DrawingTool::Arrow => {
                if self.points.len() >= 2 {
                    // 保存原始箭头的方向向量
                    let old_start = &self.points[0];
                    let old_end = &self.points[1];

                    // 计算新的尺寸比例
                    let old_width = self.rect.right - self.rect.left;
                    let old_height = self.rect.bottom - self.rect.top;
                    let new_width = new_rect.right - new_rect.left;
                    let new_height = new_rect.bottom - new_rect.top;

                    // 避免除零
                    if old_width == 0 || old_height == 0 {
                        // 如果原始尺寸为0，使用简单的映射
                        self.points[0] = POINT {
                            x: new_rect.left,
                            y: new_rect.top,
                        };
                        self.points[1] = POINT {
                            x: new_rect.right,
                            y: new_rect.bottom,
                        };
                    } else {
                        // 计算起点在原始矩形中的相对位置
                        let start_rel_x = (old_start.x - self.rect.left) as f64 / old_width as f64;
                        let start_rel_y = (old_start.y - self.rect.top) as f64 / old_height as f64;
                        let end_rel_x = (old_end.x - self.rect.left) as f64 / old_width as f64;
                        let end_rel_y = (old_end.y - self.rect.top) as f64 / old_height as f64;

                        // 按比例缩放到新矩形
                        self.points[0] = POINT {
                            x: new_rect.left + (start_rel_x * new_width as f64) as i32,
                            y: new_rect.top + (start_rel_y * new_height as f64) as i32,
                        };
                        self.points[1] = POINT {
                            x: new_rect.left + (end_rel_x * new_width as f64) as i32,
                            y: new_rect.top + (end_rel_y * new_height as f64) as i32,
                        };
                    }
                }
            }
            DrawingTool::Pen => {
                // 对于画笔，按比例缩放所有点
                let old_rect = self.rect;
                let scale_x = (new_rect.right - new_rect.left) as f64
                    / (old_rect.right - old_rect.left) as f64;
                let scale_y = (new_rect.bottom - new_rect.top) as f64
                    / (old_rect.bottom - old_rect.top) as f64;

                for point in &mut self.points {
                    let rel_x = (point.x - old_rect.left) as f64;
                    let rel_y = (point.y - old_rect.top) as f64;
                    point.x = new_rect.left + (rel_x * scale_x) as i32;
                    point.y = new_rect.top + (rel_y * scale_y) as i32;
                }
            }
            DrawingTool::Text => {
                if !self.points.is_empty() {
                    self.points[0] = POINT {
                        x: new_rect.left,
                        y: new_rect.top,
                    };
                }
            }
            _ => {}
        }
        self.rect = new_rect;
    }

    // 移动元素
    fn move_by(&mut self, dx: i32, dy: i32) {
        for point in &mut self.points {
            point.x += dx;
            point.y += dy;
        }
        self.rect.left += dx;
        self.rect.right += dx;
        self.rect.top += dy;
        self.rect.bottom += dy;
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
        let toolbar_width =
            BUTTON_WIDTH * BUTTON_COUNT + BUTTON_SPACING * (BUTTON_COUNT - 1) + TOOLBAR_PADDING * 2;

        let mut toolbar_x =
            selection_rect.left + (selection_rect.right - selection_rect.left - toolbar_width) / 2;
        let mut toolbar_y = selection_rect.bottom + TOOLBAR_MARGIN; // 这里的 10 就是距离选择框底部的距离

        if toolbar_y + TOOLBAR_HEIGHT > screen_height {
            toolbar_y = selection_rect.top - TOOLBAR_HEIGHT - TOOLBAR_MARGIN; // 这里的 10 是距离选择框顶部的距离
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

    fn set_clicked_button(&mut self, button: ToolbarButton) {
        self.clicked_button = button;
    }

    fn clear_clicked_button(&mut self) {
        self.clicked_button = ToolbarButton::None;
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

            let screen_dc = GetDC(HWND(std::ptr::null_mut()));

            // 创建截图DC
            let screenshot_dc = CreateCompatibleDC(screen_dc);
            let screenshot_bitmap = CreateCompatibleBitmap(screen_dc, screen_width, screen_height);
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

            // 创建持久的主缓冲区
            let buffer_dc = CreateCompatibleDC(screen_dc);
            let buffer_bitmap = CreateCompatibleBitmap(screen_dc, screen_width, screen_height);
            SelectObject(buffer_dc, buffer_bitmap);

            // 创建遮罩DC
            let mask_dc = CreateCompatibleDC(screen_dc);
            let mask_bitmap = CreateCompatibleBitmap(screen_dc, screen_width, screen_height);
            SelectObject(mask_dc, mask_bitmap);

            // 预填充遮罩
            let black_brush = CreateSolidBrush(RGB!(1, 1, 1));
            let full_rect = RECT {
                left: 0,
                top: 0,
                right: screen_width,
                bottom: screen_height,
            };
            FillRect(mask_dc, &full_rect, black_brush);
            DeleteObject(black_brush);

            ReleaseDC(HWND(std::ptr::null_mut()), screen_dc);

            // 创建绘图对象 - 使用常量
            let border_pen = CreatePen(PS_SOLID, 2, COLOR_SELECTION_BORDER);
            let handle_brush = CreateSolidBrush(COLOR_SELECTION_HANDLE_FILL); // 使用大框手柄颜色
            let mask_brush = CreateSolidBrush(COLOR_MASK);

            // 工具栏相关画刷 - 使用常量
            let toolbar_brush = CreateSolidBrush(COLOR_TOOLBAR_BG);
            let toolbar_border_pen = CreatePen(PS_SOLID, 1, COLOR_TOOLBAR_BORDER);
            let button_brush = CreateSolidBrush(COLOR_BUTTON_BG);
            let button_hover_brush = CreateSolidBrush(COLOR_BUTTON_HOVER);

            Ok(WindowState {
                screenshot_dc,
                screenshot_bitmap,
                screen_width,
                screen_height,
                selection_rect: RECT {
                    left: 0,
                    top: 0,
                    right: 0,
                    bottom: 0,
                },
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
                buffer_dc,
                buffer_bitmap,
                mask_dc,
                mask_bitmap,
                toolbar: Toolbar::new(),
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
        if self.history.is_empty() || self.history.last() != Some(&self.drawing_elements) {
            self.history.push(self.drawing_elements.clone());
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
                self.current_element = None;
                return true;
            }
        }
        false
    }

    fn get_element_at_position(&self, x: i32, y: i32) -> Option<usize> {
        // 首先检查点击位置是否在选择框内
        if x < self.selection_rect.left
            || x > self.selection_rect.right
            || y < self.selection_rect.top
            || y > self.selection_rect.bottom
        {
            return None;
        }

        // 检查是否在屏幕范围内
        if x < 0 || x >= self.screen_width || y < 0 || y >= self.screen_height {
            return None;
        }

        // 从后往前检查（最后绘制的在最上层）
        for (index, element) in self.drawing_elements.iter().enumerate().rev() {
            // 只检测可见的元素且点击在选择框内的部分
            if self.is_element_visible(element) && element.contains_point(x, y) {
                return Some(index);
            }
        }
        None
    }

    fn get_handle_at_position(&self, x: i32, y: i32) -> DragMode {
        // 检查工具栏区域
        if self.toolbar.visible
            && x >= self.toolbar.rect.left
            && x <= self.toolbar.rect.right
            && y >= self.toolbar.rect.top
            && y <= self.toolbar.rect.bottom
        {
            return DragMode::None;
        }

        // 检查选择框手柄
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

        // 使用圆形检测范围，更精确
        let detection_radius = HANDLE_DETECTION_RADIUS;
        for (hx, hy, mode) in handles.iter() {
            let dx = x - hx;
            let dy = y - hy;
            let distance_sq = dx * dx + dy * dy;
            let radius_sq = detection_radius * detection_radius;

            if distance_sq <= radius_sq {
                return *mode;
            }
        }

        // 检查选择框内部移动区域
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

    fn start_drag(&mut self, x: i32, y: i32) {
        // 如果已经有选择框，不允许在外面重新框选
        if self.has_selection {
            // 1. 首先检查是否点击了选中元素的手柄（最高优先级）
            if let Some(element_index) = self.selected_element {
                if element_index < self.drawing_elements.len() {
                    let element = &self.drawing_elements[element_index];

                    // 只有非画笔元素才检查手柄
                    if element.tool != DrawingTool::Pen {
                        // 额外检查：只有当元素在选择框内可见时才允许操作手柄
                        if self.is_element_visible(element) {
                            let handle_mode =
                                self.get_element_handle_at_position(x, y, &element.rect);

                            if handle_mode != DragMode::None {
                                self.drag_mode = handle_mode;
                                self.mouse_pressed = true;
                                self.drag_start_pos = POINT { x, y };
                                self.drag_start_rect = element.rect;
                                return;
                            }

                            // 检查是否点击了选中元素内部（移动）
                            // 但只允许在选择框内的部分被点击
                            if x >= self.selection_rect.left
                                && x <= self.selection_rect.right
                                && y >= self.selection_rect.top
                                && y <= self.selection_rect.bottom
                                && element.contains_point(x, y)
                            {
                                self.drag_mode = DragMode::MovingElement;
                                self.mouse_pressed = true;
                                self.drag_start_pos = POINT { x, y };
                                return;
                            }
                        }
                    }
                }
            }

            // 2. 检查是否点击了其他绘图元素（只在选择框内）
            if x >= self.selection_rect.left
                && x <= self.selection_rect.right
                && y >= self.selection_rect.top
                && y <= self.selection_rect.bottom
            {
                if let Some(element_index) = self.get_element_at_position(x, y) {
                    let element = &self.drawing_elements[element_index];

                    // 如果是画笔元素，不允许选择
                    if element.tool == DrawingTool::Pen {
                        return;
                    }

                    // 清除之前选择的元素
                    for element in &mut self.drawing_elements {
                        element.selected = false;
                    }

                    // 选择点击的元素（非画笔）
                    self.drawing_elements[element_index].selected = true;
                    self.selected_element = Some(element_index);

                    // 更新元素的边界矩形
                    self.drawing_elements[element_index].update_bounding_rect();

                    // 检查是否点击了新选中元素的调整手柄
                    let element_rect = self.drawing_elements[element_index].rect;
                    let handle_mode = self.get_element_handle_at_position(x, y, &element_rect);

                    if handle_mode != DragMode::None {
                        self.drag_mode = handle_mode;
                        self.mouse_pressed = true;
                        self.drag_start_pos = POINT { x, y };
                        self.drag_start_rect = element_rect;
                    } else {
                        // 开始移动元素
                        self.drag_mode = DragMode::MovingElement;
                        self.mouse_pressed = true;
                        self.drag_start_pos = POINT { x, y };
                    }
                    return;
                }
            }

            // 3. 如果选择了绘图工具，且在选择框内，开始绘图
            if self.current_tool != DrawingTool::None {
                if x >= self.selection_rect.left
                    && x <= self.selection_rect.right
                    && y >= self.selection_rect.top
                    && y <= self.selection_rect.bottom
                {
                    // 清除元素选择
                    for element in &mut self.drawing_elements {
                        element.selected = false;
                    }
                    self.selected_element = None;

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
                            new_element.points.push(POINT { x, y });
                        }
                        DrawingTool::Text => {
                            new_element.points.push(POINT { x, y });
                        }
                        _ => {}
                    }

                    self.current_element = Some(new_element);
                }
                return;
            }

            // 4. 如果没有选择绘图工具，只允许操作选择框手柄
            if self.current_tool == DrawingTool::None {
                // 清除元素选择
                for element in &mut self.drawing_elements {
                    element.selected = false;
                }
                self.selected_element = None;

                // 检查选择框手柄
                let handle_mode = self.get_handle_at_position(x, y);

                if matches!(
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
                    self.drag_mode = handle_mode;
                    self.mouse_pressed = true;
                    self.drag_start_pos = POINT { x, y };
                    self.drag_start_rect = self.selection_rect;
                }
                // 注意：这里移除了创建新选择框的逻辑
            }
        } else {
            // 只有在没有选择框时才允许创建新的选择框
            if self.current_tool == DrawingTool::None {
                // 清除元素选择
                for element in &mut self.drawing_elements {
                    element.selected = false;
                }
                self.selected_element = None;

                // 创建新选择框
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
            }
        }
    }
    // 修改光标检测逻辑，优先检测元素手柄
    fn get_cursor_for_position(&self, x: i32, y: i32) -> PCWSTR {
        // 检查是否在屏幕范围内
        if x < 0 || x >= self.screen_width || y < 0 || y >= self.screen_height {
            return IDC_ARROW;
        }

        // 如果已经有选择框，外面区域只显示默认光标
        if self.has_selection {
            // 检查是否在工具栏区域
            if self.toolbar.visible
                && x >= self.toolbar.rect.left
                && x <= self.toolbar.rect.right
                && y >= self.toolbar.rect.top
                && y <= self.toolbar.rect.bottom
            {
                // 在工具栏内，显示手形光标
                return IDC_HAND;
            }

            // 1. 优先检查选中元素的手柄（只检查完全在选择框内的）
            if let Some(element_index) = self.selected_element {
                if element_index < self.drawing_elements.len() {
                    let element = &self.drawing_elements[element_index];

                    if element.tool != DrawingTool::Pen && self.is_element_visible(element) {
                        // 只有在选择框内才检查手柄
                        if x >= self.selection_rect.left
                            && x <= self.selection_rect.right
                            && y >= self.selection_rect.top
                            && y <= self.selection_rect.bottom
                        {
                            let handle_mode =
                                self.get_element_handle_at_position(x, y, &element.rect);

                            if handle_mode != DragMode::None {
                                return match handle_mode {
                                    DragMode::ResizingTopLeft | DragMode::ResizingBottomRight => {
                                        IDC_SIZENWSE
                                    }
                                    DragMode::ResizingTopRight | DragMode::ResizingBottomLeft => {
                                        IDC_SIZENESW
                                    }
                                    DragMode::ResizingTopCenter
                                    | DragMode::ResizingBottomCenter => IDC_SIZENS,
                                    DragMode::ResizingMiddleLeft
                                    | DragMode::ResizingMiddleRight => IDC_SIZEWE,
                                    _ => IDC_ARROW,
                                };
                            }

                            // 检查是否在选中元素内部（移动光标）
                            if element.contains_point(x, y) {
                                return IDC_SIZEALL;
                            }
                        }
                    }
                }
            }

            // 2. 检查其他可见元素（只在选择框内）
            if x >= self.selection_rect.left
                && x <= self.selection_rect.right
                && y >= self.selection_rect.top
                && y <= self.selection_rect.bottom
            {
                if let Some(element_index) = self.get_element_at_position(x, y) {
                    let element = &self.drawing_elements[element_index];
                    if element.tool != DrawingTool::Pen {
                        return IDC_SIZEALL;
                    }
                }
            }

            // 3. 如果选择了绘图工具且在选择框内，显示相应的光标
            if self.current_tool != DrawingTool::None
                && x >= self.selection_rect.left
                && x <= self.selection_rect.right
                && y >= self.selection_rect.top
                && y <= self.selection_rect.bottom
            {
                return match self.current_tool {
                    DrawingTool::Pen => IDC_CROSS,
                    DrawingTool::Rectangle | DrawingTool::Circle | DrawingTool::Arrow => IDC_CROSS,
                    DrawingTool::Text => IDC_IBEAM,
                    _ => IDC_ARROW,
                };
            }

            // 4. 检查选择框手柄
            let handle_mode = self.get_handle_at_position(x, y);
            match handle_mode {
                DragMode::ResizingTopLeft | DragMode::ResizingBottomRight => IDC_SIZENWSE,
                DragMode::ResizingTopRight | DragMode::ResizingBottomLeft => IDC_SIZENESW,
                DragMode::ResizingTopCenter | DragMode::ResizingBottomCenter => IDC_SIZENS,
                DragMode::ResizingMiddleLeft | DragMode::ResizingMiddleRight => IDC_SIZEWE,
                DragMode::Moving => IDC_SIZEALL,
                _ => IDC_NO,
            }
        } else {
            // 没有选择框时，允许正常的光标显示
            IDC_ARROW
        }
    }
    fn get_element_handle_at_position(&self, x: i32, y: i32, rect: &RECT) -> DragMode {
        // 首先检查点击位置是否在选择框内
        if x < self.selection_rect.left
            || x > self.selection_rect.right
            || y < self.selection_rect.top
            || y > self.selection_rect.bottom
        {
            return DragMode::None;
        }

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

        let detection_radius = HANDLE_DETECTION_RADIUS;
        for (hx, hy, mode) in handles.iter() {
            // 严格检查：手柄的整个检测区域必须完全在选择框内
            let handle_left = hx - detection_radius;
            let handle_right = hx + detection_radius;
            let handle_top = hy - detection_radius;
            let handle_bottom = hy + detection_radius;

            // 检查手柄检测区域是否完全在选择框内
            if handle_left >= self.selection_rect.left
                && handle_right <= self.selection_rect.right
                && handle_top >= self.selection_rect.top
                && handle_bottom <= self.selection_rect.bottom
            {
                let dx = x - hx;
                let dy = y - hy;
                let distance_sq = dx * dx + dy * dy;
                let radius_sq = detection_radius * detection_radius;

                if distance_sq <= radius_sq {
                    return *mode;
                }
            }
        }

        DragMode::None
    }
    fn update_drag(&mut self, x: i32, y: i32) {
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
                    let selection_left = self.selection_rect.left;
                    let selection_right = self.selection_rect.right;
                    let selection_top = self.selection_rect.top;
                    let selection_bottom = self.selection_rect.bottom;

                    let clamped_x = x.max(selection_left).min(selection_right);
                    let clamped_y = y.max(selection_top).min(selection_bottom);

                    match element.tool {
                        DrawingTool::Pen => {
                            element.points.push(POINT {
                                x: clamped_x,
                                y: clamped_y,
                            });
                        }
                        DrawingTool::Rectangle | DrawingTool::Circle | DrawingTool::Arrow => {
                            if element.points.is_empty() {
                                element.points.push(self.drag_start_pos);
                            }
                            if element.points.len() == 1 {
                                element.points.push(POINT {
                                    x: clamped_x,
                                    y: clamped_y,
                                });
                            } else {
                                element.points[1] = POINT {
                                    x: clamped_x,
                                    y: clamped_y,
                                };
                            }

                            // 更新rect信息（用于边界检查）
                            let start = &element.points[0];
                            let end = &element.points[1];
                            element.rect = RECT {
                                left: start.x.min(end.x),
                                top: start.y.min(end.y),
                                right: start.x.max(end.x),
                                bottom: start.y.max(end.y),
                            };
                        }
                        _ => {}
                    }
                }
            }

            DragMode::Moving => {
                if self.current_tool == DrawingTool::None {
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

                    if self.toolbar.visible {
                        self.toolbar.update_position(
                            &self.selection_rect,
                            self.screen_width,
                            self.screen_height,
                        );
                    }
                }
            }

            DragMode::MovingElement => {
                if let Some(element_index) = self.selected_element {
                    if element_index < self.drawing_elements.len() {
                        let element = &self.drawing_elements[element_index];

                        // 不允许移动画笔元素
                        if element.tool == DrawingTool::Pen {
                            return;
                        }

                        // 限制移动：只有当鼠标在选择框内时才允许移动
                        if x >= self.selection_rect.left
                            && x <= self.selection_rect.right
                            && y >= self.selection_rect.top
                            && y <= self.selection_rect.bottom
                        {
                            let dx = x - self.drag_start_pos.x;
                            let dy = y - self.drag_start_pos.y;

                            self.drawing_elements[element_index].move_by(dx, dy);
                            self.drag_start_pos = POINT { x, y };
                        }
                    }
                }
            }

            // 元素调整大小：限制调整范围
            DragMode::ResizingTopLeft
            | DragMode::ResizingTopCenter
            | DragMode::ResizingTopRight
            | DragMode::ResizingMiddleRight
            | DragMode::ResizingBottomRight
            | DragMode::ResizingBottomCenter
            | DragMode::ResizingBottomLeft
            | DragMode::ResizingMiddleLeft => {
                // 判断是否是选择框的调整还是元素的调整
                if let Some(element_index) = self.selected_element {
                    if element_index < self.drawing_elements.len() {
                        let element = &self.drawing_elements[element_index];

                        // 不允许调整画笔元素大小
                        if element.tool == DrawingTool::Pen {
                            return;
                        }

                        // 限制调整：只有当鼠标在选择框内时才允许调整
                        if x >= self.selection_rect.left
                            && x <= self.selection_rect.right
                            && y >= self.selection_rect.top
                            && y <= self.selection_rect.bottom
                        {
                            let mut new_rect = self.drag_start_rect;
                            let dx = x - self.drag_start_pos.x;
                            let dy = y - self.drag_start_pos.y;

                            match self.drag_mode {
                                DragMode::ResizingTopLeft => {
                                    new_rect.left += dx;
                                    new_rect.top += dy;
                                }
                                DragMode::ResizingTopCenter => {
                                    new_rect.top += dy;
                                }
                                DragMode::ResizingTopRight => {
                                    new_rect.right += dx;
                                    new_rect.top += dy;
                                }
                                DragMode::ResizingMiddleRight => {
                                    new_rect.right += dx;
                                }
                                DragMode::ResizingBottomRight => {
                                    new_rect.right += dx;
                                    new_rect.bottom += dy;
                                }
                                DragMode::ResizingBottomCenter => {
                                    new_rect.bottom += dy;
                                }
                                DragMode::ResizingBottomLeft => {
                                    new_rect.left += dx;
                                    new_rect.bottom += dy;
                                }
                                DragMode::ResizingMiddleLeft => {
                                    new_rect.left += dx;
                                }
                                _ => {}
                            }

                            // 只确保最小尺寸
                            if new_rect.right - new_rect.left >= 10
                                && new_rect.bottom - new_rect.top >= 10
                            {
                                self.drawing_elements[element_index].resize(new_rect);
                            }
                        }
                    }
                } else {
                    // 调整选择框大小
                    let mut new_rect = self.drag_start_rect;
                    let dx = x - self.drag_start_pos.x;
                    let dy = y - self.drag_start_pos.y;

                    match self.drag_mode {
                        DragMode::ResizingTopLeft => {
                            new_rect.left += dx;
                            new_rect.top += dy;
                        }
                        DragMode::ResizingTopCenter => {
                            new_rect.top += dy;
                        }
                        DragMode::ResizingTopRight => {
                            new_rect.right += dx;
                            new_rect.top += dy;
                        }
                        DragMode::ResizingMiddleRight => {
                            new_rect.right += dx;
                        }
                        DragMode::ResizingBottomRight => {
                            new_rect.right += dx;
                            new_rect.bottom += dy;
                        }
                        DragMode::ResizingBottomCenter => {
                            new_rect.bottom += dy;
                        }
                        DragMode::ResizingBottomLeft => {
                            new_rect.left += dx;
                            new_rect.bottom += dy;
                        }
                        DragMode::ResizingMiddleLeft => {
                            new_rect.left += dx;
                        }
                        _ => {}
                    }

                    // 确保选择框在屏幕范围内且有最小尺寸
                    new_rect.left = new_rect.left.max(0);
                    new_rect.top = new_rect.top.max(0);
                    new_rect.right = new_rect.right.min(self.screen_width);
                    new_rect.bottom = new_rect.bottom.min(self.screen_height);

                    if new_rect.right - new_rect.left >= MIN_BOX_SIZE
                        && new_rect.bottom - new_rect.top >= MIN_BOX_SIZE
                    {
                        self.selection_rect = new_rect;

                        // 更新工具栏位置
                        if self.toolbar.visible {
                            self.toolbar.update_position(
                                &self.selection_rect,
                                self.screen_width,
                                self.screen_height,
                            );
                        }
                    }
                }
            }

            _ => {}
        }
    }
    fn is_element_visible(&self, element: &DrawingElement) -> bool {
        let element_rect = element.get_bounding_rect();

        // 检查元素是否与选择框有交集
        let intersects_selection = !(element_rect.right < self.selection_rect.left
            || element_rect.left > self.selection_rect.right
            || element_rect.bottom < self.selection_rect.top
            || element_rect.top > self.selection_rect.bottom);

        // 检查元素是否在屏幕范围内
        let within_screen = !(element_rect.right < 0
            || element_rect.left > self.screen_width
            || element_rect.bottom < 0
            || element_rect.top > self.screen_height);

        // 只要有交集且在屏幕内就认为可见（绘制时会被裁剪）
        intersects_selection && within_screen
    }
    fn draw_element_selection(&self, hdc: HDC) {
        if let Some(element_index) = self.selected_element {
            if element_index < self.drawing_elements.len() {
                let element = &self.drawing_elements[element_index];

                // 画笔元素不绘制选择框
                if element.selected && element.tool != DrawingTool::Pen {
                    // 检查元素是否与选择框有交集
                    let element_rect = element.get_bounding_rect();
                    if element_rect.right < self.selection_rect.left
                        || element_rect.left > self.selection_rect.right
                        || element_rect.bottom < self.selection_rect.top
                        || element_rect.top > self.selection_rect.bottom
                    {
                        return; // 完全不在选择框内，不绘制选择框
                    }

                    unsafe {
                        // 设置裁剪区域严格为选择框边界（不允许任何超出）
                        let clip_region = CreateRectRgn(
                            self.selection_rect.left,
                            self.selection_rect.top,
                            self.selection_rect.right,
                            self.selection_rect.bottom,
                        );

                        let old_region = CreateRectRgn(0, 0, 0, 0);
                        let region_result = GetClipRgn(hdc, old_region);
                        SelectClipRgn(hdc, clip_region);

                        // 计算裁剪后的元素边界框
                        let clipped_rect = RECT {
                            left: element.rect.left.max(self.selection_rect.left),
                            top: element.rect.top.max(self.selection_rect.top),
                            right: element.rect.right.min(self.selection_rect.right),
                            bottom: element.rect.bottom.min(self.selection_rect.bottom),
                        };

                        // 只有在裁剪后的矩形有效时才绘制
                        if clipped_rect.left < clipped_rect.right
                            && clipped_rect.top < clipped_rect.bottom
                        {
                            // 绘制深灰色虚线选择框（使用裁剪后的矩形）
                            let dash_pen = CreatePen(PS_DASH, 1, COLOR_SELECTION_DASHED);
                            let old_pen = SelectObject(hdc, dash_pen);
                            let old_brush = SelectObject(hdc, GetStockObject(NULL_BRUSH));

                            // 绘制完整的元素边界框，但会被裁剪区域限制
                            Rectangle(
                                hdc,
                                element.rect.left,
                                element.rect.top,
                                element.rect.right,
                                element.rect.bottom,
                            );

                            // 绘制圆形调整手柄（只绘制在选择框内的手柄）
                            let handle_fill_brush = CreateSolidBrush(COLOR_ELEMENT_HANDLE_FILL);
                            let handle_border_pen =
                                CreatePen(PS_SOLID, 1, COLOR_ELEMENT_HANDLE_BORDER);

                            SelectObject(hdc, handle_fill_brush);
                            SelectObject(hdc, handle_border_pen);

                            let center_x = (element.rect.left + element.rect.right) / 2;
                            let center_y = (element.rect.top + element.rect.bottom) / 2;
                            let half_handle = HANDLE_SIZE / 2;

                            let handles = [
                                (element.rect.left, element.rect.top),
                                (center_x, element.rect.top),
                                (element.rect.right, element.rect.top),
                                (element.rect.right, center_y),
                                (element.rect.right, element.rect.bottom),
                                (center_x, element.rect.bottom),
                                (element.rect.left, element.rect.bottom),
                                (element.rect.left, center_y),
                            ];

                            for (hx, hy) in handles.iter() {
                                // 检查手柄中心是否在选择框内（严格检查）
                                if *hx >= self.selection_rect.left
                                    && *hx <= self.selection_rect.right
                                    && *hy >= self.selection_rect.top
                                    && *hy <= self.selection_rect.bottom
                                {
                                    // 进一步检查手柄的边界是否完全在选择框内
                                    let handle_left = hx - half_handle;
                                    let handle_right = hx + half_handle;
                                    let handle_top = hy - half_handle;
                                    let handle_bottom = hy + half_handle;

                                    if handle_left >= self.selection_rect.left
                                        && handle_right <= self.selection_rect.right
                                        && handle_top >= self.selection_rect.top
                                        && handle_bottom <= self.selection_rect.bottom
                                    {
                                        Ellipse(
                                            hdc,
                                            handle_left,
                                            handle_top,
                                            handle_right,
                                            handle_bottom,
                                        );
                                    }
                                }
                            }

                            SelectObject(hdc, old_pen);
                            SelectObject(hdc, old_brush);
                            DeleteObject(dash_pen);
                            DeleteObject(handle_fill_brush);
                            DeleteObject(handle_border_pen);
                        }

                        // 恢复裁剪区域
                        if region_result == 1 {
                            SelectClipRgn(hdc, old_region);
                        } else {
                            SelectClipRgn(hdc, HRGN(std::ptr::null_mut()));
                        }

                        DeleteObject(clip_region);
                        DeleteObject(old_region);
                    }
                }
            }
        }
    }
    fn end_drag(&mut self) {
        if self.drag_mode == DragMode::DrawingShape {
            if let Some(mut element) = self.current_element.take() {
                // 根据不同工具类型判断是否保存
                let should_save = match element.tool {
                    DrawingTool::Pen => {
                        // 手绘工具：至少要有2个点
                        element.points.len() > 1
                    }
                    DrawingTool::Rectangle | DrawingTool::Circle | DrawingTool::Arrow => {
                        // 形状工具：检查尺寸
                        if element.points.len() >= 2 {
                            let dx = (element.points[1].x - element.points[0].x).abs();
                            let dy = (element.points[1].y - element.points[0].y).abs();
                            dx > 5 || dy > 5 // 至少有一个方向大于5像素
                        } else {
                            false
                        }
                    }
                    DrawingTool::Text => {
                        // 文本工具：有位置点就保存
                        !element.points.is_empty()
                    }
                    _ => false,
                };

                if should_save {
                    // 关键：保存前更新边界矩形
                    element.update_bounding_rect();
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
    fn handle_mouse_move(&mut self, hwnd: HWND, x: i32, y: i32) {
        if self.mouse_pressed {
            self.update_drag(x, y);
            self.update_layered_window(hwnd);
        } else {
            // 如果已经有选择框，只处理工具栏和选择框内的悬停
            if self.has_selection {
                // 检查工具栏按钮悬停
                let toolbar_button = self.toolbar.get_button_at_position(x, y);
                if toolbar_button != self.toolbar.hovered_button {
                    self.toolbar.set_hovered_button(toolbar_button);
                    if self.toolbar.visible {
                        self.update_layered_window(hwnd);
                    }
                }

                // 设置光标
                let cursor_id = if toolbar_button != ToolbarButton::None {
                    IDC_HAND
                } else {
                    self.get_cursor_for_position(x, y)
                };

                if let Ok(cursor) =
                    unsafe { LoadCursorW(HINSTANCE(std::ptr::null_mut()), cursor_id) }
                {
                    unsafe {
                        SetCursor(cursor);
                    }
                }
            } else {
                // 没有选择框时，正常处理鼠标移动
                if let Ok(cursor) =
                    unsafe { LoadCursorW(HINSTANCE(std::ptr::null_mut()), IDC_ARROW) }
                {
                    unsafe {
                        SetCursor(cursor);
                    }
                }
            }
        }
    }

    // 修改鼠标按下处理，限制点击区域
    fn handle_left_button_down(&mut self, hwnd: HWND, x: i32, y: i32) {
        // 如果已经有选择框，只允许在工具栏、选择框内或选择框手柄上点击
        if self.has_selection {
            // 检查工具栏点击
            let toolbar_button = self.toolbar.get_button_at_position(x, y);
            if toolbar_button != ToolbarButton::None {
                self.toolbar.set_clicked_button(toolbar_button);
                self.update_layered_window(hwnd);
                return;
            }

            // 检查是否在选择框内或选择框手柄上
            let in_selection_area = x >= self.selection_rect.left
                && x <= self.selection_rect.right
                && y >= self.selection_rect.top
                && y <= self.selection_rect.bottom;

            let handle_mode = self.get_handle_at_position(x, y);
            let on_selection_handle = handle_mode != DragMode::None;

            // 只有在选择框内或选择框手柄上才允许操作
            if in_selection_area || on_selection_handle {
                self.toolbar.clear_clicked_button();
                self.start_drag(x, y);
                if self.mouse_pressed {
                    unsafe {
                        SetCapture(hwnd);
                    }
                }
            }
            // 如果点击在外面，什么都不做（忽略点击）
        } else {
            // 没有选择框时，允许正常创建选择框
            self.start_drag(x, y);
            if self.mouse_pressed {
                unsafe {
                    SetCapture(hwnd);
                }
            }
        }
    }

    // 修改双击处理，只在选择框内允许
    fn handle_double_click(&self, x: i32, y: i32) -> bool {
        // 如果已经有选择框，只有在选择框内双击才保存并退出
        if self.has_selection {
            if x >= self.selection_rect.left
                && x <= self.selection_rect.right
                && y >= self.selection_rect.top
                && y <= self.selection_rect.bottom
            {
                let _ = self.save_selection();
                return true; // 退出程序
            }
            return false; // 不退出程序
        } else {
            // 没有选择框时，双击保存并退出
            let _ = self.save_selection();
            return true;
        }
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
                if self.can_undo() {
                    self.undo();
                }
                false
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
                true
            }
            ToolbarButton::Cancel => {
                // 修正：取消时清除绘图工具
                self.current_tool = DrawingTool::None;
                self.selected_element = None;
                self.current_element = None;
                true
            }
            ToolbarButton::None => false,
        }
    }

    fn save_to_file(&self) -> Result<()> {
        self.save_selection()
    }

    fn paint(&self, hwnd: HWND) {
        unsafe {
            let mut ps = PAINTSTRUCT::default();
            BeginPaint(hwnd, &mut ps);
            self.update_layered_window(hwnd);
            EndPaint(hwnd, &ps);
        }
    }

    fn draw_full_screen_overlay(&self, hdc: HDC) {
        unsafe {
            let blend_func = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 160,
                AlphaFormat: 0,
            };

            AlphaBlend(
                hdc,
                0,
                0,
                self.screen_width,
                self.screen_height,
                self.mask_dc,
                0,
                0,
                self.screen_width,
                self.screen_height,
                blend_func,
            );
        }
    }

    fn draw_dimmed_overlay(&self, hdc: HDC) {
        unsafe {
            // 使用预创建的遮罩DC绘制半透明遮罩到整个屏幕
            let blend_func = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 160,
                AlphaFormat: 0,
            };

            AlphaBlend(
                hdc,
                0,
                0,
                self.screen_width,
                self.screen_height,
                self.mask_dc,
                0,
                0,
                self.screen_width,
                self.screen_height,
                blend_func,
            );

            // 替换 BitBlt 为 StretchBlt（在相同尺寸下性能类似，但支持更多优化）
            StretchBlt(
                hdc,
                self.selection_rect.left,
                self.selection_rect.top,
                self.selection_rect.right - self.selection_rect.left,
                self.selection_rect.bottom - self.selection_rect.top,
                self.screenshot_dc,
                self.selection_rect.left,
                self.selection_rect.top,
                self.selection_rect.right - self.selection_rect.left,
                self.selection_rect.bottom - self.selection_rect.top,
                SRCCOPY,
            );
        }
    }

    fn update_layered_window(&self, hwnd: HWND) {
        unsafe {
            // 1. 绘制截图背景到缓冲区
            BitBlt(
                self.buffer_dc,
                0,
                0,
                self.screen_width,
                self.screen_height,
                self.screenshot_dc,
                0,
                0,
                SRCCOPY,
            );

            // 2. 绘制遮罩和选择框
            if !self.has_selection {
                self.draw_full_screen_overlay(self.buffer_dc);
            } else {
                self.draw_dimmed_overlay(self.buffer_dc);
                self.draw_selection_border(self.buffer_dc);

                // 3. 绘制所有已完成的绘图元素
                for element in &self.drawing_elements {
                    self.draw_element_with_points(self.buffer_dc, element);
                }

                // 4. 绘制当前正在绘制的元素
                if let Some(ref element) = self.current_element {
                    self.draw_element_with_points(self.buffer_dc, element);
                }

                // 5. 绘制元素选择框和手柄
                self.draw_element_selection(self.buffer_dc);

                self.draw_handles(self.buffer_dc);

                // 绘制工具栏
                if self.toolbar.visible {
                    self.draw_toolbar(self.buffer_dc);
                }
            }

            // 6. 一次性更新到分层窗口
            let window_pos = POINT { x: 0, y: 0 };
            let window_size = SIZE {
                cx: self.screen_width,
                cy: self.screen_height,
            };
            let source_pos = POINT { x: 0, y: 0 };

            let blend_func = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: 0,
            };

            let _ = UpdateLayeredWindow(
                hwnd,
                None,
                Some(&window_pos),
                Some(&window_size),
                self.buffer_dc,
                Some(&source_pos),
                COLORREF(0),
                Some(&blend_func),
                ULW_ALPHA,
            );
        }
    }
    fn draw_element_with_points(&self, hdc: HDC, element: &DrawingElement) {
        // 检查元素是否与选择框有任何交集
        let element_rect = element.get_bounding_rect();
        if element_rect.right < self.selection_rect.left
            || element_rect.left > self.selection_rect.right
            || element_rect.bottom < self.selection_rect.top
            || element_rect.top > self.selection_rect.bottom
        {
            return; // 完全不在选择框内，不绘制
        }

        unsafe {
            // 设置裁剪区域为选择框
            let clip_region = CreateRectRgn(
                self.selection_rect.left,
                self.selection_rect.top,
                self.selection_rect.right,
                self.selection_rect.bottom,
            );

            // 保存原来的裁剪区域
            let old_region = CreateRectRgn(0, 0, 0, 0);
            let region_result = GetClipRgn(hdc, old_region);

            // 设置新的裁剪区域
            SelectClipRgn(hdc, clip_region);

            let pen = CreatePen(PS_SOLID, element.thickness, element.color);
            let old_pen = SelectObject(hdc, pen);
            let old_brush = SelectObject(hdc, GetStockObject(NULL_BRUSH));

            match element.tool {
                DrawingTool::Rectangle => {
                    if element.points.len() >= 2 {
                        let start = &element.points[0];
                        let end = &element.points[1];
                        Rectangle(hdc, start.x, start.y, end.x, end.y);
                    }
                }
                DrawingTool::Circle => {
                    if element.points.len() >= 2 {
                        let start = &element.points[0];
                        let end = &element.points[1];
                        Ellipse(hdc, start.x, start.y, end.x, end.y);
                    }
                }
                DrawingTool::Arrow => {
                    if element.points.len() >= 2 {
                        let start = &element.points[0];
                        let end = &element.points[1];

                        // 画箭头线条
                        MoveToEx(hdc, start.x, start.y, Some(std::ptr::null_mut()));
                        LineTo(hdc, end.x, end.y);

                        // 计算箭头头部
                        let dx = end.x - start.x;
                        let dy = end.y - start.y;
                        let length = ((dx * dx + dy * dy) as f64).sqrt();

                        if length > 20.0 {
                            let arrow_length = 15.0_f64;
                            let arrow_angle = 0.5_f64;

                            let unit_x = dx as f64 / length;
                            let unit_y = dy as f64 / length;

                            let wing1_x = end.x
                                - (arrow_length
                                    * (unit_x * arrow_angle.cos() + unit_y * arrow_angle.sin()))
                                    as i32;
                            let wing1_y = end.y
                                - (arrow_length
                                    * (unit_y * arrow_angle.cos() - unit_x * arrow_angle.sin()))
                                    as i32;

                            let wing2_x = end.x
                                - (arrow_length
                                    * (unit_x * arrow_angle.cos() - unit_y * arrow_angle.sin()))
                                    as i32;
                            let wing2_y = end.y
                                - (arrow_length
                                    * (unit_y * arrow_angle.cos() + unit_x * arrow_angle.sin()))
                                    as i32;

                            MoveToEx(hdc, end.x, end.y, Some(std::ptr::null_mut()));
                            LineTo(hdc, wing1_x, wing1_y);
                            MoveToEx(hdc, end.x, end.y, Some(std::ptr::null_mut()));
                            LineTo(hdc, wing2_x, wing2_y);
                        }
                    }
                }
                DrawingTool::Pen => {
                    if element.points.len() > 1 {
                        MoveToEx(
                            hdc,
                            element.points[0].x,
                            element.points[0].y,
                            Some(std::ptr::null_mut()),
                        );
                        for point in element.points.iter().skip(1) {
                            LineTo(hdc, point.x, point.y);
                        }
                    }
                }
                DrawingTool::Text => {
                    if !element.text.is_empty() && !element.points.is_empty() {
                        SetTextColor(hdc, element.color);
                        SetBkMode(hdc, TRANSPARENT);

                        let font = CreateFontW(
                            element.thickness * 5,
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

                        TextOutW(hdc, element.points[0].x, element.points[0].y, &text_wide);

                        SelectObject(hdc, old_font);
                        DeleteObject(font);
                    }
                }
                _ => {}
            }

            SelectObject(hdc, old_pen);
            SelectObject(hdc, old_brush);
            DeleteObject(pen);

            // 恢复原来的裁剪区域
            if region_result == 1 {
                SelectClipRgn(hdc, old_region);
            } else {
                SelectClipRgn(hdc, HRGN(std::ptr::null_mut()));
            }

            DeleteObject(clip_region);
            DeleteObject(old_region);
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

    fn draw_handles(&self, hdc: HDC) {
        // 只有在没有选择绘图工具时才绘制手柄
        if self.current_tool != DrawingTool::None {
            return;
        }

        unsafe {
            // 创建白色填充和蓝色边框（使用常量）
            let handle_fill_brush = CreateSolidBrush(COLOR_SELECTION_HANDLE_FILL);
            let handle_border_pen = CreatePen(PS_SOLID, 1, COLOR_SELECTION_HANDLE_BORDER);

            let old_brush = SelectObject(hdc, handle_fill_brush);
            let old_pen = SelectObject(hdc, handle_border_pen);

            let center_x = (self.selection_rect.left + self.selection_rect.right) / 2;
            let center_y = (self.selection_rect.top + self.selection_rect.bottom) / 2;
            let half_handle = SELECTION_HANDLE_SIZE / 2;

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
                // 绘制方形手柄（使用Rectangle替代Ellipse）
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
            DeleteObject(handle_fill_brush);
            DeleteObject(handle_border_pen);
        }
    }

    fn draw_toolbar(&self, hdc: HDC) {
        unsafe {
            // 绘制工具栏背景
            let old_brush = SelectObject(hdc, self.toolbar_brush);
            let old_pen = SelectObject(hdc, self.toolbar_border_pen);

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
                let (button_brush, icon_color, needs_cleanup) = if *button_type
                    == self.toolbar.clicked_button
                {
                    let green_brush = CreateSolidBrush(RGB!(34, 197, 94));
                    (green_brush, COLOR_TEXT_WHITE, true)
                } else if *button_type == self.toolbar.hovered_button {
                    (self.button_hover_brush, COLOR_TEXT_NORMAL, false)
                } else {
                    let is_current_tool = match button_type {
                        ToolbarButton::Rectangle => self.current_tool == DrawingTool::Rectangle,
                        ToolbarButton::Circle => self.current_tool == DrawingTool::Circle,
                        ToolbarButton::Arrow => self.current_tool == DrawingTool::Arrow,
                        ToolbarButton::Pen => self.current_tool == DrawingTool::Pen,
                        ToolbarButton::Text => self.current_tool == DrawingTool::Text,
                        _ => false,
                    };

                    let is_undo_disabled = *button_type == ToolbarButton::Undo && !self.can_undo();

                    if is_current_tool {
                        let active_brush = CreateSolidBrush(COLOR_BUTTON_ACTIVE);
                        (active_brush, COLOR_TEXT_ACTIVE, true)
                    } else if is_undo_disabled {
                        (self.button_brush, COLOR_BUTTON_DISABLED, false)
                    } else {
                        (self.button_brush, COLOR_TEXT_NORMAL, false)
                    }
                };

                SelectObject(hdc, button_brush);
                SelectObject(hdc, GetStockObject(NULL_PEN));

                RoundRect(hdc, rect.left, rect.top, rect.right, rect.bottom, 6, 6);

                self.draw_text_icon(hdc, rect, icon_data, icon_color);

                if needs_cleanup {
                    DeleteObject(button_brush);
                }
            }

            SelectObject(hdc, old_brush);
            SelectObject(hdc, old_pen);
        }
    }

    fn draw_text_icon(&self, hdc: HDC, rect: &RECT, icon_data: &IconData, color: COLORREF) {
        unsafe {
            SetTextColor(hdc, color);
            SetBkMode(hdc, TRANSPARENT);

            let font = CreateFontW(
                20,
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

            let mut text_wide = to_wide_chars(&icon_data.text);
            let mut text_rect = *rect;

            DrawTextW(
                hdc,
                &mut text_wide,
                &mut text_rect as *mut RECT,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );

            SelectObject(hdc, old_font);
            DeleteObject(font);
        }
    }

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

            // 复制选中区域（包含绘图元素）
            BitBlt(
                mem_dc,
                0,
                0,
                width,
                height,
                self.buffer_dc, // 从缓冲区复制，包含所有绘图
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
            DeleteObject(self.buffer_bitmap);
            DeleteDC(self.buffer_dc);
            DeleteObject(self.mask_bitmap);
            DeleteDC(self.mask_dc);
            DeleteObject(self.border_pen);
            DeleteObject(self.handle_brush);
            DeleteObject(self.mask_brush);
            DeleteObject(self.toolbar_brush);
            DeleteObject(self.toolbar_border_pen);
            DeleteObject(self.button_brush);
            DeleteObject(self.button_hover_brush);
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

        WM_ERASEBKGND => LRESULT(1),

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

                state.handle_mouse_move(hwnd, x, y);
            }
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
            if !state_ptr.is_null() {
                let state = &mut *state_ptr;
                let x = (lparam.0 & 0xFFFF) as i16 as i32;
                let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;

                state.handle_left_button_down(hwnd, x, y);
            }
            LRESULT(0)
        }

        WM_LBUTTONUP => {
            let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
            if !state_ptr.is_null() {
                let state = &mut *state_ptr;
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

                state.update_layered_window(hwnd);
            }
            LRESULT(0)
        }

        WM_LBUTTONDBLCLK => {
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
                        if (GetKeyState(VK_CONTROL.0 as i32) & 0x8000u16 as i16) != 0 {
                            if state.undo() {
                                state.update_layered_window(hwnd);
                            }
                        }
                    }
                    _ => {}
                }
            }
            LRESULT(0)
        }

        WM_SETCURSOR => LRESULT(1),

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
            hbrBackground: HBRUSH(std::ptr::null_mut()),
            hCursor: LoadCursorW(HINSTANCE(std::ptr::null_mut()), IDC_ARROW)?,
            style: CS_DBLCLKS | CS_OWNDC | CS_HREDRAW,
            ..Default::default()
        };

        if RegisterClassW(&window_class) == 0 {
            return Err(Error::from_win32());
        }

        let screen_width = GetSystemMetrics(SM_CXSCREEN);
        let screen_height = GetSystemMetrics(SM_CYSCREEN);

        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED,
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
