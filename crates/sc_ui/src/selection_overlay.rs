use sc_rendering::{Color, Rectangle, RenderItem, RenderList, z_order};

/// Platform-neutral integer rectangle.
///
/// We intentionally reuse the core `RectI32` so UI and core share the same geometry type.
pub use sc_app::selection::RectI32;

#[inline]
fn to_rectangle_f32(rect: RectI32) -> Rectangle {
    Rectangle {
        x: rect.left as f32,
        y: rect.top as f32,
        width: (rect.right - rect.left) as f32,
        height: (rect.bottom - rect.top) as f32,
    }
}

#[derive(Debug, Clone)]
pub struct SelectionOverlayStyle {
    pub mask_color: Color,

    pub border_color: Color,
    pub border_width: f32,
    pub border_width_auto_highlight: f32,

    pub handle_size: f32,
    pub handle_fill_color: Color,
    pub handle_border_color: Color,
    pub handle_border_width: f32,
}

impl Default for SelectionOverlayStyle {
    fn default() -> Self {
        // Match existing host defaults.
        let accent = Color {
            r: 0.0,
            g: 0.47,
            b: 0.84,
            a: 1.0,
        };

        Self {
            mask_color: Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.6,
            },

            border_color: accent,
            border_width: 2.0,
            border_width_auto_highlight: 3.0,

            handle_size: 8.0,
            handle_fill_color: Color::WHITE,
            handle_border_color: accent,
            handle_border_width: 1.0,
        }
    }
}

pub fn build_selection_overlay_render_list(
    screen_size: (i32, i32),
    selection_rect: Option<RectI32>,
    show_handles: bool,
    hide_ui_for_capture: bool,
    has_auto_highlight: bool,
) -> Option<RenderList> {
    build_selection_overlay_render_list_with_style(
        screen_size,
        selection_rect,
        show_handles,
        hide_ui_for_capture,
        has_auto_highlight,
        &SelectionOverlayStyle::default(),
    )
}

pub fn build_selection_overlay_render_list_with_style(
    screen_size: (i32, i32),
    selection_rect: Option<RectI32>,
    show_handles: bool,
    hide_ui_for_capture: bool,
    has_auto_highlight: bool,
    style: &SelectionOverlayStyle,
) -> Option<RenderList> {
    if hide_ui_for_capture {
        return None;
    }

    let Some(selection_rect) = selection_rect else {
        return None;
    };

    let mut render_list = RenderList::with_capacity(4);

    let screen_rect = Rectangle {
        x: 0.0,
        y: 0.0,
        width: screen_size.0 as f32,
        height: screen_size.1 as f32,
    };

    let selection_rect_platform = to_rectangle_f32(selection_rect);

    // 1) Mask.
    render_list.submit(RenderItem::SelectionMask {
        screen_rect,
        selection_rect: selection_rect_platform,
        mask_color: style.mask_color,
        z_order: z_order::MASK,
    });

    // 2) Border.
    let border_width = if has_auto_highlight {
        style.border_width_auto_highlight
    } else {
        style.border_width
    };

    render_list.submit(RenderItem::SelectionBorder {
        rect: selection_rect_platform,
        color: style.border_color,
        width: border_width,
        dash_pattern: None,
        z_order: z_order::SELECTION_BORDER,
    });

    // 3) Handles (optional).
    if show_handles {
        render_list.submit(RenderItem::SelectionHandles {
            rect: selection_rect_platform,
            handle_size: style.handle_size,
            fill_color: style.handle_fill_color,
            border_color: style.handle_border_color,
            border_width: style.handle_border_width,
            z_order: z_order::SELECTION_HANDLES,
        });
    }

    Some(render_list)
}

#[cfg(test)]
mod tests {
    #[test]
    fn returns_none_when_hidden_or_no_selection() {
        assert!(
            super::build_selection_overlay_render_list((100, 100), None, true, false, false)
                .is_none()
        );

        let rect = super::RectI32 {
            left: 0,
            top: 0,
            right: 10,
            bottom: 10,
        };
        assert!(
            super::build_selection_overlay_render_list((100, 100), Some(rect), true, true, false)
                .is_none()
        );
    }

    #[test]
    fn builds_mask_border_and_optional_handles() {
        let rect = super::RectI32 {
            left: 10,
            top: 20,
            right: 30,
            bottom: 60,
        };

        let list = super::build_selection_overlay_render_list(
            (1920, 1080),
            Some(rect),
            true,
            false,
            false,
        )
        .unwrap();
        assert_eq!(list.len(), 3);

        let list = super::build_selection_overlay_render_list(
            (1920, 1080),
            Some(rect),
            false,
            false,
            false,
        )
        .unwrap();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn auto_highlight_uses_thicker_border() {
        let rect = super::RectI32 {
            left: 0,
            top: 0,
            right: 10,
            bottom: 10,
        };

        let style = super::SelectionOverlayStyle::default();

        let list = super::build_selection_overlay_render_list_with_style(
            (100, 100),
            Some(rect),
            false,
            false,
            true,
            &style,
        )
        .unwrap();

        let border = list
            .iter()
            .find_map(|item| match item {
                super::RenderItem::SelectionBorder { width, .. } => Some(*width),
                _ => None,
            })
            .expect("border must exist");

        assert_eq!(border, style.border_width_auto_highlight);
    }
}
