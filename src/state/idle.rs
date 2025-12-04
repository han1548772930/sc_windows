//! 空闲状态处理器
//!
//! 处理应用程序在空闲状态下的事件（等待热键触发截图）

use super::{AppStateHandler, StateContext, StateTransition};
use crate::message::Command;

/// 空闲状态
pub struct IdleState;

impl IdleState {
    pub fn new() -> Self {
        Self
    }
}

impl Default for IdleState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppStateHandler for IdleState {
    fn name(&self) -> &'static str {
        "Idle"
    }

    fn handle_mouse_move(
        &mut self,
        x: i32,
        y: i32,
        ctx: &mut StateContext<'_>,
    ) -> (Vec<Command>, bool, StateTransition) {
        // 空闲状态也要处理鼠标移动（自动高亮等）
        let (cmds, _consumed) = ctx.screenshot.handle_mouse_move(x, y);
        (cmds, false, StateTransition::None)
    }

    fn handle_mouse_down(
        &mut self,
        x: i32,
        y: i32,
        ctx: &mut StateContext<'_>,
    ) -> (Vec<Command>, bool, StateTransition) {
        // 从空闲进入框选状态
        let (cmds, consumed) = ctx.screenshot.handle_mouse_down(x, y);
        if consumed || ctx.screenshot.has_screenshot() {
            let transition = StateTransition::ToSelecting {
                start_x: x,
                start_y: y,
            };
            (cmds, true, transition)
        } else {
            (cmds, consumed, StateTransition::None)
        }
    }

    fn handle_mouse_up(
        &mut self,
        _x: i32,
        _y: i32,
        _ctx: &mut StateContext<'_>,
    ) -> (Vec<Command>, bool, StateTransition) {
        // 空闲状态不处理鼠标释放
        (vec![], false, StateTransition::None)
    }

    fn handle_key_input(
        &mut self,
        key: u32,
        ctx: &mut StateContext<'_>,
    ) -> (Vec<Command>, StateTransition) {
        // 空闲状态下只处理系统按键
        let cmds = ctx.system.handle_key_input(key);
        (cmds, StateTransition::None)
    }

    fn on_enter(&mut self, ctx: &mut StateContext<'_>) {
        // 进入空闲状态时重置各管理器
        ctx.screenshot.reset_state();
        ctx.drawing.reset_state();
        ctx.ui.reset_state();
    }
}
