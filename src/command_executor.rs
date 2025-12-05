use std::collections::VecDeque;

use windows::Win32::Foundation::HWND;

use crate::app::App;
use crate::message::{Command, DrawingMessage, Message};
use crate::ocr::PaddleOcrEngine;
use crate::settings::SettingsWindow;
use crate::utils::{command_helpers, win_api};

/// 命令队列
/// 
/// 提供命令的队列化执行，避免递归调用导致的栈溢出问题。
#[derive(Debug, Default)]
pub struct CommandQueue {
    /// 待执行的命令队列
    pending: VecDeque<Command>,
    /// 已执行的命令计数（用于调试）
    #[cfg(debug_assertions)]
    executed_count: usize,
}

impl CommandQueue {
    /// 创建新的命令队列
    pub fn new() -> Self {
        Self {
            pending: VecDeque::new(),
            #[cfg(debug_assertions)]
            executed_count: 0,
        }
    }

    /// 添加单个命令到队列
    pub fn push(&mut self, command: Command) {
        if !matches!(command, Command::None) {
            self.pending.push_back(command);
        }
    }

    /// 批量添加命令到队列
    pub fn push_batch(&mut self, commands: impl IntoIterator<Item = Command>) {
        for cmd in commands {
            self.push(cmd);
        }
    }

    /// 检查队列是否为空
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    /// 获取队列长度
    pub fn len(&self) -> usize {
        self.pending.len()
    }

    /// 清空队列
    pub fn clear(&mut self) {
        self.pending.clear();
    }

    /// 执行队列中的所有命令
    /// 
    /// 每个命令执行后可能产生新的命令，这些新命令会被添加到队列末尾。
    /// 执行继续直到队列为空。
    pub fn process_all<E: CommandExecutor + ?Sized>(&mut self, executor: &mut E, hwnd: HWND) {
        // 防止无限循环的安全阀值
        const MAX_ITERATIONS: usize = 1000;
        let mut iteration = 0;

        while let Some(command) = self.pending.pop_front() {
            #[cfg(debug_assertions)]
            {
                self.executed_count += 1;
            }

            let new_commands = executor.execute_command(command, hwnd);
            self.push_batch(new_commands);

            iteration += 1;
            if iteration >= MAX_ITERATIONS {
                #[cfg(debug_assertions)]
                eprintln!("Warning: Command queue exceeded {} iterations, breaking to prevent infinite loop", MAX_ITERATIONS);
                break;
            }
        }
    }

    /// 获取已执行命令计数（仅调试模式）
    #[cfg(debug_assertions)]
    pub fn executed_count(&self) -> usize {
        self.executed_count
    }

    /// 重置计数器（仅调试模式）
    #[cfg(debug_assertions)]
    pub fn reset_counter(&mut self) {
        self.executed_count = 0;
    }
}

/// 命令执行器 trait
pub trait CommandExecutor {
    /// 执行单个命令并返回可能产生的新命令
    fn execute_command(&mut self, command: Command, hwnd: HWND) -> Vec<Command>;

    /// 批量执行命令
    fn execute_commands(&mut self, commands: Vec<Command>, hwnd: HWND) -> Vec<Command> {
        let mut result_commands = Vec::new();
        for command in commands {
            result_commands.extend(self.execute_command(command, hwnd));
        }
        result_commands
    }

    /// 队列化执行命令直到队列为空
    /// 
    /// 使用 `CommandQueue` 来避免递归调用，确保执行顺序的可预测性。
    fn execute_command_chain(&mut self, commands: Vec<Command>, hwnd: HWND) {
        let mut queue = CommandQueue::new();
        queue.push_batch(commands);
        queue.process_all(self, hwnd);
    }
}

