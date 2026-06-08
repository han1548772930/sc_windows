use sc_app::selection::RectI32;
use sc_drawing::Rect as DrawingRect;
use sc_drawing_host::{DrawingConfig, DrawingManager, DrawingTool};
use sc_host_protocol::{Command, DrawingMessage};
use sc_platform::{HostPlatform, WindowId};
use sc_platform_windows::windows::WindowsHostPlatform;

pub struct PreviewDrawingState {
    pub manager: DrawingManager,
    pub image_area_rect: RectI32,
    /// Preview window id.
    window: WindowId,
}

impl PreviewDrawingState {
    pub fn new(window: WindowId, drawing_config: DrawingConfig) -> Result<Self, anyhow::Error> {
        let manager = DrawingManager::new(drawing_config)
            .map_err(|e| anyhow::anyhow!("Failed to create DrawingManager: {:?}", e))?;

        Ok(Self {
            manager,
            image_area_rect: RectI32 {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            window,
        })
    }

    pub fn set_image_area(&mut self, rect: RectI32) {
        self.image_area_rect = rect;
    }

    fn image_area_rect_drawing(&self) -> DrawingRect {
        DrawingRect {
            left: self.image_area_rect.left,
            top: self.image_area_rect.top,
            right: self.image_area_rect.right,
            bottom: self.image_area_rect.bottom,
        }
    }

    pub fn is_in_image_area(&self, x: i32, y: i32) -> bool {
        x >= self.image_area_rect.left
            && x <= self.image_area_rect.right
            && y >= self.image_area_rect.top
            && y <= self.image_area_rect.bottom
    }

    pub fn get_current_tool(&self) -> DrawingTool {
        self.manager.get_current_tool()
    }

    pub fn switch_tool(&mut self, tool: DrawingTool) {
        let new_tool = if self.manager.get_current_tool() == tool {
            DrawingTool::None
        } else {
            tool
        };
        self.manager
            .handle_message(DrawingMessage::SelectTool(new_tool));
    }

    pub fn handle_mouse_down(&mut self, x: i32, y: i32) -> bool {
        if !self.is_in_image_area(x, y) {
            return false;
        }

        let image_area_rect = self.image_area_rect_drawing();
        let (commands, consumed) = self.manager.handle_mouse_down(x, y, Some(image_area_rect));
        self.process_commands(&commands);
        consumed
    }

    pub fn handle_mouse_move(&mut self, x: i32, y: i32) -> bool {
        let image_area_rect = self.image_area_rect_drawing();
        let (commands, consumed) = self.manager.handle_mouse_move(x, y, Some(image_area_rect));
        self.process_commands(&commands);
        consumed
    }

    pub fn handle_mouse_up(&mut self, x: i32, y: i32) -> bool {
        let (commands, consumed) = self.manager.handle_mouse_up(x, y);
        self.process_commands(&commands);
        consumed
    }

    pub fn handle_double_click(&mut self, x: i32, y: i32) -> bool {
        if !self.is_in_image_area(x, y) {
            return false;
        }
        let image_area_rect = self.image_area_rect_drawing();
        let commands = self
            .manager
            .handle_double_click(x, y, Some(&image_area_rect));
        self.process_commands(&commands);
        !commands.is_empty()
    }

    pub fn handle_key_input(&mut self, key_code: u32) -> bool {
        let commands = self.manager.handle_key_input(key_code);
        self.process_commands(&commands);
        !commands.is_empty()
    }

    pub fn handle_char_input(&mut self, character: char) -> bool {
        if !self.manager.is_text_editing() {
            return false;
        }
        if character.is_control() && character != '\n' {
            return false;
        }
        let commands = self.manager.handle_text_input(character);
        self.process_commands(&commands);
        !commands.is_empty()
    }

    fn process_commands(&self, commands: &[Command]) {
        let platform = WindowsHostPlatform::new();

        for cmd in commands {
            match cmd {
                Command::RequestRedraw => {
                    let _ = platform.request_redraw(self.window);
                }
                Command::RequestRedrawRect(rect) => {
                    let _ = platform.request_redraw_rect(
                        self.window,
                        rect.left,
                        rect.top,
                        rect.right,
                        rect.bottom,
                    );
                }
                Command::StartTimer(timer_id, interval_ms) => {
                    let _ = platform.start_timer(self.window, *timer_id, *interval_ms);
                }
                Command::StopTimer(timer_id) => {
                    let _ = platform.stop_timer(self.window, *timer_id);
                }
                _ => {}
            }
        }
    }

    pub fn handle_cursor_timer(&mut self, timer_id: u32) -> bool {
        let commands = self.manager.handle_cursor_timer(timer_id);
        self.process_commands(&commands);
        !commands.is_empty()
    }

    pub fn is_text_editing(&self) -> bool {
        self.manager.is_text_editing()
    }
}
