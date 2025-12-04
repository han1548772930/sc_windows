use crate::message::Command;
use crate::types::{DrawingElement, DrawingTool};

use super::{DrawingError, DrawingManager};

impl DrawingManager {
    /// 使用Direct2D渲染文本元素（从原始代码完整迁移，支持多行、内边距、光标）
    pub(super) fn draw_text_element_d2d(
        &self,
        element: &DrawingElement,
        render_target: &windows::Win32::Graphics::Direct2D::ID2D1RenderTarget,
        d2d_renderer: &crate::platform::windows::d2d::Direct2DRenderer,
    ) -> Result<(), DrawingError> {
        if element.points.is_empty() {
            return Ok(());
        }

        // Create brush directly from the passed render_target
        let d2d_color = windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F {
            r: element.color.r,
            g: element.color.g,
            b: element.color.b,
            a: element.color.a,
        };
        
        let brush = unsafe {
            render_target.CreateSolidColorBrush(&d2d_color, None)
                .map_err(|e| DrawingError::RenderError(format!("Failed to create brush: {e:?}")))?
        };
        
        let cursor_brush = if self.text_editing && self.text_cursor_visible {
             let cursor_color = windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F {
                r: crate::constants::COLOR_TEXT_CURSOR.r,
                g: crate::constants::COLOR_TEXT_CURSOR.g,
                b: crate::constants::COLOR_TEXT_CURSOR.b,
                a: crate::constants::COLOR_TEXT_CURSOR.a,
            };
             unsafe {
                 Some(render_target.CreateSolidColorBrush(&cursor_color, None)
                    .map_err(|e| DrawingError::RenderError(format!("Failed to create cursor brush: {e:?}")))?)
             }
        } else {
             None
        };

        if let Some(dwrite_factory) = &d2d_renderer.dwrite_factory
        {
            unsafe {
                // 计算文本区域（从原始代码迁移）
                let text_rect = if element.points.len() >= 2 {
                    crate::utils::d2d_rect_normalized(
                        element.points[0].x,
                        element.points[0].y,
                        element.points[1].x,
                        element.points[1].y,
                    )
                } else if !element.points.is_empty() {
                    // 如果只有一个点，使用默认大小
                    crate::utils::d2d_rect(
                        element.points[0].x,
                        element.points[0].y,
                        element.points[0].x + crate::constants::DEFAULT_TEXT_WIDTH,
                        element.points[0].y + crate::constants::DEFAULT_TEXT_HEIGHT,
                    )
                } else {
                    return Ok(());
                };

                // 添加内边距
                let text_content_rect = windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F {
                    left: text_rect.left + crate::constants::TEXT_PADDING,
                    top: text_rect.top + crate::constants::TEXT_PADDING,
                    right: text_rect.right - crate::constants::TEXT_PADDING,
                    bottom: text_rect.bottom - crate::constants::TEXT_PADDING,
                };
                
                // Use cached layout or create new one
                let mut cached_layout = element.text_layout.borrow_mut();
                
                if cached_layout.is_none() {
                    // 使用辅助函数创建文本格式
                    if let Ok(text_format) = crate::utils::d2d_helpers::create_text_format_from_element(
                        dwrite_factory,
                        &element.font_name,
                        element.font_size,
                        element.font_weight,
                        element.font_italic,
                    ) {
                        // 使用辅助函数创建带样式的文本布局
                        if let Ok(layout) = crate::utils::d2d_helpers::create_text_layout_with_style(
                            dwrite_factory,
                            &text_format,
                            &element.text,
                            text_content_rect.right - text_content_rect.left,
                            text_content_rect.bottom - text_content_rect.top,
                            element.font_underline,
                            element.font_strikeout,
                        ) {
                            *cached_layout = Some(layout);
                        }
                    }
                }
                
                if let Some(layout) = cached_layout.as_ref() {
                    render_target.DrawTextLayout(
                        crate::utils::d2d_point(text_content_rect.left as i32, text_content_rect.top as i32),
                        layout,
                        &brush,
                        windows::Win32::Graphics::Direct2D::D2D1_DRAW_TEXT_OPTIONS_NONE
                    );

                    if self.text_editing && self.text_cursor_visible
                         && let Some(edit_idx) = self.editing_element_index
                            && let Some(current_idx) = self.elements.get_elements().iter().position(|e| std::ptr::eq(e, element))
                                && current_idx == edit_idx {
                                     self.draw_text_cursor_optimized(
                                         element,
                                         render_target,
                                         layout,
                                         &text_content_rect,
                                         cursor_brush.as_ref()
                                     )?;
                                }
                }
            }
        }
        Ok(())
    }

