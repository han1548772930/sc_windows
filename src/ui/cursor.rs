// 光标管理器
//
// 负责根据应用状态设置合适的鼠标光标

use crate::types::{DragMode, DrawingTool, ToolbarButton};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::PCWSTR;

/// 光标上下文 - 封装所有光标判断所需的状态
#[derive(Debug)]
pub struct CursorContext {
    pub mouse_x: i32,
    pub mouse_y: i32,
    pub hovered_button: ToolbarButton,
    pub is_button_disabled: bool,
    pub is_text_editing: bool,
    pub editing_element_info: Option<(crate::types::DrawingElement, usize)>,
    pub current_tool: DrawingTool,
    pub selection_rect: Option<windows::Win32::Foundation::RECT>,
    pub selected_element_info: Option<(crate::types::DrawingElement, usize)>,
    pub selection_handle_mode: DragMode,
}

impl CursorContext {
    pub fn new(
        mouse_x: i32,
        mouse_y: i32,
        hovered_button: ToolbarButton,
        is_button_disabled: bool,
        is_text_editing: bool,
        editing_element_info: Option<(crate::types::DrawingElement, usize)>,
        current_tool: DrawingTool,
        selection_rect: Option<windows::Win32::Foundation::RECT>,
        selected_element_info: Option<(crate::types::DrawingElement, usize)>,
        selection_handle_mode: DragMode,
    ) -> Self {
        Self {
            mouse_x,
            mouse_y,
            hovered_button,
            is_button_disabled,
            is_text_editing,
            editing_element_info,
            current_tool,
            selection_rect,
            selected_element_info,
            selection_handle_mode,
        }
    }
}

/// 光标管理器
pub struct CursorManager;

impl CursorManager {
    /// 根据上下文确定合适的光标（简化版本）
    pub fn determine_cursor_from_context(
        context: &CursorContext,
        drawing_manager: &crate::drawing::DrawingManager,
    ) -> PCWSTR {
        Self::determine_cursor(
            context.mouse_x,
            context.mouse_y,
            context.hovered_button,
            context.is_button_disabled,
            context.is_text_editing,
            context.editing_element_info.clone(),
            context.current_tool,
            context.selection_rect,
            context.selected_element_info.clone(),
            context.selection_handle_mode,
            drawing_manager,
        )
    }

    /// 根据应用状态确定合适的光标（原版本，保持向后兼容）
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

        // 1.5) 若绘图管理器处于拖拽状态（移动/调整元素），根据拖拽模式显示对应光标，避免与选择框手柄冲突
        if let Some(drag_mode) = drawing_manager.get_current_drag_mode() {
            if let Some(cursor) = Self::get_resize_cursor(drag_mode) {
                return cursor;
            }
            // 移动时兜底
            return IDC_SIZEALL;
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
        if let Some(rect) = selection_rect {
            let inside_selection =
                x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom;

            // 3.1) 无论当前工具为何，优先检查“已选元素”的手柄命中（旧逻辑优先级）
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
                }
                // 选中元素内部命中（文本未编辑时箭头，编辑中或非文本显示移动）
                if element.contains_point(x, y) {
                    if element.tool == DrawingTool::Text {
                        if is_text_editing {
                            if let Some((_, edit_idx)) = editing_element_info {
                                if edit_idx == element_index {
                                    return IDC_SIZEALL;
                                }
                            }
                        }
                        return IDC_ARROW;
                    } else {
                        return IDC_SIZEALL;
                    }
                }
            }

            // 3.2) 当前选择了绘图工具：仅在选区内显示绘图光标，选区外显示禁止
            if matches!(
                current_tool,
                DrawingTool::Pen
                    | DrawingTool::Rectangle
                    | DrawingTool::Circle
                    | DrawingTool::Arrow
            ) {
                return if inside_selection { IDC_CROSS } else { IDC_NO };
            }
            if matches!(current_tool, DrawingTool::Text) {
                return if inside_selection { IDC_IBEAM } else { IDC_NO };
            }

            // 3.3) 未选择绘图工具：检查选择框手柄
            if let Some(cursor) = Self::get_resize_cursor(selection_handle_mode) {
                return cursor;
            }

            // 3.4) 没有任何命中：在有选区时显示禁止
            return IDC_NO;
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
