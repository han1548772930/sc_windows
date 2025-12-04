use super::elements::ElementManager;
use crate::types::DrawingElement;
use windows::Win32::Foundation::{POINT, RECT};
use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;

/// 绘图操作（命令模式）
/// 每个 Action 都包含执行和撤销所需的全部信息
#[derive(Clone, Debug)]
pub enum DrawingAction {
    /// 添加元素
    AddElement {
        /// 添加的元素（用于重做时恢复）
        element: DrawingElement,
        /// 元素在列表中的索引
        index: usize,
    },
    /// 删除元素
    RemoveElement {
        /// 被删除的元素（用于撤销时恢复）
        element: DrawingElement,
        /// 元素原来的索引
        index: usize,
    },
    /// 移动元素
    MoveElement {
        /// 元素索引
        index: usize,
        /// X方向位移
        dx: i32,
        /// Y方向位移
        dy: i32,
        /// 移动前的点列表（用于精确撤销）
        old_points: Vec<POINT>,
        /// 移动前的边界矩形
        old_rect: RECT,
    },
    /// 调整元素大小
    ResizeElement {
        /// 元素索引
        index: usize,
        /// 调整前的点列表
        old_points: Vec<POINT>,
        /// 调整前的边界矩形
        old_rect: RECT,
        /// 调整前的字体大小（文本元素）
        old_font_size: f32,
        /// 调整后的点列表
        new_points: Vec<POINT>,
        /// 调整后的边界矩形
        new_rect: RECT,
        /// 调整后的字体大小
        new_font_size: f32,
    },
    /// 修改文本内容
    ModifyText {
        /// 元素索引
        index: usize,
        /// 修改前的文本
        old_text: String,
        /// 修改后的文本
        new_text: String,
        /// 修改前的点列表（文本框大小可能变化）
        old_points: Vec<POINT>,
        /// 修改前的边界矩形
        old_rect: RECT,
        /// 修改后的点列表
        new_points: Vec<POINT>,
        /// 修改后的边界矩形
        new_rect: RECT,
    },
    /// 修改元素属性（颜色、线宽等）
    ModifyProperty {
        /// 元素索引
        index: usize,
        /// 修改前的颜色
        old_color: D2D1_COLOR_F,
        /// 修改前的线宽
        old_thickness: f32,
        /// 修改后的颜色
        new_color: D2D1_COLOR_F,
        /// 修改后的线宽
        new_thickness: f32,
    },
    /// 复合操作（多个操作作为一个撤销单元）
    Compound {
        /// 子操作列表
        actions: Vec<DrawingAction>,
    },
}

