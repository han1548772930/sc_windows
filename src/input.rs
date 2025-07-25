use windows::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
    },
    core::PCWSTR,
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
                    ToolbarButton::ExtractText => !self.ocr_engine_available,
                    _ => false,
                };

                if !is_button_disabled {
                    // 如果正在编辑文本，先保存文本
                    if self.text_editing {
                        self.stop_text_editing(hwnd);
                    }

                    self.toolbar.set_clicked_button(toolbar_button);
                    self.handle_toolbar_click(toolbar_button, hwnd);
                }
                return;
            }
        }

        // 第二优先级：如果正在编辑文本，点击其他地方应该保存文本并退出编辑模式
        if self.text_editing {
            // 检查是否点击了正在编辑的文本元素
            let clicked_editing_element = if let Some(editing_index) = self.editing_element_index {
                self.get_element_at_position(x, y) == Some(editing_index)
            } else {
                false
            };

            // 如果点击的是正在编辑的文本元素，检查是否点击了手柄
            if clicked_editing_element {
                if let Some(editing_index) = self.editing_element_index {
                    if editing_index < self.drawing_elements.len() {
                        let element = &self.drawing_elements[editing_index];
                        let element_handle_mode =
                            self.get_element_handle_at_position(x, y, &element.rect);

                        // 如果点击了手柄，开始拖拽
                        if element_handle_mode != DragMode::None {
                            unsafe {
                                SetCapture(hwnd);
                            }
                            self.mouse_pressed = true;
                            self.drag_start_pos = POINT { x, y };
                            self.start_drag(x, y);
                            unsafe {
                                let _ = InvalidateRect(Some(hwnd), None, FALSE.into());
                            }
                            return;
                        }
                    }
                }
                // 点击的是正在编辑的文本元素但不是手柄，继续编辑（不做任何处理）
                return;
            } else {
                // 如果点击的不是正在编辑的文本元素，则保存并退出编辑模式
                self.stop_text_editing(hwnd);
                // 保存后继续处理点击事件，可能是选择其他元素
                // 但是不应该立即创建新的文本元素
            }
        }

        // 第三优先级：在处理文本工具之前，先检查是否点击了手柄
        // 如果有选中的元素，优先检查手柄点击
        if self.has_selection && self.selected_element.is_some() {
            let handle_mode = self.get_handle_at_position(x, y);
            let mut on_selection_handle = handle_mode != DragMode::None;

            // 如果没有检测到选择框手柄，检查是否点击了选中元素的手柄
            if !on_selection_handle {
                if let Some(element_index) = self.selected_element {
                    if element_index < self.drawing_elements.len() {
                        let element = &self.drawing_elements[element_index];
                        if element.tool != DrawingTool::Pen {
                            let element_handle_mode =
                                self.get_element_handle_at_position(x, y, &element.rect);
                            on_selection_handle = element_handle_mode != DragMode::None;
                        }
                    }
                }
            }

            if on_selection_handle {
                unsafe {
                    SetCapture(hwnd);
                }
                self.mouse_pressed = true;
                self.drag_start_pos = POINT { x, y };
                self.start_drag(x, y);
                unsafe {
                    let _ = InvalidateRect(Some(hwnd), None, FALSE.into());
                }
                return;
            }
        }

        // 第四优先级：检查是否点击了文本元素（无论当前工具是什么）
        if let Some(element_index) = self.get_element_at_position(x, y) {
            if self.drawing_elements[element_index].tool == DrawingTool::Text {
                // 如果正在编辑其他文本元素，先停止编辑
                if self.text_editing {
                    self.stop_text_editing(hwnd);
                    // 注意：stop_text_editing可能会删除元素，需要重新获取元素索引
                    if let Some(new_element_index) = self.get_element_at_position(x, y) {
                        if new_element_index < self.drawing_elements.len()
                            && self.drawing_elements[new_element_index].tool == DrawingTool::Text
                        {
                            // 选中新的文本元素
                            self.selected_element = Some(new_element_index);
                            for (i, element) in self.drawing_elements.iter_mut().enumerate() {
                                element.selected = i == new_element_index;
                            }

                            unsafe {
                                SetCapture(hwnd);
                            }
                            self.mouse_pressed = true;
                            self.drag_start_pos = POINT { x, y };
                            self.start_drag(x, y);
                            unsafe {
                                let _ = InvalidateRect(Some(hwnd), None, FALSE.into());
                            }
                            return;
                        }
                    }
                    // 如果元素被删除或不再是文本元素，继续处理其他逻辑
                } else {
                    // 没有正在编辑的文本，直接选中当前文本元素（拖动已保存的文本时不显示输入框）
                    self.selected_element = Some(element_index);
                    for (i, element) in self.drawing_elements.iter_mut().enumerate() {
                        element.selected = i == element_index;
                    }

                    // 确保不进入编辑模式，只是选中和准备拖动
                    self.text_editing = false;
                    self.editing_element_index = None;
                    self.text_cursor_visible = false;

                    unsafe {
                        SetCapture(hwnd);
                    }
                    self.mouse_pressed = true;
                    self.drag_start_pos = POINT { x, y };
                    self.start_drag(x, y);
                    unsafe {
                        let _ = InvalidateRect(Some(hwnd), None, FALSE.into());
                    }
                    return;
                }
            }
        }

        // 第五优先级：文本工具创建新元素（只有在没有正在编辑文本时才创建）
        if self.current_tool == DrawingTool::Text && !self.text_editing && !self.just_saved_text {
            // 检查是否点击了现有元素
            let clicked_element = self.get_element_at_position(x, y);

            // 只有在没有点击任何元素时才创建新的文本元素
            if clicked_element.is_none() {
                // 如果有选择区域，只能在选择区域内创建；如果没有选择区域，可以在任何地方创建
                let can_create_text = if self.has_selection {
                    x >= self.selection_rect.left
                        && x <= self.selection_rect.right
                        && y >= self.selection_rect.top
                        && y <= self.selection_rect.bottom
                } else {
                    true // 没有选择区域时，可以在任何地方创建文本
                };

                if can_create_text {
                    // 创建新的文本元素并开始编辑
                    self.create_and_edit_text_element(x, y, hwnd);
                    return;
                }
            }
        }

        // 重置标志，下次点击可以创建新文本
        self.just_saved_text = false;

        // 进行常规拖拽处理
        unsafe {
            SetCapture(hwnd);
        }

        self.mouse_pressed = true;
        self.drag_start_pos = POINT { x, y };

        // 记录鼠标按下的位置和状态，但不立即做决定
        self.mouse_pressed = true;
        self.drag_start_pos = POINT { x, y };

        unsafe {
            SetCapture(hwnd);
        }

        // 如果有自动高亮的窗口，暂时不做任何操作
        // 等到鼠标释放时再决定是选中窗口还是开始新的选择
        if self.auto_highlight_enabled && self.has_selection {
            // 保存当前状态，等待鼠标释放时的判断
            return;
        }

        // 如果没有自动高亮但有选择区域，可能是工具栏操作，先检查工具栏
        if self.has_selection && !self.auto_highlight_enabled {
            let toolbar_button = self.toolbar.get_button_at_position(x, y);
            if toolbar_button != ToolbarButton::None {
                // 工具栏点击，不开始拖拽
                return;
            }
        }

        // 处理其他情况的拖拽开始
        self.start_drag(x, y);

        unsafe {
            let _ = InvalidateRect(Some(hwnd), None, FALSE.into());
        }
    }
    pub fn create_simple_text_element(&mut self, x: i32, y: i32) {
        self.save_history();

        let mut text_element = DrawingElement::new(DrawingTool::Text);
        text_element.points.push(POINT { x, y });
        // 文字元素使用文字颜色
        let (_, text_color, _, _) = crate::constants::get_colors_from_settings();
        text_element.color = text_color;
        let settings = crate::simple_settings::SimpleSettings::load();
        text_element.thickness = settings.font_size;
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
        // 检查是否双击了文本元素
        if let Some(element_index) = self.get_element_at_position(x, y) {
            if self.drawing_elements[element_index].tool == DrawingTool::Text {
                // 双击文本元素进入编辑模式
                self.start_text_editing(element_index, hwnd);
                return;
            }
        }

        // 其他情况保存选择
        let _ = self.save_selection();
    }

    pub fn handle_left_button_up(&mut self, hwnd: HWND, x: i32, y: i32) {
        unsafe {
            let _ = ReleaseCapture();
        }

        // 如果是pin状态，只处理窗口拖动结束
        if self.is_pinned {
            self.mouse_pressed = false;
            self.drag_mode = DragMode::None;
            return;
        }

        // 检查是否是单击（没有拖拽）
        let is_click = self.mouse_pressed
            && (x - self.drag_start_pos.x).abs() < 5
            && (y - self.drag_start_pos.y).abs() < 5;

        // 处理工具栏点击
        let toolbar_button = self.toolbar.get_button_at_position(x, y);
        if toolbar_button != ToolbarButton::None && toolbar_button == self.toolbar.clicked_button {
            // 工具栏按钮已经在 handle_left_button_down 中处理
        } else {
            // 如果是单击且当前有选择区域
            if is_click && self.has_selection {
                // 如果自动高亮仍然启用，说明这是对自动高亮窗口的点击选择
                if self.auto_highlight_enabled {
                    // 更新并显示工具栏，确认选择，并禁用自动高亮（进入已选择状态）
                    self.toolbar.update_position(
                        &self.selection_rect,
                        self.screen_width,
                        self.screen_height,
                    );
                    self.toolbar.visible = true;
                    // 禁用自动高亮，进入已选择状态
                    self.auto_highlight_enabled = false;
                } else {
                    // 自动高亮已禁用，说明这是手动拖拽的结果
                    // 更新并显示工具栏
                    self.toolbar.update_position(
                        &self.selection_rect,
                        self.screen_width,
                        self.screen_height,
                    );
                    self.toolbar.visible = true;
                    // 保持自动高亮禁用状态
                }
            } else if is_click && !self.has_selection {
                // 如果是单击但没有选择区域，重新启用自动高亮
                self.auto_highlight_enabled = true;
            }

            // 如果没有选择区域，重新启用自动高亮以便下次使用
            if !self.has_selection {
                self.auto_highlight_enabled = true;
            }

            // 只有在没有选中元素且不是绘图工具时才清除工具栏状态
            if self.selected_element.is_none() && self.current_tool == DrawingTool::None {
                self.toolbar.clear_clicked_button();
            }
            if self.mouse_pressed {
                self.end_drag();
            }
        }

        self.mouse_pressed = false;
        self.drag_mode = DragMode::None;

        unsafe {
            let _ = InvalidateRect(Some(hwnd), None, FALSE.into());
        }
    }

    pub fn handle_key_down(&mut self, hwnd: HWND, key: u32) {
        // 如果是pin状态，只允许ESC键退出pin模式
        if self.is_pinned {
            match key {
                val if val == VK_ESCAPE.0 as u32 => {
                    // ESC键清除所有状态并隐藏窗口
                    // 异步停止OCR引擎
                    crate::ocr::PaddleOcrEngine::stop_ocr_engine_async();

                    self.reset_to_initial_state();
                    unsafe {
                        let _ = ShowWindow(hwnd, SW_HIDE);
                    };
                }
                _ => {} // pin状态下忽略其他按键
            }
            return;
        }

        // 处理文字编辑相关按键
        if self.text_editing {
            match key {
                val if val == VK_ESCAPE.0 as u32 => {
                    // ESC 键退出文字编辑模式
                    self.stop_text_editing(hwnd);
                    return;
                }
                val if val == VK_RETURN.0 as u32 => {
                    // Enter 键插入换行符
                    self.handle_text_input('\n', hwnd);
                    return;
                }
                val if val == VK_BACK.0 as u32 => {
                    // 退格键删除字符
                    self.handle_backspace(hwnd);
                    return;
                }
                val if val == VK_LEFT.0 as u32 => {
                    // 左箭头键：光标向左移动
                    self.move_cursor_left(hwnd);
                    return;
                }
                val if val == VK_RIGHT.0 as u32 => {
                    // 右箭头键：光标向右移动
                    self.move_cursor_right(hwnd);
                    return;
                }
                val if val == VK_HOME.0 as u32 => {
                    // Home键：光标移动到行首
                    self.move_cursor_to_line_start(hwnd);
                    return;
                }
                val if val == VK_END.0 as u32 => {
                    // End键：光标移动到行尾
                    self.move_cursor_to_line_end(hwnd);
                    return;
                }
                val if val == VK_UP.0 as u32 => {
                    // 上箭头键：光标向上移动一行
                    self.move_cursor_up(hwnd);
                    return;
                }
                val if val == VK_DOWN.0 as u32 => {
                    // 下箭头键：光标向下移动一行
                    self.move_cursor_down(hwnd);
                    return;
                }
                _ => {}
            }
        }

        // 只保留基本的键盘快捷键
        match key {
            val if val == VK_ESCAPE.0 as u32 => unsafe {
                // ESC键直接清除所有状态并隐藏窗口
                // 异步停止OCR引擎
                crate::ocr::PaddleOcrEngine::stop_ocr_engine_async();

                self.reset_to_initial_state();
                let _ = ShowWindow(hwnd, SW_HIDE);
            },
            val if val == VK_RETURN.0 as u32 => {
                let _ = self.save_selection();
                unsafe {
                    // Enter键保存后隐藏窗口而不是退出程序
                    let _ = ShowWindow(hwnd, SW_HIDE);
                }
            }
            val if val == VK_Z.0 as u32 => unsafe {
                if GetKeyState(VK_CONTROL.0 as i32) < 0 {
                    self.undo();
                    let _ = InvalidateRect(Some(hwnd), None, FALSE.into());
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
            let _ = GetWindowRect(hwnd, &mut window_rect);

            let new_x = window_rect.left + dx;
            let new_y = window_rect.top + dy;

            // 移动窗口
            let _ = SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
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
                if let Ok(cursor) = LoadCursorW(Some(HINSTANCE(std::ptr::null_mut())), IDC_SIZEALL)
                {
                    SetCursor(Some(cursor));
                }
            }
            return;
        }

        // 窗口自动高亮检测（仅在启用自动高亮且没有按下鼠标时）
        if self.auto_highlight_enabled && !self.mouse_pressed {
            if let Some(window_info) = self.window_detector.get_window_at_point(x, y) {
                // 如果检测到窗口，自动设置选择区域为窗口边界
                self.selection_rect = window_info.rect;
                self.has_selection = true;

                // 触发重绘以显示高亮框
                unsafe {
                    let _ = InvalidateRect(Some(hwnd), None, FALSE.into());
                }
            } else {
                // 如果没有检测到窗口，清除自动高亮
                if self.has_selection {
                    self.has_selection = false;
                    unsafe {
                        let _ = InvalidateRect(Some(hwnd), None, FALSE.into());
                    }
                }
            }
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
            // 检查是否开始拖拽（移动距离超过阈值）
            let drag_threshold = 5;
            let dx = (x - self.drag_start_pos.x).abs();
            let dy = (y - self.drag_start_pos.y).abs();

            if dx > drag_threshold || dy > drag_threshold {
                // 开始拖拽，禁用自动高亮
                if self.auto_highlight_enabled {
                    self.auto_highlight_enabled = false;

                    // 如果之前有自动高亮的选择，清除它并开始新的手动选择
                    if self.has_selection {
                        self.has_selection = false;
                        self.start_drag(self.drag_start_pos.x, self.drag_start_pos.y);
                    }
                }
            }

            self.update_drag(x, y);

            // 在拖拽过程中也设置正确的光标
            let cursor_id = match self.drag_mode {
                DragMode::ResizingTopLeft | DragMode::ResizingBottomRight => IDC_SIZENWSE,
                DragMode::ResizingTopRight | DragMode::ResizingBottomLeft => IDC_SIZENESW,
                DragMode::ResizingTopCenter | DragMode::ResizingBottomCenter => IDC_SIZENS,
                DragMode::ResizingMiddleLeft | DragMode::ResizingMiddleRight => IDC_SIZEWE,
                DragMode::Moving | DragMode::MovingElement => IDC_SIZEALL,
                _ => IDC_ARROW,
            };

            unsafe {
                if let Ok(cursor) = LoadCursorW(Some(HINSTANCE(std::ptr::null_mut())), cursor_id) {
                    SetCursor(Some(cursor));
                }
            }
        } else {
            // 设置光标
            let cursor_id = if hovered_button != ToolbarButton::None && !is_button_disabled {
                IDC_HAND
            } else if self.current_tool == DrawingTool::Text || self.text_editing {
                // 检查是否在文本元素上
                if let Some(element_index) = self.get_element_at_position(x, y) {
                    if self.drawing_elements[element_index].tool == DrawingTool::Text {
                        // 如果正在编辑此文本元素，检查是否在手柄上
                        if self.text_editing && self.editing_element_index == Some(element_index) {
                            let element = &self.drawing_elements[element_index];
                            let handle_mode =
                                self.get_element_handle_at_position(x, y, &element.rect);
                            match handle_mode {
                                DragMode::ResizingTopLeft | DragMode::ResizingBottomRight => {
                                    IDC_SIZENWSE
                                }
                                DragMode::ResizingTopRight | DragMode::ResizingBottomLeft => {
                                    IDC_SIZENESW
                                }
                                DragMode::ResizingTopCenter | DragMode::ResizingBottomCenter => {
                                    IDC_SIZENS
                                }
                                DragMode::ResizingMiddleLeft | DragMode::ResizingMiddleRight => {
                                    IDC_SIZEWE
                                }
                                DragMode::Moving => IDC_SIZEALL,
                                _ => IDC_IBEAM, // 在文本区域显示文本光标
                            }
                        } else {
                            IDC_ARROW // 未编辑状态下显示普通箭头光标（拖动时不显示移动光标）
                        }
                    } else {
                        IDC_SIZEALL
                    }
                } else if self.has_selection
                    && x >= self.selection_rect.left
                    && x <= self.selection_rect.right
                    && y >= self.selection_rect.top
                    && y <= self.selection_rect.bottom
                {
                    if self.current_tool == DrawingTool::Text {
                        IDC_CROSS // 文本工具在选择区域内显示十字光标
                    } else {
                        IDC_ARROW
                    }
                } else {
                    IDC_ARROW
                }
            } else if self.has_selection {
                self.get_cursor_for_position(x, y)
            } else {
                IDC_ARROW
            };

            unsafe {
                if let Ok(cursor) = LoadCursorW(Some(HINSTANCE(std::ptr::null_mut())), cursor_id) {
                    SetCursor(Some(cursor));
                }
            }
        }

        unsafe {
            let _ = InvalidateRect(Some(hwnd), None, FALSE.into());
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

        // 根据选中的元素类型决定手柄布局
        let handles = if let Some(element_index) = self.selected_element {
            if element_index < self.drawing_elements.len() {
                let element = &self.drawing_elements[element_index];
                if element.tool == DrawingTool::Text {
                    // 文本元素只使用四个对角手柄
                    vec![
                        (rect.left, rect.top, DragMode::ResizingTopLeft),
                        (rect.right, rect.top, DragMode::ResizingTopRight),
                        (rect.right, rect.bottom, DragMode::ResizingBottomRight),
                        (rect.left, rect.bottom, DragMode::ResizingBottomLeft),
                    ]
                } else {
                    // 其他元素使用8个手柄
                    let center_x = (rect.left + rect.right) / 2;
                    let center_y = (rect.top + rect.bottom) / 2;
                    vec![
                        (rect.left, rect.top, DragMode::ResizingTopLeft),
                        (center_x, rect.top, DragMode::ResizingTopCenter),
                        (rect.right, rect.top, DragMode::ResizingTopRight),
                        (rect.right, center_y, DragMode::ResizingMiddleRight),
                        (rect.right, rect.bottom, DragMode::ResizingBottomRight),
                        (center_x, rect.bottom, DragMode::ResizingBottomCenter),
                        (rect.left, rect.bottom, DragMode::ResizingBottomLeft),
                        (rect.left, center_y, DragMode::ResizingMiddleLeft),
                    ]
                }
            } else {
                // 默认使用8个手柄
                let center_x = (rect.left + rect.right) / 2;
                let center_y = (rect.top + rect.bottom) / 2;
                vec![
                    (rect.left, rect.top, DragMode::ResizingTopLeft),
                    (center_x, rect.top, DragMode::ResizingTopCenter),
                    (rect.right, rect.top, DragMode::ResizingTopRight),
                    (rect.right, center_y, DragMode::ResizingMiddleRight),
                    (rect.right, rect.bottom, DragMode::ResizingBottomRight),
                    (center_x, rect.bottom, DragMode::ResizingBottomCenter),
                    (rect.left, rect.bottom, DragMode::ResizingBottomLeft),
                    (rect.left, center_y, DragMode::ResizingMiddleLeft),
                ]
            }
        } else {
            // 默认使用8个手柄
            let center_x = (rect.left + rect.right) / 2;
            let center_y = (rect.top + rect.bottom) / 2;
            vec![
                (rect.left, rect.top, DragMode::ResizingTopLeft),
                (center_x, rect.top, DragMode::ResizingTopCenter),
                (rect.right, rect.top, DragMode::ResizingTopRight),
                (rect.right, center_y, DragMode::ResizingMiddleRight),
                (rect.right, rect.bottom, DragMode::ResizingBottomRight),
                (center_x, rect.bottom, DragMode::ResizingBottomCenter),
                (rect.left, rect.bottom, DragMode::ResizingBottomLeft),
                (rect.left, center_y, DragMode::ResizingMiddleLeft),
            ]
        };

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
        // 注释掉选择框限制，允许检测所有手柄
        // if x < self.selection_rect.left
        //     || x > self.selection_rect.right
        //     || y < self.selection_rect.top
        //     || y > self.selection_rect.bottom
        // {
        //     return DragMode::None;
        // }

        // 如果是箭头元素，只检查起点和终点
        if let Some(element_index) = self.selected_element {
            if element_index < self.drawing_elements.len() {
                let element = &self.drawing_elements[element_index];

                if element.tool == DrawingTool::Arrow && element.points.len() >= 2 {
                    let start_point = &element.points[0];
                    let end_point = &element.points[1];
                    let detection_radius = HANDLE_DETECTION_RADIUS as i32;

                    // 移除选择框限制，直接检测起点手柄
                    let dx = x - start_point.x;
                    let dy = y - start_point.y;
                    let distance_sq = dx * dx + dy * dy;
                    if distance_sq <= detection_radius * detection_radius {
                        return DragMode::ResizingTopLeft;
                    }

                    // 移除选择框限制，直接检测终点手柄
                    let dx = x - end_point.x;
                    let dy = y - end_point.y;
                    let distance_sq = dx * dx + dy * dy;
                    if distance_sq <= detection_radius * detection_radius {
                        return DragMode::ResizingBottomRight;
                    }

                    // 继续检查其他手柄，不要直接返回
                }
            }
        }

        // 检查当前选中的元素类型，决定使用哪种手柄布局
        let handles = if let Some(element_index) = self.selected_element {
            if element_index < self.drawing_elements.len() {
                let element = &self.drawing_elements[element_index];
                if element.tool == DrawingTool::Text {
                    // 文本元素只使用四个对角手柄
                    vec![
                        (rect.left, rect.top, DragMode::ResizingTopLeft),
                        (rect.right, rect.top, DragMode::ResizingTopRight),
                        (rect.right, rect.bottom, DragMode::ResizingBottomRight),
                        (rect.left, rect.bottom, DragMode::ResizingBottomLeft),
                    ]
                } else {
                    // 其他元素使用8个手柄
                    let center_x = (rect.left + rect.right) / 2;
                    let center_y = (rect.top + rect.bottom) / 2;
                    vec![
                        (rect.left, rect.top, DragMode::ResizingTopLeft),
                        (center_x, rect.top, DragMode::ResizingTopCenter),
                        (rect.right, rect.top, DragMode::ResizingTopRight),
                        (rect.right, center_y, DragMode::ResizingMiddleRight),
                        (rect.right, rect.bottom, DragMode::ResizingBottomRight),
                        (center_x, rect.bottom, DragMode::ResizingBottomCenter),
                        (rect.left, rect.bottom, DragMode::ResizingBottomLeft),
                        (rect.left, center_y, DragMode::ResizingMiddleLeft),
                    ]
                }
            } else {
                // 默认使用8个手柄
                let center_x = (rect.left + rect.right) / 2;
                let center_y = (rect.top + rect.bottom) / 2;
                vec![
                    (rect.left, rect.top, DragMode::ResizingTopLeft),
                    (center_x, rect.top, DragMode::ResizingTopCenter),
                    (rect.right, rect.top, DragMode::ResizingTopRight),
                    (rect.right, center_y, DragMode::ResizingMiddleRight),
                    (rect.right, rect.bottom, DragMode::ResizingBottomRight),
                    (center_x, rect.bottom, DragMode::ResizingBottomCenter),
                    (rect.left, rect.bottom, DragMode::ResizingBottomLeft),
                    (rect.left, center_y, DragMode::ResizingMiddleLeft),
                ]
            }
        } else {
            // 默认使用8个手柄
            let center_x = (rect.left + rect.right) / 2;
            let center_y = (rect.top + rect.bottom) / 2;
            vec![
                (rect.left, rect.top, DragMode::ResizingTopLeft),
                (center_x, rect.top, DragMode::ResizingTopCenter),
                (rect.right, rect.top, DragMode::ResizingTopRight),
                (rect.right, center_y, DragMode::ResizingMiddleRight),
                (rect.right, rect.bottom, DragMode::ResizingBottomRight),
                (center_x, rect.bottom, DragMode::ResizingBottomCenter),
                (rect.left, rect.bottom, DragMode::ResizingBottomLeft),
                (rect.left, center_y, DragMode::ResizingMiddleLeft),
            ]
        };

        let detection_radius = HANDLE_DETECTION_RADIUS as i32;

        for (hx, hy, mode) in handles.iter() {
            // 移除选择框限制，允许检测所有手柄
            let dx = x - hx;
            let dy = y - hy;
            let distance_sq = dx * dx + dy * dy;
            let radius_sq = detection_radius * detection_radius;

            if distance_sq <= radius_sq {
                return *mode;
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

            // 1. 优先检查选中元素的手柄（移除选择框限制以允许检测边界手柄）
            if let Some(element_index) = self.selected_element {
                if element_index < self.drawing_elements.len() {
                    let element = &self.drawing_elements[element_index];

                    if element.tool != DrawingTool::Pen {
                        // 直接检测元素手柄，不受选择框边界限制
                        let handle_mode = self.get_element_handle_at_position(x, y, &element.rect);

                        if handle_mode != DragMode::None {
                            return match handle_mode {
                                DragMode::ResizingTopLeft | DragMode::ResizingBottomRight => {
                                    IDC_SIZENWSE
                                }
                                DragMode::ResizingTopRight | DragMode::ResizingBottomLeft => {
                                    IDC_SIZENESW
                                }
                                DragMode::ResizingTopCenter | DragMode::ResizingBottomCenter => {
                                    IDC_SIZENS
                                }
                                DragMode::ResizingMiddleLeft | DragMode::ResizingMiddleRight => {
                                    IDC_SIZEWE
                                }
                                _ => IDC_ARROW,
                            };
                        }

                        // 只有在选择框内才检查元素内部点击
                        if x >= self.selection_rect.left
                            && x <= self.selection_rect.right
                            && y >= self.selection_rect.top
                            && y <= self.selection_rect.bottom
                            && element.contains_point(x, y)
                        {
                            // 对于文本元素，只有在编辑模式下才显示移动光标
                            if element.tool == DrawingTool::Text {
                                if self.text_editing
                                    && self.editing_element_index == Some(element_index)
                                {
                                    return IDC_SIZEALL;
                                } else {
                                    return IDC_ARROW; // 未编辑状态下显示普通箭头光标
                                }
                            } else {
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
                        // 对于文本元素，只有在编辑模式下才显示移动光标
                        if element.tool == DrawingTool::Text {
                            if self.text_editing
                                && self.editing_element_index == Some(element_index)
                            {
                                return IDC_SIZEALL;
                            } else {
                                return IDC_ARROW; // 未编辑状态下显示普通箭头光标
                            }
                        } else {
                            return IDC_SIZEALL;
                        }
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
                        } else if element.tool == DrawingTool::Text {
                            // 文本元素的特殊缩放处理
                            let mut new_rect = self.drag_start_rect;
                            let dx = x - self.drag_start_pos.x;
                            let dy = y - self.drag_start_pos.y;

                            // 计算原始尺寸
                            let original_width =
                                self.drag_start_rect.right - self.drag_start_rect.left;
                            let original_height =
                                self.drag_start_rect.bottom - self.drag_start_rect.top;

                            match self.drag_mode {
                                DragMode::ResizingTopLeft => {
                                    // 对角线缩放：按比例缩放
                                    let scale_x =
                                        (original_width - dx) as f32 / original_width as f32;
                                    let scale_y =
                                        (original_height - dy) as f32 / original_height as f32;
                                    let scale = scale_x.min(scale_y).max(0.1); // 保持比例，最小缩放0.1倍

                                    let new_width = (original_width as f32 * scale) as i32;
                                    let new_height = (original_height as f32 * scale) as i32;

                                    new_rect.left = self.drag_start_rect.right - new_width;
                                    new_rect.top = self.drag_start_rect.bottom - new_height;
                                    new_rect.right = self.drag_start_rect.right;
                                    new_rect.bottom = self.drag_start_rect.bottom;

                                    // 调整字体大小（基于原始大小）
                                    element.thickness =
                                        (self.drag_start_font_size * scale).max(8.0);
                                }
                                DragMode::ResizingTopRight => {
                                    // 对角线缩放：按比例缩放
                                    let scale_x =
                                        (original_width + dx) as f32 / original_width as f32;
                                    let scale_y =
                                        (original_height - dy) as f32 / original_height as f32;
                                    let scale = scale_x.min(scale_y).max(0.1); // 保持比例，最小缩放0.1倍

                                    let new_width = (original_width as f32 * scale) as i32;
                                    let new_height = (original_height as f32 * scale) as i32;

                                    new_rect.left = self.drag_start_rect.left;
                                    new_rect.top = self.drag_start_rect.bottom - new_height;
                                    new_rect.right = self.drag_start_rect.left + new_width;
                                    new_rect.bottom = self.drag_start_rect.bottom;

                                    // 调整字体大小（基于原始大小）
                                    element.thickness =
                                        (self.drag_start_font_size * scale).max(8.0);
                                }
                                DragMode::ResizingBottomRight => {
                                    // 对角线缩放：按比例缩放
                                    let scale_x =
                                        (original_width + dx) as f32 / original_width as f32;
                                    let scale_y =
                                        (original_height + dy) as f32 / original_height as f32;
                                    let scale = scale_x.min(scale_y).max(0.1); // 保持比例，最小缩放0.1倍

                                    let new_width = (original_width as f32 * scale) as i32;
                                    let new_height = (original_height as f32 * scale) as i32;

                                    new_rect.left = self.drag_start_rect.left;
                                    new_rect.top = self.drag_start_rect.top;
                                    new_rect.right = self.drag_start_rect.left + new_width;
                                    new_rect.bottom = self.drag_start_rect.top + new_height;

                                    // 调整字体大小（基于原始大小）
                                    element.thickness =
                                        (self.drag_start_font_size * scale).max(8.0);
                                }
                                DragMode::ResizingBottomLeft => {
                                    // 对角线缩放：按比例缩放
                                    let scale_x =
                                        (original_width - dx) as f32 / original_width as f32;
                                    let scale_y =
                                        (original_height + dy) as f32 / original_height as f32;
                                    let scale = scale_x.min(scale_y).max(0.1); // 保持比例，最小缩放0.1倍

                                    let new_width = (original_width as f32 * scale) as i32;
                                    let new_height = (original_height as f32 * scale) as i32;

                                    new_rect.left = self.drag_start_rect.right - new_width;
                                    new_rect.top = self.drag_start_rect.top;
                                    new_rect.right = self.drag_start_rect.right;
                                    new_rect.bottom = self.drag_start_rect.top + new_height;

                                    // 调整字体大小（基于原始大小）
                                    element.thickness =
                                        (self.drag_start_font_size * scale).max(8.0);
                                }
                                _ => {
                                    // 边缘手柄：只调整尺寸，不调整字体
                                    match self.drag_mode {
                                        DragMode::ResizingTopCenter => {
                                            new_rect.top += dy;
                                        }
                                        DragMode::ResizingMiddleRight => {
                                            new_rect.right += dx;
                                        }
                                        DragMode::ResizingBottomCenter => {
                                            new_rect.bottom += dy;
                                        }
                                        DragMode::ResizingMiddleLeft => {
                                            new_rect.left += dx;
                                        }
                                        _ => {}
                                    }
                                }
                            }

                            // 对于文本元素，在调整字体大小后重新计算尺寸
                            if matches!(
                                self.drag_mode,
                                DragMode::ResizingTopLeft
                                    | DragMode::ResizingTopRight
                                    | DragMode::ResizingBottomRight
                                    | DragMode::ResizingBottomLeft
                            ) {
                                // 保存锚点位置
                                let anchor_point = match self.drag_mode {
                                    DragMode::ResizingTopLeft => {
                                        (self.drag_start_rect.right, self.drag_start_rect.bottom)
                                    }
                                    DragMode::ResizingTopRight => {
                                        (self.drag_start_rect.left, self.drag_start_rect.bottom)
                                    }
                                    DragMode::ResizingBottomLeft => {
                                        (self.drag_start_rect.right, self.drag_start_rect.top)
                                    }
                                    DragMode::ResizingBottomRight => {
                                        (self.drag_start_rect.left, self.drag_start_rect.top)
                                    }
                                    _ => (self.drag_start_rect.left, self.drag_start_rect.top),
                                };

                                // 重新计算文本尺寸
                                if let Some(element_index) = self.selected_element {
                                    self.update_text_element_size(element_index);

                                    // 根据锚点调整位置
                                    let element = &mut self.drawing_elements[element_index];
                                    let width = element.rect.right - element.rect.left;
                                    let height = element.rect.bottom - element.rect.top;

                                    match self.drag_mode {
                                        DragMode::ResizingTopLeft => {
                                            element.rect.left = anchor_point.0 - width;
                                            element.rect.top = anchor_point.1 - height;
                                            element.rect.right = anchor_point.0;
                                            element.rect.bottom = anchor_point.1;
                                        }
                                        DragMode::ResizingTopRight => {
                                            element.rect.left = anchor_point.0;
                                            element.rect.top = anchor_point.1 - height;
                                            element.rect.right = anchor_point.0 + width;
                                            element.rect.bottom = anchor_point.1;
                                        }
                                        DragMode::ResizingBottomLeft => {
                                            element.rect.left = anchor_point.0 - width;
                                            element.rect.top = anchor_point.1;
                                            element.rect.right = anchor_point.0;
                                            element.rect.bottom = anchor_point.1 + height;
                                        }
                                        DragMode::ResizingBottomRight => {
                                            element.rect.left = anchor_point.0;
                                            element.rect.top = anchor_point.1;
                                            element.rect.right = anchor_point.0 + width;
                                            element.rect.bottom = anchor_point.1 + height;
                                        }
                                        _ => {}
                                    }

                                    // 更新点位置
                                    if element.points.len() >= 2 {
                                        element.points[0] = POINT {
                                            x: element.rect.left,
                                            y: element.rect.top,
                                        };
                                        element.points[1] = POINT {
                                            x: element.rect.right,
                                            y: element.rect.bottom,
                                        };
                                    }
                                }
                            } else {
                                // 边缘手柄：直接调整尺寸
                                if new_rect.right - new_rect.left >= 20
                                    && new_rect.bottom - new_rect.top >= 20
                                {
                                    element.resize(new_rect);
                                }
                            }
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
                        // 直接检查手柄，不需要 is_element_visible 限制
                        let handle_mode = self.get_element_handle_at_position(x, y, &element.rect);

                        if handle_mode != DragMode::None {
                            self.drag_mode = handle_mode;
                            self.mouse_pressed = true;
                            self.drag_start_pos = POINT { x, y };
                            self.drag_start_rect = element.rect;
                            // 保存原始字体大小
                            self.drag_start_font_size = element.thickness;
                            return;
                        }

                        // 检查是否点击了选中元素内部（移动）
                        // 但只允许在选择框内的部分被点击
                        if self.is_element_visible(element)
                            && x >= self.selection_rect.left
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

                    // 更新工具栏选中状态和当前工具以匹配选中的元素类型
                    let element_tool = self.drawing_elements[element_index].tool;
                    self.toolbar.clicked_button = match element_tool {
                        DrawingTool::Rectangle => ToolbarButton::Rectangle,
                        DrawingTool::Circle => ToolbarButton::Circle,
                        DrawingTool::Pen => ToolbarButton::Pen,
                        DrawingTool::Text => ToolbarButton::Text,
                        DrawingTool::Arrow => ToolbarButton::Arrow,
                        DrawingTool::None => ToolbarButton::Arrow, // 默认选择箭头工具
                    };

                    // 同时更新当前工具，这样后续绘画会使用选中元素的工具类型
                    self.current_tool = element_tool;

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

            // 3. 如果选择了绘图工具（除了文本工具），且在选择框内，开始绘图
            if self.current_tool != DrawingTool::None && self.current_tool != DrawingTool::Text {
                if x >= self.selection_rect.left
                    && x <= self.selection_rect.right
                    && y >= self.selection_rect.top
                    && y <= self.selection_rect.bottom
                {
                    // 清除元素选择，但保持工具栏选中状态
                    for element in &mut self.drawing_elements {
                        element.selected = false;
                    }
                    self.selected_element = None;

                    // 确保工具栏状态与当前工具保持一致
                    self.toolbar.clicked_button = match self.current_tool {
                        DrawingTool::Rectangle => ToolbarButton::Rectangle,
                        DrawingTool::Circle => ToolbarButton::Circle,
                        DrawingTool::Pen => ToolbarButton::Pen,
                        DrawingTool::Text => ToolbarButton::Text,
                        DrawingTool::Arrow => ToolbarButton::Arrow,
                        DrawingTool::None => ToolbarButton::None,
                    };

                    self.save_history();
                    self.drag_mode = DragMode::DrawingShape;
                    self.mouse_pressed = true;
                    self.drag_start_pos = POINT { x, y };

                    let mut new_element = DrawingElement::new(self.current_tool);
                    // 根据工具类型设置颜色
                    if self.current_tool == DrawingTool::Text {
                        let (_, text_color, _, _) = crate::constants::get_colors_from_settings();
                        new_element.color = text_color;
                    } else {
                        new_element.color = self.drawing_color;
                    }
                    // 文本工具使用字体大小，其他工具使用绘图线条粗细
                    new_element.thickness = if self.current_tool == DrawingTool::Text {
                        let settings = crate::simple_settings::SimpleSettings::load();
                        settings.font_size
                    } else {
                        self.drawing_thickness as f32
                    };

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
                // 检查选择框手柄（在清除选择之前）
                let handle_mode = self.get_handle_at_position(x, y);

                // 只有在没有检测到手柄时才清除元素选择
                if handle_mode == DragMode::None {
                    // 清除元素选择
                    for element in &mut self.drawing_elements {
                        element.selected = false;
                    }
                    self.selected_element = None;
                }

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

    // 文字输入相关方法
    pub fn start_text_editing(&mut self, element_index: usize, hwnd: HWND) {
        // 清除其他元素的选中状态
        for element in &mut self.drawing_elements {
            element.selected = false;
        }

        // 选中当前文字元素
        self.selected_element = Some(element_index);
        self.drawing_elements[element_index].selected = true;

        // 更新工具栏状态
        self.toolbar.clicked_button = ToolbarButton::Text;
        self.current_tool = DrawingTool::Text;

        // 开始文字编辑模式
        self.text_editing = true;
        self.editing_element_index = Some(element_index);
        self.text_cursor_pos = self.drawing_elements[element_index].text.chars().count();
        self.text_cursor_visible = true;

        unsafe {
            SetCapture(hwnd);
            // 启动光标闪烁定时器，每500毫秒闪烁一次
            let _ = SetTimer(Some(hwnd), self.cursor_timer_id, 500, None);
            let _ = InvalidateRect(Some(hwnd), None, FALSE.into());
        }
    }

    pub fn create_and_edit_text_element(&mut self, x: i32, y: i32, hwnd: HWND) {
        // 清除所有元素的选择状态
        for element in &mut self.drawing_elements {
            element.selected = false;
        }
        self.selected_element = None;

        // 确保工具栏状态与当前工具保持一致
        self.toolbar.clicked_button = ToolbarButton::Text;

        self.save_history();

        // 创建新的文字元素
        let mut text_element = DrawingElement::new(DrawingTool::Text);
        text_element.points.push(POINT { x, y });
        // 文字元素使用文字颜色
        let (_, text_color, _, _) = crate::constants::get_colors_from_settings();
        text_element.color = text_color;
        // 文字元素使用设置中的字体大小
        let settings = crate::simple_settings::SimpleSettings::load();
        text_element.thickness = settings.font_size;
        text_element.text = String::new(); // 空文本，等待用户输入
        text_element.selected = true;

        // 根据字体大小设置初始文本框尺寸
        let font_size = settings.font_size;
        let dynamic_line_height = (font_size * 1.2) as i32;
        let initial_width = (font_size * 6.0) as i32; // 大约6个字符的宽度
        let initial_height = dynamic_line_height + (crate::constants::TEXT_PADDING * 2.0) as i32;

        // 设置第二个点来定义文本框尺寸
        text_element.points.push(POINT {
            x: x + initial_width,
            y: y + initial_height,
        });

        // 更新边界矩形
        text_element.update_bounding_rect();

        // 添加到绘图元素列表
        let element_index = self.drawing_elements.len();
        self.drawing_elements.push(text_element);

        // 开始编辑新创建的文字元素
        self.start_text_editing(element_index, hwnd);
    }

    pub fn stop_text_editing(&mut self, hwnd: HWND) {
        if self.text_editing {
            // 立即隐藏光标，确保保存时光标不可见
            self.text_cursor_visible = false;

            // 先停止编辑状态，再检查是否需要删除空元素
            self.text_editing = false;
            let editing_index = self.editing_element_index;
            self.editing_element_index = None;
            self.text_cursor_pos = 0;

            // 保存当前工具状态，确保在保存文本后保持文本工具
            self.current_tool = DrawingTool::Text;
            self.toolbar.clicked_button = ToolbarButton::Text;

            // 设置标志，防止立即创建新的文本元素
            self.just_saved_text = true;

            // 检查当前编辑的文本元素是否为空，如果为空则删除
            if let Some(element_index) = editing_index {
                if element_index < self.drawing_elements.len() {
                    let should_delete = {
                        let element = &self.drawing_elements[element_index];
                        // 如果文本为空或只包含空白字符，删除该元素（空的输入框不需要保存）
                        element.text.trim().is_empty()
                    };

                    if should_delete {
                        self.drawing_elements.remove(element_index);

                        // 更新选中元素索引
                        if let Some(selected) = self.selected_element {
                            if selected == element_index {
                                self.selected_element = None;
                            } else if selected > element_index {
                                self.selected_element = Some(selected - 1);
                            }
                        }

                        // 清除所有元素的选中状态
                        for element in &mut self.drawing_elements {
                            element.selected = false;
                        }
                    }
                }
            }

            unsafe {
                // 停止光标闪烁定时器
                let _ = KillTimer(Some(hwnd), self.cursor_timer_id);
                // 释放鼠标捕获
                let _ = ReleaseCapture();
                // 立即刷新界面以确保光标被隐藏
                let _ = InvalidateRect(Some(hwnd), None, FALSE.into());
            }

            // 强制确保工具状态保持为文本工具，防止被其他逻辑重置
            self.current_tool = DrawingTool::Text;
            self.toolbar.clicked_button = ToolbarButton::Text;

            // 清除选中状态，这样保存文本后就不会进入手柄检查逻辑
            self.selected_element = None;
            for element in &mut self.drawing_elements {
                element.selected = false;
            }
        }
    }

    // 处理光标闪烁定时器
    pub fn handle_cursor_timer(&mut self, hwnd: HWND) {
        if self.text_editing {
            // 切换光标可见性
            self.text_cursor_visible = !self.text_cursor_visible;
            unsafe {
                let _ = InvalidateRect(Some(hwnd), None, FALSE.into());
            }
        }
    }

    pub fn handle_text_input(&mut self, character: char, hwnd: HWND) {
        if !self.text_editing {
            return;
        }

        if let Some(element_index) = self.editing_element_index {
            if element_index < self.drawing_elements.len() {
                let element = &mut self.drawing_elements[element_index];

                // 在光标位置插入字符
                let char_count = element.text.chars().count();
                if self.text_cursor_pos <= char_count {
                    // 将字符索引转换为字节索引
                    let byte_pos = element
                        .text
                        .char_indices()
                        .nth(self.text_cursor_pos)
                        .map(|(i, _)| i)
                        .unwrap_or(element.text.len());
                    element.text.insert(byte_pos, character);
                    self.text_cursor_pos += 1;

                    // 动态调整文字框大小
                    self.update_text_element_size(element_index);

                    unsafe {
                        let _ = InvalidateRect(Some(hwnd), None, FALSE.into());
                    }
                }
            }
        }
    }

    pub fn handle_backspace(&mut self, hwnd: HWND) {
        if !self.text_editing {
            return;
        }

        if let Some(element_index) = self.editing_element_index {
            if element_index < self.drawing_elements.len() {
                let element = &mut self.drawing_elements[element_index];

                // 删除光标前的字符
                if self.text_cursor_pos > 0 && !element.text.is_empty() {
                    self.text_cursor_pos -= 1;
                    // 将字符索引转换为字节索引
                    let byte_pos = element
                        .text
                        .char_indices()
                        .nth(self.text_cursor_pos)
                        .map(|(i, _)| i)
                        .unwrap_or(element.text.len());
                    element.text.remove(byte_pos);

                    // 动态调整文字框大小
                    self.update_text_element_size(element_index);

                    unsafe {
                        let _ = InvalidateRect(Some(hwnd), None, FALSE.into());
                    }
                }
            }
        }
    }

    // 动态调整文字元素大小
    pub fn update_text_element_size(&mut self, element_index: usize) {
        if element_index >= self.drawing_elements.len() {
            return;
        }

        // 先获取文字内容、起始点和字体大小，避免借用冲突
        let (text_content, start_point, font_size) = {
            let element = &self.drawing_elements[element_index];
            if element.tool != DrawingTool::Text || element.points.is_empty() {
                return;
            }
            (element.text.clone(), element.points[0], element.thickness)
        };

        // 计算文字的实际尺寸
        let lines: Vec<&str> = if text_content.is_empty() {
            vec![""] // 空文本时显示一个空行
        } else {
            text_content.lines().collect()
        };

        // 正确计算行数：如果文本以换行符结尾，增加一行
        let line_count = if text_content.is_empty() {
            1 // 空文本显示一行
        } else if text_content.ends_with('\n') {
            lines.len() + 1 // 以换行符结尾时增加一行
        } else {
            lines.len() // 正常情况下的行数
        };

        // 使用精确测量计算最长行的宽度，考虑字体大小
        let max_line_width = if text_content.is_empty() {
            MIN_TEXT_WIDTH as f32
        } else {
            let mut max_width = 0.0f32;
            for line in &lines {
                if let Ok((width, _)) =
                    self.measure_text_precise_with_font_size(line, f32::MAX, font_size)
                {
                    max_width = max_width.max(width);
                }
            }
            // 如果测量失败，使用最小宽度
            if max_width == 0.0 {
                MIN_TEXT_WIDTH as f32
            } else {
                // 增加适当的缓冲空间以确保字符不会被挤压
                // DirectWrite已经给出精确宽度，增加更多缓冲以适应不同字体大小
                max_width + (font_size * 0.2).max(4.0)
            }
        };

        // 计算新的尺寸，使用向上取整确保不丢失精度
        // 移除最大宽度限制，允许文本框无限延长
        // 使用动态行高，基于字体大小
        let dynamic_line_height = (font_size * 1.2) as i32;
        let new_width = ((max_line_width + TEXT_PADDING * 2.0).ceil() as i32).max(MIN_TEXT_WIDTH);
        let new_height = (line_count as i32 * dynamic_line_height + (TEXT_PADDING * 2.0) as i32)
            .max(MIN_TEXT_HEIGHT);

        // 现在可以安全地修改元素
        let element = &mut self.drawing_elements[element_index];

        // 更新文字框的第二个点（如果存在）或创建第二个点
        if element.points.len() >= 2 {
            element.points[1] = POINT {
                x: start_point.x + new_width,
                y: start_point.y + new_height,
            };
        } else {
            element.points.push(POINT {
                x: start_point.x + new_width,
                y: start_point.y + new_height,
            });
        }

        // 更新边界矩形
        element.update_bounding_rect();
    }

    // 光标移动方法
    pub fn move_cursor_left(&mut self, hwnd: HWND) {
        if self.text_cursor_pos > 0 {
            self.text_cursor_pos -= 1;
            unsafe {
                let _ = InvalidateRect(Some(hwnd), None, FALSE.into());
            }
        }
    }

    pub fn move_cursor_right(&mut self, hwnd: HWND) {
        if let Some(element_index) = self.editing_element_index {
            if element_index < self.drawing_elements.len() {
                let element = &self.drawing_elements[element_index];
                let char_count = element.text.chars().count();
                if self.text_cursor_pos < char_count {
                    self.text_cursor_pos += 1;
                    unsafe {
                        let _ = InvalidateRect(Some(hwnd), None, FALSE.into());
                    }
                }
            }
        }
    }

    pub fn move_cursor_to_line_start(&mut self, hwnd: HWND) {
        if let Some(element_index) = self.editing_element_index {
            if element_index < self.drawing_elements.len() {
                let element = &self.drawing_elements[element_index];

                // 找到当前光标所在行的开始位置
                let text_before_cursor = element
                    .text
                    .chars()
                    .take(self.text_cursor_pos)
                    .collect::<String>();
                if let Some(last_newline_pos) = text_before_cursor.rfind('\n') {
                    // 如果找到换行符，光标移动到换行符后的位置
                    self.text_cursor_pos = last_newline_pos + 1;
                } else {
                    // 如果没有换行符，移动到文本开始
                    self.text_cursor_pos = 0;
                }

                unsafe {
                    let _ = InvalidateRect(Some(hwnd), None, FALSE.into());
                }
            }
        }
    }

    pub fn move_cursor_to_line_end(&mut self, hwnd: HWND) {
        if let Some(element_index) = self.editing_element_index {
            if element_index < self.drawing_elements.len() {
                let element = &self.drawing_elements[element_index];

                // 找到当前光标所在行的结束位置
                let text_after_cursor = element
                    .text
                    .chars()
                    .skip(self.text_cursor_pos)
                    .collect::<String>();
                if let Some(next_newline_pos) = text_after_cursor.find('\n') {
                    // 如果找到换行符，光标移动到换行符前
                    self.text_cursor_pos += next_newline_pos;
                } else {
                    // 如果没有换行符，移动到文本结尾
                    self.text_cursor_pos = element.text.chars().count();
                }

                unsafe {
                    let _ = InvalidateRect(Some(hwnd), None, FALSE.into());
                }
            }
        }
    }

    pub fn move_cursor_up(&mut self, hwnd: HWND) {
        if let Some(element_index) = self.editing_element_index {
            if element_index < self.drawing_elements.len() {
                let element = &self.drawing_elements[element_index];

                // 找到当前光标所在行和列
                let text_before_cursor = element
                    .text
                    .chars()
                    .take(self.text_cursor_pos)
                    .collect::<String>();
                let lines_before: Vec<&str> = text_before_cursor.lines().collect();

                if lines_before.len() > 1 {
                    // 不是第一行，可以向上移动
                    let current_line_start = if text_before_cursor.ends_with('\n') {
                        lines_before.len()
                    } else {
                        lines_before.len() - 1
                    };

                    if current_line_start > 0 {
                        let current_line_text = if text_before_cursor.ends_with('\n') {
                            ""
                        } else {
                            lines_before.last().map_or("", |&line| line)
                        };
                        let current_column = current_line_text.chars().count();

                        // 获取上一行的文本
                        let prev_line_text = lines_before[current_line_start - 1];
                        let prev_line_length = prev_line_text.chars().count();

                        // 计算新的光标位置
                        let target_column = current_column.min(prev_line_length);
                        let chars_before_prev_line: usize = lines_before[..current_line_start - 1]
                            .iter()
                            .map(|line| line.chars().count() + 1) // +1 for newline
                            .sum();

                        self.text_cursor_pos = chars_before_prev_line + target_column;

                        unsafe {
                            let _ = InvalidateRect(Some(hwnd), None, FALSE.into());
                        }
                    }
                }
            }
        }
    }

    pub fn move_cursor_down(&mut self, hwnd: HWND) {
        if let Some(element_index) = self.editing_element_index {
            if element_index < self.drawing_elements.len() {
                let element = &self.drawing_elements[element_index];

                // 找到当前光标所在行和列
                let text_before_cursor = element
                    .text
                    .chars()
                    .take(self.text_cursor_pos)
                    .collect::<String>();
                let text_after_cursor = element
                    .text
                    .chars()
                    .skip(self.text_cursor_pos)
                    .collect::<String>();

                if let Some(next_newline_pos) = text_after_cursor.find('\n') {
                    // 有下一行，可以向下移动
                    let lines_before: Vec<&str> = text_before_cursor.lines().collect();
                    let current_line_text = if text_before_cursor.ends_with('\n') {
                        ""
                    } else {
                        lines_before.last().map_or("", |&line| line)
                    };
                    let current_column = current_line_text.chars().count();

                    // 获取下一行的文本
                    let text_from_next_line = &text_after_cursor[next_newline_pos + 1..];
                    let next_line_text = if let Some(end_pos) = text_from_next_line.find('\n') {
                        &text_from_next_line[..end_pos]
                    } else {
                        text_from_next_line
                    };
                    let next_line_length = next_line_text.chars().count();

                    // 计算新的光标位置
                    let target_column = current_column.min(next_line_length);
                    self.text_cursor_pos =
                        self.text_cursor_pos + next_newline_pos + 1 + target_column;

                    unsafe {
                        let _ = InvalidateRect(Some(hwnd), None, FALSE.into());
                    }
                }
            }
        }
    }

    /// 切换自动窗口高亮功能
    pub fn toggle_auto_highlight(&mut self) {
        self.auto_highlight_enabled = !self.auto_highlight_enabled;

        // 如果禁用自动高亮，清除当前的自动高亮选择
        if !self.auto_highlight_enabled && self.has_selection {
            self.has_selection = false;
        }
    }

    /// 获取自动高亮状态
    pub fn is_auto_highlight_enabled(&self) -> bool {
        self.auto_highlight_enabled
    }
}
