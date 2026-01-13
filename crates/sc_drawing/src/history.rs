use crate::element::{Color, DrawingElement, Point, Rect};

// ==================== 绘图操作（命令模式） ====================

/// 绘图操作枚举
///
/// 每个 Action 都包含执行和撤销所需的全部信息。
#[derive(Clone, Debug)]
pub enum DrawingAction {
    /// 添加元素
    AddElement {
        element: DrawingElement,
        index: usize,
    },
    /// 删除元素
    RemoveElement {
        element: DrawingElement,
        index: usize,
    },
    /// 移动元素
    MoveElement {
        index: usize,
        dx: i32,
        dy: i32,
        old_points: Vec<Point>,
        old_rect: Rect,
    },
    /// 调整元素大小
    ResizeElement {
        index: usize,
        old_points: Vec<Point>,
        old_rect: Rect,
        old_font_size: f32,
        new_points: Vec<Point>,
        new_rect: Rect,
        new_font_size: f32,
    },
    /// 修改文本
    ModifyText {
        index: usize,
        old_text: String,
        new_text: String,
        old_points: Vec<Point>,
        old_rect: Rect,
        new_points: Vec<Point>,
        new_rect: Rect,
    },
    /// 修改属性
    ModifyProperty {
        index: usize,
        old_color: Color,
        old_thickness: f32,
        new_color: Color,
        new_thickness: f32,
    },
    /// 复合操作
    Compound { actions: Vec<DrawingAction> },
}

impl DrawingAction {
    /// 获取受影响的元素索引
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

// ==================== 绘图 Action 历史（带选中状态） ====================

/// 历史记录项（绘图命令模式）
#[derive(Clone, Debug)]
pub struct HistoryEntry {
    /// 执行的操作
    pub action: DrawingAction,
    /// 操作前的选中元素
    pub selected_before: Option<usize>,
    /// 操作后的选中元素
    pub selected_after: Option<usize>,
}

/// 历史状态（用于基准快照/兼容）
#[derive(Clone, Debug)]
pub struct HistoryState {
    /// 绘图元素快照
    pub elements: Vec<DrawingElement>,
    /// 当时选中的元素索引（用于恢复选择状态）
    pub selected_element: Option<usize>,
    /// 变更的元素索引列表（用于增量式缓存失效）
    pub changed_indices: Vec<usize>,
}

/// 绘图历史管理器（命令模式）
///
/// 记录 `DrawingAction`，并保留选中元素的前/后状态。
pub struct ActionHistory {
    /// 基准状态（所有操作的起点）
    base_state: Option<HistoryState>,
    /// 操作历史栈（命令模式）
    action_stack: Vec<HistoryEntry>,
    /// 当前位置（指向下一个要撤销的操作）
    current_position: usize,
    /// 最大历史记录数
    max_history: usize,
}

impl Default for ActionHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl ActionHistory {
    /// 创建新的历史管理器
    pub fn new() -> Self {
        Self {
            base_state: None,
            action_stack: Vec::new(),
            current_position: 0,
            max_history: 50,
        }
    }

    /// 记录操作（命令模式）
    pub fn record_action(
        &mut self,
        action: DrawingAction,
        selected_before: Option<usize>,
        selected_after: Option<usize>,
    ) {
        // 如果当前位置不在栈顶，清除后面的历史
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

        // 限制历史记录数量
        if self.action_stack.len() > self.max_history {
            self.action_stack.remove(0);
            self.current_position = self.action_stack.len();
        }
    }

    /// 撤销操作（命令模式）
    /// 返回需要应用的操作和选中状态
    pub fn undo_action(&mut self) -> Option<(DrawingAction, Option<usize>)> {
        if self.current_position == 0 {
            return None;
        }

        self.current_position -= 1;
        let entry = &self.action_stack[self.current_position];
        Some((entry.action.clone(), entry.selected_before))
    }

    /// 重做操作（命令模式）
    pub fn redo_action(&mut self) -> Option<(DrawingAction, Option<usize>)> {
        if self.current_position >= self.action_stack.len() {
            return None;
        }

        let entry = &self.action_stack[self.current_position];
        self.current_position += 1;
        Some((entry.action.clone(), entry.selected_after))
    }

    /// 保存当前状态（设置基准状态）
    pub fn save_state(&mut self, elements: &[DrawingElement], selected_element: Option<usize>) {
        // 仅在没有基准状态时保存
        if self.base_state.is_none() {
            self.base_state = Some(HistoryState {
                elements: elements.to_vec(),
                selected_element,
                changed_indices: vec![],
            });
        }
    }

    /// 是否可以撤销
    pub fn can_undo(&self) -> bool {
        self.current_position > 0
    }

    /// 是否可以重做
    pub fn can_redo(&self) -> bool {
        self.current_position < self.action_stack.len()
    }

    /// 清空历史记录
    pub fn clear(&mut self) {
        self.base_state = None;
        self.action_stack.clear();
        self.current_position = 0;
    }

    /// 获取最近一次操作变更的元素索引
    pub fn get_last_changed_indices(&self) -> Vec<usize> {
        if self.current_position > 0 && self.current_position <= self.action_stack.len() {
            self.action_stack[self.current_position - 1]
                .action
                .affected_indices()
        } else {
            vec![]
        }
    }