    /// 使用Direct2D渲染文本元素（带坐标偏移，用于离屏合成，不绘制光标）
    pub(super) fn draw_text_element_d2d_with_offset(
        &self,
        element: &DrawingElement,
        render_target: &windows::Win32::Graphics::Direct2D::ID2D1RenderTarget,
        d2d_renderer: &crate::platform::windows::d2d::Direct2DRenderer,
        offset_x: f32,
        offset_y: f32,
    ) -> Result<(), DrawingError> {
        if element.points.is_empty() {
            return Ok(());
        }

        // 创建画笔
        let d2d_color = windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F {
            r: element.color.r,
            g: element.color.g,
            b: element.color.b,
            a: element.color.a,
        };

        let brush = unsafe {
            render_target
                .CreateSolidColorBrush(&d2d_color, None)
                .map_err(|e| DrawingError::RenderError(format!("Failed to create brush: {e:?}")))?    
        };

        if let Some(dwrite_factory) = &d2d_renderer.dwrite_factory {
            unsafe {
                // 计算文本区域（应用偏移）
                let text_rect = if element.points.len() >= 2 {
                    windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F {
                        left: (element.points[0].x as f32 - offset_x).min(element.points[1].x as f32 - offset_x),
                        top: (element.points[0].y as f32 - offset_y).min(element.points[1].y as f32 - offset_y),
                        right: (element.points[0].x as f32 - offset_x).max(element.points[1].x as f32 - offset_x),
                        bottom: (element.points[0].y as f32 - offset_y).max(element.points[1].y as f32 - offset_y),
                    }
                } else if !element.points.is_empty() {
                    windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F {
                        left: element.points[0].x as f32 - offset_x,
                        top: element.points[0].y as f32 - offset_y,
                        right: element.points[0].x as f32 - offset_x + crate::constants::DEFAULT_TEXT_WIDTH as f32,
                        bottom: element.points[0].y as f32 - offset_y + crate::constants::DEFAULT_TEXT_HEIGHT as f32,
                    }
                } else {
                    return Ok(());
                };

                // 添加内边距
                let text_content_rect = windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F {
                    left: text_rect.left + crate::constants::TEXT_PADDING,
                    top: text_rect.top + crate::constants::TEXT_PADDING,
                    right: text_rect.right - crate::constants::TEXT_PADDING,
                    bottom: text_rect.bottom - crate::constants::TEXT_PADDING,
                };

                // 使用辅助函数创建文本格式
                if let Ok(text_format) = crate::utils::d2d_helpers::create_text_format_from_element(
                    dwrite_factory,
                    &element.font_name,
                    element.font_size,
                    element.font_weight,
                    element.font_italic,
                ) {
                    // 使用辅助函数创建带样式的文本布局
                    if let Ok(layout) = crate::utils::d2d_helpers::create_text_layout_with_style(
                        dwrite_factory,
                        &text_format,
                        &element.text,
                        text_content_rect.right - text_content_rect.left,
                        text_content_rect.bottom - text_content_rect.top,
                        element.font_underline,
                        element.font_strikeout,
                    ) {
                        render_target.DrawTextLayout(
                            crate::utils::d2d_helpers::d2d_point_f(
                                text_content_rect.left,
                                text_content_rect.top,
                            ),
                            &layout,
                            &brush,
                            windows::Win32::Graphics::Direct2D::D2D1_DRAW_TEXT_OPTIONS_NONE,
                        );
                    }
                }
            }
        }
        Ok(())
    }

