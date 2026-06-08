use crate::element::{Color, DrawingElement, Point, Rect};

#[derive(Clone, Debug)]
pub enum DrawingAction {
    AddElement {
        element: DrawingElement,
        index: usize,
    },
    RemoveElement {
        element: DrawingElement,
        index: usize,
    },
    MoveElement {
        index: usize,
        dx: i32,
        dy: i32,
        old_points: Vec<Point>,
        old_rect: Rect,
    },
    ResizeElement {
        index: usize,
        old_points: Vec<Point>,
        old_rect: Rect,
        old_font_size: f32,
        new_points: Vec<Point>,
        new_rect: Rect,
        new_font_size: f32,
    },
    ModifyText {
        index: usize,
        old_text: String,
        new_text: String,
        old_points: Vec<Point>,
        old_rect: Rect,
        new_points: Vec<Point>,
        new_rect: Rect,
    },
    ModifyProperty {
        index: usize,
        old_color: Color,
        old_thickness: f32,
        new_color: Color,
        new_thickness: f32,
    },
    Compound {
        actions: Vec<DrawingAction>,
    },
}

impl DrawingAction {
    pub fn affected_indices(&self) -> Vec<usize> {
        match self {
            DrawingAction::AddElement { index, .. } => vec![*index],
            DrawingAction::RemoveElement { index, .. } => vec![*index],
            DrawingAction::MoveElement { index, .. } => vec![*index],
            DrawingAction::ResizeElement { index, .. } => vec![*index],
            DrawingAction::ModifyText { index, .. } => vec![*index],
            DrawingAction::ModifyProperty { index, .. } => vec![*index],
            DrawingAction::Compound { actions } => {
                actions.iter().flat_map(|a| a.affected_indices()).collect()
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct HistoryEntry {
    pub action: DrawingAction,
    pub selected_before: Option<usize>,
    pub selected_after: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct HistoryState {
    pub elements: Vec<DrawingElement>,
    pub selected_element: Option<usize>,
    pub changed_indices: Vec<usize>,
}

pub struct ActionHistory {
    base_state: Option<HistoryState>,
    action_stack: Vec<HistoryEntry>,
    current_position: usize,
    max_history: usize,
}

impl Default for ActionHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl ActionHistory {
    pub fn new() -> Self {
        Self {
            base_state: None,
            action_stack: Vec::new(),
            current_position: 0,
            max_history: 50,
        }
    }

    pub fn record_action(
        &mut self,
        action: DrawingAction,
        selected_before: Option<usize>,
        selected_after: Option<usize>,
    ) {
        if self.current_position < self.action_stack.len() {
            self.action_stack.truncate(self.current_position);
        }

        let entry = HistoryEntry {
            action,
            selected_before,
            selected_after,
        };

        self.action_stack.push(entry);
        self.current_position = self.action_stack.len();

        if self.action_stack.len() > self.max_history {
            self.action_stack.remove(0);
            self.current_position = self.action_stack.len();
        }
    }

    pub fn undo_action(&mut self) -> Option<(DrawingAction, Option<usize>)> {
        if self.current_position == 0 {
            return None;
        }

        self.current_position -= 1;
        let entry = &self.action_stack[self.current_position];
        Some((entry.action.clone(), entry.selected_before))
    }

    pub fn redo_action(&mut self) -> Option<(DrawingAction, Option<usize>)> {
        if self.current_position >= self.action_stack.len() {
            return None;
        }

        let entry = &self.action_stack[self.current_position];
        self.current_position += 1;
        Some((entry.action.clone(), entry.selected_after))
    }

    pub fn save_state(&mut self, elements: &[DrawingElement], selected_element: Option<usize>) {
        if self.base_state.is_none() {
            self.base_state = Some(HistoryState {
                elements: elements.to_vec(),
                selected_element,
                changed_indices: vec![],
            });
        }
    }

    pub fn can_undo(&self) -> bool {
        self.current_position > 0
    }

    pub fn can_redo(&self) -> bool {
        self.current_position < self.action_stack.len()
    }

    pub fn clear(&mut self) {
        self.base_state = None;
        self.action_stack.clear();
        self.current_position = 0;
    }

    pub fn get_last_changed_indices(&self) -> Vec<usize> {
        if self.current_position > 0 && self.current_position <= self.action_stack.len() {
            self.action_stack[self.current_position - 1]
                .action
                .affected_indices()
        } else {
            vec![]
        }
    }

    pub fn get_base_state(&self) -> Option<&HistoryState> {
        self.base_state.as_ref()
    }

    pub fn set_base_state(&mut self, elements: Vec<DrawingElement>, selected: Option<usize>) {
        self.base_state = Some(HistoryState {
            elements,
            selected_element: selected,
            changed_indices: vec![],
        });
    }
}

pub trait Command: std::fmt::Debug {
    fn execute(&mut self);

    fn undo(&mut self);

    fn description(&self) -> &str;
}

#[derive(Debug, Default)]
pub struct HistoryManager<C> {
    undo_stack: Vec<C>,
    redo_stack: Vec<C>,
    max_history: usize,
}

impl<C> HistoryManager<C> {
    pub fn new() -> Self {
        Self::with_capacity(100)
    }

    pub fn with_capacity(max_history: usize) -> Self {
        Self {
            undo_stack: Vec::with_capacity(max_history),
            redo_stack: Vec::new(),
            max_history,
        }
    }

    pub fn push(&mut self, command: C) {
        self.redo_stack.clear();

        if self.undo_stack.len() >= self.max_history {
            self.undo_stack.remove(0);
        }

        self.undo_stack.push(command);
    }

    pub fn undo(&mut self) -> Option<&C> {
        if let Some(command) = self.undo_stack.pop() {
            self.redo_stack.push(command);
            self.redo_stack.last()
        } else {
            None
        }
    }

    pub fn redo(&mut self) -> Option<&C> {
        if let Some(command) = self.redo_stack.pop() {
            self.undo_stack.push(command);
            self.undo_stack.last()
        } else {
            None
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo_count(&self) -> usize {
        self.undo_stack.len()
    }

    pub fn redo_count(&self) -> usize {
        self.redo_stack.len()
    }

    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    pub fn undo_stack(&self) -> &[C] {
        &self.undo_stack
    }

    pub fn redo_stack(&self) -> &[C] {
        &self.redo_stack
    }
}

#[derive(Debug, Default)]
pub struct SimpleHistory<T> {
    states: Vec<T>,
    current_index: isize,
    max_states: usize,
}

impl<T: Clone> SimpleHistory<T> {
    pub fn new(max_states: usize) -> Self {
        Self {
            states: Vec::with_capacity(max_states),
            current_index: -1,
            max_states,
        }
    }

    pub fn save(&mut self, state: T) {
        if self.current_index >= 0 && (self.current_index as usize) < self.states.len() - 1 {
            self.states.truncate(self.current_index as usize + 1);
        }

        if self.states.len() >= self.max_states {
            self.states.remove(0);
        }

        self.states.push(state);
        self.current_index = self.states.len() as isize - 1;
    }

    pub fn undo(&mut self) -> Option<&T> {
        if self.current_index > 0 {
            self.current_index -= 1;
            self.states.get(self.current_index as usize)
        } else {
            None
        }
    }

    pub fn redo(&mut self) -> Option<&T> {
        if self.current_index >= 0 && (self.current_index as usize) < self.states.len() - 1 {
            self.current_index += 1;
            self.states.get(self.current_index as usize)
        } else {
            None
        }
    }

    pub fn current(&self) -> Option<&T> {
        if self.current_index >= 0 {
            self.states.get(self.current_index as usize)
        } else {
            None
        }
    }

    pub fn can_undo(&self) -> bool {
        self.current_index > 0
    }

    pub fn can_redo(&self) -> bool {
        self.current_index >= 0 && (self.current_index as usize) < self.states.len() - 1
    }

    pub fn clear(&mut self) {
        self.states.clear();
        self.current_index = -1;
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_history_manager_basic() {
        let mut history: super::HistoryManager<i32> = super::HistoryManager::new();

        assert!(!history.can_undo());
        assert!(!history.can_redo());

        history.push(1);
        history.push(2);
        history.push(3);

        assert!(history.can_undo());
        assert!(!history.can_redo());
        assert_eq!(history.undo_count(), 3);
    }

    #[test]
    fn test_history_manager_undo_clears_redo() {
        let mut history: super::HistoryManager<i32> = super::HistoryManager::new();

        history.push(1);
        history.push(2);
        history.undo();

        assert!(history.can_redo());

        history.push(3);
        assert!(!history.can_redo());
    }

    #[test]
    fn test_simple_history() {
        let mut history = super::SimpleHistory::<String>::new(10);

        history.save("state1".to_string());
        history.save("state2".to_string());
        history.save("state3".to_string());

        assert_eq!(history.current(), Some(&"state3".to_string()));

        let prev = history.undo();
        assert_eq!(prev, Some(&"state2".to_string()));

        let next = history.redo();
        assert_eq!(next, Some(&"state3".to_string()));
    }
}