impl DrawingAction {
    /// 获取受影响的元素索引列表（用于缓存失效）
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

/// 历史记录项（命令模式）
#[derive(Clone, Debug)]
pub struct HistoryEntry {
    /// 执行的操作
    pub action: DrawingAction,
    /// 操作前的选中元素
    pub selected_before: Option<usize>,
    /// 操作后的选中元素
    pub selected_after: Option<usize>,
}

/// 历史状态（保留兼容性）
#[derive(Clone, Debug)]
pub struct HistoryState {
    /// 绘图元素快照
    pub elements: Vec<DrawingElement>,
    /// 当时选中的元素索引（用于恢复选择状态）
    pub selected_element: Option<usize>,
    /// 变更的元素索引列表（用于增量式缓存失效）
    pub changed_indices: Vec<usize>,
}

/// 历史管理器（命令模式）
/// 使用增量式操作记录代替完整快照，大幅减少内存占用
pub struct HistoryManager {
    /// 基准状态（所有操作的起点）
    base_state: Option<HistoryState>,
    /// 操作历史栈（命令模式）
    action_stack: Vec<HistoryEntry>,
    /// 当前位置（指向下一个要撤销的操作）
    current_position: usize,
    /// 最大历史记录数
    max_history: usize,
    /// 兼容模式：使用旧的快照栈（用于平滑过渡）
    legacy_stack: Vec<HistoryState>,
    /// 是否使用命令模式
    use_command_mode: bool,
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
            base_state: None,
            action_stack: Vec::new(),
            current_position: 0,
            max_history: 50, // 命令模式占用内存少，可以增加历史记录数
            legacy_stack: Vec::new(),
            use_command_mode: true, // 默认启用命令模式
        }
    }

    /// 记录操作（命令模式）
    pub fn record_action(
        &mut self,
        action: DrawingAction,
        selected_before: Option<usize>,
        selected_after: Option<usize>,
    ) {
        if !self.use_command_mode {
            return;
        }

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
        if !self.use_command_mode || self.current_position == 0 {
            return None;
        }

        self.current_position -= 1;
        let entry = &self.action_stack[self.current_position];
        Some((entry.action.clone(), entry.selected_before))
    }

    /// 重做操作（命令模式）
    pub fn redo_action(&mut self) -> Option<(DrawingAction, Option<usize>)> {
        if !self.use_command_mode || self.current_position >= self.action_stack.len() {
            return None;
        }

        let entry = &self.action_stack[self.current_position];
        self.current_position += 1;
        Some((entry.action.clone(), entry.selected_after))
    }

    /// 保存当前状态（兼容旧接口）
    /// 注意：命令模式下此方法仅设置基准状态，不会每次都创建快照
    pub fn save_state(
        &mut self,
        element_manager: &ElementManager,
        selected_element: Option<usize>,
    ) {
        if self.use_command_mode {
            // 命令模式：仅在没有基准状态时保存
            if self.base_state.is_none() {
                self.base_state = Some(HistoryState {
                    elements: element_manager.get_elements().clone(),
                    selected_element,
                    changed_indices: vec![],
                });
            }
            return;
        }

        // 兼容模式：使用旧的快照逻辑
        let changed_indices = self.compute_changed_indices_legacy(element_manager);
        
        let state = HistoryState {
            elements: element_manager.get_elements().clone(),
            selected_element,
            changed_indices,
        };

        if self.current_position < self.legacy_stack.len() {
            self.legacy_stack.truncate(self.current_position);
        }

        self.legacy_stack.push(state);
        self.current_position = self.legacy_stack.len();

        if self.legacy_stack.len() > self.max_history {
            self.legacy_stack.remove(0);
            self.current_position = self.legacy_stack.len();
        }
    }

    /// 撤销操作（兼容旧接口）
    pub fn undo(&mut self) -> Option<(Vec<DrawingElement>, Option<usize>)> {
        if self.use_command_mode {
            // 命令模式不支持此接口，返回 None
            // 调用方应使用 undo_action + apply_undo
            return None;
        }

        if self.current_position > 0 {
            self.current_position -= 1;
            let state = &self.legacy_stack[self.current_position];
            Some((state.elements.clone(), state.selected_element))
        } else {
            None
        }
    }

    /// 重做操作（兼容旧接口）
    pub fn redo(&mut self) -> Option<(Vec<DrawingElement>, Option<usize>)> {
        if self.use_command_mode {
            return None;
        }

        if self.current_position < self.legacy_stack.len() {
            let state = &self.legacy_stack[self.current_position];
            self.current_position += 1;
            Some((state.elements.clone(), state.selected_element))
        } else {
            None
        }
    }

    /// 是否可以撤销
    pub fn can_undo(&self) -> bool {
        if self.use_command_mode {
            self.current_position > 0
        } else {
            self.current_position > 0
        }
    }

    /// 是否可以重做
    pub fn can_redo(&self) -> bool {
        if self.use_command_mode {
            self.current_position < self.action_stack.len()
        } else {
            self.current_position < self.legacy_stack.len()
        }
    }

    /// 清空历史记录
    pub fn clear(&mut self) {
        self.base_state = None;
        self.action_stack.clear();
        self.legacy_stack.clear();
        self.current_position = 0;
    }

    /// 获取最近一次操作变更的元素索引
    pub fn get_last_changed_indices(&self) -> Vec<usize> {
        if self.use_command_mode {
            if self.current_position > 0 && self.current_position <= self.action_stack.len() {
                self.action_stack[self.current_position - 1]
                    .action
                    .affected_indices()
            } else {
                vec![]
            }
        } else {
            if self.current_position > 0 && self.current_position <= self.legacy_stack.len() {
                self.legacy_stack[self.current_position - 1]
                    .changed_indices
                    .clone()
            } else {
                vec![]
            }
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

    /// 检查是否使用命令模式
    pub fn is_command_mode(&self) -> bool {
        self.use_command_mode
    }

    /// 计算与上一个状态相比变更的元素索引（兼容模式）
    fn compute_changed_indices_legacy(&self, element_manager: &ElementManager) -> Vec<usize> {
        let current_elements = element_manager.get_elements();
        
        if self.legacy_stack.is_empty() || self.current_position == 0 {
            return (0..current_elements.len()).collect();
        }
        
        let prev_state = &self.legacy_stack[self.current_position - 1];
        let prev_elements = &prev_state.elements;
        
        let mut changed = Vec::new();
        
        for i in 0..current_elements.len() {
            if i >= prev_elements.len() {
                changed.push(i);
            }
        }
        
        if current_elements.len() != prev_elements.len() {
            return (0..current_elements.len().max(prev_elements.len())).collect();
        }
        
        if changed.is_empty() && !current_elements.is_empty() {
            changed.push(current_elements.len() - 1);
        }
        
        changed
    }
}