    /// 绘制文本光标（优化版：使用 DirectWrite HitTest）
    fn draw_text_cursor_optimized(
        &self,
        _element: &DrawingElement,
        render_target: &windows::Win32::Graphics::Direct2D::ID2D1RenderTarget,
        layout: &windows::Win32::Graphics::DirectWrite::IDWriteTextLayout,
        text_content_rect: &windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F,
        cursor_brush: Option<&windows::Win32::Graphics::Direct2D::ID2D1SolidColorBrush>,
    ) -> Result<(), DrawingError> {
        let mut point_x = 0.0f32;
        let mut point_y = 0.0f32;
        let mut metrics = windows::Win32::Graphics::DirectWrite::DWRITE_HIT_TEST_METRICS::default();
        
        unsafe {
            let font_size = _element.get_effective_font_size();
            // 与 update_text_element_size 中的行高系数保持一致
            let line_height = font_size * crate::constants::TEXT_LINE_HEIGHT_SCALE;
            
            // Convert text_cursor_pos (char index) to UTF-16 index for DirectWrite
            let text_utf16: Vec<u16> = _element.text.encode_utf16().collect();
            let utf16_len = text_utf16.len();
            
            // Calculate UTF-16 offset corresponding to text_cursor_pos
            let utf16_pos = _element.text.chars().take(self.text_cursor_pos).map(|c| c.len_utf16()).sum::<usize>();
            
            // 检查光标是否在换行符之后（需要特殊处理）
            let text_before_cursor: String = _element.text.chars().take(self.text_cursor_pos).collect();
            let cursor_after_newline = text_before_cursor.ends_with('\n');
            
            if utf16_len == 0 {
                // 空文本：光标在起始位置
                point_x = 0.0;
                point_y = 0.0;
                metrics.height = line_height;
            } else if cursor_after_newline {
                // 光标在换行符之后：手动计算新行的位置
                // 计算光标之前有多少行
                let lines_before: Vec<&str> = text_before_cursor.lines().collect();
                let line_count = if text_before_cursor.ends_with('\n') {
                    lines_before.len() // 换行符后是新的一行
                } else {
                    lines_before.len().saturating_sub(1)
                };
                
                point_x = 0.0; // 新行从行首开始
                point_y = line_count as f32 * line_height;
                metrics.height = line_height;
            } else if utf16_pos >= utf16_len {
                // 光标在文本末尾（但不是在换行符之后）
                let _ = layout.HitTestTextPosition(
                    (utf16_len - 1) as u32,
                    true,
                    &mut point_x,
                    &mut point_y,
                    &mut metrics
                );
            } else {
                // 光标在文本中间
                let _ = layout.HitTestTextPosition(
                    utf16_pos as u32,
                    false,
                    &mut point_x,
                    &mut point_y,
                    &mut metrics
                );
            }
            
            // 确保 metrics.height 有效
            if metrics.height <= 0.0 {
                metrics.height = line_height;
            }
            
            let abs_x = text_content_rect.left + point_x;
            let abs_y = text_content_rect.top + point_y;
            
            let cursor_rect = windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F {
                left: abs_x,
                top: abs_y,
                right: abs_x + crate::constants::TEXT_CURSOR_WIDTH,
                bottom: abs_y + metrics.height,
            };
             
            if let Some(brush) = cursor_brush {
                render_target.FillRectangle(&cursor_rect, brush);
            }
        }
        Ok(())
    }

    // ===== 文本编辑相关方法（从原始代码迁移） =====

    /// 获取指定位置的文本元素索引
    pub(super) fn get_text_element_at_position(&self, x: i32, y: i32) -> Option<usize> {
        for (index, element) in self.elements.get_elements().iter().enumerate() {
            if element.tool == DrawingTool::Text
                && x >= element.rect.left
                && x <= element.rect.right
                && y >= element.rect.top
                && y <= element.rect.bottom
            {
                return Some(index);
            }
        }
        None
    }

