use windows::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
    },
    core::{PCWSTR, w},
};

use crate::*;

impl WindowState {
    pub fn handle_left_button_down(&mut self, hwnd: HWND, x: i32, y: i32) {
        // 如果是pin状态，只处理窗口拖动
        if self.is_pinned {
            self.mouse_pressed = true;
            self.drag_start_pos = POINT { x, y };
            self.drag_mode = DragMode::Moving;

            unsafe {
                SetCapture(hwnd);
            }
            return;
        }

        // 第一优先级：检查工具栏点击
        if self.has_selection {
            let toolbar_button = self.toolbar.get_button_at_position(x, y);
            if toolbar_button != ToolbarButton::None {
                let is_button_disabled = match toolbar_button {
                    ToolbarButton::Undo => !self.can_undo(),
                    _ => false,
                };

                if !is_button_disabled {
                    self.toolbar.set_clicked_button(toolbar_button);
                    self.handle_toolbar_click(toolbar_button, hwnd);
                }
                return;
            }
        }

        // ...existing code for non-pinned state...
        // 第二优先级：检查文本工具的简化处理
        if self.current_tool == DrawingTool::Text {
            // 简化：只检查是否点击了已有文本元素进行移动
            if let Some(element_index) = self.get_element_at_position(x, y) {
                if self.drawing_elements[element_index].tool == DrawingTool::Text {
                    for element in &mut self.drawing_elements {
                        element.selected = false;
                    }

                    self.selected_element = Some(element_index);
                    self.drawing_elements[element_index].selected = true;

                    self.mouse_pressed = true;
                    self.drag_start_pos = POINT { x, y };
                    self.drag_mode = DragMode::MovingElement;
                    self.drag_start_rect = self.drawing_elements[element_index].rect;

                    unsafe {
                        SetCapture(hwnd);
                        InvalidateRect(hwnd, None, FALSE);
                    }
                    return;
                }
            } else if self.has_selection
                && x >= self.selection_rect.left
                && x <= self.selection_rect.right
                && y >= self.selection_rect.top
                && y <= self.selection_rect.bottom
            {
                // 简化：直接创建文本元素，不需要复杂输入框
                self.create_simple_text_element(x, y);
                unsafe {
                    InvalidateRect(hwnd, None, FALSE);
                }
                return;
            }
        }

        unsafe {
            SetCapture(hwnd);
        }

        self.mouse_pressed = true;
        self.drag_start_pos = POINT { x, y };

        // 处理选择框相关逻辑
        if self.has_selection {
            let in_selection_area = x >= self.selection_rect.left
                && x <= self.selection_rect.right
                && y >= self.selection_rect.top
                && y <= self.selection_rect.bottom;

            let handle_mode = self.get_handle_at_position(x, y);
            let on_selection_handle = handle_mode != DragMode::None;

            if in_selection_area || on_selection_handle {
                self.toolbar.clear_clicked_button();
                self.start_drag(x, y);
            }
        } else {
            self.start_drag(x, y);
        }

        unsafe {
            InvalidateRect(hwnd, None, FALSE);
        }
    }
    pub fn create_simple_text_element(&mut self, x: i32, y: i32) {
        self.save_history();

        let mut text_element = DrawingElement::new(DrawingTool::Text);
        text_element.points.push(POINT { x, y });
        text_element.color = self.drawing_color;
        text_element.thickness = 20.0;
        text_element.text = String::from("Sample Text");

        // 🔧 使用常量确保一致性
        text_element.rect = RECT {
            left: x,
            top: y,
            right: x + TEXT_BOX_WIDTH,   // 使用常量
            bottom: y + TEXT_BOX_HEIGHT, // 使用常量
        };

        self.drawing_elements.push(text_element);
    }

    pub fn handle_double_click(&mut self, hwnd: HWND, x: i32, y: i32) {
        // 暂时什么都不做，或者显示一个系统消息框
        unsafe {
            self.save_selection();
        }
    }

    pub fn handle_left_button_up(&mut self, hwnd: HWND, x: i32, y: i32) {
        unsafe {
            ReleaseCapture();
        }

        // 如果是pin状态，只处理窗口拖动结束
        if self.is_pinned {
            self.mouse_pressed = false;
            self.drag_mode = DragMode::None;
            return;
        }

        // 处理工具栏点击
        let toolbar_button = self.toolbar.get_button_at_position(x, y);
        if toolbar_button != ToolbarButton::None && toolbar_button == self.toolbar.clicked_button {
            // 工具栏按钮已经在 handle_left_button_down 中处理
        } else {
            self.toolbar.clear_clicked_button();
            if self.mouse_pressed {
                self.end_drag();
            }
        }

        self.mouse_pressed = false;
        self.drag_mode = DragMode::None;

        unsafe {
            InvalidateRect(hwnd, None, FALSE);
        }
    }

