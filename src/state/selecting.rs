//! 框选状态处理器
//!
//! 处理用户正在框选屏幕区域时的事件

use super::{AppStateHandler, SelectingContext, StateContext, StateTransition};
use crate::message::Command;
use crate::drawing::DrawingTool;

/// 框选状态
///
/// 此状态处理器只负责事件流处理，坐标数据由 `SelectionState` 统一管理。
pub struct SelectingState;

impl SelectingState {
    /// 创建框选状态处理器
    pub fn new() -> Self {
        Self
    }
}

impl Default for SelectingState {
    fn default() -> Self {
        Self::new()
    }
}

impl SelectingState {
    /// 创建 SelectingContext 进行实际操作
    fn with_selecting_context<'a, F, R>(
        &mut self,
        ctx: &'a mut StateContext<'_>,
        f: F,
    ) -> R
    where
        F: FnOnce(&mut SelectingContext<'a>) -> R,
    {
        let mut selecting_ctx = SelectingContext::from_state_context(
            ctx.screenshot,
            ctx.ui,
        );
        f(&mut selecting_ctx)
    }
}

impl AppStateHandler for SelectingState {
    fn name(&self) -> &'static str {
        "Selecting"
    }

    fn handle_mouse_move(
        &mut self,
        x: i32,
        y: i32,
        ctx: &mut StateContext<'_>,
    ) -> (Vec<Command>, bool, StateTransition) {
        // 坐标更新完全由 SelectionState 管理
        self.with_selecting_context(ctx, |selecting_ctx| {
            let (cmds, _consumed) = selecting_ctx.screenshot.handle_mouse_move(x, y);
            (cmds, true, StateTransition::None)
        })
    }

    fn handle_mouse_down(
        &mut self,
        x: i32,
        y: i32,
        ctx: &mut StateContext<'_>,
    ) -> (Vec<Command>, bool, StateTransition) {
        self.with_selecting_context(ctx, |selecting_ctx| {
            let (cmds, _consumed) = selecting_ctx.screenshot.handle_mouse_down(x, y);
            (cmds, true, StateTransition::None)
        })
    }

    fn handle_mouse_up(
        &mut self,
        x: i32,
        y: i32,
        ctx: &mut StateContext<'_>,
    ) -> (Vec<Command>, bool, StateTransition) {
        self.with_selecting_context(ctx, |selecting_ctx| {
            let (cmds, _consumed) = selecting_ctx.screenshot.handle_mouse_up(x, y);
            
            // 检查是否有有效的选择区域
            if let Some(selection) = selecting_ctx.screenshot.get_selection() {
                let transition = StateTransition::ToEditing {
                    selection,
                    tool: DrawingTool::None,
                };
                (cmds, true, transition)
            } else {
                // 没有有效选择，回到空闲状态
                (cmds, true, StateTransition::ToIdle)
            }
        })
    }

    fn handle_key_input(
        &mut self,
        _key: u32,
        _ctx: &mut StateContext<'_>,
    ) -> (Vec<Command>, StateTransition) {
        // 框选状态下不处理其他按键（ESC 由 App 层处理）
        (vec![], StateTransition::None)
    }
}