    /// 开始文本编辑模式
    pub(super) fn start_text_editing(&mut self, element_index: usize) -> Vec<Command> {
        // 清除其他元素的选中状态并选中当前文字元素（统一通过ElementManager）
        self.elements.set_selected(None);
        self.selected_element = Some(element_index);
        self.elements.set_selected(self.selected_element);

        // 开始文字编辑模式
        self.text_editing = true;
        self.editing_element_index = Some(element_index);
        if let Some(el) = self.elements.get_elements().get(element_index) {
            self.text_cursor_pos = el.text.chars().count();
        } else {
            self.text_cursor_pos = 0;
        }
        self.text_cursor_visible = true;

        vec![
            Command::StartTimer(self.cursor_timer_id as u32, 500), // 启动光标闪烁定时器
            Command::RequestRedraw,
        ]
    }

    /// 创建新文本元素并开始编辑
    pub(super) fn create_and_edit_text_element(&mut self, x: i32, y: i32) -> (Vec<Command>, bool) {
        // 清除所有元素的选择状态
        self.elements.set_selected(None);
        self.selected_element = None;

        // 确保工具栏状态与当前工具保持一致（与原始代码一致）
        self.current_tool = DrawingTool::Text;

        // 命令模式：不在此处保存历史，而是在 stop_text_editing 时
        // 如果文本非空则记录 AddElement 操作

        // 创建新的文字元素
        let mut text_element = DrawingElement::new(DrawingTool::Text);
        text_element
            .points
            .push(windows::Win32::Foundation::POINT { x, y });

        // 使用设置中的字体大小、颜色和样式（仅在创建时读取一次并保存到元素上）
        let settings = crate::settings::Settings::load();
        text_element.color = windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F {
            r: settings.font_color.0 as f32 / 255.0,
            g: settings.font_color.1 as f32 / 255.0,
            b: settings.font_color.2 as f32 / 255.0,
            a: 1.0,
        };
        text_element.font_size = settings.font_size;
        text_element.font_name = settings.font_name.clone();
        text_element.font_weight = settings.font_weight;
        text_element.font_italic = settings.font_italic;
        text_element.font_underline = settings.font_underline;
        text_element.font_strikeout = settings.font_strikeout;
        text_element.text = String::new(); // 空文本，等待用户输入
        text_element.selected = true;

        // 根据字体大小动态计算初始文本框尺寸
        let font_size = text_element.font_size;
        // 与 update_text_element_size 和 draw_text_cursor_optimized 保持一致的行高系数
        let dynamic_line_height = (font_size * crate::constants::TEXT_LINE_HEIGHT_SCALE) as i32;
        let initial_width = (font_size * 6.0) as i32; // 大约6个字符的宽度
        let initial_height = dynamic_line_height + (crate::constants::TEXT_PADDING * 2.0) as i32;

        // 设置第二个点来定义文本框尺寸（与旧代码保持一致）
        text_element.points.push(windows::Win32::Foundation::POINT {
            x: x + initial_width,
            y: y + initial_height,
        });

        // 更新边界矩形
        text_element.update_bounding_rect();

        // 通过 ElementManager 添加，并以其索引作为编辑目标
        self.elements.add_element(text_element);
        let element_index = self.elements.count().saturating_sub(1);

        // 开始编辑模式
        self.text_editing = true;
        self.editing_element_index = Some(element_index);
        self.text_cursor_pos = 0;
        self.text_cursor_visible = true;
        self.selected_element = Some(element_index);
        self.elements.set_selected(self.selected_element);

        (
            vec![
                Command::StartTimer(self.cursor_timer_id as u32, 500), // 启动光标闪烁定时器
                Command::RequestRedraw,
            ],
            true,
        )
    }

