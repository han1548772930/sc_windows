use std::collections::VecDeque;

use sc_host_protocol::{Command, DrawingMessage};
use sc_platform::WindowId;
use sc_ui_windows::{PreviewWindow, SettingsWindow};

use crate::app::App;

#[derive(Debug, Default)]
pub struct CommandQueue {
    pending: VecDeque<Command>,
    #[cfg(debug_assertions)]
    executed_count: usize,
}

impl CommandQueue {
    pub fn new() -> Self {
        Self {
            pending: VecDeque::new(),
            #[cfg(debug_assertions)]
            executed_count: 0,
        }
    }

    pub fn push(&mut self, command: Command) {
        if !matches!(command, Command::None) {
            self.pending.push_back(command);
        }
    }

    pub fn push_batch(&mut self, commands: impl IntoIterator<Item = Command>) {
        for cmd in commands {
            self.push(cmd);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    pub fn len(&self) -> usize {
        self.pending.len()
    }

    pub fn clear(&mut self) {
        self.pending.clear();
    }

    pub fn process_all<E: CommandExecutor + ?Sized>(&mut self, executor: &mut E, window: WindowId) {
        const MAX_ITERATIONS: usize = 1000;
        let mut iteration = 0;

        while let Some(command) = self.pending.pop_front() {
            #[cfg(debug_assertions)]
            {
                self.executed_count += 1;
            }

            let new_commands = executor.execute_command(command, window);
            self.push_batch(new_commands);

            iteration += 1;
            if iteration >= MAX_ITERATIONS {
                #[cfg(debug_assertions)]
                eprintln!(
                    "Warning: Command queue exceeded {} iterations, breaking to prevent infinite loop",
                    MAX_ITERATIONS
                );
                break;
            }
        }
    }

    #[cfg(debug_assertions)]
    pub fn executed_count(&self) -> usize {
        self.executed_count
    }

    #[cfg(debug_assertions)]
    pub fn reset_counter(&mut self) {
        self.executed_count = 0;
    }
}

pub trait CommandExecutor {
    fn execute_command(&mut self, command: Command, window: WindowId) -> Vec<Command>;

    fn execute_commands(&mut self, commands: Vec<Command>, window: WindowId) -> Vec<Command> {
        let mut result_commands = Vec::new();
        for command in commands {
            result_commands.extend(self.execute_command(command, window));
        }
        result_commands
    }

    fn execute_command_chain(&mut self, commands: Vec<Command>, window: WindowId) {
        let mut queue = CommandQueue::new();
        queue.push_batch(commands);
        queue.process_all(self, window);
    }
}

impl CommandExecutor for App {
    fn execute_command(&mut self, command: Command, window: WindowId) -> Vec<Command> {
        match command {
            Command::Core(action) => self.dispatch_core_action(action),
            Command::RequestRedraw => {
                let _ = self.host_platform().request_redraw(window);
                vec![]
            }
            Command::RequestRedrawRect(rect) => {
                let _ = self.host_platform().request_redraw_rect(
                    window,
                    rect.left,
                    rect.top,
                    rect.right,
                    rect.bottom,
                );
                vec![]
            }
            Command::UI(ui_message) => self.handle_ui_message(ui_message),
            Command::Drawing(drawing_message) => {
                let should_execute = match &drawing_message {
                    DrawingMessage::Undo => self.can_undo(),
                    _ => true,
                };

                if should_execute {
                    self.handle_drawing_message(drawing_message)
                } else {
                    vec![]
                }
            }
            Command::SelectDrawingTool(tool) => {
                let mut commands = self.select_drawing_tool(tool);
                commands.push(Command::RequestRedraw);
                commands
            }
            Command::HideWindow => {
                let _ = self.host_platform().hide_window(window);
                vec![]
            }
            Command::QuitApp => {
                self.cleanup_before_quit();
                let _ = self.host_platform().destroy_window(window);
                vec![]
            }
            Command::SaveSelectionToFile => {
                if self.has_valid_selection() {
                    self.handle_save_to_file(window)
                } else {
                    vec![Command::ShowError("请先选择区域".to_string())]
                }
            }
            Command::SaveSelectionToClipboard => {
                if self.has_valid_selection() {
                    self.handle_save_to_clipboard(window)
                } else {
                    vec![Command::ShowError("请先选择区域".to_string())]
                }
            }
            Command::UpdateToolbar => {
                self.update_toolbar_state();
                vec![]
            }
            Command::ShowSettings => match SettingsWindow::show(window) {
                Ok(true) => vec![Command::ReloadSettings],
                Ok(false) => vec![],
                Err(e) => {
                    eprintln!("Failed to show settings window: {e}");
                    vec![Command::ShowError(format!("打开设置失败: {e}"))]
                }
            },
            Command::TakeScreenshot => match self.take_screenshot(window) {
                Ok(()) => vec![],
                Err(e) => {
                    eprintln!("截图失败: {e}");
                    vec![Command::ShowError(format!("截图失败: {e}"))]
                }
            },
            Command::ExtractText => {
                if self.has_valid_selection() {
                    self.handle_extract_text(window)
                } else {
                    vec![Command::ShowError("请先选择区域".to_string())]
                }
            }
            Command::ShowOcrPreview => {
                self.show_cached_ocr_preview(window);
                vec![]
            }
            Command::CopyTextToClipboard(text) => {
                if let Err(e) = self.host_platform().copy_text_to_clipboard(&text) {
                    eprintln!("Failed to copy OCR text to clipboard: {e}");
                }
                vec![]
            }
            Command::ShowOcrNoTextMessage => {
                self.show_ocr_no_text_message(window);
                vec![]
            }
            Command::StopOcrEngine => {
                self.stop_ocr_engine_async();
                vec![]
            }
            Command::PinSelection => match self.pin_selection(window) {
                Ok(cmds) => cmds,
                Err(e) => {
                    eprintln!("固定失败: {e}");
                    vec![Command::ShowError(format!("固定失败: {e}"))]
                }
            },
            Command::ResetToInitialState => self.reset_to_initial_state(),
            Command::StartTimer(timer_id, interval_ms) => {
                let _ = self
                    .host_platform()
                    .start_timer(window, timer_id, interval_ms);
                vec![]
            }
            Command::StopTimer(timer_id) => {
                let _ = self.host_platform().stop_timer(window, timer_id);
                vec![]
            }
            Command::ReloadSettings => {
                let commands = self.reload_settings();
                let _ = self.reregister_hotkey(window);
                commands
            }
            Command::ShowError(msg) => {
                eprintln!("Error: {msg}");
                vec![]
            }
            Command::None => {
                vec![]
            }
        }
    }
}

impl App {
    fn handle_save_to_file(&mut self, window: WindowId) -> Vec<Command> {
        match self.save_selection_to_file(window) {
            Ok(true) => {
                let _ = self.host_platform().hide_window(window);
                self.reset_to_initial_state()
            }
            Ok(false) => {
                vec![]
            }
            Err(e) => {
                eprintln!("Failed to save selection to file: {e}");
                vec![Command::ShowError(format!("保存失败: {e}"))]
            }
        }
    }

    fn handle_save_to_clipboard(&mut self, window: WindowId) -> Vec<Command> {
        match self.save_selection_to_clipboard(window) {
            Ok(_) => {
                let _ = self.host_platform().hide_window(window);
                self.reset_to_initial_state()
            }
            Err(e) => {
                eprintln!("Failed to copy selection to clipboard: {e}");
                vec![Command::ShowError(format!("复制失败: {e}"))]
            }
        }
    }

    fn handle_extract_text(&mut self, window: WindowId) -> Vec<Command> {
        match self.extract_text_from_selection(window) {
            Ok(_) => {
                // Keep state until completion so core can track the running OCR job.
                let _ = self.host_platform().hide_window(window);
                vec![]
            }
            Err(e) => {
                eprintln!("Failed to extract text: {e}");
                vec![Command::ShowError(format!("文本提取失败: {e}"))]
            }
        }
    }

    fn show_cached_ocr_preview(&mut self, _window: WindowId) {
        let Some(data) = self.take_ocr_completion() else {
            return;
        };

        if let Err(e) = PreviewWindow::show(
            data.image_data,
            data.ocr_results,
            data.selection_rect,
            false,
            self.current_drawing_config(),
            None,
        ) {
            eprintln!("Failed to show OCR result window: {e:?}");
        }
    }

    fn show_ocr_no_text_message(&self, window: WindowId) {
        self.host_platform().show_info_message(
            window,
            "OCR结果",
            "未识别到文本内容。\n\n请确保选择区域包含清晰的文字。",
        );
    }
}
