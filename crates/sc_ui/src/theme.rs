pub mod preview {
    use sc_rendering::Color;

    // Preview window layout.
    pub const TITLE_BAR_HEIGHT: i32 = 64;

    pub const BUTTON_WIDTH_OCR: i32 = TITLE_BAR_HEIGHT; // Square button.
    pub const BUTTON_HEIGHT_OCR: i32 = TITLE_BAR_HEIGHT;

    pub const ICON_SIZE: i32 = 24;
    pub const ICON_START_X: i32 = 20;

    pub const LEFT_ICON_SPACING: i32 = 16;
    pub const LEFT_ICON_SEPARATOR_WIDTH: i32 = 20;

    pub const ICON_HOVER_PADDING: i32 = 8;
    pub const ICON_CLICK_PADDING: i32 = 16;
    pub const ICON_HOVER_RADIUS: f32 = 6.0;

    // OCR mode layout.
    pub const OCR_TEXT_PANEL_WIDTH: i32 = 350;
    pub const OCR_PANEL_GAP: i32 = 20;

    pub const OCR_CONTENT_PADDING_X: i32 = 20;
    pub const OCR_CONTENT_PADDING_TOP: i32 = 20;
    pub const OCR_CONTENT_PADDING_BOTTOM: i32 = 20;

    // We intentionally keep a small offset from the title bar when rendering the image.
    pub const OCR_IMAGE_START_Y_OFFSET: i32 = 10;

    pub const OCR_TEXT_PADDING_LEFT: i32 = 20;
    pub const OCR_TEXT_PADDING_RIGHT: i32 = 20;
    pub const OCR_TEXT_PADDING_TOP: i32 = 15;
    pub const OCR_TEXT_PADDING_BOTTOM: i32 = 15;

    pub const OCR_TEXT_FONT_FAMILY: &str = "Microsoft YaHei";
    pub const OCR_TEXT_FONT_SIZE: f32 = 18.0;
    pub const OCR_TEXT_LINE_HEIGHT: i32 = 24;

    // Colors.
    pub const ICON_HOVER_BG_COLOR: Color = Color::rgba(0.88, 0.95, 1.0, 1.0);
    pub const TITLE_BAR_BUTTON_HOVER_BG_COLOR: Color = Color::rgba(0.88, 0.88, 0.88, 1.0);
    pub const CLOSE_BUTTON_HOVER_BG_COLOR: Color = Color::rgba(0.91, 0.07, 0.14, 1.0);

    pub const TITLE_BAR_BG_COLOR: Color = Color::rgba(0.93, 0.93, 0.93, 1.0);
    pub const CONTENT_BG_COLOR: Color = Color::rgb(1.0, 1.0, 1.0);

    pub const OCR_TEXT_COLOR: Color = Color::rgb(0.0, 0.0, 0.0);
    pub const OCR_TEXT_SELECTION_BG_COLOR: Color = Color::rgba(0.78, 0.97, 0.77, 1.0);

    // Visual separators between icon groups on the custom title bar.
    pub const TITLE_BAR_SEPARATOR_COLOR: Color = Color::rgba(0.75, 0.75, 0.75, 1.0);

    pub const PIN_ACTIVE_COLOR: (u8, u8, u8) = (7, 193, 96);
}

pub mod settings {
    // Default window size.
    pub const WINDOW_DEFAULT_WIDTH: i32 = 480;
    pub const WINDOW_DEFAULT_HEIGHT: i32 = 480;

    // Layout constants.
    pub const MARGIN: i32 = 15;
    pub const ROW_HEIGHT: i32 = 32;
    pub const ROW_SPACING: i32 = 8;
    pub const LABEL_WIDTH: i32 = 80;
    pub const LABEL_HEIGHT: i32 = 18;
    pub const LABEL_Y_OFFSET: i32 = 3;
    pub const CONTROL_HEIGHT: i32 = 28;

    pub const BUTTON_WIDTH: i32 = 90;
    pub const BUTTON_HEIGHT: i32 = 30;
    pub const BUTTON_SPACING: i32 = 15;

    // Tab layout.
    pub const TAB_PAGE_X: i32 = 5;
    pub const TAB_PAGE_Y: i32 = 25;
    pub const TAB_PAGE_WIDTH_ADJUST: i32 = 11;
    pub const TAB_PAGE_HEIGHT_ADJUST: i32 = 33;

    pub const TAB_CONTENT_MARGIN: i32 = 10;
    pub const LABEL_CONTROL_GAP: i32 = 10;
}
