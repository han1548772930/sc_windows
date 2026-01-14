use sc_app::selection::RectI32;
use sc_platform::CursorIcon;

use super::ToolbarButton;
use sc_drawing_host::{DragMode, DrawingElement, DrawingManager, DrawingTool};

/// 光标上下文 - 封装所有光标判断所需的状态
#[derive(Debug)]
pub struct CursorContext {
    pub mouse_x: i32,
    pub mouse_y: i32,
    pub hovered_button: ToolbarButton,
    pub is_button_disabled: bool,
    pub is_text_editing: bool,
    pub editing_element_info: Option<(DrawingElement, usize)>,
    pub current_tool: DrawingTool,
    pub selection_rect: Option<RectI32>,
    pub selected_element_info: Option<(DrawingElement, usize)>,
    pub selection_handle_mode: DragMode,
}

/// 光标管理器
pub struct CursorManager;

impl CursorManager {
    /// 根据应用状态确定合适的光标
    pub fn determine_cursor(ctx: &CursorContext, drawing_manager: &DrawingManager) -> CursorIcon {
        let x = ctx.mouse_x;
        let y = ctx.mouse_y;

        // 1) 按钮悬停优先
        if ctx.hovered_button != ToolbarButton::None && !ctx.is_button_disabled {
            return CursorIcon::Hand;
        }

        // 1.5) 若绘图管理器处于拖拽状态（移动/调整元素），根据拖拽模式显示对应光标，避免与选择框手柄冲突
        if let Some(drag_mode) = drawing_manager.get_current_drag_mode() {
            if let Some(cursor) = Self::get_resize_cursor(drag_mode) {
                return cursor;
            }
            // 移动时兜底
            return CursorIcon::SizeAll;
        }

        // 2) 文本编辑状态
        if ctx.is_text_editing {
            if let Some((element, element_index)) = ctx.editing_element_info.as_ref() {
                let handle_mode = drawing_manager.get_element_handle_at_position(
                    x,
                    y,
                    &element.rect,
                    element.tool,
                    *element_index,
                );
                return Self::get_resize_cursor(handle_mode).unwrap_or(CursorIcon::IBeam);
            }
            return CursorIcon::IBeam;
        }

        // 3) 有选择框的情况
        if let Some(rect) = ctx.selection_rect {
            let inside_selection =
                x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom;

            // 3.1) 无论当前工具为何，优先检查“已选元素”的手柄命中（旧逻辑优先级）
            if let Some((element, element_index)) = ctx.selected_element_info.as_ref() {
                let handle_mode = drawing_manager.get_element_handle_at_position(
                    x,
                    y,
                    &element.rect,
                    element.tool,
                    *element_index,
                );
                if handle_mode != DragMode::None {
                    return Self::get_resize_cursor(handle_mode).unwrap_or(CursorIcon::Arrow);
                }

                // 选中元素内部命中（文本未编辑时箭头，非文本显示移动）
                if element.contains_point(x, y) {
                    if element.tool == DrawingTool::Text {
                        return CursorIcon::Arrow;
                    }
                    return CursorIcon::SizeAll;
                }
            }

            // 3.2) 当前选择了绘图工具：仅在选区内显示绘图光标，选区外显示禁止
            if matches!(
                ctx.current_tool,
                DrawingTool::Pen
                    | DrawingTool::Rectangle
                    | DrawingTool::Circle
                    | DrawingTool::Arrow
            ) {
                return if inside_selection {
                    CursorIcon::Crosshair
                } else {
                    CursorIcon::NotAllowed
                };
            }
            if matches!(ctx.current_tool, DrawingTool::Text) {
                return if inside_selection {
                    CursorIcon::IBeam
                } else {
                    CursorIcon::NotAllowed
                };
            }

            // 3.3) 未选择绘图工具：检查选择框手柄
            if let Some(cursor) = Self::get_resize_cursor(ctx.selection_handle_mode) {
                return cursor;
            }

            // 3.4) 没有任何命中：在有选区时显示禁止
            return CursorIcon::NotAllowed;
        }

        // 4) 默认箭头光标
        CursorIcon::Arrow
    }

    /// 根据拖拽模式获取对应的调整大小光标
    fn get_resize_cursor(drag_mode: DragMode) -> Option<CursorIcon> {
        match drag_mode {
            DragMode::ResizingTopLeft | DragMode::ResizingBottomRight => Some(CursorIcon::SizeNWSE),
            DragMode::ResizingTopRight | DragMode::ResizingBottomLeft => Some(CursorIcon::SizeNESW),
            DragMode::ResizingTopCenter | DragMode::ResizingBottomCenter => {
                Some(CursorIcon::SizeNS)
            }
            DragMode::ResizingMiddleLeft | DragMode::ResizingMiddleRight => {
                Some(CursorIcon::SizeWE)
            }
            DragMode::Moving => Some(CursorIcon::SizeAll),
            _ => None,
        }
    }
}
