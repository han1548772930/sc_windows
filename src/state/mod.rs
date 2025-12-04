//! 状态模式模块
//!
//! 将应用程序的不同状态（空闲、框选、编辑、处理中）封装为独立的处理器，
//! 实现关注点分离，提高代码可维护性。

mod idle;
mod selecting;
mod editing;
mod processing;

pub use idle::IdleState;
pub use selecting::SelectingState;
pub use editing::EditingState;
pub use processing::ProcessingState;

use crate::drawing::DrawingManager;
use crate::message::Command;
use crate::screenshot::ScreenshotManager;
use crate::system::SystemManager;
use crate::ui::UIManager;
use crate::types::DrawingTool;
use windows::Win32::Foundation::RECT;

/// 状态上下文，提供对各 Manager 的访问
/// 
/// 状态处理器通过此上下文访问应用程序的各个子系统，
/// 而不是直接持有它们的引用。
pub struct StateContext<'a> {
    pub screenshot: &'a mut ScreenshotManager,
    pub drawing: &'a mut DrawingManager,
    pub ui: &'a mut UIManager,
    pub system: &'a mut SystemManager,
}

/// 状态转换请求
/// 
/// 状态处理器返回此枚举来请求状态转换
#[derive(Debug, Clone)]
pub enum StateTransition {
    /// 保持当前状态
    None,
    /// 转换到空闲状态
    ToIdle,
    /// 转换到框选状态
    ToSelecting { start_x: i32, start_y: i32 },
    /// 转换到编辑状态
    ToEditing { selection: RECT, tool: DrawingTool },
    /// 转换到处理中状态
    ToProcessing { operation: crate::types::ProcessingOperation },
}

/// 应用状态处理器 trait
/// 
/// 每个应用状态都实现此 trait，封装该状态下的事件处理逻辑。
pub trait AppStateHandler: Send {
    /// 获取状态名称（用于调试）
    fn name(&self) -> &'static str;
    
    /// 处理鼠标移动事件
    /// 
    /// 返回 (命令列表, 是否消费事件, 状态转换请求)
    fn handle_mouse_move(
        &mut self,
        x: i32,
        y: i32,
        ctx: &mut StateContext<'_>,
    ) -> (Vec<Command>, bool, StateTransition);

    /// 处理鼠标按下事件
    fn handle_mouse_down(
        &mut self,
        x: i32,
        y: i32,
        ctx: &mut StateContext<'_>,
    ) -> (Vec<Command>, bool, StateTransition);

    /// 处理鼠标释放事件
    fn handle_mouse_up(
        &mut self,
        x: i32,
        y: i32,
        ctx: &mut StateContext<'_>,
    ) -> (Vec<Command>, bool, StateTransition);

    /// 处理双击事件
    fn handle_double_click(
        &mut self,
        x: i32,
        y: i32,
        ctx: &mut StateContext<'_>,
    ) -> (Vec<Command>, StateTransition) {
        // 默认实现：不处理双击
        let _ = (x, y, ctx);
        (vec![], StateTransition::None)
    }

    /// 处理键盘输入
    fn handle_key_input(
        &mut self,
        key: u32,
        ctx: &mut StateContext<'_>,
    ) -> (Vec<Command>, StateTransition);

    /// 处理文本输入
    fn handle_text_input(
        &mut self,
        character: char,
        ctx: &mut StateContext<'_>,
    ) -> Vec<Command> {
        // 默认实现：不处理文本输入
        let _ = (character, ctx);
        vec![]
    }

    /// 进入此状态时调用
    fn on_enter(&mut self, ctx: &mut StateContext<'_>) {
        let _ = ctx;
    }

    /// 退出此状态时调用
    fn on_exit(&mut self, ctx: &mut StateContext<'_>) {
        let _ = ctx;
    }
}

/// 创建状态处理器的工厂函数
pub fn create_state(state: &crate::types::AppState) -> Box<dyn AppStateHandler> {
    match state {
        crate::types::AppState::Idle => Box::new(IdleState::new()),
        crate::types::AppState::Selecting { start_x, start_y, current_x, current_y } => {
            Box::new(SelectingState::new(*start_x, *start_y, *current_x, *current_y))
        }
        crate::types::AppState::Editing { selection, tool } => {
            Box::new(EditingState::new(*selection, *tool))
        }
        crate::types::AppState::Processing { operation } => {
            Box::new(ProcessingState::new(operation.clone()))
        }
    }
}
