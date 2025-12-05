use windows::Win32::Foundation::RECT;

use super::history::DrawingAction;
use super::types::{DrawingElement, DrawingTool};

/// 元素管理器
///
/// 负责管理所有绘图元素的添加、删除、查询和渲染。
/// 包含资源限制功能，防止内存无限增长。
pub struct ElementManager {
    /// 所有绘图元素
    elements: Vec<DrawingElement>,
    /// 最大元素数量限制（防止内存无限增长）
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
        // 如果达到限制，移除最早的非文本元素
        if self.elements.len() >= self.max_elements {
            // 优先移除最早的非文本元素
            if let Some(pos) = self
                .elements
                .iter()
                .position(|e| e.tool != DrawingTool::Text)
            {
                self.elements.remove(pos);
            } else {
                // 如果全是文本元素，移除最早的
                self.elements.remove(0);
            }
            #[cfg(debug_assertions)]
            eprintln!(
                "ElementManager: 达到最大元素限制 {}，已移除最早的元素",
                self.max_elements
            );
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

    /// 获取指定位置的元素索引（考虑选择框约束）
    pub fn get_element_at_position(&self, x: i32, y: i32) -> Option<usize> {
        // 从后往前查找（最后绘制的元素在最上层）
        for (index, element) in self.elements.iter().enumerate().rev() {
            if element.contains_point(x, y) {
                return Some(index);
            }
        }
        None
    }

    /// 获取指定位置的元素索引（带选择框可见性检查）
    pub fn get_element_at_position_with_selection(
        &self,
        x: i32,
        y: i32,
        selection_rect: Option<RECT>,
    ) -> Option<usize> {
        // 从后往前查找（最后绘制的元素在最上层）
        for (index, element) in self.elements.iter().enumerate().rev() {
            if element.tool == DrawingTool::Pen {
                continue;
            }

            if element.contains_point(x, y) {
                // 如果有选择框，检查元素是否在选择框内可见
                if let Some(sel_rect) = selection_rect {
                    if self.is_element_visible_in_selection(element, &sel_rect) {
                        return Some(index);
                    }
                } else {
                    return Some(index);
                }
            }
        }
        None
    }

    /// 检查元素是否在选择框内可见
    pub fn is_element_visible_in_selection(
        &self,
        element: &DrawingElement,
        selection_rect: &RECT,
    ) -> bool {
        let element_rect = element.get_bounding_rect();

        // 检查元素是否与选择框有交集
        !(element_rect.right < selection_rect.left
            || element_rect.left > selection_rect.right
            || element_rect.bottom < selection_rect.top
            || element_rect.top > selection_rect.bottom)
    }

    /// 设置选中状态
    pub fn set_selected(&mut self, index: Option<usize>) {
        // 清除所有选中状态
        for element in &mut self.elements {
            element.selected = false;
        }

        // 设置新的选中状态
        if let Some(idx) = index
            && idx < self.elements.len()
        {
            self.elements[idx].selected = true;
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

    /// 恢复状态（用于撤销/重做）
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

    // ==================== 命令模式支持 ====================

    /// 应用撤销操作
    pub fn apply_undo(&mut self, action: &DrawingAction) {
        match action {
            DrawingAction::AddElement { index, .. } => {
                // 撤销添加 = 删除元素
                if *index < self.elements.len() {
                    self.elements.remove(*index);
                }
            }
            DrawingAction::RemoveElement { element, index } => {
                // 撤销删除 = 恢复元素
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
                // 撤销移动 = 恢复原位置
                if let Some(element) = self.elements.get_mut(*index) {
                    element.points = old_points.clone();
                    element.rect = *old_rect;
                    // 缓存失效由 DrawingManager 的 geometry_cache 管理
                }
            }
            DrawingAction::ResizeElement {
                index,
                old_points,
                old_rect,
                old_font_size,
                ..
            } => {
                // 撤销调整大小 = 恢复原尺寸
                if let Some(element) = self.elements.get_mut(*index) {
                    element.points = old_points.clone();
                    element.rect = *old_rect;
                    element.font_size = *old_font_size;
                    // 缓存失效由 DrawingManager 的 geometry_cache 管理
                }
            }
            DrawingAction::ModifyText {
                index,
                old_text,
                old_points,
                old_rect,
                ..
            } => {
                // 撤销文本修改 = 恢复原文本
                if let Some(element) = self.elements.get_mut(*index) {
                    element.text = old_text.clone();
                    element.points = old_points.clone();
                    element.rect = *old_rect;
                    // 缓存失效由 DrawingManager 的 geometry_cache 管理
                }
            }
            DrawingAction::ModifyProperty {
                index,
                old_color,
                old_thickness,
                ..
            } => {
                // 撤销属性修改 = 恢复原属性
                if let Some(element) = self.elements.get_mut(*index) {
                    element.color = *old_color;
                    element.thickness = *old_thickness;
                }
            }
            DrawingAction::Compound { actions } => {
                // 复合操作：逆序撤销所有子操作
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
                // 重做添加 = 插入元素
                if *index <= self.elements.len() {
                    self.elements.insert(*index, element.clone());
                }
            }
            DrawingAction::RemoveElement { index, .. } => {
                // 重做删除 = 删除元素
                if *index < self.elements.len() {
                    self.elements.remove(*index);
                }
            }
            DrawingAction::MoveElement { index, dx, dy, .. } => {
                // 重做移动 = 再次移动
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
                // 重做调整大小 = 应用新尺寸
                if let Some(element) = self.elements.get_mut(*index) {
                    element.points = new_points.clone();
                    element.rect = *new_rect;
                    element.font_size = *new_font_size;
                    // 缓存失效由 DrawingManager 的 geometry_cache 管理
                }
            }
            DrawingAction::ModifyText {
                index,
                new_text,
                new_points,
                new_rect,
                ..
            } => {
                // 重做文本修改 = 应用新文本
                if let Some(element) = self.elements.get_mut(*index) {
                    element.text = new_text.clone();
                    element.points = new_points.clone();
                    element.rect = *new_rect;
                    // 缓存失效由 DrawingManager 的 geometry_cache 管理
                }
            }
            DrawingAction::ModifyProperty {
                index,
                new_color,
                new_thickness,
                ..
            } => {
                // 重做属性修改 = 应用新属性
                if let Some(element) = self.elements.get_mut(*index) {
                    element.color = *new_color;
                    element.thickness = *new_thickness;
                }
            }
            DrawingAction::Compound { actions } => {
                // 复合操作：顺序重做所有子操作
                for action in actions {
                    self.apply_redo(action);
                }
            }
        }
    }

    /// 在指定位置插入元素
    pub fn insert_element(&mut self, index: usize, element: DrawingElement) {
        if index <= self.elements.len() {
            self.elements.insert(index, element);
        }
    }
}
