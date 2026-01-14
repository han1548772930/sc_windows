use sc_app::selection::RectI32;

use crate::theme::preview::{
    BUTTON_WIDTH_OCR, ICON_CLICK_PADDING, ICON_HOVER_PADDING, ICON_SIZE, TITLE_BAR_HEIGHT,
};

#[inline]
pub fn icon_contains_hover_point(rect: RectI32, x: i32, y: i32) -> bool {
    let pad = ICON_HOVER_PADDING;
    x >= rect.left - pad && x <= rect.right + pad && y >= rect.top - pad && y <= rect.bottom + pad
}

#[inline]
pub fn icon_contains_click_point(rect: RectI32, is_title_bar_button: bool, x: i32, y: i32) -> bool {
    if is_title_bar_button {
        let button_left = rect.left - (BUTTON_WIDTH_OCR - ICON_SIZE) / 2;
        let button_right = button_left + BUTTON_WIDTH_OCR;
        x >= button_left && x <= button_right && (0..TITLE_BAR_HEIGHT).contains(&y)
    } else {
        let pad = ICON_CLICK_PADDING;
        x >= rect.left - pad
            && x <= rect.right + pad
            && y >= rect.top - pad
            && y <= rect.bottom + pad
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hover_includes_padding_bounds() {
        let rect = RectI32 {
            left: 100,
            top: 20,
            right: 124,
            bottom: 44,
        };

        assert!(icon_contains_hover_point(
            rect,
            100 - ICON_HOVER_PADDING,
            20
        ));
        assert!(!icon_contains_hover_point(
            rect,
            100 - ICON_HOVER_PADDING - 1,
            20
        ));
    }

    #[test]
    fn click_left_icon_uses_click_padding() {
        let rect = RectI32 {
            left: 100,
            top: 20,
            right: 124,
            bottom: 44,
        };

        assert!(icon_contains_click_point(
            rect,
            false,
            100 - ICON_CLICK_PADDING,
            20
        ));
        assert!(!icon_contains_click_point(
            rect,
            false,
            100 - ICON_CLICK_PADDING - 1,
            20
        ));
    }

    #[test]
    fn click_title_bar_button_uses_button_width_and_title_bar_y_range() {
        let rect = RectI32 {
            left: 200,
            top: 20,
            right: 224,
            bottom: 44,
        };

        let button_left = rect.left - (BUTTON_WIDTH_OCR - ICON_SIZE) / 2;
        let button_right = button_left + BUTTON_WIDTH_OCR;

        assert!(icon_contains_click_point(rect, true, button_left, 0));
        assert!(icon_contains_click_point(
            rect,
            true,
            button_right,
            TITLE_BAR_HEIGHT - 1
        ));
        assert!(!icon_contains_click_point(rect, true, button_left - 1, 0));
        assert!(!icon_contains_click_point(rect, true, button_right + 1, 0));
        assert!(!icon_contains_click_point(
            rect,
            true,
            button_left,
            TITLE_BAR_HEIGHT
        ));
    }
}
