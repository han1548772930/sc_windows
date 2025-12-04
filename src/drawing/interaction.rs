//! 绘图交互模块
//!
//! 将 DrawingManager 的鼠标/键盘交互逻辑剥离到此文件，
//! 保持 mod.rs 专注于组装和核心状态管理。

use crate::message::{Command, DrawingMessage};
use super::history;
use super::types::{DrawingElement, DrawingTool, ElementInteractionMode};
use super::DrawingManager;

impl DrawingManager {
    /// 处理鼠标移动事件
    pub fn handle_mouse_move(
        &mut self,
        x: i32,
        y: i32,
        selection_rect: Option<windows::Win32::Foundation::RECT>,
    ) -> (Vec<Command>, bool) {
        if self.mouse_pressed {
            // 添加拖拽距离阈值检查
            // 只有当移动距离超过阈值时才开始真正的拖拽
            if crate::utils::is_drag_threshold_exceeded(
                self.interaction_start_pos.x,
                self.interaction_start_pos.y,
                x,
                y,
            ) {
                self.update_drag(x, y, selection_rect);
                (vec![Command::RequestRedraw], true)
            } else {
                // 移动距离不够，不进行拖拽，但仍然消费事件（因为鼠标已按下）
                (vec![], true)
            }
        } else {
            // 检查是否悬停在元素上（用于改变光标/预览）
            if let Some(_index) = self.elements.get_element_at_position(x, y) {
                // 可在后续添加悬停反馈，但不消费事件
                (vec![], false)
            } else {
                (vec![], false)
            }
        }
    }

    /// 更新拖拽状态
    pub(super) fn update_drag(
        &mut self,
        x: i32,
        y: i32,
        selection_rect: Option<windows::Win32::Foundation::RECT>,
    ) {
        match &self.interaction_mode {
            ElementInteractionMode::Drawing => {
                if let Some(ref mut element) = self.current_element {
                    // 如果有选择框，限制绘制在选择框内
                    let (clamped_x, clamped_y) = if let Some(rect) = selection_rect {
                        crate::utils::clamp_to_rect(x, y, &rect)
                    } else {
                        (x, y)
                    };

                    match element.tool {
                        DrawingTool::Pen => {
                            element.points.push(windows::Win32::Foundation::POINT {
                                x: clamped_x,
                                y: clamped_y,
                            });
                            // Invalidate geometry cache
                            element.path_geometry.replace(None);
                        }
                        DrawingTool::Rectangle | DrawingTool::Circle | DrawingTool::Arrow => {
                            if element.points.is_empty() {
                                element.points.push(self.interaction_start_pos);
                            }
                            if element.points.len() == 1 {
                                element.points.push(windows::Win32::Foundation::POINT {
                                    x: clamped_x,
                                    y: clamped_y,
                                });
                            } else {
                                element.points[1] = windows::Win32::Foundation::POINT {
                                    x: clamped_x,
                                    y: clamped_y,
                                };
                            }
                            // 更新rect信息（用于边界检查）
                            let start = &element.points[0];
                            let end = &element.points[1];
                            element.rect = windows::Win32::Foundation::RECT {
                                left: start.x.min(end.x),
                                top: start.y.min(end.y),
                                right: start.x.max(end.x),
                                bottom: start.y.max(end.y),
                            };
                        }
                        _ => {}
                    }
                }
            }
            ElementInteractionMode::MovingElement => {
                if let Some(index) = self.selected_element
                    && let Some(element) = self.elements.get_elements().get(index)
                        && element.tool != DrawingTool::Pen {
                            let dx = x - self.interaction_start_pos.x;
                            let dy = y - self.interaction_start_pos.y;
                            if let Some(el) = self.elements.get_element_mut(index) {
                                let current_dx = el.rect.left - self.interaction_start_rect.left;
                                let current_dy = el.rect.top - self.interaction_start_rect.top;
                                el.move_by(-current_dx, -current_dy);
                                el.move_by(dx, dy);
                            }
                        }
            }
            ElementInteractionMode::ResizingElement(resize_mode) => {
                self.handle_resize_drag(x, y, *resize_mode);
            }
            _ => {}
        }
    }

