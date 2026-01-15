use sc_ui::preview_layout;

use crate::ToolbarButton;

pub fn preview_icon_svg(name: &str) -> Option<&'static str> {
    match name {
        preview_layout::ICON_PIN => Some(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../apps/sc_windows/icons/pin.svg"
        ))),
        preview_layout::ICON_OCR => Some(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../apps/sc_windows/icons/extracttext.svg"
        ))),
        preview_layout::ICON_SAVE => Some(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../apps/sc_windows/icons/download.svg"
        ))),
        preview_layout::ICON_WINDOW_CLOSE => Some(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../apps/sc_windows/icons/window-close.svg"
        ))),
        preview_layout::ICON_WINDOW_MAXIMIZE => Some(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../apps/sc_windows/icons/window-maximize.svg"
        ))),
        preview_layout::ICON_WINDOW_MINIMIZE => Some(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../apps/sc_windows/icons/window-minimize.svg"
        ))),
        preview_layout::ICON_WINDOW_RESTORE => Some(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../apps/sc_windows/icons/window-restore.svg"
        ))),
        preview_layout::ICON_TOOL_SQUARE => Some(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../apps/sc_windows/icons/square.svg"
        ))),
        preview_layout::ICON_TOOL_CIRCLE => Some(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../apps/sc_windows/icons/circle.svg"
        ))),
        preview_layout::ICON_TOOL_ARROW => Some(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../apps/sc_windows/icons/move-up-right.svg"
        ))),
        preview_layout::ICON_TOOL_PEN => Some(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../apps/sc_windows/icons/pen.svg"
        ))),
        preview_layout::ICON_TOOL_TEXT => Some(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../apps/sc_windows/icons/type.svg"
        ))),
        _ => None,
    }
}

pub fn toolbar_icon_svg(button: ToolbarButton) -> Option<&'static str> {
    match button {
        ToolbarButton::Arrow => Some(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../apps/sc_windows/icons/move-up-right.svg"
        ))),
        ToolbarButton::Rectangle => Some(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../apps/sc_windows/icons/square.svg"
        ))),
        ToolbarButton::Circle => Some(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../apps/sc_windows/icons/circle.svg"
        ))),
        ToolbarButton::Pen => Some(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../apps/sc_windows/icons/pen.svg"
        ))),
        ToolbarButton::Text => Some(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../apps/sc_windows/icons/type.svg"
        ))),
        ToolbarButton::Undo => Some(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../apps/sc_windows/icons/undo-2.svg"
        ))),
        ToolbarButton::ExtractText => Some(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../apps/sc_windows/icons/extracttext.svg"
        ))),
        ToolbarButton::Languages => Some(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../apps/sc_windows/icons/languages.svg"
        ))),
        ToolbarButton::Save => Some(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../apps/sc_windows/icons/download.svg"
        ))),
        ToolbarButton::Pin => Some(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../apps/sc_windows/icons/pin.svg"
        ))),
        ToolbarButton::Confirm => Some(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../apps/sc_windows/icons/check.svg"
        ))),
        ToolbarButton::Cancel => Some(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../apps/sc_windows/icons/x.svg"
        ))),
        _ => None,
    }
}
