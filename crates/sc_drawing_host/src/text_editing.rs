use crate::constants::{MIN_TEXT_HEIGHT, MIN_TEXT_WIDTH, TEXT_LINE_HEIGHT_SCALE, TEXT_PADDING};
use sc_host_protocol::Command;

use super::{DrawingAction, DrawingElement, DrawingManager, DrawingTool};

impl DrawingManager {
    // 文本渲染（包含光标绘制）已迁移到 `sc_drawing::windows::DrawingRenderer`。

    // ===== 文本编辑相关方法 =====

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
        if self.selected_element.is_some() {
            self.static_layer_dirty = true;
        }
        // 清除所有元素的选择状态
        self.elements.set_selected(None);
        self.selected_element = None;

        // 确保工具栏状态与当前工具保持一致
        self.current_tool = DrawingTool::Text;

        // 命令模式：不在此处保存历史，而是在 stop_text_editing 时
        // 如果文本非空则记录 AddElement 操作

        // 创建新的文字元素
        let mut text_element = DrawingElement::new(DrawingTool::Text);
        text_element.add_point(x, y);

        // Use host-injected config (stored in the drawing manager).
        let (r, g, b) = self.config.font_color;
        text_element.color =
            sc_drawing::Color::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0);
        text_element.font_size = self.config.font_size;
        text_element.font_name = self.config.font_name.clone();
        text_element.font_weight = self.config.font_weight;
        text_element.font_italic = self.config.font_italic;
        text_element.font_underline = self.config.font_underline;
        text_element.font_strikeout = self.config.font_strikeout;
        text_element.text = String::new(); // 空文本，等待用户输入
        text_element.selected = true;

        // 根据字体大小动态计算初始文本框尺寸
        let font_size = text_element.font_size;
        // 与 update_text_element_size（以及渲染器的光标定位）保持一致的行高系数
        let dynamic_line_height = (font_size * TEXT_LINE_HEIGHT_SCALE).ceil() as i32;
        let initial_width = (font_size * 6.0) as i32; // 大约6个字符的宽度
        let initial_height = dynamic_line_height + (TEXT_PADDING * 2.0) as i32;

        // 设置第二个点来定义文本框尺寸
        text_element.set_end_point(x + initial_width, y + initial_height);

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

        // 保存当前工具状态，确保在保存文本后保持文本工具
        self.current_tool = DrawingTool::Text;

        // 检查当前编辑的文本元素是否为空，如果为空则删除
        if let Some(element_index) = editing_index
            && let Some(element) = self.elements.get_elements().get(element_index).cloned()
        {
            let should_delete = element.text.trim().is_empty();

            if should_delete {
                // 删除空元素（不记录到历史，因为这是取消创建操作）
                let _ = self.elements.remove_element(element_index);

                // 更新选中元素索引
                if let Some(selected) = self.selected_element {
                    if selected == element_index {
                        self.selected_element = None;
                    } else if selected > element_index {
                        self.selected_element = Some(selected - 1);
                    }
                }
            } else {
                // 文本非空，记录 AddElement 操作到历史
                let action = DrawingAction::AddElement {
                    element,
                    index: element_index,
                };
                self.history.record_action(
                    action, None, // 创建前无选中
                    None, // 创建后清除选中
                );
            }
        }

        // 强制确保工具状态保持为文本工具，防止被其他逻辑重置
        self.current_tool = DrawingTool::Text;

        // 设置标志，防止立即创建新的文本元素
        self.just_saved_text = true;

        // 清除选中状态，这样保存文本后就不会进入手柄检查逻辑
        self.selected_element = None;
        self.elements.set_selected(None);
        self.static_layer_dirty = true;

        vec![
            Command::StopTimer(self.cursor_timer_id as u32), // 停止光标闪烁定时器
            Command::UpdateToolbar,                          // 更新工具栏状态（启用撤回按钮）
            Command::RequestRedraw,
        ]
    }

    /// 处理文本输入
    pub fn handle_text_input(&mut self, character: char) -> Vec<Command> {
        if !self.text_editing {
            return vec![];
        }

        // 输入时保持光标可见
        self.text_cursor_visible = true;

        if let Some(element_index) = self.editing_element_index
            && let Some(element) = self.elements.get_element_mut(element_index)
        {
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

    /// 处理光标定时器
    pub fn handle_cursor_timer(&mut self, timer_id: u32) -> Vec<Command> {
        if self.text_editing && timer_id == self.cursor_timer_id as u32 {
            // 切换光标可见性
            self.text_cursor_visible = !self.text_cursor_visible;

            // 只重绘光标所在的文本元素区域
            if let Some(element_index) = self.editing_element_index
                && let Some(element) = self.elements.get_elements().get(element_index)
            {
                // 计算光标区域（稍微扩大以确保完整重绘）
                let cursor_margin = 5;
                let dirty_rect = sc_app::selection::RectI32 {
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

        // 操作时保持光标可见
        self.text_cursor_visible = true;

        if let Some(element_index) = self.editing_element_index
            && self.text_cursor_pos > 0
            && let Some(element) = self.elements.get_element_mut(element_index)
        {
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
            self.text_cursor_visible = true; // 移动时保持光标可见
            vec![Command::RequestRedraw]
        } else {
            vec![]
        }
    }

    /// 光标向右移动
    pub(super) fn move_cursor_right(&mut self) -> Vec<Command> {
        if let Some(element_index) = self.editing_element_index
            && let Some(el) = self.elements.get_elements().get(element_index)
        {
            let char_count = el.text.chars().count();
            if self.text_cursor_pos < char_count {
                self.text_cursor_pos += 1;
                self.text_cursor_visible = true; // 移动时保持光标可见
                return vec![Command::RequestRedraw];
            }
        }
        vec![]
    }

    /// 光标移动到行首（准确到当前行）
    pub(super) fn move_cursor_to_line_start(&mut self) -> Vec<Command> {
        if let Some(element_index) = self.editing_element_index
            && let Some(el) = self.elements.get_elements().get(element_index)
        {
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
            self.text_cursor_visible = true; // 移动时保持光标可见
            return vec![Command::RequestRedraw];
        }
        vec![]
    }

    /// 光标移动到行尾（准确到当前行）
    pub(super) fn move_cursor_to_line_end(&mut self) -> Vec<Command> {
        if let Some(element_index) = self.editing_element_index
            && let Some(el) = self.elements.get_elements().get(element_index)
        {
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
            self.text_cursor_visible = true; // 移动时保持光标可见
            return vec![Command::RequestRedraw];
        }
        vec![]
    }

    /// 光标向上移动一行（基于字符计算）
    pub(super) fn move_cursor_up(&mut self) -> Vec<Command> {
        if let Some(element_index) = self.editing_element_index
            && let Some(el) = self.elements.get_elements().get(element_index)
        {
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
                    self.text_cursor_visible = true; // 移动时保持光标可见
                    return vec![Command::RequestRedraw];
                }
            }
        }
        vec![]
    }

    /// 光标向下移动一行（基于字符计算）
    pub(super) fn move_cursor_down(&mut self) -> Vec<Command> {
        if let Some(element_index) = self.editing_element_index
            && let Some(el) = self.elements.get_elements().get(element_index)
        {
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
                self.text_cursor_visible = true; // 移动时保持光标可见
                return vec![Command::RequestRedraw];
            }
        }
        vec![]
    }

    /// 动态调整文字框大小（使用 DirectWrite 精确测量）
    pub(super) fn update_text_element_size(&mut self, element_index: usize) {
        if let Some(element) = self.elements.get_element_mut(element_index) {
            // 渲染相关缓存由 win_renderer 内部管理；这里仅更新元素尺寸
            sc_drawing::windows::update_text_element_size_dwrite(
                element,
                MIN_TEXT_WIDTH,
                MIN_TEXT_HEIGHT,
                TEXT_PADDING,
                TEXT_LINE_HEIGHT_SCALE,
            );
        }
    }
}
