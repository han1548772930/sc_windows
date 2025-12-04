use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;

pub const WINDOW_CLASS_NAME: &str = "sc_windows_main";
pub const MIN_BOX_SIZE: i32 = 50;
pub const TEXT_BOX_WIDTH: i32 = 100;
pub const TEXT_BOX_HEIGHT: i32 = 30;
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
    r: 0.85,
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

pub const COLOR_TEXT_BORDER: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.5,
    g: 0.5,
    b: 0.5,
    a: 1.0,
};

pub const COLOR_TEXT_CURSOR: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.0,
    g: 1.0,
    b: 0.0,
    a: 1.0,
};

pub const TEXT_CURSOR_WIDTH: f32 = 3.0;
pub const TOOLBAR_HEIGHT: f32 = 40.0;
pub const BUTTON_WIDTH: f32 = 30.0;
pub const BUTTON_HEIGHT: f32 = 30.0;
pub const BUTTON_SPACING: f32 = 4.0;
pub const TOOLBAR_PADDING: f32 = 8.0;
pub const TOOLBAR_MARGIN: f32 = 3.0;
pub const BUTTON_COUNT: i32 = 12;

pub const HANDLE_SIZE: f32 = 8.0;
pub const HANDLE_DETECTION_RADIUS: f32 = 10.0;

pub const DEFAULT_TEXT_WIDTH: i32 = 120;
pub const DEFAULT_TEXT_HEIGHT: i32 = 32;
pub const MIN_TEXT_WIDTH: i32 = 40;
pub const MIN_TEXT_HEIGHT: i32 = 20;
pub const MAX_TEXT_WIDTH: i32 = 400;
pub const DRAG_THRESHOLD: i32 = 5;
pub const LINE_HEIGHT: i32 = 24;
pub const CHAR_WIDTH: f32 = 15.0;
pub const TEXT_PADDING: f32 = 8.0;
pub const TEXT_LINE_HEIGHT_SCALE: f32 = 1.35;

/// 从设置文件加载颜色，如果加载失败则使用默认值
pub fn get_colors_from_settings() -> (D2D1_COLOR_F, D2D1_COLOR_F, D2D1_COLOR_F, D2D1_COLOR_F) {
    let settings = crate::settings::Settings::load();

    let drawing_color = D2D1_COLOR_F {
        r: settings.drawing_color_red as f32 / 255.0,
        g: settings.drawing_color_green as f32 / 255.0,
        b: settings.drawing_color_blue as f32 / 255.0,
        a: 1.0,
    };

    let text_color = D2D1_COLOR_F {
        r: settings.text_color_red as f32 / 255.0,
        g: settings.text_color_green as f32 / 255.0,
        b: settings.text_color_blue as f32 / 255.0,
        a: 1.0,
    };

    let selection_border_color = D2D1_COLOR_F {
        r: (settings.drawing_color_red as f32 / 255.0 * 0.8 + 0.2).min(1.0),
        g: (settings.drawing_color_green as f32 / 255.0 * 0.8 + 0.2).min(1.0),
        b: (settings.drawing_color_blue as f32 / 255.0 * 0.8 + 0.2).min(1.0),
        a: 1.0,
    };

    let toolbar_bg_color = D2D1_COLOR_F {
        r: 0.2,
        g: 0.2,
        b: 0.2,
        a: 0.95,
    };

    (
        drawing_color,
        text_color,
        selection_border_color,
        toolbar_bg_color,
    )
}

pub const TITLE_BAR_HEIGHT: i32 = 40;
pub const BUTTON_WIDTH_OCR: i32 = 46;
pub const BUTTON_HEIGHT_OCR: i32 = TITLE_BAR_HEIGHT;

// 窗口消息常量
pub const WM_TRAY_MESSAGE: u32 = 0x0400 + 1; // WM_USER + 1
pub const WM_HIDE_WINDOW_CUSTOM: u32 = 0x0400 + 2; // WM_USER + 2
pub const WM_RELOAD_SETTINGS: u32 = 0x0400 + 3; // WM_USER + 3
pub const WM_OCR_STATUS_UPDATE: u32 = 0x0400 + 10; // WM_USER + 10
pub const WM_OCR_COMPLETED: u32 = 0x0400 + 11; // WM_USER + 11

// 热键和定时器常量
pub const HOTKEY_SCREENSHOT_ID: i32 = 1001;
pub const TIMER_CAPTURE_DELAY_ID: usize = 2001;
pub const TIMER_CAPTURE_DELAY_MS: u32 = 50;

// 箭头绘制常量
pub const ARROW_HEAD_MARGIN: i32 = 20;
pub const ARROW_MIN_LENGTH: f64 = 20.0;
pub const ARROW_HEAD_LENGTH: f64 = 15.0;
pub const ARROW_HEAD_ANGLE: f64 = 0.5;

// 元素检测和默认尺寸常量
pub const ELEMENT_CLICK_TOLERANCE: f32 = 5.0;
pub const DEFAULT_ELEMENT_WIDTH: i32 = 50;
pub const DEFAULT_ELEMENT_HEIGHT: i32 = 30;
pub const MIN_FONT_SIZE: f32 = 8.0;
pub const MAX_FONT_SIZE: f32 = 200.0;

pub const ICON_SIZE: i32 = 24;
pub const ICON_SPACING: i32 = 20;
pub const ICON_START_X: i32 = 12;
pub const ICON_HOVER_PADDING: i32 = 8;
pub const ICON_CLICK_PADDING: i32 = 16;
pub const ICON_HOVER_RADIUS: f32 = 6.0;

pub const ICON_HOVER_BG_COLOR_D2D: D2D1_COLOR_F = D2D1_COLOR_F { r: 0.88, g: 0.95, b: 1.0, a: 1.0 };
pub const TITLE_BAR_BUTTON_HOVER_BG_COLOR_D2D: D2D1_COLOR_F = D2D1_COLOR_F { r: 0.88, g: 0.88, b: 0.88, a: 1.0 };
pub const CLOSE_BUTTON_HOVER_BG_COLOR_D2D: D2D1_COLOR_F = D2D1_COLOR_F { r: 0.91, g: 0.07, b: 0.14, a: 1.0 };
pub const TITLE_BAR_BG_COLOR_D2D: D2D1_COLOR_F = D2D1_COLOR_F { r: 0.93, g: 0.93, b: 0.93, a: 1.0 };

// 绘图元素默认值
pub const DEFAULT_LINE_THICKNESS: f32 = 3.0;
pub const DEFAULT_FONT_SIZE: f32 = 20.0;
pub const DEFAULT_FONT_NAME: &str = "Microsoft YaHei";
pub const DEFAULT_FONT_WEIGHT: i32 = 400;
pub const DEFAULT_DRAWING_COLOR: D2D1_COLOR_F = D2D1_COLOR_F { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };

// 光标闪烁定时器
pub const CURSOR_BLINK_TIMER_ID: u32 = 1;
pub const CURSOR_BLINK_INTERVAL_MS: u32 = 500;
