use crate::element::{DrawingElement, Rect};
use crate::history::DrawingAction;
use crate::types::DrawingTool;

/// 元素管理器
///
/// 负责管理所有绘图元素的添加、删除、查询。
/// 包含资源限制功能，防止内存无限增长。
pub struct ElementManager {
    /// 所有绘图元素
    elements: Vec<DrawingElement>,
    /// 最大元素数量限制
    max_elements: usize,
}

impl Default for ElementManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ElementManager {
    /// 默认最大元素数量
    pub const DEFAULT_MAX_ELEMENTS: usize = 1000;

    /// 创建新的元素管理器
    pub fn new() -> Self {
        Self {
            elements: Vec::new(),
            max_elements: Self::DEFAULT_MAX_ELEMENTS,
        }
    }

    /// 创建带自定义限制的元素管理器
    pub fn with_max_elements(max_elements: usize) -> Self {
        Self {
            elements: Vec::new(),
            max_elements,
        }
    }

    /// 设置最大元素数量
    pub fn set_max_elements(&mut self, max: usize) {
        self.max_elements = max;
    }

    /// 获取当前元素数量
    pub fn element_count(&self) -> usize {
        self.elements.len()
    }

    /// 添加元素
    ///
    /// 如果达到最大数量限制，将移除最早的元素（非文本元素优先）
    pub fn add_element(&mut self, element: DrawingElement) {
        if self.elements.len() >= self.max_elements {
            // 优先移除最早的非文本元素
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

    /// 移除元素
    pub fn remove_element(&mut self, index: usize) -> bool {
        if index < self.elements.len() {
            self.elements.remove(index);
            true
        } else {
            false
        }
    }

    /// 获取指定位置的元素索引
    pub fn get_element_at_position(&self, x: i32, y: i32) -> Option<usize> {
        for (index, element) in self.elements.iter().enumerate().rev() {
            if element.contains_point(x, y) {
                return Some(index);
            }
        }
        None
    }

    /// 获取指定位置的元素索引（带选择框约束）
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

    /// 检查元素是否在矩形内可见
    pub fn is_element_visible_in_rect(&self, element: &DrawingElement, rect: &Rect) -> bool {
        let elem_rect = element.get_bounding_rect();
        !(elem_rect.right < rect.left
            || elem_rect.left > rect.right
            || elem_rect.bottom < rect.top
            || elem_rect.top > rect.bottom)
    }

    /// 设置选中状态
    pub fn set_selected(&mut self, index: Option<usize>) {
        for element in &mut self.elements {
            element.selected = false;
        }
        if let Some(idx) = index {
            if idx < self.elements.len() {
                self.elements[idx].selected = true;
            }
        }
    }

    /// 获取所有元素的引用
    pub fn get_elements(&self) -> &Vec<DrawingElement> {
        &self.elements
    }

    /// 获取可变元素引用
    pub fn get_element_mut(&mut self, index: usize) -> Option<&mut DrawingElement> {
        self.elements.get_mut(index)
    }

    /// 设置指定索引的元素
    pub fn set_element(&mut self, index: usize, element: DrawingElement) -> bool {
        if index < self.elements.len() {
            self.elements[index] = element;
            true
        } else {
            false
        }
    }

    /// 恢复状态
    pub fn restore_state(&mut self, elements: Vec<DrawingElement>) {
        self.elements = elements;
    }

    /// 清空所有元素
    pub fn clear(&mut self) {
        self.elements.clear();
    }

    /// 获取元素数量
    pub fn count(&self) -> usize {
        self.elements.len()
    }

    /// 在指定位置插入元素
    pub fn insert_element(&mut self, index: usize, element: DrawingElement) {
        if index <= self.elements.len() {
            self.elements.insert(index, element);
        }
    }

    // ==================== 命令模式支持 ====================

    /// 应用撤销操作
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

    /// 应用重做操作
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