    pub fn handle_key_down(&mut self, hwnd: HWND, key: u32) {
        // 如果是pin状态，只允许ESC键退出pin模式
        if self.is_pinned {
            match key {
                val if val == VK_ESCAPE.0 as u32 => {
                    unsafe { PostQuitMessage(0) };
                }
                _ => {} // pin状态下忽略其他按键
            }
            return;
        }

        // 只保留基本的键盘快捷键
        match key {
            val if val == VK_ESCAPE.0 as u32 => unsafe {
                PostQuitMessage(0);
            },
            val if val == VK_RETURN.0 as u32 => {
                let _ = self.save_selection();
                unsafe {
                    PostQuitMessage(0);
                }
            }
            val if val == VK_Z.0 as u32 => unsafe {
                if GetKeyState(VK_CONTROL.0 as i32) < 0 {
                    self.undo();
                    InvalidateRect(hwnd, None, FALSE);
                }
            },
            _ => {}
        }
    }
    pub fn update_pinned_window_position(&mut self, hwnd: HWND, x: i32, y: i32) {
        unsafe {
            let dx = x - self.drag_start_pos.x;
            let dy = y - self.drag_start_pos.y;

            // 获取当前窗口位置
            let mut window_rect = RECT::default();
            GetWindowRect(hwnd, &mut window_rect);

            let new_x = window_rect.left + dx;
            let new_y = window_rect.top + dy;

            // 移动窗口
            SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                new_x,
                new_y,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER,
            );
        }
    }
    pub fn handle_mouse_move(&mut self, hwnd: HWND, x: i32, y: i32) {
        // 如果是pin状态，只处理窗口拖动
        if self.is_pinned {
            if self.mouse_pressed && self.drag_mode == DragMode::Moving {
                self.update_pinned_window_position(hwnd, x, y);
            }

            // pin状态下始终显示移动光标
            unsafe {
                if let Ok(cursor) = LoadCursorW(HINSTANCE(std::ptr::null_mut()), IDC_SIZEALL) {
                    SetCursor(cursor);
                }
            }
            return;
        }

        // 检查工具栏悬停
        let hovered_button = self.toolbar.get_button_at_position(x, y);
        let is_button_disabled = match hovered_button {
            ToolbarButton::Undo => !self.can_undo(),
            _ => false,
        };

        if is_button_disabled {
            self.toolbar.set_hovered_button(ToolbarButton::None);
        } else {
            self.toolbar.set_hovered_button(hovered_button);
        }

        // 处理拖拽
        if self.mouse_pressed {
            self.update_drag(x, y);
        } else {
            // 设置光标
            let cursor_id = if hovered_button != ToolbarButton::None && !is_button_disabled {
                IDC_HAND
            } else if self.current_tool == DrawingTool::Text {
                if self.get_element_at_position(x, y).is_some() {
                    IDC_SIZEALL
                } else if self.has_selection
                    && x >= self.selection_rect.left
                    && x <= self.selection_rect.right
                    && y >= self.selection_rect.top
                    && y <= self.selection_rect.bottom
                {
                    IDC_CROSS
                } else {
                    IDC_ARROW
                }
            } else if self.has_selection {
                self.get_cursor_for_position(x, y)
            } else {
                IDC_ARROW
            };

            unsafe {
                if let Ok(cursor) = LoadCursorW(HINSTANCE(std::ptr::null_mut()), cursor_id) {
                    SetCursor(cursor);
                }
            }
        }

        unsafe {
            InvalidateRect(hwnd, None, FALSE);
        }
    }
    pub fn get_handle_at_position(&self, x: i32, y: i32) -> DragMode {
        if self.toolbar.visible
            && x as f32 >= self.toolbar.rect.left
            && x as f32 <= self.toolbar.rect.right
            && y as f32 >= self.toolbar.rect.top
            && y as f32 <= self.toolbar.rect.bottom
        {
            return DragMode::None;
        }

        if !self.has_selection {
            return DragMode::None;
        }

        let rect = &self.selection_rect;
        let center_x = (rect.left + rect.right) / 2;
        let center_y = (rect.top + rect.bottom) / 2;

        let handles = [
            (rect.left, rect.top, DragMode::ResizingTopLeft),
            (center_x, rect.top, DragMode::ResizingTopCenter),
            (rect.right, rect.top, DragMode::ResizingTopRight),
            (rect.right, center_y, DragMode::ResizingMiddleRight),
            (rect.right, rect.bottom, DragMode::ResizingBottomRight),
            (center_x, rect.bottom, DragMode::ResizingBottomCenter),
            (rect.left, rect.bottom, DragMode::ResizingBottomLeft),
            (rect.left, center_y, DragMode::ResizingMiddleLeft),
        ];

        let detection_radius = HANDLE_DETECTION_RADIUS as i32;
        for (hx, hy, mode) in handles.iter() {
            let dx = x - hx;
            let dy = y - hy;
            let distance_sq = dx * dx + dy * dy;
            let radius_sq = detection_radius * detection_radius;

            if distance_sq <= radius_sq {
                return *mode;
            }
        }

        let border_margin = 5;
        if x >= rect.left + border_margin
            && x <= rect.right - border_margin
            && y >= rect.top + border_margin
            && y <= rect.bottom - border_margin
        {
            return DragMode::Moving;
        }

        DragMode::None
    }
    pub fn get_element_handle_at_position(&self, x: i32, y: i32, rect: &RECT) -> DragMode {
        // 修改：只有当手柄在选择框内时才能被检测到
        if x < self.selection_rect.left
            || x > self.selection_rect.right
            || y < self.selection_rect.top
            || y > self.selection_rect.bottom
        {
            return DragMode::None;
        }

        // 如果是箭头元素，只检查起点和终点
        if let Some(element_index) = self.selected_element {
            if element_index < self.drawing_elements.len() {
                let element = &self.drawing_elements[element_index];

                if element.tool == DrawingTool::Arrow && element.points.len() >= 2 {
                    let start_point = &element.points[0];
                    let end_point = &element.points[1];
                    let detection_radius = HANDLE_DETECTION_RADIUS as i32;

                    // 只有当起点在选择框内时才能检测
                    if start_point.x >= self.selection_rect.left
                        && start_point.x <= self.selection_rect.right
                        && start_point.y >= self.selection_rect.top
                        && start_point.y <= self.selection_rect.bottom
                    {
                        let dx = x - start_point.x;
                        let dy = y - start_point.y;
                        let distance_sq = dx * dx + dy * dy;
                        if distance_sq <= detection_radius * detection_radius {
                            return DragMode::ResizingTopLeft;
                        }
                    }

                    // 只有当终点在选择框内时才能检测
                    if end_point.x >= self.selection_rect.left
                        && end_point.x <= self.selection_rect.right
                        && end_point.y >= self.selection_rect.top
                        && end_point.y <= self.selection_rect.bottom
                    {
                        let dx = x - end_point.x;
                        let dy = y - end_point.y;
                        let distance_sq = dx * dx + dy * dy;
                        if distance_sq <= detection_radius * detection_radius {
                            return DragMode::ResizingBottomRight;
                        }
                    }

                    return DragMode::None;
                }
            }
        }

        // 对于其他元素，使用原有的8个手柄检测逻辑
        let center_x = (rect.left + rect.right) / 2;
        let center_y = (rect.top + rect.bottom) / 2;

        let handles = [
            (rect.left, rect.top, DragMode::ResizingTopLeft),
            (center_x, rect.top, DragMode::ResizingTopCenter),
            (rect.right, rect.top, DragMode::ResizingTopRight),
            (rect.right, center_y, DragMode::ResizingMiddleRight),
            (rect.right, rect.bottom, DragMode::ResizingBottomRight),
            (center_x, rect.bottom, DragMode::ResizingBottomCenter),
            (rect.left, rect.bottom, DragMode::ResizingBottomLeft),
            (rect.left, center_y, DragMode::ResizingMiddleLeft),
        ];

        let detection_radius = HANDLE_DETECTION_RADIUS as i32;
        for (hx, hy, mode) in handles.iter() {
            // 只有当手柄在选择框内时才能被检测
            if *hx >= self.selection_rect.left
                && *hx <= self.selection_rect.right
                && *hy >= self.selection_rect.top
                && *hy <= self.selection_rect.bottom
            {
                let dx = x - hx;
                let dy = y - hy;
                let distance_sq = dx * dx + dy * dy;
                let radius_sq = detection_radius * detection_radius;

                if distance_sq <= radius_sq {
                    return *mode;
                }
            }
        }

        DragMode::None
    }
    pub fn get_cursor_for_position(&self, x: i32, y: i32) -> PCWSTR {
        // 检查是否在屏幕范围内
        if x < 0 || x >= self.screen_width || y < 0 || y >= self.screen_height {
            return IDC_ARROW;
        }

        // 如果已经有选择框，外面区域只显示默认光标
        if self.has_selection {
            // 检查是否在工具栏区域
            if self.toolbar.visible
                && x as f32 >= self.toolbar.rect.left
                && x as f32 <= self.toolbar.rect.right
                && y as f32 >= self.toolbar.rect.top
                && y as f32 <= self.toolbar.rect.bottom
            {
                return IDC_HAND;
            }

            // 1. 优先检查选中元素的手柄（只检查完全在选择框内的）
            if let Some(element_index) = self.selected_element {
                if element_index < self.drawing_elements.len() {
                    let element = &self.drawing_elements[element_index];

                    if element.tool != DrawingTool::Pen && self.is_element_visible(element) {
                        if x >= self.selection_rect.left
                            && x <= self.selection_rect.right
                            && y >= self.selection_rect.top
                            && y <= self.selection_rect.bottom
                        {
                            let handle_mode =
                                self.get_element_handle_at_position(x, y, &element.rect);

                            if handle_mode != DragMode::None {
                                return match handle_mode {
                                    DragMode::ResizingTopLeft | DragMode::ResizingBottomRight => {
                                        IDC_SIZENWSE
                                    }
                                    DragMode::ResizingTopRight | DragMode::ResizingBottomLeft => {
                                        IDC_SIZENESW
                                    }
                                    DragMode::ResizingTopCenter
                                    | DragMode::ResizingBottomCenter => IDC_SIZENS,
                                    DragMode::ResizingMiddleLeft
                                    | DragMode::ResizingMiddleRight => IDC_SIZEWE,
                                    _ => IDC_ARROW,
                                };
                            }

                            if element.contains_point(x, y) {
                                return IDC_SIZEALL;
                            }
                        }
                    }
                }
            }

            // 2. 检查其他可见元素（只在选择框内）
            if x >= self.selection_rect.left
                && x <= self.selection_rect.right
                && y >= self.selection_rect.top
                && y <= self.selection_rect.bottom
            {
                if let Some(element_index) = self.get_element_at_position(x, y) {
                    let element = &self.drawing_elements[element_index];
                    if element.tool != DrawingTool::Pen {
                        return IDC_SIZEALL;
                    }
                }
            }

            // 3. 如果选择了绘图工具且在选择框内，显示相应的光标
            if self.current_tool != DrawingTool::None
                && x >= self.selection_rect.left
                && x <= self.selection_rect.right
                && y >= self.selection_rect.top
                && y <= self.selection_rect.bottom
            {
                return match self.current_tool {
                    DrawingTool::Pen => IDC_CROSS,
                    DrawingTool::Rectangle | DrawingTool::Circle | DrawingTool::Arrow => IDC_CROSS,
                    DrawingTool::Text => IDC_IBEAM,
                    _ => IDC_ARROW,
                };
            }

            // 4. 检查选择框手柄
            let handle_mode = self.get_handle_at_position(x, y);
            match handle_mode {
                DragMode::ResizingTopLeft | DragMode::ResizingBottomRight => IDC_SIZENWSE,
                DragMode::ResizingTopRight | DragMode::ResizingBottomLeft => IDC_SIZENESW,
                DragMode::ResizingTopCenter | DragMode::ResizingBottomCenter => IDC_SIZENS,
                DragMode::ResizingMiddleLeft | DragMode::ResizingMiddleRight => IDC_SIZEWE,
                DragMode::Moving => IDC_SIZEALL,
                _ => IDC_NO,
            }
        } else {
            IDC_ARROW
        }
    }
    pub fn update_drag(&mut self, x: i32, y: i32) {
        if !self.mouse_pressed {
            return;
        }

        match self.drag_mode {
            DragMode::Drawing => {
                let left = self.drag_start_pos.x.min(x);
                let right = self.drag_start_pos.x.max(x);
                let top = self.drag_start_pos.y.min(y);
                let bottom = self.drag_start_pos.y.max(y);

                self.selection_rect = RECT {
                    left: left.max(0),
                    top: top.max(0),
                    right: right.min(self.screen_width),
                    bottom: bottom.min(self.screen_height),
                };
            }

            DragMode::DrawingShape => {
                if let Some(ref mut element) = self.current_element {
                    let selection_left = self.selection_rect.left;
                    let selection_right = self.selection_rect.right;
                    let selection_top = self.selection_rect.top;
                    let selection_bottom = self.selection_rect.bottom;

                    let clamped_x = x.max(selection_left).min(selection_right);
                    let clamped_y = y.max(selection_top).min(selection_bottom);

                    match element.tool {
                        DrawingTool::Pen => {
                            element.points.push(POINT {
                                x: clamped_x,
                                y: clamped_y,
                            });
                        }
                        DrawingTool::Rectangle | DrawingTool::Circle | DrawingTool::Arrow => {
                            if element.points.is_empty() {
                                element.points.push(self.drag_start_pos);
                            }
                            if element.points.len() == 1 {
                                element.points.push(POINT {
                                    x: clamped_x,
                                    y: clamped_y,
                                });
                            } else {
                                element.points[1] = POINT {
                                    x: clamped_x,
                                    y: clamped_y,
                                };
                            }

                            // 更新rect信息（用于边界检查）
                            let start = &element.points[0];
                            let end = &element.points[1];
                            element.rect = RECT {
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

            DragMode::Moving => {
                if self.current_tool == DrawingTool::None {
                    let dx = x - self.drag_start_pos.x;
                    let dy = y - self.drag_start_pos.y;
                    let start_rect = self.drag_start_rect;
                    let width = start_rect.right - start_rect.left;
                    let height = start_rect.bottom - start_rect.top;

                    let new_left = (start_rect.left + dx).max(0).min(self.screen_width - width);
                    let new_top = (start_rect.top + dy)
                        .max(0)
                        .min(self.screen_height - height);

                    self.selection_rect = RECT {
                        left: new_left,
                        top: new_top,
                        right: new_left + width,
                        bottom: new_top + height,
                    };

                    if self.toolbar.visible {
                        self.toolbar.update_position(
                            &self.selection_rect,
                            self.screen_width,
                            self.screen_height,
                        );
                    }
                }
            }

            DragMode::MovingElement => {
                if let Some(element_index) = self.selected_element {
                    if element_index < self.drawing_elements.len() {
                        let element = &self.drawing_elements[element_index];

                        // 不允许移动画笔元素
                        if element.tool == DrawingTool::Pen {
                            return;
                        }

                        // 修改：移除鼠标位置限制，允许拖动到任何位置
                        // 但元素如果拖出选择框外将不可见（通过裁剪实现）

                        // 计算相对于初始拖拽位置的总偏移量
                        let dx = x - self.drag_start_pos.x;
                        let dy = y - self.drag_start_pos.y;

                        // 先恢复到初始位置，然后移动到新位置
                        let element = &mut self.drawing_elements[element_index];

                        // 计算当前位置与初始位置的差值
                        let current_dx = element.rect.left - self.drag_start_rect.left;
                        let current_dy = element.rect.top - self.drag_start_rect.top;

                        // 恢复到初始位置
                        element.move_by(-current_dx, -current_dy);

                        // 移动到新位置（不限制范围）
                        element.move_by(dx, dy);
                    }
                }
            }

            // 元素调整大小：移除鼠标位置限制
            DragMode::ResizingTopLeft
            | DragMode::ResizingTopCenter
            | DragMode::ResizingTopRight
            | DragMode::ResizingMiddleRight
            | DragMode::ResizingBottomRight
            | DragMode::ResizingBottomCenter
            | DragMode::ResizingBottomLeft
            | DragMode::ResizingMiddleLeft => {
                // 判断是否是选择框的调整还是元素的调整
                if let Some(element_index) = self.selected_element {
                    if element_index < self.drawing_elements.len() {
                        let element = &mut self.drawing_elements[element_index];

                        // 不允许调整画笔元素大小
                        if element.tool == DrawingTool::Pen {
                            return;
                        }

                        // 修改：移除鼠标位置限制
                        // 箭头元素的特殊处理
                        if element.tool == DrawingTool::Arrow && element.points.len() >= 2 {
                            match self.drag_mode {
                                DragMode::ResizingTopLeft => {
                                    // 调整起点（不限制范围）
                                    element.points[0] = POINT { x, y };
                                }
                                DragMode::ResizingBottomRight => {
                                    // 调整终点（不限制范围）
                                    element.points[1] = POINT { x, y };
                                }
                                _ => {} // 箭头只支持起点和终点调整
                            }

                            // 更新箭头的边界矩形
                            element.update_bounding_rect();
                        } else {
                            // 其他元素保持原有的调整逻辑（移除位置限制）
                            let mut new_rect = self.drag_start_rect;
                            let dx = x - self.drag_start_pos.x;
                            let dy = y - self.drag_start_pos.y;

                            match self.drag_mode {
                                DragMode::ResizingTopLeft => {
                                    new_rect.left += dx;
                                    new_rect.top += dy;
                                }
                                DragMode::ResizingTopCenter => {
                                    new_rect.top += dy;
                                }
                                DragMode::ResizingTopRight => {
                                    new_rect.right += dx;
                                    new_rect.top += dy;
                                }
                                DragMode::ResizingMiddleRight => {
                                    new_rect.right += dx;
                                }
                                DragMode::ResizingBottomRight => {
                                    new_rect.right += dx;
                                    new_rect.bottom += dy;
                                }
                                DragMode::ResizingBottomCenter => {
                                    new_rect.bottom += dy;
                                }
                                DragMode::ResizingBottomLeft => {
                                    new_rect.left += dx;
                                    new_rect.bottom += dy;
                                }
                                DragMode::ResizingMiddleLeft => {
                                    new_rect.left += dx;
                                }
                                _ => {}
                            }

                            // 只确保最小尺寸（移除位置限制）
                            if new_rect.right - new_rect.left >= 10
                                && new_rect.bottom - new_rect.top >= 10
                            {
                                element.resize(new_rect);
                            }
                        }
                    }
                } else {
                    // 调整选择框大小（保持原有逻辑）
                    let mut new_rect = self.drag_start_rect;
                    let dx = x - self.drag_start_pos.x;
                    let dy = y - self.drag_start_pos.y;

                    match self.drag_mode {
                        DragMode::ResizingTopLeft => {
                            new_rect.left += dx;
                            new_rect.top += dy;
                        }
                        DragMode::ResizingTopCenter => {
                            new_rect.top += dy;
                        }
                        DragMode::ResizingTopRight => {
                            new_rect.right += dx;
                            new_rect.top += dy;
                        }
                        DragMode::ResizingMiddleRight => {
                            new_rect.right += dx;
                        }
                        DragMode::ResizingBottomRight => {
                            new_rect.right += dx;
                            new_rect.bottom += dy;
                        }
                        DragMode::ResizingBottomCenter => {
                            new_rect.bottom += dy;
                        }
                        DragMode::ResizingBottomLeft => {
                            new_rect.left += dx;
                            new_rect.bottom += dy;
                        }
                        DragMode::ResizingMiddleLeft => {
                            new_rect.left += dx;
                        }
                        _ => {}
                    }

                    // 确保选择框在屏幕范围内且有最小尺寸
                    new_rect.left = new_rect.left.max(0);
                    new_rect.top = new_rect.top.max(0);
                    new_rect.right = new_rect.right.min(self.screen_width);
                    new_rect.bottom = new_rect.bottom.min(self.screen_height);

                    if new_rect.right - new_rect.left >= MIN_BOX_SIZE
                        && new_rect.bottom - new_rect.top >= MIN_BOX_SIZE
                    {
                        self.selection_rect = new_rect;

                        // 更新工具栏位置
                        if self.toolbar.visible {
                            self.toolbar.update_position(
                                &self.selection_rect,
                                self.screen_width,
                                self.screen_height,
                            );
                        }
                    }
                }
            }

            _ => {}
        }
    }
    pub fn start_drag(&mut self, x: i32, y: i32) {
        // 如果已经有选择框，不允许在外面重新框选
        if self.has_selection {
            // 1. 首先检查是否点击了选中元素的手柄（最高优先级）
            if let Some(element_index) = self.selected_element {
                if element_index < self.drawing_elements.len() {
                    let element = &self.drawing_elements[element_index];

                    // 只有非画笔元素才检查手柄
                    if element.tool != DrawingTool::Pen {
                        // 额外检查：只有当元素在选择框内可见时才允许操作手柄
                        if self.is_element_visible(element) {
                            let handle_mode =
                                self.get_element_handle_at_position(x, y, &element.rect);

                            if handle_mode != DragMode::None {
                                self.drag_mode = handle_mode;
                                self.mouse_pressed = true;
                                self.drag_start_pos = POINT { x, y };
                                self.drag_start_rect = element.rect;
                                return;
                            }

                            // 检查是否点击了选中元素内部（移动）
                            // 但只允许在选择框内的部分被点击
                            if x >= self.selection_rect.left
                                && x <= self.selection_rect.right
                                && y >= self.selection_rect.top
                                && y <= self.selection_rect.bottom
                                && element.contains_point(x, y)
                            {
                                self.drag_mode = DragMode::MovingElement;
                                self.mouse_pressed = true;
                                self.drag_start_pos = POINT { x, y };
                                self.drag_start_rect = element.rect;
                                return;
                            }
                        }
                    }
                }
            }

            // 2. 检查是否点击了其他绘图元素（只在选择框内）
            if x >= self.selection_rect.left
                && x <= self.selection_rect.right
                && y >= self.selection_rect.top
                && y <= self.selection_rect.bottom
            {
                if let Some(element_index) = self.get_element_at_position(x, y) {
                    let element = &self.drawing_elements[element_index];

                    // 如果是画笔元素，不允许选择
                    if element.tool == DrawingTool::Pen {
                        return;
                    }

                    // 清除之前选择的元素
                    for element in &mut self.drawing_elements {
                        element.selected = false;
                    }

                    // 选择点击的元素（非画笔）
                    self.drawing_elements[element_index].selected = true;
                    self.selected_element = Some(element_index);

                    // 更新元素的边界矩形
                    self.drawing_elements[element_index].update_bounding_rect();

                    // 检查是否点击了新选中元素的调整手柄
                    let element_rect = self.drawing_elements[element_index].rect;
                    let handle_mode = self.get_element_handle_at_position(x, y, &element_rect);

                    if handle_mode != DragMode::None {
                        self.drag_mode = handle_mode;
                        self.mouse_pressed = true;
                        self.drag_start_pos = POINT { x, y };
                        self.drag_start_rect = element_rect;
                    } else {
                        // 开始移动元素
                        self.drag_mode = DragMode::MovingElement;
                        self.mouse_pressed = true;
                        self.drag_start_pos = POINT { x, y };
                        self.drag_start_rect = element_rect;
                    }
                    return;
                }
            }

            // 3. 如果选择了绘图工具，且在选择框内，开始绘图
            if self.current_tool != DrawingTool::None {
                if x >= self.selection_rect.left
                    && x <= self.selection_rect.right
                    && y >= self.selection_rect.top
                    && y <= self.selection_rect.bottom
                {
                    // 清除元素选择
                    for element in &mut self.drawing_elements {
                        element.selected = false;
                    }
                    self.selected_element = None;

                    self.save_history();
                    self.drag_mode = DragMode::DrawingShape;
                    self.mouse_pressed = true;
                    self.drag_start_pos = POINT { x, y };

                    let mut new_element = DrawingElement::new(self.current_tool);
                    new_element.color = self.drawing_color;
                    new_element.thickness = self.drawing_thickness as f32;

                    match self.current_tool {
                        DrawingTool::Pen => {
                            new_element.points.push(POINT { x, y });
                        }
                        DrawingTool::Rectangle | DrawingTool::Circle | DrawingTool::Arrow => {
                            new_element.points.push(POINT { x, y });
                        }
                        DrawingTool::Text => {
                            new_element.points.push(POINT { x, y });
                        }
                        _ => {}
                    }

                    self.current_element = Some(new_element);
                }
                return;
            }

            // 4. 如果没有选择绘图工具，只允许操作选择框手柄
            if self.current_tool == DrawingTool::None {
                // 清除元素选择
                for element in &mut self.drawing_elements {
                    element.selected = false;
                }
                self.selected_element = None;

                // 检查选择框手柄
                let handle_mode = self.get_handle_at_position(x, y);

                if matches!(
                    handle_mode,
                    DragMode::Moving
                        | DragMode::ResizingTopLeft
                        | DragMode::ResizingTopCenter
                        | DragMode::ResizingTopRight
                        | DragMode::ResizingMiddleRight
                        | DragMode::ResizingBottomRight
                        | DragMode::ResizingBottomCenter
                        | DragMode::ResizingBottomLeft
                        | DragMode::ResizingMiddleLeft
                ) {
                    self.drag_mode = handle_mode;
                    self.mouse_pressed = true;
                    self.drag_start_pos = POINT { x, y };
                    self.drag_start_rect = self.selection_rect;
                }
                // 注意：这里移除了创建新选择框的逻辑
            }
        } else {
            // 只有在没有选择框时才允许创建新的选择框
            if self.current_tool == DrawingTool::None {
                // 清除元素选择
                for element in &mut self.drawing_elements {
                    element.selected = false;
                }
                self.selected_element = None;

                // 创建新选择框
                self.drag_mode = DragMode::Drawing;
                self.mouse_pressed = true;
                self.drag_start_pos = POINT { x, y };
                self.selection_rect = RECT {
                    left: x,
                    top: y,
                    right: x,
                    bottom: y,
                };
                self.has_selection = true;
                self.toolbar.hide();
            }
        }
    }

    //未用到
    pub fn ensure_minimum_size(&mut self) {
        // 确保选择框有最小尺寸
        if self.selection_rect.right - self.selection_rect.left < MIN_BOX_SIZE {
            if self.drag_mode == DragMode::ResizingTopLeft
                || self.drag_mode == DragMode::ResizingBottomLeft
                || self.drag_mode == DragMode::ResizingMiddleLeft
            {
                self.selection_rect.left = self.selection_rect.right - MIN_BOX_SIZE;
            } else {
                self.selection_rect.right = self.selection_rect.left + MIN_BOX_SIZE;
            }
        }

        if self.selection_rect.bottom - self.selection_rect.top < MIN_BOX_SIZE {
            if self.drag_mode == DragMode::ResizingTopLeft
                || self.drag_mode == DragMode::ResizingTopCenter
                || self.drag_mode == DragMode::ResizingTopRight
            {
                self.selection_rect.top = self.selection_rect.bottom - MIN_BOX_SIZE;
            } else {
                self.selection_rect.bottom = self.selection_rect.top + MIN_BOX_SIZE;
            }
        }

        // 确保不超出屏幕边界
        self.clamp_selection_to_screen();
    }
    pub fn clamp_selection_to_screen(&mut self) {
        let width = self.selection_rect.right - self.selection_rect.left;
        let height = self.selection_rect.bottom - self.selection_rect.top;

        // 确保选择框在屏幕内
        if self.selection_rect.left < 0 {
            self.selection_rect.left = 0;
            self.selection_rect.right = width;
        }
        if self.selection_rect.top < 0 {
            self.selection_rect.top = 0;
            self.selection_rect.bottom = height;
        }
        if self.selection_rect.right > self.screen_width {
            self.selection_rect.right = self.screen_width;
            self.selection_rect.left = self.screen_width - width;
        }
        if self.selection_rect.bottom > self.screen_height {
            self.selection_rect.bottom = self.screen_height;
            self.selection_rect.top = self.screen_height - height;
        }
    }
    pub fn get_drag_mode_at_position(&self, x: i32, y: i32) -> DragMode {
        if !self.has_selection {
            return DragMode::None;
        }

        let center_x = (self.selection_rect.left + self.selection_rect.right) / 2;
        let center_y = (self.selection_rect.top + self.selection_rect.bottom) / 2;
        let detection_radius = HANDLE_DETECTION_RADIUS as i32;

        // 检查8个手柄
        if self.point_near(
            x,
            y,
            self.selection_rect.left,
            self.selection_rect.top,
            detection_radius,
        ) {
            return DragMode::ResizingTopLeft;
        }
        if self.point_near(x, y, center_x, self.selection_rect.top, detection_radius) {
            return DragMode::ResizingTopCenter;
        }
        if self.point_near(
            x,
            y,
            self.selection_rect.right,
            self.selection_rect.top,
            detection_radius,
        ) {
            return DragMode::ResizingTopRight;
        }
        if self.point_near(x, y, self.selection_rect.right, center_y, detection_radius) {
            return DragMode::ResizingMiddleRight;
        }
        if self.point_near(
            x,
            y,
            self.selection_rect.right,
            self.selection_rect.bottom,
            detection_radius,
        ) {
            return DragMode::ResizingBottomRight;
        }
        if self.point_near(x, y, center_x, self.selection_rect.bottom, detection_radius) {
            return DragMode::ResizingBottomCenter;
        }
        if self.point_near(
            x,
            y,
            self.selection_rect.left,
            self.selection_rect.bottom,
            detection_radius,
        ) {
            return DragMode::ResizingBottomLeft;
        }
        if self.point_near(x, y, self.selection_rect.left, center_y, detection_radius) {
            return DragMode::ResizingMiddleLeft;
        }

        // 检查是否在选择框内部（用于移动）
        if x >= self.selection_rect.left
            && x <= self.selection_rect.right
            && y >= self.selection_rect.top
            && y <= self.selection_rect.bottom
        {
            return DragMode::Moving;
        }

        DragMode::None
    }
    pub fn point_near(&self, x1: i32, y1: i32, x2: i32, y2: i32, radius: i32) -> bool {
        let dx = x1 - x2;
        let dy = y1 - y2;
        (dx * dx + dy * dy) <= (radius * radius)
    }
}