    /// 处理调整大小拖拽
    fn handle_resize_drag(&mut self, x: i32, y: i32, resize_mode: crate::types::DragMode) {
        if let Some(index) = self.selected_element
            && let Some(el) = self.elements.get_element_mut(index) {
                if el.tool == DrawingTool::Pen {
                    return;
                }
                let start_rect = self.interaction_start_rect;
                let start_font_size = self.interaction_start_font_size;
                let mut new_rect = start_rect;
                let dx = x - self.interaction_start_pos.x;
                let dy = y - self.interaction_start_pos.y;
                
                match resize_mode {
                    crate::types::DragMode::ResizingTopLeft => {
                        new_rect.left += dx;
                        new_rect.top += dy;
                    }
                    crate::types::DragMode::ResizingTopCenter => {
                        new_rect.top += dy;
                    }
                    crate::types::DragMode::ResizingTopRight => {
                        new_rect.right += dx;
                        new_rect.top += dy;
                    }
                    crate::types::DragMode::ResizingMiddleRight => {
                        new_rect.right += dx;
                    }
                    crate::types::DragMode::ResizingBottomRight => {
                        new_rect.right += dx;
                        new_rect.bottom += dy;
                    }
                    crate::types::DragMode::ResizingBottomCenter => {
                        new_rect.bottom += dy;
                    }
                    crate::types::DragMode::ResizingBottomLeft => {
                        new_rect.left += dx;
                        new_rect.bottom += dy;
                    }
                    crate::types::DragMode::ResizingMiddleLeft => {
                        new_rect.left += dx;
                    }
                    _ => {}
                }

                match el.tool {
                    DrawingTool::Arrow => {
                        // 仅支持通过左上角/右下角手柄调整起点/终点
                        if el.points.len() >= 2 {
                            match resize_mode {
                                crate::types::DragMode::ResizingTopLeft => {
                                    el.points[0] = windows::Win32::Foundation::POINT { x, y };
                                    el.update_bounding_rect();
                                }
                                crate::types::DragMode::ResizingBottomRight => {
                                    el.points[1] = windows::Win32::Foundation::POINT { x, y };
                                    el.update_bounding_rect();
                                }
                                _ => {}
                            }
                        }
                    }
                    DrawingTool::Text => {
                        Self::apply_text_resize(el, resize_mode, dx, dy, new_rect, start_rect, start_font_size);
                    }
                    _ => {
                        // 其他元素按矩形调整
                        el.resize(new_rect);
                    }
                }
            }
    }

    /// 应用文本元素调整大小（静态方法，避免借用冲突）
    fn apply_text_resize(
        el: &mut DrawingElement,
        resize_mode: crate::types::DragMode,
        dx: i32,
        dy: i32,
        new_rect: windows::Win32::Foundation::RECT,
        start_rect: windows::Win32::Foundation::RECT,
        start_font_size: f32,
    ) {
        // 文本元素：只允许通过四个角等比例缩放
        let is_corner_resize = matches!(
            resize_mode,
            crate::types::DragMode::ResizingTopLeft
                | crate::types::DragMode::ResizingTopRight
                | crate::types::DragMode::ResizingBottomLeft
                | crate::types::DragMode::ResizingBottomRight
        );

        if is_corner_resize {
            let original_width = (start_rect.right - start_rect.left).max(1);
            let original_height = (start_rect.bottom - start_rect.top).max(1);

            // 计算缩放比例
            let (scale_x, scale_y) = match resize_mode {
                crate::types::DragMode::ResizingTopLeft => (
                    (original_width - dx) as f32 / original_width as f32,
                    (original_height - dy) as f32 / original_height as f32,
                ),
                crate::types::DragMode::ResizingTopRight => (
                    (original_width + dx) as f32 / original_width as f32,
                    (original_height - dy) as f32 / original_height as f32,
                ),
                crate::types::DragMode::ResizingBottomRight => (
                    (original_width + dx) as f32 / original_width as f32,
                    (original_height + dy) as f32 / original_height as f32,
                ),
                crate::types::DragMode::ResizingBottomLeft => (
                    (original_width - dx) as f32 / original_width as f32,
                    (original_height + dy) as f32 / original_height as f32,
                ),
                _ => (1.0, 1.0),
            };

            // 等比例缩放：取两个方向的平均值
            let scale = ((scale_x + scale_y) / 2.0).max(0.7);

            // 计算新字体大小
            let new_font_size = (start_font_size * scale).max(8.0);
            el.set_font_size(new_font_size);

            // 计算等比例缩放后的新尺寸
            let new_width = (original_width as f32 * scale) as i32;
            let new_height = (original_height as f32 * scale) as i32;

            // 根据拖拽的角确定新矩形的位置
            let proportional_rect = match resize_mode {
                crate::types::DragMode::ResizingTopLeft => {
                    windows::Win32::Foundation::RECT {
                        left: start_rect.right - new_width,
                        top: start_rect.bottom - new_height,
                        right: start_rect.right,
                        bottom: start_rect.bottom,
                    }
                }
                crate::types::DragMode::ResizingTopRight => {
                    windows::Win32::Foundation::RECT {
                        left: start_rect.left,
                        top: start_rect.bottom - new_height,
                        right: start_rect.left + new_width,
                        bottom: start_rect.bottom,
                    }
                }
                crate::types::DragMode::ResizingBottomRight => {
                    windows::Win32::Foundation::RECT {
                        left: start_rect.left,
                        top: start_rect.top,
                        right: start_rect.left + new_width,
                        bottom: start_rect.top + new_height,
                    }
                }
                crate::types::DragMode::ResizingBottomLeft => {
                    windows::Win32::Foundation::RECT {
                        left: start_rect.right - new_width,
                        top: start_rect.top,
                        right: start_rect.right,
                        bottom: start_rect.top + new_height,
                    }
                }
                _ => new_rect,
            };

            el.resize(proportional_rect);
        }
        // 边缘中点拖拽不做任何处理
    }

