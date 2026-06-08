use crate::constants::{MIN_TEXT_HEIGHT, MIN_TEXT_WIDTH, TEXT_LINE_HEIGHT_SCALE};
use sc_host_protocol::Command;

use super::{DrawingAction, DrawingElement, DrawingManager, DrawingTool};

impl DrawingManager {
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

    pub(super) fn start_text_editing(&mut self, element_index: usize) -> Vec<Command> {
        self.elements.set_selected(None);
        self.selected_element = Some(element_index);
        self.elements.set_selected(self.selected_element);

        self.text_editing = true;
        self.editing_element_index = Some(element_index);
        if let Some(el) = self.elements.get_elements().get(element_index) {
            self.text_cursor_pos = el.text.chars().count();
        } else {
            self.text_cursor_pos = 0;
        }
        self.text_cursor_visible = true;

        vec![
            Command::StartTimer(self.cursor_timer_id as u32, 500),
            Command::RequestRedraw,
        ]
    }

    pub(super) fn create_and_edit_text_element(&mut self, x: i32, y: i32) -> (Vec<Command>, bool) {
        if self.selected_element.is_some() {
            self.static_layer_dirty = true;
        }
        self.elements.set_selected(None);
        self.selected_element = None;

        self.current_tool = DrawingTool::Text;

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
        text_element.text = String::new();
        text_element.selected = true;

        let font_size = text_element.font_size;
        let dynamic_line_height = (font_size * TEXT_LINE_HEIGHT_SCALE).ceil() as i32;
        let padding = sc_drawing::windows::text_padding_for_font_size(font_size);
        let initial_width = (font_size * 6.0) as i32;
        let initial_height = dynamic_line_height + (padding * 2.0).ceil() as i32;

        text_element.set_end_point(x + initial_width, y + initial_height);

        text_element.update_bounding_rect();

        self.elements.add_element(text_element);
        let element_index = self.elements.count().saturating_sub(1);

        self.text_editing = true;
        self.editing_element_index = Some(element_index);
        self.text_cursor_pos = 0;
        self.text_cursor_visible = true;
        self.selected_element = Some(element_index);
        self.elements.set_selected(self.selected_element);

        (
            vec![
                Command::StartTimer(self.cursor_timer_id as u32, 500),
                Command::RequestRedraw,
            ],
            true,
        )
    }

    pub(super) fn stop_text_editing(&mut self) -> Vec<Command> {
        if !self.text_editing {
            return vec![];
        }

        self.text_cursor_visible = false;

        self.text_editing = false;
        let editing_index = self.editing_element_index;
        self.editing_element_index = None;
        self.text_cursor_pos = 0;

        self.current_tool = DrawingTool::Text;

        if let Some(element_index) = editing_index
            && let Some(element) = self.elements.get_elements().get(element_index).cloned()
        {
            let should_delete = element.text.trim().is_empty();

            if should_delete {
                let _ = self.elements.remove_element(element_index);

                if let Some(selected) = self.selected_element {
                    if selected == element_index {
                        self.selected_element = None;
                    } else if selected > element_index {
                        self.selected_element = Some(selected - 1);
                    }
                }
            } else {
                let action = DrawingAction::AddElement {
                    element,
                    index: element_index,
                };
                self.history.record_action(action, None, None);
            }
        }

        self.current_tool = DrawingTool::Text;

        self.just_saved_text = true;

        self.selected_element = None;
        self.elements.set_selected(None);
        self.static_layer_dirty = true;

        vec![
            Command::StopTimer(self.cursor_timer_id as u32),
            Command::UpdateToolbar,
            Command::RequestRedraw,
        ]
    }

    pub fn handle_text_input(&mut self, character: char) -> Vec<Command> {
        if !self.text_editing {
            return vec![];
        }

        self.text_cursor_visible = true;

        if let Some(element_index) = self.editing_element_index
            && let Some(element) = self.elements.get_element_mut(element_index)
        {
            let char_count = element.text.chars().count();
            if self.text_cursor_pos <= char_count {
                let byte_pos = element
                    .text
                    .char_indices()
                    .nth(self.text_cursor_pos)
                    .map(|(i, _)| i)
                    .unwrap_or(element.text.len());
                element.text.insert(byte_pos, character);
                self.text_cursor_pos += 1;

                self.update_text_element_size(element_index);

                return vec![Command::RequestRedraw];
            }
        }
        vec![]
    }

    pub fn handle_cursor_timer(&mut self, timer_id: u32) -> Vec<Command> {
        if self.text_editing && timer_id == self.cursor_timer_id as u32 {
            self.text_cursor_visible = !self.text_cursor_visible;

            if let Some(element_index) = self.editing_element_index
                && let Some(element) = self.elements.get_elements().get(element_index)
            {
                let cursor_margin = 5;
                let dirty_rect = sc_app::selection::RectI32 {
                    left: element.rect.left - cursor_margin,
                    top: element.rect.top - cursor_margin,
                    right: element.rect.right + cursor_margin,
                    bottom: element.rect.bottom + cursor_margin,
                };
                return vec![Command::RequestRedrawRect(dirty_rect)];
            }

            vec![Command::RequestRedraw]
        } else {
            vec![]
        }
    }

    pub(super) fn handle_backspace(&mut self) -> Vec<Command> {
        if !self.text_editing {
            return vec![];
        }

        self.text_cursor_visible = true;

        if let Some(element_index) = self.editing_element_index
            && self.text_cursor_pos > 0
            && let Some(element) = self.elements.get_element_mut(element_index)
        {
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

            self.update_text_element_size(element_index);

            return vec![Command::RequestRedraw];
        }
        vec![]
    }

    pub(super) fn move_cursor_left(&mut self) -> Vec<Command> {
        if self.text_cursor_pos > 0 {
            self.text_cursor_pos -= 1;
            self.text_cursor_visible = true;
            vec![Command::RequestRedraw]
        } else {
            vec![]
        }
    }

    pub(super) fn move_cursor_right(&mut self) -> Vec<Command> {
        if let Some(element_index) = self.editing_element_index
            && let Some(el) = self.elements.get_elements().get(element_index)
        {
            let char_count = el.text.chars().count();
            if self.text_cursor_pos < char_count {
                self.text_cursor_pos += 1;
                self.text_cursor_visible = true;
                return vec![Command::RequestRedraw];
            }
        }
        vec![]
    }

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
            self.text_cursor_visible = true;
            return vec![Command::RequestRedraw];
        }
        vec![]
    }

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
            self.text_cursor_visible = true;
            return vec![Command::RequestRedraw];
        }
        vec![]
    }

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
                    self.text_cursor_visible = true;
                    return vec![Command::RequestRedraw];
                }
            }
        }
        vec![]
    }

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
                self.text_cursor_visible = true;
                return vec![Command::RequestRedraw];
            }
        }
        vec![]
    }

    pub(super) fn update_text_element_size(&mut self, element_index: usize) {
        if let Some(element) = self.elements.get_element_mut(element_index) {
            sc_drawing::windows::update_text_element_size_dwrite(
                element,
                MIN_TEXT_WIDTH,
                MIN_TEXT_HEIGHT,
                TEXT_LINE_HEIGHT_SCALE,
            );
        }
    }
}
