pub use sc_app::selection::RectI32;

use crate::theme::preview::{
    BUTTON_WIDTH_OCR, ICON_SIZE, ICON_START_X, LEFT_ICON_SEPARATOR_WIDTH, LEFT_ICON_SPACING,
    TITLE_BAR_HEIGHT,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewIconKind {
    Left,
    TitleBarButton,
}

#[derive(Debug, Clone)]
pub struct PreviewIconLayout {
    pub name: &'static str,
    pub rect: RectI32,
    pub kind: PreviewIconKind,
}

impl PreviewIconLayout {
    #[inline]
    pub fn is_title_bar_button(&self) -> bool {
        matches!(self.kind, PreviewIconKind::TitleBarButton)
    }
}

pub const ICON_PIN: &str = "pin";
pub const ICON_OCR: &str = "extracttext";
pub const ICON_SAVE: &str = "download";

pub const ICON_WINDOW_CLOSE: &str = "window-close";
pub const ICON_WINDOW_MAXIMIZE: &str = "window-maximize";
pub const ICON_WINDOW_MINIMIZE: &str = "window-minimize";
pub const ICON_WINDOW_RESTORE: &str = "window-restore";

pub const ICON_TOOL_SQUARE: &str = "square";
pub const ICON_TOOL_CIRCLE: &str = "circle";
pub const ICON_TOOL_ARROW: &str = "move-up-right";
pub const ICON_TOOL_PEN: &str = "pen";
pub const ICON_TOOL_TEXT: &str = "type";

pub const PREVIEW_DRAWING_TOOL_ICONS: [&str; 5] = [
    ICON_TOOL_SQUARE,
    ICON_TOOL_CIRCLE,
    ICON_TOOL_ARROW,
    ICON_TOOL_PEN,
    ICON_TOOL_TEXT,
];

pub fn create_left_icons() -> Vec<PreviewIconLayout> {
    let mut icons = Vec::new();

    let mut icon_x = ICON_START_X;
    let icon_y = (TITLE_BAR_HEIGHT - ICON_SIZE) / 2;

    // Pin icon.
    icons.push(PreviewIconLayout {
        name: ICON_PIN,
        rect: RectI32 {
            left: icon_x,
            top: icon_y,
            right: icon_x + ICON_SIZE,
            bottom: icon_y + ICON_SIZE,
        },
        kind: PreviewIconKind::Left,
    });
    icon_x += ICON_SIZE + LEFT_ICON_SEPARATOR_WIDTH;

    // Drawing tool icons.
    for (i, name) in PREVIEW_DRAWING_TOOL_ICONS.iter().enumerate() {
        icons.push(PreviewIconLayout {
            name,
            rect: RectI32 {
                left: icon_x,
                top: icon_y,
                right: icon_x + ICON_SIZE,
                bottom: icon_y + ICON_SIZE,
            },
            kind: PreviewIconKind::Left,
        });
        icon_x += ICON_SIZE;
        if i + 1 < PREVIEW_DRAWING_TOOL_ICONS.len() {
            icon_x += LEFT_ICON_SPACING;
        }
    }

    // OCR icon.
    icon_x += LEFT_ICON_SEPARATOR_WIDTH;
    icons.push(PreviewIconLayout {
        name: ICON_OCR,
        rect: RectI32 {
            left: icon_x,
            top: icon_y,
            right: icon_x + ICON_SIZE,
            bottom: icon_y + ICON_SIZE,
        },
        kind: PreviewIconKind::Left,
    });
    icon_x += ICON_SIZE + LEFT_ICON_SPACING;

    // Save icon.
    icons.push(PreviewIconLayout {
        name: ICON_SAVE,
        rect: RectI32 {
            left: icon_x,
            top: icon_y,
            right: icon_x + ICON_SIZE,
            bottom: icon_y + ICON_SIZE,
        },
        kind: PreviewIconKind::Left,
    });

    icons
}

pub fn create_title_bar_buttons(window_width: i32, is_maximized: bool) -> Vec<PreviewIconLayout> {
    // From right to left.
    let button_names: [&str; 3] = if is_maximized {
        [ICON_WINDOW_CLOSE, ICON_WINDOW_RESTORE, ICON_WINDOW_MINIMIZE]
    } else {
        [
            ICON_WINDOW_CLOSE,
            ICON_WINDOW_MAXIMIZE,
            ICON_WINDOW_MINIMIZE,
        ]
    };

    let icon_y = (TITLE_BAR_HEIGHT - ICON_SIZE) / 2;

    let mut buttons = Vec::with_capacity(button_names.len());
    for (i, name) in button_names.iter().enumerate() {
        let button_x = window_width - (i as i32 + 1) * BUTTON_WIDTH_OCR;
        let icon_x = button_x + (BUTTON_WIDTH_OCR - ICON_SIZE) / 2;

        buttons.push(PreviewIconLayout {
            name,
            rect: RectI32 {
                left: icon_x,
                top: icon_y,
                right: icon_x + ICON_SIZE,
                bottom: icon_y + ICON_SIZE,
            },
            kind: PreviewIconKind::TitleBarButton,
        });
    }

    buttons
}