    /// 检测指定元素矩形上的手柄命中
    pub fn get_element_handle_at_position(
        &self,
        x: i32,
        y: i32,
        rect: &windows::Win32::Foundation::RECT,
        tool: DrawingTool,
        element_index: usize,
    ) -> crate::types::DragMode {
        // 获取元素的点集合（用于箭头等特殊元素）
        let element_points = self
            .elements
            .get_elements()
            .get(element_index)
            .map(|element| element.points.as_slice());

        // 根据工具类型选择合适的配置
        let config = match tool {
            crate::types::DrawingTool::Arrow => {
                // 箭头元素的特殊处理
                let detection_radius = crate::constants::HANDLE_DETECTION_RADIUS as i32;
                if let Some(points) = element_points
                    && points.len() >= 2 {
                        let start = points[0];
                        let end = points[1];
                        let dx = x - start.x;
                        let dy = y - start.y;
                        if dx * dx + dy * dy <= detection_radius * detection_radius {
                            return crate::types::DragMode::ResizingTopLeft;
                        }
                        let dx2 = x - end.x;
                        let dy2 = y - end.y;
                        if dx2 * dx2 + dy2 * dy2 <= detection_radius * detection_radius {
                            return crate::types::DragMode::ResizingBottomRight;
                        }
                    }
                return crate::types::DragMode::None;
            }
            crate::types::DrawingTool::Text => crate::utils::HandleConfig::Corners,
            _ => crate::utils::HandleConfig::Full,
        };

        // 委托给统一的检测函数
        crate::utils::detect_handle_at_position_unified(x, y, rect, config, false)
    }

