//! UI 相关类型定义
//!
//! 包含工具栏按钮等 UI 组件类型。

/// 工具栏按钮类型
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
