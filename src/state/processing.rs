//! 处理中状态处理器
//!
//! 处理应用程序在执行OCR或保存等操作时的事件

use super::{AppStateHandler, StateContext, StateTransition};
use crate::message::Command;
use crate::types::ProcessingOperation;

/// 处理中状态
pub struct ProcessingState {
    /// 处理操作类型
    pub operation: ProcessingOperation,
}

impl ProcessingState {
    pub fn new(operation: ProcessingOperation) -> Self {
        Self { operation }
    }
}

impl AppStateHandler for ProcessingState {
    fn name(&self) -> &'static str {
        "Processing"
    }

    fn handle_mouse_move(
        &mut self,
        _x: i32,
        _y: i32,
        _ctx: &mut StateContext<'_>,
    ) -> (Vec<Command>, bool, StateTransition) {
        // 处理中状态不响应鼠标移动
        (vec![], false, StateTransition::None)
    }

    fn handle_mouse_down(
        &mut self,
        _x: i32,
        _y: i32,
        _ctx: &mut StateContext<'_>,
    ) -> (Vec<Command>, bool, StateTransition) {
        // 处理中状态不响应鼠标按下
        (vec![], false, StateTransition::None)
    }

    fn handle_mouse_up(
        &mut self,
        _x: i32,
        _y: i32,
        _ctx: &mut StateContext<'_>,
    ) -> (Vec<Command>, bool, StateTransition) {
        // 处理中状态不响应鼠标释放
        (vec![], false, StateTransition::None)
    }

    fn handle_key_input(
        &mut self,
        _key: u32,
        _ctx: &mut StateContext<'_>,
    ) -> (Vec<Command>, StateTransition) {
        // 处理中状态不处理按键（ESC 由 App 层处理）
        (vec![], StateTransition::None)
    }
}