    /// 处理鼠标按下事件
    pub fn handle_mouse_down(
        &mut self,
        x: i32,
        y: i32,
        selection_rect: Option<windows::Win32::Foundation::RECT>,
    ) -> (Vec<Command>, bool) {
        // 重置标志
        self.just_saved_text = false;

        // 约束：除UI外，绘图交互仅在选择框内生效
        let inside_selection = match selection_rect {
            Some(r) => x >= r.left && x <= r.right && y >= r.top && y <= r.bottom,
            None => true,
        };

        // 文本编辑状态下的特殊处理
        if self.text_editing
            && let Some(editing_index) = self.editing_element_index {
                if let Some(element) = self.elements.get_elements().get(editing_index)
                    && element.contains_point(x, y) {
                        // 点击正在编辑的文本元素，检查是否点击了手柄
                        let handle_mode = self.get_element_handle_at_position(
                            x, y, &element.rect, element.tool, editing_index,
                        );
                        if handle_mode != crate::types::DragMode::None {
                            self.interaction_mode = ElementInteractionMode::from_drag_mode(handle_mode);
                            self.mouse_pressed = true;
                            self.interaction_start_pos = windows::Win32::Foundation::POINT { x, y };
                            self.interaction_start_rect = element.rect;
                            self.interaction_start_font_size = element.font_size;
                            self.interaction_start_points = element.points.clone();
                            return (vec![Command::RequestRedraw], true);
                        }
                        return (vec![Command::RequestRedraw], true);
                    }
                let stop_commands = self.stop_text_editing();
                return (stop_commands, true);
            }

        // 文本工具特殊处理
        if inside_selection
            && self.current_tool == DrawingTool::Text
            && !self.text_editing
            && !self.just_saved_text
        {
            if let Some(idx) = self.elements.get_element_at_position(x, y) {
                if let Some(element) = self.elements.get_elements().get(idx)
                    && element.tool == DrawingTool::Text {
                        let element_rect = element.rect;
                        let element_font_size = element.font_size;
                        let element_points = element.points.clone();

                        self.handle_message(DrawingMessage::SelectElement(Some(idx)));

                        self.interaction_mode = ElementInteractionMode::MovingElement;
                        self.mouse_pressed = true;
                        self.interaction_start_pos = windows::Win32::Foundation::POINT { x, y };
                        self.interaction_start_rect = element_rect;
                        self.interaction_start_font_size = element_font_size;
                        self.interaction_start_points = element_points;

                        return (vec![Command::UpdateToolbar, Command::RequestRedraw], true);
                    }
            } else {
                return self.create_and_edit_text_element(x, y);
            }
        }

        // 选择框外不消费
        if !inside_selection && selection_rect.is_some() {
            return (vec![], false);
        }

        // 先尝试与现有元素交互
        // 1) 已选元素的手柄优先
        if inside_selection
            && let Some(sel_idx) = self.selected_element
                && let Some(element) = self.elements.get_elements().get(sel_idx)
                    && element.tool != DrawingTool::Pen {
                        let handle_mode = self.get_element_handle_at_position(
                            x, y, &element.rect, element.tool, sel_idx,
                        );
                        if handle_mode != crate::types::DragMode::None {
                            self.interaction_mode = ElementInteractionMode::from_drag_mode(handle_mode);
                            self.mouse_pressed = true;
                            self.interaction_start_pos = windows::Win32::Foundation::POINT { x, y };
                            self.interaction_start_rect = element.rect;
                            self.interaction_start_font_size = element.font_size;
                            self.interaction_start_points = element.points.clone();
                            return (vec![Command::RequestRedraw], true);
                        }
                        // 2) 已选元素内部（移动）
                        if element.contains_point(x, y) {
                            let element_visible = if let Some(sel_rect) = selection_rect {
                                self.elements.is_element_visible_in_selection(element, &sel_rect)
                            } else {
                                true
                            };

                            if element_visible {
                                self.interaction_mode = ElementInteractionMode::MovingElement;
                                self.mouse_pressed = true;
                                self.interaction_start_pos = windows::Win32::Foundation::POINT { x, y };
                                self.interaction_start_rect = element.rect;
                                self.interaction_start_points = element.points.clone();
                                return (vec![Command::RequestRedraw], true);
                            }
                        }
                    }

        // 3) 检查是否点击其他元素
        if inside_selection
            && let Some(idx) = self.elements.get_element_at_position_with_selection(x, y, selection_rect)
        {
            let (element_tool, element_rect, element_font_size, element_points) = {
                if let Some(element) = self.elements.get_elements().get(idx) {
                    if element.tool == DrawingTool::Pen {
                        return (vec![], false);
                    }
                    (element.tool, element.rect, element.font_size, element.points.clone())
                } else {
                    return (vec![], false);
                }
            };

            if element_tool == DrawingTool::Pen {
                return (vec![], false);
            }

            self.handle_message(DrawingMessage::SelectElement(Some(idx)));

            self.interaction_start_rect = element_rect;
            self.interaction_start_pos = windows::Win32::Foundation::POINT { x, y };
            self.interaction_start_font_size = element_font_size;
            self.interaction_start_points = element_points;

            let handle_mode = self.get_element_handle_at_position(x, y, &element_rect, element_tool, idx);

            if handle_mode != crate::types::DragMode::None {
                self.interaction_mode = ElementInteractionMode::from_drag_mode(handle_mode);
                self.mouse_pressed = true;
                return (vec![Command::UpdateToolbar, Command::RequestRedraw], true);
            } else {
                self.interaction_mode = ElementInteractionMode::MovingElement;
                self.mouse_pressed = true;
                return (vec![Command::UpdateToolbar, Command::RequestRedraw], true);
            }
        }

        // 4) 若没有元素命中，且选择了绘图工具，则尝试开始绘制
        if self.current_tool != DrawingTool::None {
            if inside_selection {
                self.interaction_start_pos = windows::Win32::Foundation::POINT { x, y };
                self.start_drawing_shape(x, y);
                self.mouse_pressed = true;
                return (vec![Command::RequestRedraw], true);
            }
            return (vec![], false);
        }

        // 5) 工具为None且未命中元素：清除选中
        if self.selected_element.is_some() {
            self.selected_element = None;
            self.elements.set_selected(None);
            (vec![Command::UpdateToolbar, Command::RequestRedraw], true)
        } else {
            (vec![], false)
        }
    }

