/// 绘图工具类型
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DrawingTool {
    /// 无工具（选择模式）
    #[default]
    None,
    /// 矩形工具
    Rectangle,
    /// 圆形/椭圆工具
    Circle,
    /// 箭头工具
    Arrow,
    /// 自由画笔工具
    Pen,
    /// 文本工具
    Text,
}

impl DrawingTool {
    /// 是否是形状工具
    pub fn is_shape(&self) -> bool {
        matches!(self, Self::Rectangle | Self::Circle | Self::Arrow)
    }

    /// 是否是自由绘制工具
    pub fn is_freeform(&self) -> bool {
        matches!(self, Self::Pen)
    }

    /// 是否是文本工具
    pub fn is_text(&self) -> bool {
        matches!(self, Self::Text)
    }

    /// 是否可以绘制（非 None）
    pub fn can_draw(&self) -> bool {
        !matches!(self, Self::None)
    }
}

/// 拖拽模式枚举
///
/// 描述用户当前的拖拽操作类型。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DragMode {
    /// 无拖拽
    #[default]
    None,
    /// 绘制选择框
    Drawing,
    /// 绘制图形元素
    DrawingShape,
    /// 移动选择框
    Moving,
    /// 移动绘图元素
    MovingElement,
    /// 调整大小 - 左上角
    ResizingTopLeft,
    /// 调整大小 - 上边中心
    ResizingTopCenter,
    /// 调整大小 - 右上角
    ResizingTopRight,
    /// 调整大小 - 右边中心
    ResizingMiddleRight,
    /// 调整大小 - 右下角
    ResizingBottomRight,
    /// 调整大小 - 下边中心
    ResizingBottomCenter,
    /// 调整大小 - 左下角
    ResizingBottomLeft,
    /// 调整大小 - 左边中心
    ResizingMiddleLeft,
}

impl DragMode {
    /// 是否是调整大小操作
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

    /// 是否是绘制操作
    pub fn is_drawing(&self) -> bool {
        matches!(self, Self::Drawing | Self::DrawingShape)
    }

    /// 是否是移动操作
    pub fn is_moving(&self) -> bool {
        matches!(self, Self::Moving | Self::MovingElement)
    }

    /// 是否是活动操作（非 None）
    pub fn is_active(&self) -> bool {
        !matches!(self, Self::None)
    }
}

/// 元素交互模式（用于绘图模块内部）
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ElementInteractionMode {
    /// 无交互
    #[default]
    None,
    /// 正在绘制
    Drawing,
    /// 正在移动元素
    MovingElement,
    /// 正在调整元素大小
    ResizingElement(DragMode),
}

impl ElementInteractionMode {
    /// 从拖拽模式转换
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

    /// 是否有活动交互
    pub fn is_active(&self) -> bool {
        !matches!(self, Self::None)
    }
}

/// 绘画元素的基础信息（平台无关）
#[derive(Debug, Clone, PartialEq)]
pub struct ElementInfo {
    /// 元素唯一标识符
    pub id: u64,
    /// 工具类型
    pub tool: DrawingTool,
    /// 是否被选中
    pub selected: bool,
    /// 线条粗细
    pub thickness: f32,
    /// 文本内容（仅对文本元素有效）
    pub text: Option<String>,
    /// 字体大小（仅对文本元素有效）
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
