use crate::types::{Color, DrawStyle, Point, Rectangle, TextStyle};

/// Platform-specific backend for executing render items.
pub trait RenderBackend {
    type Error;

    fn draw_rectangle(&mut self, rect: Rectangle, style: &DrawStyle) -> Result<(), Self::Error>;
    fn draw_rounded_rectangle(
        &mut self,
        rect: Rectangle,
        radius: f32,
        style: &DrawStyle,
    ) -> Result<(), Self::Error>;
    fn draw_circle(
        &mut self,
        center: Point,
        radius: f32,
        style: &DrawStyle,
    ) -> Result<(), Self::Error>;
    fn draw_line(&mut self, start: Point, end: Point, style: &DrawStyle)
    -> Result<(), Self::Error>;
    fn draw_text(
        &mut self,
        text: &str,
        position: Point,
        style: &TextStyle,
    ) -> Result<(), Self::Error>;
    fn draw_dashed_rectangle(
        &mut self,
        rect: Rectangle,
        style: &DrawStyle,
        dash_pattern: &[f32],
    ) -> Result<(), Self::Error>;

    fn draw_selection_mask(
        &mut self,
        screen_rect: Rectangle,
        selection_rect: Rectangle,
        mask_color: Color,
    ) -> Result<(), Self::Error>;

    fn draw_selection_border(
        &mut self,
        rect: Rectangle,
        color: Color,
        width: f32,
        dash_pattern: Option<&[f32]>,
    ) -> Result<(), Self::Error>;

    fn draw_selection_handles(
        &mut self,
        rect: Rectangle,
        handle_size: f32,
        fill_color: Color,
        border_color: Color,
        border_width: f32,
    ) -> Result<(), Self::Error>;

    fn draw_element_handles(
        &mut self,
        rect: Rectangle,
        handle_radius: f32,
        fill_color: Color,
        border_color: Color,
        border_width: f32,
    ) -> Result<(), Self::Error>;

    fn push_clip_rect(&mut self, rect: Rectangle) -> Result<(), Self::Error>;
    fn pop_clip_rect(&mut self) -> Result<(), Self::Error>;
}

/// Render primitive.
#[derive(Debug, Clone)]
pub enum RenderItem {
    /// Rectangle.
    Rectangle {
        rect: Rectangle,
        style: DrawStyle,
        z_order: i32,
    },

    /// Rounded rectangle.
    RoundedRectangle {
        rect: Rectangle,
        radius: f32,
        style: DrawStyle,
        z_order: i32,
    },

    /// Circle/ellipse.
    Circle {
        center: Point,
        radius: f32,
        style: DrawStyle,
        z_order: i32,
    },

    /// Line.
    Line {
        start: Point,
        end: Point,
        style: DrawStyle,
        z_order: i32,
    },

    /// Text.
    Text {
        text: String,
        position: Point,
        style: TextStyle,
        z_order: i32,
    },

    /// Dashed rectangle.
    DashedRectangle {
        rect: Rectangle,
        style: DrawStyle,
        dash_pattern: Vec<f32>,
        z_order: i32,
    },

    /// Selection mask.
    SelectionMask {
        screen_rect: Rectangle,
        selection_rect: Rectangle,
        mask_color: Color,
        z_order: i32,
    },

    /// Selection border.
    SelectionBorder {
        rect: Rectangle,
        color: Color,
        width: f32,
        dash_pattern: Option<Vec<f32>>,
        z_order: i32,
    },

    /// Selection handles.
    SelectionHandles {
        rect: Rectangle,
        handle_size: f32,
        fill_color: Color,
        border_color: Color,
        border_width: f32,
        z_order: i32,
    },

    /// Element handles.
    ElementHandles {
        rect: Rectangle,
        handle_radius: f32,
        fill_color: Color,
        border_color: Color,
        border_width: f32,
        z_order: i32,
    },

    /// Begin clip.
    PushClipRect { rect: Rectangle, z_order: i32 },

    /// End clip.
    PopClipRect { z_order: i32 },
}

impl RenderItem {
    /// Get z-order for sorting.
    pub fn z_order(&self) -> i32 {
        match self {
            RenderItem::Rectangle { z_order, .. } => *z_order,
            RenderItem::RoundedRectangle { z_order, .. } => *z_order,
            RenderItem::Circle { z_order, .. } => *z_order,
            RenderItem::Line { z_order, .. } => *z_order,
            RenderItem::Text { z_order, .. } => *z_order,
            RenderItem::DashedRectangle { z_order, .. } => *z_order,
            RenderItem::SelectionMask { z_order, .. } => *z_order,
            RenderItem::SelectionBorder { z_order, .. } => *z_order,
            RenderItem::SelectionHandles { z_order, .. } => *z_order,
            RenderItem::ElementHandles { z_order, .. } => *z_order,
            RenderItem::PushClipRect { z_order, .. } => *z_order,
            RenderItem::PopClipRect { z_order, .. } => *z_order,
        }
    }
}

