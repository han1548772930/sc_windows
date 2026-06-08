use sc_app::selection::RectI32;
use sc_platform::CursorIcon;

use super::ToolbarButton;
use sc_drawing_host::{DragMode, DrawingElement, DrawingManager, DrawingTool};

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

pub struct CursorManager;

impl CursorManager {
    pub fn determine_cursor(ctx: &CursorContext, drawing_manager: &DrawingManager) -> CursorIcon {
        let x = ctx.mouse_x;
        let y = ctx.mouse_y;

        if ctx.hovered_button != ToolbarButton::None && !ctx.is_button_disabled {
            return CursorIcon::Hand;
        }

        if let Some(drag_mode) = drawing_manager.get_current_drag_mode() {
            if let Some(cursor) = Self::get_resize_cursor(drag_mode) {
                return cursor;
            }
            return CursorIcon::SizeAll;
        }

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

        if let Some(rect) = ctx.selection_rect {
            let inside_selection =
                x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom;

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

                if element.contains_point(x, y) {
                    if element.tool == DrawingTool::Text {
                        return CursorIcon::Arrow;
                    }
                    return CursorIcon::SizeAll;
                }
            }

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

            if let Some(cursor) = Self::get_resize_cursor(ctx.selection_handle_mode) {
                return cursor;
            }

            return CursorIcon::NotAllowed;
        }

        CursorIcon::Arrow
    }

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
