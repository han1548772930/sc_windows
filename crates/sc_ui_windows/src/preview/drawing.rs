use windows::Win32::Foundation::RECT;

use sc_drawing::Rect as DrawingRect;
use sc_drawing_host::{DrawingConfig, DrawingManager, DrawingTool};
use sc_host_protocol::{Command, DrawingMessage};
use sc_platform::{HostPlatform, WindowId};
use sc_platform_windows::windows::WindowsHostPlatform;

/// Preview 窗口绘图状态管理
pub struct PreviewDrawingState {
    /// 绘图管理器
    pub manager: DrawingManager,
    /// 图片区域边界（用于限制绘图）
    pub image_area_rect: RECT,
    /// Preview window id.
    window: WindowId,
}

impl PreviewDrawingState {
    /// 创建新的绘图状态
    pub fn new(window: WindowId, drawing_config: DrawingConfig) -> Result<Self, anyhow::Error> {
        let manager = DrawingManager::new(drawing_config)
            .map_err(|e| anyhow::anyhow!("Failed to create DrawingManager: {:?}", e))?;

        Ok(Self {
            manager,
            image_area_rect: RECT::default(),
            window,
        })
    }

    /// 设置图片区域
    pub fn set_image_area(&mut self, rect: RECT) {
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

    /// 检查点是否在图片区域内
    pub fn is_in_image_area(&self, x: i32, y: i32) -> bool {
        x >= self.image_area_rect.left
            && x <= self.image_area_rect.right
            && y >= self.image_area_rect.top
            && y <= self.image_area_rect.bottom
    }

    /// 获取当前工具
    pub fn get_current_tool(&self) -> DrawingTool {
        self.manager.get_current_tool()
    }

    /// 切换工具
    pub fn switch_tool(&mut self, tool: DrawingTool) {
        let new_tool = if self.manager.get_current_tool() == tool {
            DrawingTool::None
        } else {
            tool
        };
        self.manager
            .handle_message(DrawingMessage::SelectTool(new_tool));
    }

    /// 处理鼠标按下
    pub fn handle_mouse_down(&mut self, x: i32, y: i32) -> bool {
        if !self.is_in_image_area(x, y) {
            return false;
        }

        let image_area_rect = self.image_area_rect_drawing();
        let (commands, consumed) = self.manager.handle_mouse_down(x, y, Some(image_area_rect));
        self.process_commands(&commands);
        consumed
    }

    /// 处理鼠标移动
    pub fn handle_mouse_move(&mut self, x: i32, y: i32) -> bool {
        let image_area_rect = self.image_area_rect_drawing();
        let (commands, consumed) = self.manager.handle_mouse_move(x, y, Some(image_area_rect));
        self.process_commands(&commands);
        consumed
    }

    /// 处理鼠标释放
    pub fn handle_mouse_up(&mut self, x: i32, y: i32) -> bool {
        let (commands, consumed) = self.manager.handle_mouse_up(x, y);
        self.process_commands(&commands);
        consumed
    }

    /// 处理双击（用于编辑文本）
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

    /// 处理键盘输入
    pub fn handle_key_input(&mut self, key_code: u32) -> bool {
        let commands = self.manager.handle_key_input(key_code);
        self.process_commands(&commands);
        !commands.is_empty()
    }

    /// 处理字符输入（文本编辑时）
    pub fn handle_char_input(&mut self, character: char) -> bool {
        if !self.manager.is_text_editing() {
            return false;
        }
        // 过滤控制字符（除了换行符）
        if character.is_control() && character != '\n' {
            return false;
        }
        let commands = self.manager.handle_text_input(character);
        self.process_commands(&commands);
        !commands.is_empty()
    }

    /// 处理命令
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

    /// 处理光标定时器
    pub fn handle_cursor_timer(&mut self, timer_id: u32) -> bool {
        let commands = self.manager.handle_cursor_timer(timer_id);
        self.process_commands(&commands);
        !commands.is_empty()
    }

    /// 是否正在文本编辑
    pub fn is_text_editing(&self) -> bool {
        self.manager.is_text_editing()
    }
}
