// 光标管理器
//
// 负责根据应用状态设置合适的鼠标光标

use crate::types::{DragMode, DrawingTool, ToolbarButton};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::PCWSTR;

/// 光标管理器
pub struct CursorManager;

impl CursorManager {
    /// 根据应用状态确定合适的光标
    pub fn determine_cursor(
        x: i32,
        y: i32,
        hovered_button: ToolbarButton,
        is_button_disabled: bool,
        is_text_editing: bool,
        editing_element_info: Option<(crate::types::DrawingElement, usize)>,
        current_tool: DrawingTool,
        selection_rect: Option<windows::Win32::Foundation::RECT>,
        selected_element_info: Option<(crate::types::DrawingElement, usize)>,
        selection_handle_mode: DragMode,
        drawing_manager: &crate::drawing::DrawingManager,
    ) -> PCWSTR {
        // 1) 按钮悬停优先
        if hovered_button != ToolbarButton::None && !is_button_disabled {
            return IDC_HAND;
        }

        // 2) 文本编辑状态
        if is_text_editing {
            if let Some((element, element_index)) = editing_element_info {
                let handle_mode = drawing_manager.get_element_handle_at_position(
                    x,
                    y,
                    &element.rect,
                    element.tool,
                    element_index,
                );
                return Self::get_resize_cursor(handle_mode).unwrap_or(IDC_IBEAM);
            } else {
                return IDC_IBEAM;
            }
        }

        // 3) 有选择框的情况
        if selection_rect.is_some() {
            match current_tool {
                DrawingTool::Pen
                | DrawingTool::Rectangle
                | DrawingTool::Circle
                | DrawingTool::Arrow => return IDC_CROSS,
                DrawingTool::Text => return IDC_IBEAM,
                DrawingTool::None => {
                    // 检查已选元素手柄
                    if let Some((element, element_index)) = selected_element_info {
                        let handle_mode = drawing_manager.get_element_handle_at_position(
                            x,
                            y,
                            &element.rect,
                            element.tool,
                            element_index,
                        );
                        if handle_mode != DragMode::None {
                            return Self::get_resize_cursor(handle_mode).unwrap_or(IDC_ARROW);
                        } else if element.contains_point(x, y) {
                            return IDC_SIZEALL;
                        }
                    }

                    // 检查选择框手柄
                    return Self::get_resize_cursor(selection_handle_mode).unwrap_or(IDC_ARROW);
                }
            }
        }

        // 4) 默认箭头光标
        IDC_ARROW
    }

    /// 根据拖拽模式获取对应的调整大小光标
    fn get_resize_cursor(drag_mode: DragMode) -> Option<PCWSTR> {
        match drag_mode {
            DragMode::ResizingTopLeft | DragMode::ResizingBottomRight => Some(IDC_SIZENWSE),
            DragMode::ResizingTopRight | DragMode::ResizingBottomLeft => Some(IDC_SIZENESW),
            DragMode::ResizingTopCenter | DragMode::ResizingBottomCenter => Some(IDC_SIZENS),
            DragMode::ResizingMiddleLeft | DragMode::ResizingMiddleRight => Some(IDC_SIZEWE),
            DragMode::Moving => Some(IDC_SIZEALL),
            _ => None,
        }
    }

    /// 设置系统光标
    pub fn set_cursor(cursor_id: PCWSTR) {
        unsafe {
            if let Ok(cursor) = LoadCursorW(
                Some(windows::Win32::Foundation::HINSTANCE(std::ptr::null_mut())),
                cursor_id,
            ) {
                let _ = SetCursor(Some(cursor));
            }
        }
    }
}
