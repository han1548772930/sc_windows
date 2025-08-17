// 撤销/重做系统
//
// 负责管理绘图操作的历史记录

use super::elements::ElementManager;
use crate::types::DrawingElement;

/// 历史状态
#[derive(Clone, Debug)]
pub struct HistoryState {
    /// 绘图元素快照
    pub elements: Vec<DrawingElement>,
    /// 当时选中的元素索引（用于恢复选择状态）
    pub selected_element: Option<usize>,
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
        let state = HistoryState {
            elements: element_manager.get_elements().clone(),
            selected_element,
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

    /// 撤销操作
    pub fn undo(&mut self) -> Option<(Vec<DrawingElement>, Option<usize>)> {
        if self.current_position > 1 {
            self.current_position -= 1;
            let state = &self.history_stack[self.current_position - 1];
            Some((state.elements.clone(), state.selected_element))
        } else if self.current_position == 1 {
            self.current_position = 0;
            Some((Vec::new(), None)) // 返回空状态
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
}
