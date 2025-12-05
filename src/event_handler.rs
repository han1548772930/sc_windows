
use crate::message::Command;

/// 鼠标事件处理器 trait
pub trait MouseEventHandler {
    /// 处理鼠标移动
    fn handle_mouse_move(&mut self, x: i32, y: i32) -> Vec<Command>;

    /// 处理鼠标按下
    fn handle_mouse_down(&mut self, x: i32, y: i32) -> Vec<Command>;

    /// 处理鼠标释放
    fn handle_mouse_up(&mut self, x: i32, y: i32) -> Vec<Command>;

    /// 处理双击事件
    fn handle_double_click(&mut self, x: i32, y: i32) -> Vec<Command>;
}

/// 键盘事件处理器 trait
pub trait KeyboardEventHandler {
    /// 处理键盘输入
    fn handle_key_input(&mut self, key: u32) -> Vec<Command>;

    /// 处理文本输入
    fn handle_text_input(&mut self, character: char) -> Vec<Command>;
}

/// 系统事件处理器 trait
pub trait SystemEventHandler {
    /// 处理托盘消息
    fn handle_tray_message(&mut self, wparam: u32, lparam: u32) -> Vec<Command>;

    /// 处理光标定时器
    fn handle_cursor_timer(&mut self, timer_id: u32) -> Vec<Command>;
}

