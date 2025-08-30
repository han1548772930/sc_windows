// 文本编辑模块 - 处理文本输入、光标和文本渲染

use crate::message::Command;
use crate::types::{DrawingElement, DrawingTool};
use crate::utils::d2d_helpers::create_solid_brush;

use super::{DrawingError, DrawingManager};

impl DrawingManager {
    /// 使用Direct2D渲染文本元素（从原始代码完整迁移，支持多行、内边距、光标）
    pub(super) fn draw_text_element_d2d(
        &self,
        element: &DrawingElement,
        d2d_renderer: &crate::platform::windows::d2d::Direct2DRenderer,
    ) -> Result<(), DrawingError> {
        if element.points.is_empty() {
            return Ok(());
        }

        if let (Some(render_target), Some(dwrite_factory)) =
            (&d2d_renderer.render_target, &d2d_renderer.dwrite_factory)
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

                // 使用元素自身的颜色属性创建文本画刷
                let font_color = element.color;

                // 使用新的辅助函数创建文本画刷
                let text_brush = create_solid_brush(render_target, &font_color);

                if let Ok(brush) = text_brush {
                    // 添加内边距（从原始代码迁移）
                    let text_content_rect =
                        windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F {
                            left: text_rect.left + crate::constants::TEXT_PADDING,
                            top: text_rect.top + crate::constants::TEXT_PADDING,
                            right: text_rect.right - crate::constants::TEXT_PADDING,
                            bottom: text_rect.bottom - crate::constants::TEXT_PADDING,
                        };

                    // 支持多行文字显示（从原始代码迁移）
                    let lines: Vec<&str> = if element.text.is_empty() {
                        vec![""] // 空文本时显示一个空行（用于显示光标）
                    } else {
                        element.text.lines().collect()
                    };

                    let font_size = element.font_size.max(12.0); // 最小字体大小12
                    let line_height = font_size * 1.2;
                    let font_name_wide = crate::utils::to_wide_chars(&element.font_name);
                    let font_weight = if element.font_weight > 400 {
                        windows::Win32::Graphics::DirectWrite::DWRITE_FONT_WEIGHT_BOLD
                    } else {
                        windows::Win32::Graphics::DirectWrite::DWRITE_FONT_WEIGHT_NORMAL
                    };
                    let font_style = if element.font_italic {
                        windows::Win32::Graphics::DirectWrite::DWRITE_FONT_STYLE_ITALIC
                    } else {
                        windows::Win32::Graphics::DirectWrite::DWRITE_FONT_STYLE_NORMAL
                    };

                    // 创建动态字体格式，使用设置中的字体属性（与旧代码保持一致）
                    if let Ok(text_format) = dwrite_factory.CreateTextFormat(
                        windows::core::PCWSTR(font_name_wide.as_ptr()),
                        None,
                        font_weight,
                        font_style,
                        windows::Win32::Graphics::DirectWrite::DWRITE_FONT_STRETCH_NORMAL,
                        font_size,
                        windows::core::w!(""),
                    ) {
                        // 设置文本对齐
                        let _ = text_format.SetTextAlignment(
                            windows::Win32::Graphics::DirectWrite::DWRITE_TEXT_ALIGNMENT_LEADING,
                        );
                        let _ = text_format.SetParagraphAlignment(
                            windows::Win32::Graphics::DirectWrite::DWRITE_PARAGRAPH_ALIGNMENT_NEAR,
                        );

                        // 逐行绘制文本（从原始代码迁移）
                        for (line_index, line) in lines.iter().enumerate() {
                            let line_y = text_content_rect.top + (line_index as f32 * line_height);

                            // 检查是否超出文本区域
                            if line_y + line_height > text_content_rect.bottom {
                                break;
                            }

                            let line_rect =
                                windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F {
                                    left: text_content_rect.left,
                                    top: line_y,
                                    right: text_content_rect.right,
                                    bottom: line_y + line_height,
                                };

                            // 转换文本为宽字符
                            let wide_text: Vec<u16> = line.encode_utf16().collect();

                            // 如果需要下划线或删除线，使用TextLayout；否则使用简单的DrawText
                            if element.font_underline || element.font_strikeout {
                                // 创建文本布局以支持下划线和删除线（使用元素属性）
                                if let Ok(text_layout) = dwrite_factory.CreateTextLayout(
                                    &wide_text,
                                    &text_format,
                                    line_rect.right - line_rect.left,
                                    line_rect.bottom - line_rect.top,
                                ) {
                                    // 应用下划线和删除线（与旧代码保持一致）
                                    if element.font_underline && !wide_text.is_empty() {
                                        let _ = text_layout.SetUnderline(
                                            true,
                                            windows::Win32::Graphics::DirectWrite::DWRITE_TEXT_RANGE {
                                                startPosition: 0,
                                                length: wide_text.len() as u32,
                                            },
                                        );
                                    }
                                    if element.font_strikeout && !wide_text.is_empty() {
                                        let _ = text_layout.SetStrikethrough(
                                            true,
                                            windows::Win32::Graphics::DirectWrite::DWRITE_TEXT_RANGE {
                                                startPosition: 0,
                                                length: wide_text.len() as u32,
                                            },
                                        );
                                    }

                                    // 绘制文本布局（与旧代码保持一致）
                                    render_target.DrawTextLayout(
                                        crate::utils::d2d_point(line_rect.left as i32, line_rect.top as i32),
                                        &text_layout,
                                        &brush,
                                        windows::Win32::Graphics::Direct2D::D2D1_DRAW_TEXT_OPTIONS_NONE,
                                    );
                                }
                            } else {
                                // 简单文本绘制（无特殊样式）
                                render_target.DrawText(
                                    &wide_text,
                                    &text_format,
                                    &line_rect,
                                    &brush,
                                    windows::Win32::Graphics::Direct2D::D2D1_DRAW_TEXT_OPTIONS_NONE,
                                    windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL,
                                );
                            }
                        }

                        // 绘制文本输入光标（caret），仅当该元素正在编辑且光标可见
                        if self.text_editing && self.text_cursor_visible {
                            if let Some(edit_idx) = self.editing_element_index {
                                // 通过指针相等找到当前元素索引
                                if let Some(current_idx) = self
                                    .elements
                                    .get_elements()
                                    .iter()
                                    .position(|e| std::ptr::eq(e, element))
                                {
                                    if current_idx == edit_idx {
                                        self.draw_text_cursor(
                                            element,
                                            render_target,
                                            dwrite_factory,
                                            &text_format,
                                            &text_content_rect,
                                            line_height,
                                        )?;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// 绘制文本光标
    fn draw_text_cursor(
        &self,
        element: &DrawingElement,
        render_target: &windows::Win32::Graphics::Direct2D::ID2D1RenderTarget,
        dwrite_factory: &windows::Win32::Graphics::DirectWrite::IDWriteFactory,
        text_format: &windows::Win32::Graphics::DirectWrite::IDWriteTextFormat,
        text_content_rect: &windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F,
        line_height: f32,
    ) -> Result<(), DrawingError> {
        unsafe {
            // 基于字符计算当前行与列
            let before: String = element.text.chars().take(self.text_cursor_pos).collect();
            let lines_before: Vec<&str> = before.lines().collect();
            let caret_line = if before.ends_with('\n') {
                lines_before.len()
            } else {
                lines_before.len().saturating_sub(1)
            };
            let current_line_text = if before.ends_with('\n') {
                ""
            } else {
                lines_before.last().copied().unwrap_or("")
            };

            // 使用 DirectWrite 精确测量光标前文本宽度
            let before_wide: Vec<u16> = current_line_text.encode_utf16().collect();
            let mut caret_x = text_content_rect.left;
            if let Ok(layout) =
                dwrite_factory.CreateTextLayout(&before_wide, text_format, f32::MAX, f32::MAX)
            {
                let mut metrics =
                    windows::Win32::Graphics::DirectWrite::DWRITE_TEXT_METRICS::default();
                let _ = layout.GetMetrics(&mut metrics);
                caret_x += metrics.width;
            }

            let caret_y_top = text_content_rect.top + (caret_line as f32) * line_height;
            let caret_y_bottom = (caret_y_top + line_height).min(text_content_rect.bottom);

            let caret_rect = windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F {
                left: caret_x,
                top: caret_y_top,
                right: caret_x + 1.0,
                bottom: caret_y_bottom,
            };

            if let Ok(cursor_brush) =
                create_solid_brush(render_target, &crate::constants::COLOR_TEXT_CURSOR)
            {
                render_target.FillRectangle(&caret_rect, &cursor_brush);
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

        // 保存历史状态（在操作开始前保存，以便精确撤销）
        self.history
            .save_state(&self.elements, self.selected_element);

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

        // 根据字体大小动态计算初始文本框尺寸（与旧代码保持一致）
        let font_size = text_element.font_size;
        let dynamic_line_height = (font_size * 1.2) as i32;
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
        if let Some(element_index) = editing_index {
            if let Some(element) = self.elements.get_elements().get(element_index) {
                let should_delete = element.text.trim().is_empty();

                if should_delete {
                    // 删除空元素
                    let _ = self.elements.remove_element(element_index);

                    // 更新选中元素索引（与原始代码逻辑一致）
                    if let Some(selected) = self.selected_element {
                        if selected == element_index {
                            self.selected_element = None;
                        } else if selected > element_index {
                            self.selected_element = Some(selected - 1);
                        }
                    }
                }
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
            Command::RequestRedraw,
        ]
    }

    /// 处理文本输入（从原始代码迁移）
    pub fn handle_text_input(&mut self, character: char) -> Vec<Command> {
        if !self.text_editing {
            return vec![];
        }

        if let Some(element_index) = self.editing_element_index {
            if let Some(element) = self.elements.get_element_mut(element_index) {
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
        }
        vec![]
    }

    /// 处理光标定时器（从原始代码迁移）
    pub fn handle_cursor_timer(&mut self, timer_id: u32) -> Vec<Command> {
        if self.text_editing && timer_id == self.cursor_timer_id as u32 {
            // 切换光标可见性
            self.text_cursor_visible = !self.text_cursor_visible;
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

        if let Some(element_index) = self.editing_element_index {
            if self.text_cursor_pos > 0 {
                if let Some(element) = self.elements.get_element_mut(element_index) {
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
            }
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
        if let Some(element_index) = self.editing_element_index {
            if let Some(el) = self.elements.get_elements().get(element_index) {
                let char_count = el.text.chars().count();
                if self.text_cursor_pos < char_count {
                    self.text_cursor_pos += 1;
                    return vec![Command::RequestRedraw];
                }
            }
        }
        vec![]
    }

    /// 光标移动到行首（准确到当前行）
    pub(super) fn move_cursor_to_line_start(&mut self) -> Vec<Command> {
        if let Some(element_index) = self.editing_element_index {
            if let Some(el) = self.elements.get_elements().get(element_index) {
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
        }
        vec![]
    }

    /// 光标移动到行尾（准确到当前行）
    pub(super) fn move_cursor_to_line_end(&mut self) -> Vec<Command> {
        if let Some(element_index) = self.editing_element_index {
            if let Some(el) = self.elements.get_elements().get(element_index) {
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
        }
        vec![]
    }

    /// 光标向上移动一行（基于字符计算）
    pub(super) fn move_cursor_up(&mut self) -> Vec<Command> {
        if let Some(element_index) = self.editing_element_index {
            if let Some(el) = self.elements.get_elements().get(element_index) {
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
        }
        vec![]
    }

    /// 光标向下移动一行（基于字符计算）
    pub(super) fn move_cursor_down(&mut self) -> Vec<Command> {
        if let Some(element_index) = self.editing_element_index {
            if let Some(el) = self.elements.get_elements().get(element_index) {
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
        }
        vec![]
    }

    /// 动态调整文字框大小（使用 DirectWrite 精确测量）
    pub(super) fn update_text_element_size(&mut self, element_index: usize) {
        use crate::constants::{MIN_TEXT_HEIGHT, MIN_TEXT_WIDTH, TEXT_PADDING};
        use windows::Win32::Graphics::DirectWrite::*;
        use windows::core::w;

        if let Some(element) = self.elements.get_element_mut(element_index) {
            let font_size = element.get_effective_font_size();
            let dynamic_line_height = (font_size * 1.2).ceil() as i32;

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
                    let font_name_wide = crate::utils::to_wide_chars(&element.font_name);
                    let weight = if element.font_weight > 400 {
                        DWRITE_FONT_WEIGHT_BOLD
                    } else {
                        DWRITE_FONT_WEIGHT_NORMAL
                    };
                    let style = if element.font_italic {
                        DWRITE_FONT_STYLE_ITALIC
                    } else {
                        DWRITE_FONT_STYLE_NORMAL
                    };
                    if let Ok(text_format) = factory.CreateTextFormat(
                        windows::core::PCWSTR(font_name_wide.as_ptr()),
                        None,
                        weight,
                        style,
                        DWRITE_FONT_STRETCH_NORMAL,
                        font_size,
                        w!(""),
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