    /// 开始绘制形状
    pub(super) fn start_drawing_shape(&mut self, x: i32, y: i32) {
        self.selected_element = None;
        self.elements.set_selected(None);

        self.interaction_mode = ElementInteractionMode::Drawing;

        let mut new_element = DrawingElement::new(self.current_tool);
        if self.current_tool == DrawingTool::Text {
            if let Ok(settings) = self.settings.read() {
                new_element.color = windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F {
                    r: settings.font_color.0 as f32 / 255.0,
                    g: settings.font_color.1 as f32 / 255.0,
                    b: settings.font_color.2 as f32 / 255.0,
                    a: 1.0,
                };
                new_element.font_size = settings.font_size;
                new_element.font_name = settings.font_name.clone();
                new_element.font_weight = settings.font_weight;
                new_element.font_italic = settings.font_italic;
                new_element.font_underline = settings.font_underline;
                new_element.font_strikeout = settings.font_strikeout;
            }
        } else {
            new_element.color = self.tools.get_brush_color();
            new_element.thickness = self.tools.get_line_thickness();
        }

        match self.current_tool {
            DrawingTool::Pen
            | DrawingTool::Rectangle
            | DrawingTool::Circle
            | DrawingTool::Arrow
            | DrawingTool::Text => {
                new_element.points.push(windows::Win32::Foundation::POINT { x, y });
            }
            _ => {}
        }

        self.current_element = Some(new_element);
    }

    /// 处理鼠标释放事件
    pub fn handle_mouse_up(&mut self, _x: i32, _y: i32) -> (Vec<Command>, bool) {
        if self.mouse_pressed {
            self.end_drag();
            self.mouse_pressed = false;
            self.interaction_mode = ElementInteractionMode::None;
            (vec![Command::UpdateToolbar, Command::RequestRedraw], true)
        } else {
            (vec![], false)
        }
    }

