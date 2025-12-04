use super::elements::ElementManager;
use crate::types::DrawingElement;

/// 历史状态
#[derive(Clone, Debug)]
pub struct HistoryState {
    /// 绘图元素快照
    pub elements: Vec<DrawingElement>,
    /// 当时选中的元素索引（用于恢复选择状态）
    pub selected_element: Option<usize>,
    /// 变更的元素索引列表（用于增量式缓存失效）
    pub changed_indices: Vec<usize>,
}

/// 历史管理器
pub struct HistoryManager {
    /// 历史记录栈
    history_stack: Vec<HistoryState>,
    /// 当前位置
    current_position: usize,
    /// 最大历史记录数
    max_history: usize,
}

impl Default for HistoryManager {
    fn default() -> Self {
        Self::new()
    }
}

impl HistoryManager {
    /// 创建新的历史管理器
    pub fn new() -> Self {
        Self {
            history_stack: Vec::new(),
            current_position: 0,
            max_history: 20,
        }
    }

    /// 保存当前状态
    pub fn save_state(
        &mut self,
        element_manager: &ElementManager,
        selected_element: Option<usize>,
    ) {
        // 计算变更的元素索引（与上一个状态比较）
        let changed_indices = self.compute_changed_indices(element_manager);
        
        let state = HistoryState {
            elements: element_manager.get_elements().clone(),
            selected_element,
            changed_indices,
        };

        // 如果当前位置不在栈顶，清除后面的历史
        if self.current_position < self.history_stack.len() {
            self.history_stack.truncate(self.current_position);
        }

        // 添加新状态
        self.history_stack.push(state);
        self.current_position = self.history_stack.len();

        // 限制历史记录数量
        if self.history_stack.len() > self.max_history {
            self.history_stack.remove(0);
            self.current_position = self.history_stack.len();
        }
    }

    /// 撤销操作（逐步撤销）
    /// 语义：current_position 始终指向“当前状态之后”的位置，范围 [0..=len]
    /// - 当 > 0 时，向前移动一格，并返回对应的历史状态
    pub fn undo(&mut self) -> Option<(Vec<DrawingElement>, Option<usize>)> {
        if self.current_position > 0 {
            self.current_position -= 1;
            let state = &self.history_stack[self.current_position];
            Some((state.elements.clone(), state.selected_element))
        } else {
            None
        }
    }

    /// 重做操作
    pub fn redo(&mut self) -> Option<(Vec<DrawingElement>, Option<usize>)> {
        if self.current_position < self.history_stack.len() {
            let state = &self.history_stack[self.current_position];
            self.current_position += 1;
            Some((state.elements.clone(), state.selected_element))
        } else {
            None
        }
    }

    /// 是否可以撤销
    pub fn can_undo(&self) -> bool {
        self.current_position > 0
    }

    /// 是否可以重做
    pub fn can_redo(&self) -> bool {
        self.current_position < self.history_stack.len()
    }

    /// 清空历史记录
    pub fn clear(&mut self) {
        self.history_stack.clear();
        self.current_position = 0;
    }

    /// 计算与上一个状态相比变更的元素索引
    fn compute_changed_indices(&self, element_manager: &ElementManager) -> Vec<usize> {
        let current_elements = element_manager.get_elements();
        
        // 如果没有历史记录，返回所有元素索引
        if self.history_stack.is_empty() || self.current_position == 0 {
            return (0..current_elements.len()).collect();
        }
        
        // 获取上一个状态
        let prev_state = &self.history_stack[self.current_position - 1];
        let prev_elements = &prev_state.elements;
        
        let mut changed = Vec::new();
        
        // 检查新增或修改的元素
        for i in 0..current_elements.len() {
            if i >= prev_elements.len() {
                // 新增的元素
                changed.push(i);
            }
            // 注意：我们不比较元素内容，因为 DrawingElement 没有实现 PartialEq
            // 最简单的策略是标记所有新索引
        }
        
        // 如果元素数量发生变化，返回所有索引
        if current_elements.len() != prev_elements.len() {
            return (0..current_elements.len().max(prev_elements.len())).collect();
        }
        
        // 如果没有变化，返回空列表
        if changed.is_empty() {
            // 保守策略：返回最后一个元素的索引（最可能被修改）
            if !current_elements.is_empty() {
                changed.push(current_elements.len() - 1);
            }
        }
        
        changed
    }

    /// 获取最近一次操作变更的元素索引
    pub fn get_last_changed_indices(&self) -> &[usize] {
        if self.current_position > 0 && self.current_position <= self.history_stack.len() {
            &self.history_stack[self.current_position - 1].changed_indices
        } else {
            &[]
        }
    }
}