impl CommandExecutor for App {
    fn execute_command(&mut self, command: Command, hwnd: HWND) -> Vec<Command> {
        match command {
            Command::RequestRedraw => {
                let _ = win_api::request_redraw(hwnd);
                vec![]
            }
            Command::RequestRedrawRect(rect) => {
                let _ = win_api::request_redraw_rect(hwnd, &rect);
                vec![]
            }
            Command::UI(ui_message) => self.handle_message(Message::UI(ui_message)),
            Command::Drawing(drawing_message) => {
                // 检查是否应该禁用某些绘图命令
                let should_execute = match &drawing_message {
                    DrawingMessage::Undo => self.can_undo(),
                    _ => true,
                };

                if should_execute {
                    self.handle_message(Message::Drawing(drawing_message))
                } else {
                    vec![]
                }
            }
            Command::SelectDrawingTool(tool) => {
                let mut commands = self.select_drawing_tool(tool);
                commands.push(Command::RequestRedraw);
                commands
            }
            Command::ShowOverlay => {
                // 显示覆盖层（截图成功）
                // 覆盖层显示时异步预热OCR引擎，完成后自动发送状态更新消息
                PaddleOcrEngine::start_ocr_engine_async_with_hwnd(hwnd);
                vec![]
            }
            Command::HideOverlay | Command::HideWindow => {
                let _ = win_api::hide_window(hwnd);
                vec![]
            }
            Command::SaveSelectionToFile => {
                // 检查前置条件：是否有有效选区
                if self.has_valid_selection() {
                    self.handle_save_to_file(hwnd)
                } else {
                    vec![Command::ShowError("请先选择区域".to_string())]
                }
            }
            Command::SaveSelectionToClipboard => {
                // 检查前置条件：是否有有效选区
                if self.has_valid_selection() {
                    self.handle_save_to_clipboard(hwnd)
                } else {
                    vec![Command::ShowError("请先选择区域".to_string())]
                }
            }
            Command::UpdateToolbar => {
                self.update_toolbar_state();
                vec![]
            }
            Command::ShowSettings => {
                let _ = SettingsWindow::show(HWND::default());
                vec![]
            }
            Command::TakeScreenshot => command_helpers::execute_with_error_handling(
                || self.take_screenshot(hwnd),
                "截图失败",
            ),
            Command::ExtractText => {
                // 检查前置条件：是否有有效选区
                if self.has_valid_selection() {
                    self.handle_extract_text(hwnd)
                } else {
                    vec![Command::ShowError("请先选择区域".to_string())]
                }
            }
            Command::PinSelection => command_helpers::execute_with_error_handling(
                || self.pin_selection(hwnd),
                "固定失败",
            ),
            Command::ResetToInitialState => {
                self.reset_to_initial_state();
                vec![]
            }
            Command::CopyToClipboard => {
                self.execute_command(Command::SaveSelectionToClipboard, hwnd)
            }
            Command::ShowSaveDialog => self.handle_save_to_file(hwnd),
            Command::StartTimer(timer_id, interval_ms) => {
                let _ = win_api::start_timer(hwnd, timer_id, interval_ms);
                vec![]
            }
            Command::StopTimer(timer_id) => {
                let _ = win_api::stop_timer(hwnd, timer_id);
                vec![]
            }
            Command::ReloadSettings => self.reload_settings(),
            Command::ShowError(msg) => {
                eprintln!("Error: {msg}");
                vec![]
            }
            Command::Quit => {
                win_api::quit_message_loop(0);
                vec![]
            }
            Command::None => {
                vec![]
            }
        }
    }
}

// 辅助方法实现
impl App {
    fn handle_save_to_file(&mut self, hwnd: HWND) -> Vec<Command> {
        match self.save_selection_to_file(hwnd) {
            Ok(true) => {
                let _ = win_api::hide_window(hwnd);
                self.reset_to_initial_state();
                vec![]
            }
            Ok(false) => {
                // 用户取消，不做任何操作
                vec![]
            }
            Err(e) => {
                eprintln!("Failed to save selection to file: {e}");
                vec![Command::ShowError(format!("保存失败: {e}"))]
            }
        }
    }

    fn handle_save_to_clipboard(&mut self, hwnd: HWND) -> Vec<Command> {
        match self.save_selection_to_clipboard(hwnd) {
            Ok(_) => {
                let _ = win_api::hide_window(hwnd);
                self.reset_to_initial_state();
                vec![]
            }
            Err(e) => {
                eprintln!("Failed to copy selection to clipboard: {e}");
                vec![Command::ShowError(format!("复制失败: {e}"))]
            }
        }
    }

    fn handle_extract_text(&mut self, hwnd: HWND) -> Vec<Command> {
        match self.extract_text_from_selection(hwnd) {
            Ok(_) => {
                let _ = win_api::hide_window(hwnd);
                self.reset_to_initial_state();
                vec![]
            }
            Err(e) => {
                eprintln!("Failed to extract text: {e}");
                vec![Command::ShowError(format!("文本提取失败: {e}"))]
            }
        }
    }
}
