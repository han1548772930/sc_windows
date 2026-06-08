#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DrawingTool {
    #[default]
    None,
    Rectangle,
    Circle,
    Arrow,
    Pen,
    Text,
}

impl DrawingTool {
    pub fn is_shape(&self) -> bool {
        matches!(self, Self::Rectangle | Self::Circle | Self::Arrow)
    }

    pub fn is_freeform(&self) -> bool {
        matches!(self, Self::Pen)
    }

    pub fn is_text(&self) -> bool {
        matches!(self, Self::Text)
    }

    pub fn can_draw(&self) -> bool {
        !matches!(self, Self::None)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DragMode {
    #[default]
    None,
    Drawing,
    DrawingShape,
    Moving,
    MovingElement,
    ResizingTopLeft,
    ResizingTopCenter,
    ResizingTopRight,
    ResizingMiddleRight,
    ResizingBottomRight,
    ResizingBottomCenter,
    ResizingBottomLeft,
    ResizingMiddleLeft,
}

impl DragMode {
    pub fn is_resizing(&self) -> bool {
        matches!(
            self,
            Self::ResizingTopLeft
                | Self::ResizingTopCenter
                | Self::ResizingTopRight
                | Self::ResizingMiddleRight
                | Self::ResizingBottomRight
                | Self::ResizingBottomCenter
                | Self::ResizingBottomLeft
                | Self::ResizingMiddleLeft
        )
    }

    pub fn is_drawing(&self) -> bool {
        matches!(self, Self::Drawing | Self::DrawingShape)
    }

    pub fn is_moving(&self) -> bool {
        matches!(self, Self::Moving | Self::MovingElement)
    }

    pub fn is_active(&self) -> bool {
        !matches!(self, Self::None)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ElementInteractionMode {
    #[default]
    None,
    Drawing,
    MovingElement,
    ResizingElement(DragMode),
}

impl ElementInteractionMode {
    pub fn from_drag_mode(drag_mode: DragMode) -> Self {
        match drag_mode {
            DragMode::None => ElementInteractionMode::None,
            DragMode::DrawingShape => ElementInteractionMode::Drawing,
            DragMode::MovingElement => ElementInteractionMode::MovingElement,
            DragMode::ResizingTopLeft
            | DragMode::ResizingTopCenter
            | DragMode::ResizingTopRight
            | DragMode::ResizingMiddleRight
            | DragMode::ResizingBottomRight
            | DragMode::ResizingBottomCenter
            | DragMode::ResizingBottomLeft
            | DragMode::ResizingMiddleLeft => ElementInteractionMode::ResizingElement(drag_mode),
            _ => ElementInteractionMode::None,
        }
    }

    pub fn is_active(&self) -> bool {
        !matches!(self, Self::None)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ElementInfo {
    pub id: u64,
    pub tool: DrawingTool,
    pub selected: bool,
    pub thickness: f32,
    pub text: Option<String>,
    pub font_size: Option<f32>,
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_drawing_tool_properties() {
        assert!(super::DrawingTool::Rectangle.is_shape());
        assert!(super::DrawingTool::Circle.is_shape());
        assert!(super::DrawingTool::Arrow.is_shape());
        assert!(!super::DrawingTool::Pen.is_shape());
        assert!(!super::DrawingTool::Text.is_shape());

        assert!(super::DrawingTool::Pen.is_freeform());
        assert!(!super::DrawingTool::Rectangle.is_freeform());

        assert!(super::DrawingTool::Text.is_text());
        assert!(!super::DrawingTool::Pen.is_text());

        assert!(super::DrawingTool::Pen.can_draw());
        assert!(!super::DrawingTool::None.can_draw());
    }

    #[test]
    fn test_drag_mode_properties() {
        assert!(super::DragMode::ResizingTopLeft.is_resizing());
        assert!(super::DragMode::ResizingBottomRight.is_resizing());
        assert!(!super::DragMode::Moving.is_resizing());
        assert!(!super::DragMode::None.is_resizing());

        assert!(super::DragMode::Drawing.is_drawing());
        assert!(super::DragMode::DrawingShape.is_drawing());
        assert!(!super::DragMode::Moving.is_drawing());

        assert!(super::DragMode::Moving.is_moving());
        assert!(super::DragMode::MovingElement.is_moving());
        assert!(!super::DragMode::Drawing.is_moving());
    }

    #[test]
    fn test_element_interaction_mode_from_drag_mode() {
        assert_eq!(
            super::ElementInteractionMode::from_drag_mode(super::DragMode::None),
            super::ElementInteractionMode::None
        );
        assert_eq!(
            super::ElementInteractionMode::from_drag_mode(super::DragMode::DrawingShape),
            super::ElementInteractionMode::Drawing
        );
        assert_eq!(
            super::ElementInteractionMode::from_drag_mode(super::DragMode::MovingElement),
            super::ElementInteractionMode::MovingElement
        );
        assert_eq!(
            super::ElementInteractionMode::from_drag_mode(super::DragMode::ResizingTopLeft),
            super::ElementInteractionMode::ResizingElement(super::DragMode::ResizingTopLeft)
        );
    }
}
