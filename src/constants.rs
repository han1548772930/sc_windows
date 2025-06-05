use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;

pub const WINDOW_CLASS_NAME: &str = "ScreenshotWindow";
pub const MIN_BOX_SIZE: i32 = 50;
pub const TEXT_BOX_WIDTH: i32 = 100;
pub const TEXT_BOX_HEIGHT: i32 = 30;
// 颜色常量 - 使用D2D1_COLOR_F格式
pub const COLOR_SELECTION_BORDER: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.0,
    g: 0.47,
    b: 0.84,
    a: 1.0,
};
pub const COLOR_SELECTION_DASHED: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.31,
    g: 0.31,
    b: 0.31,
    a: 1.0,
};
pub const COLOR_HANDLE_FILL: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 1.0,
};
pub const COLOR_HANDLE_BORDER: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.0,
    g: 0.47,
    b: 0.84,
    a: 1.0,
};
pub const COLOR_MASK: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.6,
};
pub const COLOR_TOOLBAR_BG: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 0.95,
};
pub const COLOR_BUTTON_HOVER: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.85, // 更深的hover颜色
    g: 0.85,
    b: 0.85,
    a: 1.0,
};
pub const COLOR_BUTTON_ACTIVE: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.78,
    g: 0.9,
    b: 1.0,
    a: 1.0,
};
pub const COLOR_TEXT_NORMAL: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.25,
    g: 0.25,
    b: 0.25,
    a: 1.0,
};

// 工具栏尺寸常量
pub const TOOLBAR_HEIGHT: f32 = 40.0;
pub const BUTTON_WIDTH: f32 = 30.0;
pub const BUTTON_HEIGHT: f32 = 30.0;
pub const BUTTON_SPACING: f32 = 4.0;
pub const TOOLBAR_PADDING: f32 = 8.0;
pub const TOOLBAR_MARGIN: f32 = 3.0;
pub const BUTTON_COUNT: i32 = 11;

// 尺寸常量
pub const HANDLE_SIZE: f32 = 8.0;
pub const HANDLE_DETECTION_RADIUS: f32 = 10.0;

// 工具栏图标
pub const SAVE_ICON: &str = "💾";
pub const COPY_ICON: &str = "📋";
pub const RECT_ICON: &str = "⬜";
pub const CIRCLE_ICON: &str = "◯";
pub const ARROW_ICON: &str = "ↆ";
pub const PEN_ICON: &str = "🖊";
pub const TEXT_ICON: &str = "T₊";
pub const UNDO_ICON: &str = "↩";
pub const CONFIRM_ICON: &str = "✔";
pub const CANCEL_ICON: &str = "✖";
pub const PIN_ICON: &str = "📌";  