/// Z-order layer constants.
pub mod z_order {
    /// Background (screenshot).
    pub const BACKGROUND: i32 = 0;
    /// Mask.
    pub const MASK: i32 = 100;
    /// Static drawing elements.
    pub const STATIC_ELEMENTS: i32 = 200;
    /// Selection border.
    pub const SELECTION_BORDER: i32 = 300;
    /// Current element.
    pub const CURRENT_ELEMENT: i32 = 400;
    /// Selection handles.
    pub const SELECTION_HANDLES: i32 = 500;
    /// Element handles.
    pub const ELEMENT_HANDLES: i32 = 600;
    /// Toolbar.
    pub const TOOLBAR: i32 = 700;
    /// Cursor.
    pub const CURSOR: i32 = 800;
}

/// Render list.
#[derive(Debug, Default)]
pub struct RenderList {
    items: Vec<RenderItem>,
}

impl RenderList {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
        }
    }

    pub fn submit(&mut self, item: RenderItem) {
        self.items.push(item);
    }

    pub fn submit_batch(&mut self, items: impl IntoIterator<Item = RenderItem>) {
        self.items.extend(items);
    }

    pub fn sort_by_z_order(&mut self) {
        self.items.sort_by_key(|item| item.z_order());
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, RenderItem> {
        self.items.iter()
    }

    /// Execute all items against a backend.
    pub fn execute<B: RenderBackend>(&mut self, backend: &mut B) -> Result<(), B::Error> {
        self.sort_by_z_order();

        for item in &self.items {
            Self::render_item(backend, item)?;
        }

        Ok(())
    }

    fn render_item<B: RenderBackend>(backend: &mut B, item: &RenderItem) -> Result<(), B::Error> {
        match item {
            RenderItem::Rectangle { rect, style, .. } => backend.draw_rectangle(*rect, style)?,
            RenderItem::RoundedRectangle {
                rect,
                radius,
                style,
                ..
            } => backend.draw_rounded_rectangle(*rect, *radius, style)?,
            RenderItem::Circle {
                center,
                radius,
                style,
                ..
            } => backend.draw_circle(*center, *radius, style)?,
            RenderItem::Line {
                start, end, style, ..
            } => backend.draw_line(*start, *end, style)?,
            RenderItem::Text {
                text,
                position,
                style,
                ..
            } => backend.draw_text(text, *position, style)?,
            RenderItem::DashedRectangle {
                rect,
                style,
                dash_pattern,
                ..
            } => backend.draw_dashed_rectangle(*rect, style, dash_pattern)?,
            RenderItem::SelectionMask {
                screen_rect,
                selection_rect,
                mask_color,
                ..
            } => backend.draw_selection_mask(*screen_rect, *selection_rect, *mask_color)?,
            RenderItem::SelectionBorder {
                rect,
                color,
                width,
                dash_pattern,
                ..
            } => backend.draw_selection_border(*rect, *color, *width, dash_pattern.as_deref())?,
            RenderItem::SelectionHandles {
                rect,
                handle_size,
                fill_color,
                border_color,
                border_width,
                ..
            } => backend.draw_selection_handles(
                *rect,
                *handle_size,
                *fill_color,
                *border_color,
                *border_width,
            )?,
            RenderItem::ElementHandles {
                rect,
                handle_radius,
                fill_color,
                border_color,
                border_width,
                ..
            } => backend.draw_element_handles(
                *rect,
                *handle_radius,
                *fill_color,
                *border_color,
                *border_width,
            )?,
            RenderItem::PushClipRect { rect, .. } => backend.push_clip_rect(*rect)?,
            RenderItem::PopClipRect { .. } => backend.pop_clip_rect()?,
        }

        Ok(())
    }
}

/// Convenience builder for `RenderList`.
pub struct RenderListBuilder {
    list: RenderList,
    default_z_order: i32,
}

impl RenderListBuilder {
    pub fn new() -> Self {
        Self {
            list: RenderList::new(),
            default_z_order: 0,
        }
    }

    pub fn with_z_order(mut self, z_order: i32) -> Self {
        self.default_z_order = z_order;
        self
    }

    pub fn rectangle(mut self, rect: Rectangle, style: DrawStyle) -> Self {
        self.list.submit(RenderItem::Rectangle {
            rect,
            style,
            z_order: self.default_z_order,
        });
        self
    }

    pub fn circle(mut self, center: Point, radius: f32, style: DrawStyle) -> Self {
        self.list.submit(RenderItem::Circle {
            center,
            radius,
            style,
            z_order: self.default_z_order,
        });
        self
    }

    pub fn line(mut self, start: Point, end: Point, style: DrawStyle) -> Self {
        self.list.submit(RenderItem::Line {
            start,
            end,
            style,
            z_order: self.default_z_order,
        });
        self
    }

    pub fn text(mut self, text: impl Into<String>, position: Point, style: TextStyle) -> Self {
        self.list.submit(RenderItem::Text {
            text: text.into(),
            position,
            style,
            z_order: self.default_z_order,
        });
        self
    }

    pub fn build(self) -> RenderList {
        self.list
    }
}

impl Default for RenderListBuilder {
    fn default() -> Self {
        Self::new()
    }
}