    /// 停止文本编辑模式
    pub(super) fn stop_text_editing(&mut self) -> Vec<Command> {
        if !self.text_editing {
            return vec![];
        }

        // 立即隐藏光标，确保保存时光标不可见
        self.text_cursor_visible = false;

        // 先停止编辑状态，再检查是否需要删除空元素
        self.text_editing = false;
        let editing_index = self.editing_element_index;
        self.editing_element_index = None;
        self.text_cursor_pos = 0;

        // 保存当前工具状态，确保在保存文本后保持文本工具（与原始代码一致）
        self.current_tool = DrawingTool::Text;

        // 检查当前编辑的文本元素是否为空，如果为空则删除
        if let Some(element_index) = editing_index
            && let Some(element) = self.elements.get_elements().get(element_index).cloned() {
                let should_delete = element.text.trim().is_empty();

                if should_delete {
                    // 删除空元素（不记录到历史，因为这是取消创建操作）
                    let _ = self.elements.remove_element(element_index);

                    // 更新选中元素索引（与原始代码逻辑一致）
                    if let Some(selected) = self.selected_element {
                        if selected == element_index {
                            self.selected_element = None;
                        } else if selected > element_index {
                            self.selected_element = Some(selected - 1);
                        }
                    }
                } else {
                    // 文本非空，记录 AddElement 操作到历史
                    let action = super::history::DrawingAction::AddElement {
                        element,
                        index: element_index,
                    };
                    self.history.record_action(
                        action,
                        None, // 创建前无选中
                        None, // 创建后清除选中
                    );
                }
            }

        // 强制确保工具状态保持为文本工具，防止被其他逻辑重置（与原始代码一致）
        self.current_tool = DrawingTool::Text;

        // 设置标志，防止立即创建新的文本元素（与原代码保持一致）
        self.just_saved_text = true;

        // 清除选中状态，这样保存文本后就不会进入手柄检查逻辑（与原始代码一致）
        self.selected_element = None;
        self.elements.set_selected(None);

        vec![
            Command::StopTimer(self.cursor_timer_id as u32), // 停止光标闪烁定时器
            Command::UpdateToolbar, // 更新工具栏状态（启用撤回按钮）
            Command::RequestRedraw,
        ]
    }

