//! 框选状态处理器
//!
//! 处理用户正在框选屏幕区域时的事件

use super::{AppStateHandler, StateContext, StateTransition};
use crate::message::Command;
use crate::types::DrawingTool;

/// 框选状态
pub struct SelectingState {
    /// 框选起点 X
    pub start_x: i32,
    /// 框选起点 Y
    pub start_y: i32,
    /// 当前位置 X
    pub current_x: i32,
    /// 当前位置 Y
    pub current_y: i32,
}

impl SelectingState {
    pub fn new(start_x: i32, start_y: i32, current_x: i32, current_y: i32) -> Self {
        Self {
            start_x,
            start_y,
            current_x,
            current_y,
        }
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
        // 同步当前点并更新框选
        self.current_x = x;
        self.current_y = y;
        let (cmds, _consumed) = ctx.screenshot.handle_mouse_move(x, y);
        (cmds, true, StateTransition::None)
    }

    fn handle_mouse_down(
        &mut self,
        x: i32,
        y: i32,
        ctx: &mut StateContext<'_>,
    ) -> (Vec<Command>, bool, StateTransition) {
        let (cmds, _consumed) = ctx.screenshot.handle_mouse_down(x, y);
        (cmds, true, StateTransition::None)
    }

    fn handle_mouse_up(
        &mut self,
        x: i32,
        y: i32,
        ctx: &mut StateContext<'_>,
    ) -> (Vec<Command>, bool, StateTransition) {
        let (cmds, _consumed) = ctx.screenshot.handle_mouse_up(x, y);
        
        // 检查是否有有效的选择区域
        if let Some(selection) = ctx.screenshot.get_selection() {
            let transition = StateTransition::ToEditing {
                selection,
                tool: DrawingTool::None,
            };
            (cmds, true, transition)
        } else {
            // 没有有效选择，回到空闲状态
            (cmds, true, StateTransition::ToIdle)
        }
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
