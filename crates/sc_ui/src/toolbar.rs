use sc_rendering::{Color, DrawStyle, Rectangle, RenderItem, RenderList, z_order};

use crate::selection_overlay::RectI32;

/// Toolbar button identifiers (platform-neutral).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolbarButton {
    Save,
    Rectangle,
    Circle,
    Arrow,
    Pen,
    Text,
    Undo,
    ExtractText,
    Languages,
    Confirm,
    Cancel,
    None,
    Pin,
}

/// Deterministic ordering of buttons in the toolbar.
///
/// Keep this aligned with the legacy host toolbar ordering.
pub const TOOLBAR_BUTTONS: [ToolbarButton; 12] = [
    ToolbarButton::Rectangle,
    ToolbarButton::Circle,
    ToolbarButton::Arrow,
    ToolbarButton::Pen,
    ToolbarButton::Text,
    ToolbarButton::Undo,
    ToolbarButton::ExtractText,
    ToolbarButton::Languages,
    ToolbarButton::Save,
    ToolbarButton::Pin,
    ToolbarButton::Confirm,
    ToolbarButton::Cancel,
];

#[derive(Debug, Clone)]
pub struct ToolbarStyle {
    // Layout
    pub toolbar_height: f32,
    pub button_width: f32,
    pub button_height: f32,
    pub button_spacing: f32,
    pub toolbar_padding: f32,
    pub toolbar_margin: f32,

    // Background rendering
    pub toolbar_background_color: Color,
    pub toolbar_background_radius: f32,

    pub hover_background_color: Color,
    pub hover_background_radius: f32,
}

