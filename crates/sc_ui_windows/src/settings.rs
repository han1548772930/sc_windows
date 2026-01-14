use std::sync::atomic::{AtomicBool, AtomicIsize};

use crate::constants::{
    BUTTON_HEIGHT, BUTTON_SPACING, BUTTON_WIDTH, CONTROL_HEIGHT, LABEL_CONTROL_GAP, LABEL_HEIGHT,
    LABEL_WIDTH, LABEL_Y_OFFSET, MARGIN, ROW_HEIGHT, ROW_SPACING, TAB_CONTENT_MARGIN,
    TAB_PAGE_HEIGHT_ADJUST, TAB_PAGE_WIDTH_ADJUST, TAB_PAGE_X, TAB_PAGE_Y, WINDOW_DEFAULT_HEIGHT,
    WINDOW_DEFAULT_WIDTH,
};

mod events;
mod hotkey_edit;
mod layout;
mod window;

/// Ensure only a single settings window exists.
static SETTINGS_WINDOW: AtomicIsize = AtomicIsize::new(0);
/// Tracks whether the user clicked "OK" and successfully saved.
static SETTINGS_SAVED: AtomicBool = AtomicBool::new(false);

// Control IDs.
const ID_LINE_THICKNESS: i32 = 1001;
const ID_FONT_CHOOSE_BUTTON: i32 = 1003;
const ID_DRAWING_COLOR_BUTTON: i32 = 1006;
const ID_HOTKEY_EDIT: i32 = 1008;
const ID_CONFIG_PATH_EDIT: i32 = 1011;
const ID_CONFIG_PATH_BROWSE: i32 = 1012;
const ID_OCR_LANGUAGE_COMBO: i32 = 1013;
const ID_OK: i32 = 1009;
const ID_CANCEL: i32 = 1010;

pub use window::SettingsWindow;
