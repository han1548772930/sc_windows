//! 状态模式模块
//!
//! 将应用程序的不同状态（空闲、框选、编辑、处理中）封装为独立的处理器，
//! 实现关注点分离，提高代码可维护性。

mod idle;
mod selecting;
mod editing;
mod processing;
pub mod types;

// Re-export types for convenience
pub use types::{AppState, ProcessingOperation};

pub use idle::IdleState;
pub use selecting::SelectingState;
pub use editing::EditingState;
pub use processing::ProcessingState;

use crate::drawing::{DrawingManager, DrawingTool};
use crate::message::Command;
use crate::screenshot::ScreenshotManager;
use crate::system::SystemManager;
use crate::ui::UIManager;
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

// ========== 细粒度上下文类型 ==========

/// 空闲状态上下文
/// 
/// IdleState 只需要访问系统管理器（托盘消息等）
pub struct IdleContext<'a> {
    pub system: &'a mut SystemManager,
}

impl<'a> From<&'a mut StateContext<'a>> for IdleContext<'a> {
    fn from(ctx: &'a mut StateContext<'a>) -> Self {
        IdleContext {
            system: ctx.system,
        }
    }
}

/// 框选状态上下文
/// 
/// SelectingState 只需要访问截图和 UI 管理器
pub struct SelectingContext<'a> {
    pub screenshot: &'a mut ScreenshotManager,
    pub ui: &'a mut UIManager,
}

impl<'a> SelectingContext<'a> {
    /// 从 StateContext 创建 SelectingContext
    pub fn from_state_context(screenshot: &'a mut ScreenshotManager, ui: &'a mut UIManager) -> Self {
        SelectingContext { screenshot, ui }
    }
}

/// 编辑状态上下文
/// 
/// EditingState 需要访问截图、绘图和 UI 管理器
pub struct EditingContext<'a> {
    pub screenshot: &'a mut ScreenshotManager,
    pub drawing: &'a mut DrawingManager,
    pub ui: &'a mut UIManager,
}

impl<'a> EditingContext<'a> {
    /// 从 StateContext 创建 EditingContext
    pub fn from_state_context(
        screenshot: &'a mut ScreenshotManager,
        drawing: &'a mut DrawingManager,
        ui: &'a mut UIManager,
    ) -> Self {
        EditingContext {
            screenshot,
            drawing,
            ui,
        }
    }
}

/// 处理中状态上下文
/// 
/// ProcessingState 需要访问截图和系统管理器（OCR 等）
pub struct ProcessingContext<'a> {
    pub screenshot: &'a mut ScreenshotManager,
    pub system: &'a mut SystemManager,
}

impl<'a> ProcessingContext<'a> {
    /// 从 StateContext 创建 ProcessingContext
    pub fn from_state_context(
        screenshot: &'a mut ScreenshotManager,
        system: &'a mut SystemManager,
    ) -> Self {
        ProcessingContext { screenshot, system }
    }
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
    ToSelecting,
    /// 转换到编辑状态
    ToEditing { selection: RECT, tool: DrawingTool },
    /// 转换到处理中状态
    ToProcessing { operation: ProcessingOperation },
}

/// 应用状态处理器 trait
/// 
/// 每个应用状态都实现此 trait，封装该状态下的事件处理逻辑。
/// 提供默认的"无操作"实现，简化不需要处理某些事件的状态实现。
pub trait AppStateHandler: Send {
    /// 获取状态名称（用于调试）
    fn name(&self) -> &'static str;
    
    /// 处理鼠标移动事件
    /// 
    /// 返回 (命令列表, 是否消费事件, 状态转换请求)
    /// 默认实现: 不处理，不消费事件
    fn handle_mouse_move(
        &mut self,
        _x: i32,
        _y: i32,
        _ctx: &mut StateContext<'_>,
    ) -> (Vec<Command>, bool, StateTransition) {
        (vec![], false, StateTransition::None)
    }

    /// 处理鼠标按下事件
    /// 默认实现: 不处理，不消费事件
    fn handle_mouse_down(
        &mut self,
        _x: i32,
        _y: i32,
        _ctx: &mut StateContext<'_>,
    ) -> (Vec<Command>, bool, StateTransition) {
        (vec![], false, StateTransition::None)
    }

    /// 处理鼠标释放事件
    /// 默认实现: 不处理，不消费事件
    fn handle_mouse_up(
        &mut self,
        _x: i32,
        _y: i32,
        _ctx: &mut StateContext<'_>,
    ) -> (Vec<Command>, bool, StateTransition) {
        (vec![], false, StateTransition::None)
    }

    /// 处理双击事件
    /// 默认实现: 不处理双击
    fn handle_double_click(
        &mut self,
        _x: i32,
        _y: i32,
        _ctx: &mut StateContext<'_>,
    ) -> (Vec<Command>, StateTransition) {
        (vec![], StateTransition::None)
    }

    /// 处理键盘输入
    /// 默认实现: 不处理键盘输入
    fn handle_key_input(
        &mut self,
        _key: u32,
        _ctx: &mut StateContext<'_>,
    ) -> (Vec<Command>, StateTransition) {
        (vec![], StateTransition::None)
    }

    /// 处理文本输入
    /// 默认实现: 不处理文本输入
    fn handle_text_input(
        &mut self,
        _character: char,
        _ctx: &mut StateContext<'_>,
    ) -> Vec<Command> {
        vec![]
    }

    /// 进入此状态时调用
    fn on_enter(&mut self, _ctx: &mut StateContext<'_>) {}

    /// 退出此状态时调用
    fn on_exit(&mut self, _ctx: &mut StateContext<'_>) {}
}

/// 创建状态处理器的工厂函数
pub fn create_state(state: &AppState) -> Box<dyn AppStateHandler> {
    match state {
        AppState::Idle => Box::new(IdleState::new()),
        AppState::Selecting => {
            Box::new(SelectingState::new())
        }
        AppState::Editing { selection, tool } => {
            Box::new(EditingState::new(*selection, *tool))
        }
        AppState::Processing { operation } => {
            Box::new(ProcessingState::new(operation.clone()))
        }
    }
}