impl Default for ToolbarStyle {
    fn default() -> Self {
        // Match existing host defaults.
        Self {
            toolbar_height: 40.0,
            button_width: 30.0,
            button_height: 30.0,
            button_spacing: 4.0,
            toolbar_padding: 8.0,
            toolbar_margin: 3.0,

            toolbar_background_color: Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.95,
            },
            toolbar_background_radius: 10.0,

            hover_background_color: Color {
                r: 0.75,
                g: 0.75,
                b: 0.75,
                a: 1.0,
            },
            hover_background_radius: 6.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ToolbarButtonLayout {
    pub button: ToolbarButton,
    pub rect: Rectangle,
}

#[derive(Debug, Clone)]
pub struct ToolbarLayout {
    pub toolbar_rect: Rectangle,
    pub buttons: Vec<ToolbarButtonLayout>,
}

#[derive(Debug)]
pub struct ToolbarView {
    pub layout: ToolbarLayout,
    pub background: RenderList,
}

impl ToolbarLayout {
    pub fn hit_test(&self, x: i32, y: i32) -> ToolbarButton {
        let x = x as f32;
        let y = y as f32;
        for b in &self.buttons {
            if b.rect.contains(x, y) {
                return b.button;
            }
        }
        ToolbarButton::None
    }

    pub fn hovered_button_rect(&self, hovered: ToolbarButton) -> Option<Rectangle> {
        if hovered == ToolbarButton::None {
            return None;
        }
        self.buttons
            .iter()
            .find(|b| b.button == hovered)
            .map(|b| b.rect)
    }
}

/// Compute toolbar layout for a given selection.
///
/// Returns `None` when there is no selection rect.
pub fn layout_toolbar(
    screen_size: (i32, i32),
    selection_rect: Option<RectI32>,
    style: &ToolbarStyle,
) -> Option<ToolbarLayout> {
    let selection_rect = selection_rect?;

    let screen_width = screen_size.0 as f32;
    let screen_height = screen_size.1 as f32;

    let button_count = TOOLBAR_BUTTONS.len() as f32;
    let toolbar_width = style.button_width * button_count
        + style.button_spacing * (button_count - 1.0)
        + style.toolbar_padding * 2.0;

    let mut toolbar_x = selection_rect.left as f32
        + (selection_rect.right - selection_rect.left) as f32 / 2.0
        - toolbar_width / 2.0;
    let mut toolbar_y = selection_rect.bottom as f32 + style.toolbar_margin;

    // Prefer below selection; if it would go out of bounds, move above.
    if toolbar_y + style.toolbar_height > screen_height {
        toolbar_y = selection_rect.top as f32 - style.toolbar_height - style.toolbar_margin;
    }

    // Clamp within screen.
    toolbar_x = toolbar_x.max(0.0).min(screen_width - toolbar_width);
    toolbar_y = toolbar_y.max(0.0).min(screen_height - style.toolbar_height);

    let toolbar_rect = Rectangle {
        x: toolbar_x,
        y: toolbar_y,
        width: toolbar_width,
        height: style.toolbar_height,
    };

    // Buttons: vertically centered squares.
    let button_y = toolbar_y + (style.toolbar_height - style.button_height) / 2.0;
    let mut button_x = toolbar_x + style.toolbar_padding;

    let mut buttons = Vec::with_capacity(TOOLBAR_BUTTONS.len());
    for button in TOOLBAR_BUTTONS {
        let rect = Rectangle {
            x: button_x,
            y: button_y,
            width: style.button_width,
            height: style.button_height,
        };
        buttons.push(ToolbarButtonLayout { button, rect });
        button_x += style.button_width + style.button_spacing;
    }

    Some(ToolbarLayout {
        toolbar_rect,
        buttons,
    })
}

/// Build a render list for the toolbar background and hovered-button highlight.
///
/// Icon rendering is intentionally excluded.
pub fn build_toolbar_background_render_list(
    layout: &ToolbarLayout,
    hovered: ToolbarButton,
    style: &ToolbarStyle,
) -> RenderList {
    let mut list = RenderList::with_capacity(2);

    let bg_style = DrawStyle {
        stroke_color: style.toolbar_background_color,
        fill_color: Some(style.toolbar_background_color),
        stroke_width: 0.0,
    };

    list.submit(RenderItem::RoundedRectangle {
        rect: layout.toolbar_rect,
        radius: style.toolbar_background_radius,
        style: bg_style,
        z_order: z_order::TOOLBAR,
    });

    if let Some(hover_rect) = layout.hovered_button_rect(hovered) {
        let hover_style = DrawStyle {
            stroke_color: style.hover_background_color,
            fill_color: Some(style.hover_background_color),
            stroke_width: 0.0,
        };

        list.submit(RenderItem::RoundedRectangle {
            rect: hover_rect,
            radius: style.hover_background_radius,
            style: hover_style,
            z_order: z_order::TOOLBAR,
        });
    }

    list
}

/// Build a full toolbar view (layout + background render list) for a given selection.
///
/// Returns `None` when there is no selection rect.
pub fn build_toolbar_view(
    screen_size: (i32, i32),
    selection_rect: Option<RectI32>,
    hovered: ToolbarButton,
    style: &ToolbarStyle,
) -> Option<ToolbarView> {
    let layout = layout_toolbar(screen_size, selection_rect, style)?;
    let background = build_toolbar_background_render_list(&layout, hovered, style);
    Some(ToolbarView { layout, background })
}

#[cfg(test)]
mod tests {
    #[test]
    fn layout_places_buttons_and_clamps_to_screen() {
        let style = super::ToolbarStyle::default();

        let selection = super::RectI32 {
            left: 10,
            top: 10,
            right: 110,
            bottom: 110,
        };

        // Use a realistic screen size (toolbar width is ~420px with defaults).
        let screen = (1920, 1080);
        let layout = super::layout_toolbar(screen, Some(selection), &style).unwrap();
        assert_eq!(layout.buttons.len(), super::TOOLBAR_BUTTONS.len());

        // Should be fully on-screen.
        assert!(layout.toolbar_rect.x >= 0.0);
        assert!(layout.toolbar_rect.y >= 0.0);
        assert!(layout.toolbar_rect.right() <= screen.0 as f32);
        assert!(layout.toolbar_rect.bottom() <= screen.1 as f32);
    }

    #[test]
    fn hit_test_returns_button_or_none() {
        let style = super::ToolbarStyle::default();

        let selection = super::RectI32 {
            left: 0,
            top: 0,
            right: 200,
            bottom: 200,
        };

        let layout = super::layout_toolbar((400, 400), Some(selection), &style).unwrap();

        // Hit the first button.
        let first = &layout.buttons[0];
        let x = (first.rect.x + 1.0) as i32;
        let y = (first.rect.y + 1.0) as i32;
        assert_eq!(layout.hit_test(x, y), first.button);

        // Outside toolbar.
        assert_eq!(layout.hit_test(-10, -10), super::ToolbarButton::None);
    }

    #[test]
    fn build_view_returns_layout_and_background() {
        let style = super::ToolbarStyle::default();

        let selection = super::RectI32 {
            left: 10,
            top: 10,
            right: 110,
            bottom: 110,
        };

        let view = super::build_toolbar_view(
            (1920, 1080),
            Some(selection),
            super::ToolbarButton::None,
            &style,
        )
        .unwrap();

        assert_eq!(view.layout.buttons.len(), super::TOOLBAR_BUTTONS.len());
        assert!(!view.background.is_empty());
    }
}
