use crate::message::Command;
use windows::Win32::Foundation::HWND;

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

/// 窗口事件处理器 trait
pub trait WindowEventHandler {
    /// 绘制窗口内容
    fn paint(&mut self, hwnd: HWND) -> Result<(), crate::error::AppError>;

    /// 重置到初始状态
    fn reset_to_initial_state(&mut self);
}

/// 组合所有事件处理器的 trait
pub trait EventHandler:
    MouseEventHandler + KeyboardEventHandler + SystemEventHandler + WindowEventHandler
{
    /// 处理窗口消息的便捷方法
    fn handle_window_message(
        &mut self,
        msg: u32,
        wparam: usize,
        lparam: isize,
        _hwnd: HWND,
    ) -> Vec<Command> {
        use windows::Win32::UI::WindowsAndMessaging::*;

        match msg {
            WM_MOUSEMOVE => {
                let (x, y) =
                    crate::utils::extract_mouse_coords(windows::Win32::Foundation::LPARAM(lparam));
                self.handle_mouse_move(x, y)
            }
            WM_LBUTTONDOWN => {
                let (x, y) =
                    crate::utils::extract_mouse_coords(windows::Win32::Foundation::LPARAM(lparam));
                self.handle_mouse_down(x, y)
            }
            WM_LBUTTONUP => {
                let (x, y) =
                    crate::utils::extract_mouse_coords(windows::Win32::Foundation::LPARAM(lparam));
                self.handle_mouse_up(x, y)
            }
            WM_LBUTTONDBLCLK => {
                let (x, y) =
                    crate::utils::extract_mouse_coords(windows::Win32::Foundation::LPARAM(lparam));
                self.handle_double_click(x, y)
            }
            WM_KEYDOWN => self.handle_key_input(wparam as u32),
            WM_CHAR => {
                if let Some(character) = char::from_u32(wparam as u32) {
                    if !character.is_control() || character == ' ' || character == '\t' {
                        self.handle_text_input(character)
                    } else {
                        vec![]
                    }
                } else {
                    vec![]
                }
            }
            WM_TIMER => self.handle_cursor_timer(wparam as u32),
            _ => vec![],
        }
    }
}

// 为 App 实现 EventHandler（整合所有事件处理 traits）
impl EventHandler for crate::app::App {}
