//! 编辑状态处理器
//!
//! 处理用户在已选择区域上进行绘图编辑时的事件

use super::{AppStateHandler, StateContext, StateTransition};
use crate::message::{Command, DrawingMessage};
use crate::types::DrawingTool;
use windows::Win32::Foundation::RECT;

/// 编辑状态
pub struct EditingState {
    /// 选择区域
    pub selection: RECT,
    /// 当前绘图工具
    pub tool: DrawingTool,
}

impl EditingState {
    pub fn new(selection: RECT, tool: DrawingTool) -> Self {
        Self { selection, tool }
    }
}

impl AppStateHandler for EditingState {
    fn name(&self) -> &'static str {
        "Editing"
    }

    fn handle_mouse_move(
        &mut self,
        x: i32,
        y: i32,
        ctx: &mut StateContext<'_>,
    ) -> (Vec<Command>, bool, StateTransition) {
        let mut commands = Vec::new();

        // UI -> Drawing -> Screenshot 的处理顺序
        let (ui_commands, ui_consumed) = ctx.ui.handle_mouse_move(x, y);
        commands.extend(ui_commands);

        if !ui_consumed {
            let selection_rect = ctx.screenshot.get_selection();
            let (drawing_commands, drawing_consumed) =
                ctx.drawing.handle_mouse_move(x, y, selection_rect);
            commands.extend(drawing_commands);

            if !drawing_consumed && !ctx.drawing.is_dragging() {
                let (screenshot_commands, _screenshot_consumed) =
                    ctx.screenshot.handle_mouse_move(x, y);
                commands.extend(screenshot_commands);
            }
        }

        (commands, true, StateTransition::None)
    }

    fn handle_mouse_down(
        &mut self,
        x: i32,
        y: i32,
        ctx: &mut StateContext<'_>,
    ) -> (Vec<Command>, bool, StateTransition) {
        let mut commands = Vec::new();

        let (ui_commands, ui_consumed) = ctx.ui.handle_mouse_down(x, y);
        commands.extend(ui_commands);

        if !ui_consumed {
            let selection_rect = ctx.screenshot.get_selection();
            let (drawing_commands, drawing_consumed) =
                ctx.drawing.handle_mouse_down(x, y, selection_rect);
            commands.extend(drawing_commands);

            if !drawing_consumed {
                let (screenshot_commands, screenshot_consumed) =
                    ctx.screenshot.handle_mouse_down(x, y);
                commands.extend(screenshot_commands);

                if !screenshot_consumed {
                    commands.extend(
                        ctx.drawing
                            .handle_message(DrawingMessage::SelectElement(None)),
                    );
                }
            }
        }

        (commands, true, StateTransition::None)
    }

    fn handle_mouse_up(
        &mut self,
        x: i32,
        y: i32,
        ctx: &mut StateContext<'_>,
    ) -> (Vec<Command>, bool, StateTransition) {
        let mut commands = Vec::new();

        let (ui_commands, ui_consumed) = ctx.ui.handle_mouse_up(x, y);
        commands.extend(ui_commands);

        if !ui_consumed {
            let (drawing_commands, drawing_consumed) = ctx.drawing.handle_mouse_up(x, y);
            commands.extend(drawing_commands);

            if !drawing_consumed {
                let (screenshot_commands, _screenshot_consumed) =
                    ctx.screenshot.handle_mouse_up(x, y);
                commands.extend(screenshot_commands);
            }
        }

        (commands, true, StateTransition::None)
    }

    fn handle_double_click(
        &mut self,
        x: i32,
        y: i32,
        ctx: &mut StateContext<'_>,
    ) -> (Vec<Command>, StateTransition) {
        let mut commands = Vec::new();

        // UI层优先处理
        commands.extend(ctx.ui.handle_double_click(x, y));

        if commands.is_empty() {
            let selection_rect = ctx.screenshot.get_selection();
            // 优先让Drawing处理（双击文本进入编辑）
            let dcmds = ctx
                .drawing
                .handle_double_click(x, y, selection_rect.as_ref());
            if dcmds.is_empty() {
                // 若未消费，再交给Screenshot（双击确认选择保存）
                commands.extend(ctx.screenshot.handle_double_click(x, y));
            } else {
                commands.extend(dcmds);
            }
        }

        (commands, StateTransition::None)
    }

    fn handle_key_input(
        &mut self,
        key: u32,
        ctx: &mut StateContext<'_>,
    ) -> (Vec<Command>, StateTransition) {
        // 编辑状态下传递给各个管理器处理
        let mut commands = Vec::new();

        commands.extend(ctx.system.handle_key_input(key));
        if commands.is_empty() {
            commands.extend(ctx.drawing.handle_key_input(key));
        }
        if commands.is_empty() {
            commands.extend(ctx.ui.handle_key_input(key));
        }

        (commands, StateTransition::None)
    }

    fn handle_text_input(&mut self, character: char, ctx: &mut StateContext<'_>) -> Vec<Command> {
        // 文本输入主要由绘图管理器处理（文本工具）
        ctx.drawing.handle_text_input(character)
    }

    fn on_enter(&mut self, ctx: &mut StateContext<'_>) {
        // 进入编辑状态，更新选择区域和工具
        ctx.screenshot.update_selection(self.selection);
        ctx.drawing.set_current_tool(self.tool);
    }
}