    /// 处理文本输入（从原始代码迁移）
    pub fn handle_text_input(&mut self, character: char) -> Vec<Command> {
        if !self.text_editing {
            return vec![];
        }

        if let Some(element_index) = self.editing_element_index
            && let Some(element) = self.elements.get_element_mut(element_index) {
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

                    return vec![Command::RequestRedraw];
                }
            }
        vec![]
    }

    /// 处理光标定时器（优化：只重绘光标区域）
    pub fn handle_cursor_timer(&mut self, timer_id: u32) -> Vec<Command> {
        if self.text_editing && timer_id == self.cursor_timer_id as u32 {
            // 切换光标可见性
            self.text_cursor_visible = !self.text_cursor_visible;
            
            // 脏矩形优化：只重绘光标所在的文本元素区域
            if let Some(element_index) = self.editing_element_index
                && let Some(element) = self.elements.get_elements().get(element_index) {
                    // 计算光标区域（稍微扩大以确保完整重绘）
                    let cursor_margin = 5;
                    let dirty_rect = windows::Win32::Foundation::RECT {
                        left: element.rect.left - cursor_margin,
                        top: element.rect.top - cursor_margin,
                        right: element.rect.right + cursor_margin,
                        bottom: element.rect.bottom + cursor_margin,
                    };
                    return vec![Command::RequestRedrawRect(dirty_rect)];
                }
            
            // 回退到全屏重绘
            vec![Command::RequestRedraw]
        } else {
            vec![]
        }
    }

    /// 处理退格键
    pub(super) fn handle_backspace(&mut self) -> Vec<Command> {
        if !self.text_editing {
            return vec![];
        }

        if let Some(element_index) = self.editing_element_index
            && self.text_cursor_pos > 0
                && let Some(element) = self.elements.get_element_mut(element_index) {
                    // 删除光标前的字符
                    let char_count = element.text.chars().count();
                    if self.text_cursor_pos <= char_count {
                        let chars: Vec<char> = element.text.chars().collect();
                        if self.text_cursor_pos > 0 {
                            chars
                                .iter()
                                .take(self.text_cursor_pos - 1)
                                .chain(chars.iter().skip(self.text_cursor_pos))
                                .collect::<String>()
                                .clone_into(&mut element.text);
                            self.text_cursor_pos -= 1;
                        }
                    }

                    // 动态调整文字框大小
                    self.update_text_element_size(element_index);

                    return vec![Command::RequestRedraw];
                }
        vec![]
    }

    /// 光标向左移动
    pub(super) fn move_cursor_left(&mut self) -> Vec<Command> {
        if self.text_cursor_pos > 0 {
            self.text_cursor_pos -= 1;
            vec![Command::RequestRedraw]
        } else {
            vec![]
        }
    }

    /// 光标向右移动
    pub(super) fn move_cursor_right(&mut self) -> Vec<Command> {
        if let Some(element_index) = self.editing_element_index
            && let Some(el) = self.elements.get_elements().get(element_index) {
                let char_count = el.text.chars().count();
                if self.text_cursor_pos < char_count {
                    self.text_cursor_pos += 1;
                    return vec![Command::RequestRedraw];
                }
            }
        vec![]
    }

    /// 光标移动到行首（准确到当前行）
    pub(super) fn move_cursor_to_line_start(&mut self) -> Vec<Command> {
        if let Some(element_index) = self.editing_element_index
            && let Some(el) = self.elements.get_elements().get(element_index) {
                let before = el
                    .text
                    .chars()
                    .take(self.text_cursor_pos)
                    .collect::<String>();
                if let Some(last_nl) = before.rfind('\n') {
                    self.text_cursor_pos = last_nl + 1;
                } else {
                    self.text_cursor_pos = 0;
                }
                return vec![Command::RequestRedraw];
            }
        vec![]
    }

    /// 光标移动到行尾（准确到当前行）
    pub(super) fn move_cursor_to_line_end(&mut self) -> Vec<Command> {
        if let Some(element_index) = self.editing_element_index
            && let Some(el) = self.elements.get_elements().get(element_index) {
                let after = el
                    .text
                    .chars()
                    .skip(self.text_cursor_pos)
                    .collect::<String>();
                if let Some(next_nl) = after.find('\n') {
                    self.text_cursor_pos += next_nl;
                } else {
                    self.text_cursor_pos = el.text.chars().count();
                }
                return vec![Command::RequestRedraw];
            }
        vec![]
    }

    /// 光标向上移动一行（基于字符计算）
    pub(super) fn move_cursor_up(&mut self) -> Vec<Command> {
        if let Some(element_index) = self.editing_element_index
            && let Some(el) = self.elements.get_elements().get(element_index) {
                let before = el
                    .text
                    .chars()
                    .take(self.text_cursor_pos)
                    .collect::<String>();
                let lines_before: Vec<&str> = before.lines().collect();
                if lines_before.len() > 1 {
                    let current_line_text = if before.ends_with('\n') {
                        ""
                    } else {
                        lines_before.last().copied().unwrap_or("")
                    };
                    let current_col = current_line_text.chars().count();
                    let current_line_start = if before.ends_with('\n') {
                        lines_before.len()
                    } else {
                        lines_before.len() - 1
                    };
                    if current_line_start > 0 {
                        let prev_line_text = lines_before[current_line_start - 1];
                        let prev_len = prev_line_text.chars().count();
                        let target_col = current_col.min(prev_len);
                        let chars_before_prev: usize = lines_before[..current_line_start - 1]
                            .iter()
                            .map(|line| line.chars().count() + 1)
                            .sum();
                        self.text_cursor_pos = chars_before_prev + target_col;
                        return vec![Command::RequestRedraw];
                    }
                }
            }
        vec![]
    }

    /// 光标向下移动一行（基于字符计算）
    pub(super) fn move_cursor_down(&mut self) -> Vec<Command> {
        if let Some(element_index) = self.editing_element_index
            && let Some(el) = self.elements.get_elements().get(element_index) {
                let before = el
                    .text
                    .chars()
                    .take(self.text_cursor_pos)
                    .collect::<String>();
                let after = el
                    .text
                    .chars()
                    .skip(self.text_cursor_pos)
                    .collect::<String>();
                if let Some(next_nl) = after.find('\n') {
                    let lines_before: Vec<&str> = before.lines().collect();
                    let current_line_text = if before.ends_with('\n') {
                        ""
                    } else {
                        lines_before.last().copied().unwrap_or("")
                    };
                    let current_col = current_line_text.chars().count();
                    let from_next = &after[next_nl + 1..];
                    let next_line_text = if let Some(end_pos) = from_next.find('\n') {
                        &from_next[..end_pos]
                    } else {
                        from_next
                    };
                    let next_len = next_line_text.chars().count();
                    let target_col = current_col.min(next_len);
                    self.text_cursor_pos = self.text_cursor_pos + next_nl + 1 + target_col;
                    return vec![Command::RequestRedraw];
                }
            }
        vec![]
    }

    /// 动态调整文字框大小（使用 DirectWrite 精确测量）
    pub(super) fn update_text_element_size(&mut self, element_index: usize) {
        use crate::constants::{MIN_TEXT_HEIGHT, MIN_TEXT_WIDTH, TEXT_PADDING};
        use windows::Win32::Graphics::DirectWrite::*;

        if let Some(element) = self.elements.get_element_mut(element_index) {
            // Invalidate text layout cache as text or size changed
            element.text_layout.replace(None);

            let font_size = element.get_effective_font_size();
            let dynamic_line_height = (font_size * crate::constants::TEXT_LINE_HEIGHT_SCALE).ceil() as i32;

            let text_content = element.text.clone();
            let lines: Vec<&str> = if text_content.is_empty() {
                vec![""]
            } else {
                text_content.lines().collect()
            };
            let line_count = if text_content.is_empty() {
                1
            } else if text_content.ends_with('\n') {
                lines.len() + 1
            } else {
                lines.len()
            } as i32;

            // 使用 DirectWrite 精确测量最长行宽度
            let mut max_width_f = 0.0f32;
            unsafe {
                if let Ok(factory) =
                    DWriteCreateFactory::<IDWriteFactory>(DWRITE_FACTORY_TYPE_SHARED)
                {
                    if let Ok(text_format) = crate::utils::d2d_helpers::create_text_format_from_element(
                        &factory,
                        &element.font_name,
                        font_size,
                        element.font_weight,
                        element.font_italic,
                    ) {
                        for line in &lines {
                            let wide: Vec<u16> = line.encode_utf16().collect();
                            if let Ok(layout) =
                                factory.CreateTextLayout(&wide, &text_format, f32::MAX, f32::MAX)
                            {
                                let mut metrics = DWRITE_TEXT_METRICS::default();
                                let _ = layout.GetMetrics(&mut metrics);
                                if metrics.width > max_width_f {
                                    max_width_f = metrics.width;
                                }
                            }
                        }
                    }
                }
            }

            if max_width_f == 0.0 {
                max_width_f = MIN_TEXT_WIDTH as f32;
            } else {
                // 增加适当的缓冲，避免字符被挤压
                max_width_f += (font_size * 0.2).max(4.0);
            }

            let new_width = ((max_width_f + TEXT_PADDING * 2.0).ceil() as i32).max(MIN_TEXT_WIDTH);
            // 使用行高系数计算高度（与光标定位一致）
            let new_height = (line_count * dynamic_line_height + (TEXT_PADDING * 2.0) as i32)
                .max(MIN_TEXT_HEIGHT);

            element.rect.right = element.rect.left + new_width;
            element.rect.bottom = element.rect.top + new_height;

            // 保持 points 与 rect 同步，确保渲染和选择区域一致
            if !element.points.is_empty() {
                if element.points.len() >= 2 {
                    element.points[1].x = element.rect.right;
                    element.points[1].y = element.rect.bottom;
                } else {
                    element.points.push(windows::Win32::Foundation::POINT {
                        x: element.rect.right,
                        y: element.rect.bottom,
                    });
                }
            }
        }
    }
}