    /// 结束拖拽
    pub(super) fn end_drag(&mut self) {
        match &self.interaction_mode {
            ElementInteractionMode::Drawing => {
                if let Some(mut element) = self.current_element.take() {
                    let should_save = match element.tool {
                        DrawingTool::Pen => element.points.len() > 1,
                        DrawingTool::Rectangle | DrawingTool::Circle | DrawingTool::Arrow => {
                            if element.points.len() >= 2 {
                                let dx = (element.points[1].x - element.points[0].x).abs();
                                let dy = (element.points[1].y - element.points[0].y).abs();
                                dx > 5 || dy > 5
                            } else {
                                false
                            }
                        }
                        DrawingTool::Text => {
                            !element.points.is_empty() && !element.text.trim().is_empty()
                        }
                        _ => false,
                    };

                    if should_save {
                        element.update_bounding_rect();
                        
                        let index = self.elements.count();
                        let action = history::DrawingAction::AddElement {
                            element: element.clone(),
                            index,
                        };
                        self.history.record_action(action, self.selected_element, None);
                        
                        self.elements.add_element(element);
                    }
                }
            }
            ElementInteractionMode::MovingElement => {
                if let Some(index) = self.selected_element {
                    if let Some(element) = self.elements.get_elements().get(index) {
                        let dx = element.rect.left - self.interaction_start_rect.left;
                        let dy = element.rect.top - self.interaction_start_rect.top;
                        
                        if dx != 0 || dy != 0 {
                            let action = history::DrawingAction::MoveElement {
                                index,
                                dx,
                                dy,
                                old_points: self.interaction_start_points.clone(),
                                old_rect: self.interaction_start_rect,
                            };
                            self.history.record_action(
                                action,
                                self.selected_element,
                                self.selected_element,
                            );
                        }
                    }
                }
            }
            ElementInteractionMode::ResizingElement(_) => {
                if let Some(index) = self.selected_element {
                    if let Some(element) = self.elements.get_elements().get(index) {
                        let rect_changed = element.rect.left != self.interaction_start_rect.left
                            || element.rect.top != self.interaction_start_rect.top
                            || element.rect.right != self.interaction_start_rect.right
                            || element.rect.bottom != self.interaction_start_rect.bottom;
                        let points_changed = element.points != self.interaction_start_points;
                        let font_size_changed = (element.font_size - self.interaction_start_font_size).abs() > 0.01;
                        
                        if rect_changed || points_changed || font_size_changed {
                            let action = history::DrawingAction::ResizeElement {
                                index,
                                old_points: self.interaction_start_points.clone(),
                                old_rect: self.interaction_start_rect,
                                old_font_size: self.interaction_start_font_size,
                                new_points: element.points.clone(),
                                new_rect: element.rect,
                                new_font_size: element.font_size,
                            };
                            self.history.record_action(
                                action,
                                self.selected_element,
                                self.selected_element,
                            );
                        }
                    }
                }
            }
            ElementInteractionMode::None => {}
        }
    }

    /// 处理键盘输入
    pub fn handle_key_input(&mut self, key: u32) -> Vec<Command> {
        if self.text_editing {
            match key {
                0x1B => return self.stop_text_editing(),      // VK_ESCAPE
                0x0D => return self.handle_text_input('\n'), // VK_RETURN
                0x08 => return self.handle_backspace(),       // VK_BACK
                0x25 => return self.move_cursor_left(),       // VK_LEFT
                0x27 => return self.move_cursor_right(),      // VK_RIGHT
                0x24 => return self.move_cursor_to_line_start(), // VK_HOME
                0x23 => return self.move_cursor_to_line_end(),   // VK_END
                0x26 => return self.move_cursor_up(),         // VK_UP
                0x28 => return self.move_cursor_down(),       // VK_DOWN
                _ => {}
            }
        }

        // 常规键盘快捷键
        match key {
            26 => self.handle_message(DrawingMessage::Undo), // Ctrl+Z
            25 => self.handle_message(DrawingMessage::Redo), // Ctrl+Y
            46 => {
                // Delete
                if let Some(index) = self.selected_element {
                    self.handle_message(DrawingMessage::DeleteElement(index))
                } else {
                    vec![]
                }
            }
            _ => vec![],
        }
    }

    /// 处理双击事件
    pub fn handle_double_click(
        &mut self,
        x: i32,
        y: i32,
        _selection_rect: Option<&windows::Win32::Foundation::RECT>,
    ) -> Vec<Command> {
        if let Some(index) = self.get_text_element_at_position(x, y) {
            return self.start_text_editing(index);
        }
        vec![]
    }

    /// 获取当前拖拽模式（用于光标显示）
    pub fn get_current_drag_mode(&self) -> Option<crate::types::DragMode> {
        match &self.interaction_mode {
            ElementInteractionMode::MovingElement => Some(crate::types::DragMode::Moving),
            ElementInteractionMode::ResizingElement(mode) => Some(*mode),
            _ => None,
        }
    }

    /// 是否正在进行任何绘图交互
    pub fn is_dragging(&self) -> bool {
        self.mouse_pressed && self.interaction_mode != ElementInteractionMode::None
    }
}