    /// 获取基准状态（命令模式）
    pub fn get_base_state(&self) -> Option<&HistoryState> {
        self.base_state.as_ref()
    }

    /// 设置基准状态（命令模式）
    pub fn set_base_state(&mut self, elements: Vec<DrawingElement>, selected: Option<usize>) {
        self.base_state = Some(HistoryState {
            elements,
            selected_element: selected,
            changed_indices: vec![],
        });
    }
}

// ==================== 通用历史管理器 ====================

/// 命令接口（用于撤销/重做）
pub trait Command: std::fmt::Debug {
    /// 执行命令
    fn execute(&mut self);

    /// 撤销命令
    fn undo(&mut self);

    /// 获取命令描述
    fn description(&self) -> &str;
}

/// 历史记录管理器
///
/// 管理命令历史，支持撤销和重做操作。
#[derive(Debug, Default)]
pub struct HistoryManager<C> {
    /// 撤销栈
    undo_stack: Vec<C>,
    /// 重做栈
    redo_stack: Vec<C>,
    /// 最大历史记录数
    max_history: usize,
}

impl<C> HistoryManager<C> {
    /// 创建新的历史记录管理器
    pub fn new() -> Self {
        Self::with_capacity(100)
    }

    /// 创建指定容量的历史记录管理器
    pub fn with_capacity(max_history: usize) -> Self {
        Self {
            undo_stack: Vec::with_capacity(max_history),
            redo_stack: Vec::new(),
            max_history,
        }
    }

    /// 添加命令到历史记录
    ///
    /// 添加新命令会清空重做栈。
    pub fn push(&mut self, command: C) {
        // 清空重做栈
        self.redo_stack.clear();

        // 如果超过最大历史记录数，移除最旧的
        if self.undo_stack.len() >= self.max_history {
            self.undo_stack.remove(0);
        }

        self.undo_stack.push(command);
    }

    /// 撤销最后一个命令
    ///
    /// 返回被撤销的命令的引用（如果有）。
    /// 调用者需要在返回后执行实际的撤销逻辑。
    pub fn undo(&mut self) -> Option<&C> {
        if let Some(command) = self.undo_stack.pop() {
            self.redo_stack.push(command);
            self.redo_stack.last()
        } else {
            None
        }
    }

    /// 重做上一个撤销的命令
    ///
    /// 返回被重做的命令的引用（如果有）。
    /// 调用者需要在返回后执行实际的重做逻辑。
    pub fn redo(&mut self) -> Option<&C> {
        if let Some(command) = self.redo_stack.pop() {
            self.undo_stack.push(command);
            self.undo_stack.last()
        } else {
            None
        }
    }

    /// 检查是否可以撤销
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// 检查是否可以重做
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// 获取撤销栈大小
    pub fn undo_count(&self) -> usize {
        self.undo_stack.len()
    }

    /// 获取重做栈大小
    pub fn redo_count(&self) -> usize {
        self.redo_stack.len()
    }

    /// 清空所有历史记录
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    /// 获取撤销栈的只读引用
    pub fn undo_stack(&self) -> &[C] {
        &self.undo_stack
    }

    /// 获取重做栈的只读引用
    pub fn redo_stack(&self) -> &[C] {
        &self.redo_stack
    }
}

/// 简单的历史管理器（用于存储状态快照）
///
/// 适用于不需要命令模式的场景，直接存储状态快照。
#[derive(Debug, Default)]
pub struct SimpleHistory<T> {
    /// 历史状态列表
    states: Vec<T>,
    /// 当前状态索引
    current_index: isize,
    /// 最大历史记录数
    max_states: usize,
}

impl<T: Clone> SimpleHistory<T> {
    /// 创建新的简单历史管理器
    pub fn new(max_states: usize) -> Self {
        Self {
            states: Vec::with_capacity(max_states),
            current_index: -1,
            max_states,
        }
    }

    /// 保存当前状态
    pub fn save(&mut self, state: T) {
        // 如果不在最新状态，丢弃后面的状态
        if self.current_index >= 0 && (self.current_index as usize) < self.states.len() - 1 {
            self.states.truncate(self.current_index as usize + 1);
        }

        // 如果超过最大状态数，移除最旧的
        if self.states.len() >= self.max_states {
            self.states.remove(0);
        }

        self.states.push(state);
        self.current_index = self.states.len() as isize - 1;
    }

    /// 撤销到上一个状态
    pub fn undo(&mut self) -> Option<&T> {
        if self.current_index > 0 {
            self.current_index -= 1;
            self.states.get(self.current_index as usize)
        } else {
            None
        }
    }

    /// 重做到下一个状态
    pub fn redo(&mut self) -> Option<&T> {
        if self.current_index >= 0 && (self.current_index as usize) < self.states.len() - 1 {
            self.current_index += 1;
            self.states.get(self.current_index as usize)
        } else {
            None
        }
    }

    /// 获取当前状态
    pub fn current(&self) -> Option<&T> {
        if self.current_index >= 0 {
            self.states.get(self.current_index as usize)
        } else {
            None
        }
    }

    /// 检查是否可以撤销
    pub fn can_undo(&self) -> bool {
        self.current_index > 0
    }

    /// 检查是否可以重做
    pub fn can_redo(&self) -> bool {
        self.current_index >= 0 && (self.current_index as usize) < self.states.len() - 1
    }

    /// 清空历史
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

        // 添加新命令应清空重做栈
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
