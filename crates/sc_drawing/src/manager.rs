use crate::element::{DrawingElement, Rect};
use crate::history::DrawingAction;
use crate::types::DrawingTool;

pub struct ElementManager {
    elements: Vec<DrawingElement>,
    max_elements: usize,
}

impl Default for ElementManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ElementManager {
    pub const DEFAULT_MAX_ELEMENTS: usize = 1000;

    pub fn new() -> Self {
        Self {
            elements: Vec::new(),
            max_elements: Self::DEFAULT_MAX_ELEMENTS,
        }
    }

    pub fn with_max_elements(max_elements: usize) -> Self {
        Self {
            elements: Vec::new(),
            max_elements,
        }
    }

    pub fn set_max_elements(&mut self, max: usize) {
        self.max_elements = max;
    }

    pub fn element_count(&self) -> usize {
        self.elements.len()
    }

    pub fn add_element(&mut self, element: DrawingElement) {
        if self.elements.len() >= self.max_elements {
            if let Some(pos) = self
                .elements
                .iter()
                .position(|e| e.tool != DrawingTool::Text)
            {
                self.elements.remove(pos);
            } else {
                self.elements.remove(0);
            }
        }
        self.elements.push(element);
    }

    pub fn remove_element(&mut self, index: usize) -> bool {
        if index < self.elements.len() {
            self.elements.remove(index);
            true
        } else {
            false
        }
    }

    pub fn get_element_at_position(&self, x: i32, y: i32) -> Option<usize> {
        for (index, element) in self.elements.iter().enumerate().rev() {
            if element.contains_point(x, y) {
                return Some(index);
            }
        }
        None
    }

    pub fn get_element_at_position_with_rect(
        &self,
        x: i32,
        y: i32,
        selection_rect: Option<Rect>,
    ) -> Option<usize> {
        for (index, element) in self.elements.iter().enumerate().rev() {
            if element.tool == DrawingTool::Pen {
                continue;
            }
            if element.contains_point(x, y) {
                if let Some(sel_rect) = selection_rect {
                    if self.is_element_visible_in_rect(element, &sel_rect) {
                        return Some(index);
                    }
                } else {
                    return Some(index);
                }
            }
        }
        None
    }

    pub fn is_element_visible_in_rect(&self, element: &DrawingElement, rect: &Rect) -> bool {
        let elem_rect = element.get_bounding_rect();
        !(elem_rect.right < rect.left
            || elem_rect.left > rect.right
            || elem_rect.bottom < rect.top
            || elem_rect.top > rect.bottom)
    }

    pub fn set_selected(&mut self, index: Option<usize>) {
        for element in &mut self.elements {
            element.selected = false;
        }
        if let Some(idx) = index
            && idx < self.elements.len()
        {
            self.elements[idx].selected = true;
        }
    }

    pub fn get_elements(&self) -> &Vec<DrawingElement> {
        &self.elements
    }

    pub fn get_element_mut(&mut self, index: usize) -> Option<&mut DrawingElement> {
        self.elements.get_mut(index)
    }

    pub fn set_element(&mut self, index: usize, element: DrawingElement) -> bool {
        if index < self.elements.len() {
            self.elements[index] = element;
            true
        } else {
            false
        }
    }

    pub fn restore_state(&mut self, elements: Vec<DrawingElement>) {
        self.elements = elements;
    }

    pub fn clear(&mut self) {
        self.elements.clear();
    }

    pub fn count(&self) -> usize {
        self.elements.len()
    }

    pub fn insert_element(&mut self, index: usize, element: DrawingElement) {
        if index <= self.elements.len() {
            self.elements.insert(index, element);
        }
    }

