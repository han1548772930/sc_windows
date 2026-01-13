pub mod preview {
    use sc_rendering::Color;

    // Preview window layout
    pub const TITLE_BAR_HEIGHT: i32 = 64;

    pub const BUTTON_WIDTH_OCR: i32 = TITLE_BAR_HEIGHT; // Square button
    pub const BUTTON_HEIGHT_OCR: i32 = TITLE_BAR_HEIGHT;

    pub const ICON_SIZE: i32 = 24;
    pub const ICON_START_X: i32 = 20;

    pub const LEFT_ICON_SPACING: i32 = 12;
    pub const LEFT_ICON_SEPARATOR_WIDTH: i32 = 20;

    pub const ICON_HOVER_PADDING: i32 = 8;
    pub const ICON_CLICK_PADDING: i32 = 16;
    pub const ICON_HOVER_RADIUS: f32 = 6.0;

    pub const ICON_HOVER_BG_COLOR: Color = Color::rgba(0.88, 0.95, 1.0, 1.0);
    pub const TITLE_BAR_BUTTON_HOVER_BG_COLOR: Color = Color::rgba(0.88, 0.88, 0.88, 1.0);
    pub const CLOSE_BUTTON_HOVER_BG_COLOR: Color = Color::rgba(0.91, 0.07, 0.14, 1.0);

    pub const TITLE_BAR_BG_COLOR: Color = Color::rgba(0.93, 0.93, 0.93, 1.0);
    pub const CONTENT_BG_COLOR: Color = Color::rgb(1.0, 1.0, 1.0);

    pub const PIN_ACTIVE_COLOR: (u8, u8, u8) = (7, 193, 96);
}
