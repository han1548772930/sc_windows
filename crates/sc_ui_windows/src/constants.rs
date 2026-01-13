use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;

// Preview window layout
pub const TITLE_BAR_HEIGHT: i32 = 64;

pub const BUTTON_WIDTH_OCR: i32 = TITLE_BAR_HEIGHT; // Square button
pub const BUTTON_HEIGHT_OCR: i32 = TITLE_BAR_HEIGHT;

pub const ICON_SIZE: i32 = 24;
pub const ICON_SPACING: i32 = 20;
pub const ICON_START_X: i32 = 20;
pub const ICON_HOVER_PADDING: i32 = 8;
pub const ICON_CLICK_PADDING: i32 = 16;
pub const ICON_HOVER_RADIUS: f32 = 6.0;

pub const ICON_HOVER_BG_COLOR_D2D: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.88,
    g: 0.95,
    b: 1.0,
    a: 1.0,
};

pub const TITLE_BAR_BUTTON_HOVER_BG_COLOR_D2D: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.88,
    g: 0.88,
    b: 0.88,
    a: 1.0,
};

pub const CLOSE_BUTTON_HOVER_BG_COLOR_D2D: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.91,
    g: 0.07,
    b: 0.14,
    a: 1.0,
};

pub const TITLE_BAR_BG_COLOR_D2D: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.93,
    g: 0.93,
    b: 0.93,
    a: 1.0,
};

pub const CONTENT_BG_COLOR_D2D: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 1.0,
};

pub const PIN_ACTIVE_COLOR: (u8, u8, u8) = (7, 193, 96);
