use crate::app::App;
use crate::message::{Command, DrawingMessage, Message};
use crate::settings::SettingsWindow;
use crate::utils::{command_helpers, win_api};
use windows::Win32::Foundation::HWND;

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

    /// 递归执行命令直到没有新命令产生
    fn execute_command_chain(&mut self, commands: Vec<Command>, hwnd: HWND) {
        let mut pending_commands = commands;

        while !pending_commands.is_empty() {
            let mut new_commands = Vec::new();

            for command in pending_commands {
                let result_commands = self.execute_command(command, hwnd);
                new_commands.extend(result_commands);
            }

            pending_commands = new_commands;
        }
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
                // 覆盖层显示时再次异步预热OCR引擎，避免后续再次使用时冷启动卡顿
                crate::ocr::PaddleOcrEngine::start_ocr_engine_async();
                self.start_async_ocr_check(hwnd);
                vec![]
            }
            Command::HideOverlay | Command::HideWindow => {
                let _ = win_api::hide_window(hwnd);
                vec![]
            }
            Command::SaveSelectionToFile => self.handle_save_to_file(hwnd),
            Command::SaveSelectionToClipboard => self.handle_save_to_clipboard(hwnd),
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
            Command::ExtractText => self.handle_extract_text(hwnd),
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