    pub fn apply_undo(&mut self, action: &DrawingAction) {
        match action {
            DrawingAction::AddElement { index, .. } => {
                if *index < self.elements.len() {
                    self.elements.remove(*index);
                }
            }
            DrawingAction::RemoveElement { element, index } => {
                if *index <= self.elements.len() {
                    self.elements.insert(*index, element.clone());
                }
            }
            DrawingAction::MoveElement {
                index,
                old_points,
                old_rect,
                ..
            } => {
                if let Some(element) = self.elements.get_mut(*index) {
                    element.points = old_points.clone();
                    element.rect = *old_rect;
                }
            }
            DrawingAction::ResizeElement {
                index,
                old_points,
                old_rect,
                old_font_size,
                ..
            } => {
                if let Some(element) = self.elements.get_mut(*index) {
                    element.points = old_points.clone();
                    element.rect = *old_rect;
                    element.font_size = *old_font_size;
                }
            }
            DrawingAction::ModifyText {
                index,
                old_text,
                old_points,
                old_rect,
                ..
            } => {
                if let Some(element) = self.elements.get_mut(*index) {
                    element.text = old_text.clone();
                    element.points = old_points.clone();
                    element.rect = *old_rect;
                }
            }
            DrawingAction::ModifyProperty {
                index,
                old_color,
                old_thickness,
                ..
            } => {
                if let Some(element) = self.elements.get_mut(*index) {
                    element.color = *old_color;
                    element.thickness = *old_thickness;
                }
            }
            DrawingAction::Compound { actions } => {
                for action in actions.iter().rev() {
                    self.apply_undo(action);
                }
            }
        }
    }

    pub fn apply_redo(&mut self, action: &DrawingAction) {
        match action {
            DrawingAction::AddElement { element, index } => {
                if *index <= self.elements.len() {
                    self.elements.insert(*index, element.clone());
                }
            }
            DrawingAction::RemoveElement { index, .. } => {
                if *index < self.elements.len() {
                    self.elements.remove(*index);
                }
            }
            DrawingAction::MoveElement { index, dx, dy, .. } => {
                if let Some(element) = self.elements.get_mut(*index) {
                    element.move_by(*dx, *dy);
                }
            }
            DrawingAction::ResizeElement {
                index,
                new_points,
                new_rect,
                new_font_size,
                ..
            } => {
                if let Some(element) = self.elements.get_mut(*index) {
                    element.points = new_points.clone();
                    element.rect = *new_rect;
                    element.font_size = *new_font_size;
                }
            }
            DrawingAction::ModifyText {
                index,
                new_text,
                new_points,
                new_rect,
                ..
            } => {
                if let Some(element) = self.elements.get_mut(*index) {
                    element.text = new_text.clone();
                    element.points = new_points.clone();
                    element.rect = *new_rect;
                }
            }
            DrawingAction::ModifyProperty {
                index,
                new_color,
                new_thickness,
                ..
            } => {
                if let Some(element) = self.elements.get_mut(*index) {
                    element.color = *new_color;
                    element.thickness = *new_thickness;
                }
            }
            DrawingAction::Compound { actions } => {
                for action in actions {
                    self.apply_redo(action);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_element_manager_new() {
        let manager = super::ElementManager::new();
        assert_eq!(manager.count(), 0);
        assert_eq!(manager.element_count(), 0);
    }

    #[test]
    fn test_add_and_remove_element() {
        let mut manager = super::ElementManager::new();
        let element = super::DrawingElement::new(super::DrawingTool::Rectangle);

        manager.add_element(element);
        assert_eq!(manager.count(), 1);

        manager.remove_element(0);
        assert_eq!(manager.count(), 0);
    }

    #[test]
    fn test_max_elements_limit() {
        let mut manager = super::ElementManager::with_max_elements(3);

        for _ in 0..5 {
            manager.add_element(super::DrawingElement::new(super::DrawingTool::Rectangle));
        }

        assert_eq!(manager.count(), 3);
    }

    #[test]
    fn test_set_selected() {
        let mut manager = super::ElementManager::new();
        manager.add_element(super::DrawingElement::new(super::DrawingTool::Rectangle));
        manager.add_element(super::DrawingElement::new(super::DrawingTool::Circle));

        manager.set_selected(Some(1));

        assert!(!manager.get_elements()[0].selected);
        assert!(manager.get_elements()[1].selected);
    }
}
