//! 平台无关的输入事件定义
//!
//! 将操作系统的原始消息（如 WM_LBUTTONDOWN）转换为平台无关的事件类型，
//! 使业务逻辑层不需要直接接触 Win32 API。

/// 鼠标按钮类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// 键盘修饰键状态
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

impl Modifiers {
    /// 从当前键盘状态获取修饰键
    pub fn current() -> Self {
        use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CONTROL, VK_MENU, VK_SHIFT};
        
        unsafe {
            Self {
                ctrl: (GetKeyState(VK_CONTROL.0 as i32) as u16 & 0x8000) != 0,
                shift: (GetKeyState(VK_SHIFT.0 as i32) as u16 & 0x8000) != 0,
                alt: (GetKeyState(VK_MENU.0 as i32) as u16 & 0x8000) != 0,
            }
        }
    }
}

/// 虚拟键码（平台无关的键盘按键标识）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyCode(pub u32);

impl KeyCode {
    // 常用键码常量
    pub const ESCAPE: KeyCode = KeyCode(0x1B);
    pub const ENTER: KeyCode = KeyCode(0x0D);
    pub const BACKSPACE: KeyCode = KeyCode(0x08);
    pub const DELETE: KeyCode = KeyCode(0x2E);
    pub const LEFT: KeyCode = KeyCode(0x25);
    pub const UP: KeyCode = KeyCode(0x26);
    pub const RIGHT: KeyCode = KeyCode(0x27);
    pub const DOWN: KeyCode = KeyCode(0x28);
    pub const HOME: KeyCode = KeyCode(0x24);
    pub const END: KeyCode = KeyCode(0x23);
    pub const TAB: KeyCode = KeyCode(0x09);
    pub const A: KeyCode = KeyCode(0x41);
    pub const C: KeyCode = KeyCode(0x43);
    pub const V: KeyCode = KeyCode(0x56);
    pub const X: KeyCode = KeyCode(0x58);
    pub const Z: KeyCode = KeyCode(0x5A);
    pub const Y: KeyCode = KeyCode(0x59);
}

/// 平台无关的输入事件
#[derive(Debug, Clone)]
pub enum InputEvent {
    /// 鼠标移动
    MouseMove { x: i32, y: i32 },
    
    /// 鼠标按下
    MouseDown { x: i32, y: i32, button: MouseButton },
    
    /// 鼠标释放
    MouseUp { x: i32, y: i32, button: MouseButton },
    
    /// 鼠标双击
    DoubleClick { x: i32, y: i32, button: MouseButton },
    
    /// 键盘按下
    KeyDown { key: KeyCode, modifiers: Modifiers },
    
    /// 键盘释放
    KeyUp { key: KeyCode, modifiers: Modifiers },
    
    /// 文本输入（经过输入法处理的字符）
    TextInput { character: char },
    
    /// 定时器事件
    Timer { id: u32 },
    
    /// 鼠标滚轮
    MouseWheel { x: i32, y: i32, delta: i32 },
}

/// 从 Win32 消息参数提取鼠标坐标
#[inline]
pub fn extract_mouse_coords(lparam: windows::Win32::Foundation::LPARAM) -> (i32, i32) {
    let x = (lparam.0 as i16) as i32;
    let y = ((lparam.0 >> 16) as i16) as i32;
    (x, y)
}

/// 事件转换器：将 Win32 消息转换为平台无关事件
pub struct EventConverter;

impl EventConverter {
    /// 尝试将 Win32 窗口消息转换为 InputEvent
    /// 
    /// 返回 None 表示该消息不是输入事件
    pub fn convert(
        msg: u32,
        wparam: windows::Win32::Foundation::WPARAM,
        lparam: windows::Win32::Foundation::LPARAM,
    ) -> Option<InputEvent> {
        use windows::Win32::UI::WindowsAndMessaging::*;
        
        match msg {
            WM_MOUSEMOVE => {
                let (x, y) = extract_mouse_coords(lparam);
                Some(InputEvent::MouseMove { x, y })
            }
            
            WM_LBUTTONDOWN => {
                let (x, y) = extract_mouse_coords(lparam);
                Some(InputEvent::MouseDown { x, y, button: MouseButton::Left })
            }
            
            WM_LBUTTONUP => {
                let (x, y) = extract_mouse_coords(lparam);
                Some(InputEvent::MouseUp { x, y, button: MouseButton::Left })
            }
            
            WM_RBUTTONDOWN => {
                let (x, y) = extract_mouse_coords(lparam);
                Some(InputEvent::MouseDown { x, y, button: MouseButton::Right })
            }
            
            WM_RBUTTONUP => {
                let (x, y) = extract_mouse_coords(lparam);
                Some(InputEvent::MouseUp { x, y, button: MouseButton::Right })
            }
            
            WM_MBUTTONDOWN => {
                let (x, y) = extract_mouse_coords(lparam);
                Some(InputEvent::MouseDown { x, y, button: MouseButton::Middle })
            }
            
            WM_MBUTTONUP => {
                let (x, y) = extract_mouse_coords(lparam);
                Some(InputEvent::MouseUp { x, y, button: MouseButton::Middle })
            }
            
            WM_LBUTTONDBLCLK => {
                let (x, y) = extract_mouse_coords(lparam);
                Some(InputEvent::DoubleClick { x, y, button: MouseButton::Left })
            }
            
            WM_RBUTTONDBLCLK => {
                let (x, y) = extract_mouse_coords(lparam);
                Some(InputEvent::DoubleClick { x, y, button: MouseButton::Right })
            }
            
            WM_KEYDOWN => {
                let key = KeyCode(wparam.0 as u32);
                let modifiers = Modifiers::current();
                Some(InputEvent::KeyDown { key, modifiers })
            }
            
            WM_KEYUP => {
                let key = KeyCode(wparam.0 as u32);
                let modifiers = Modifiers::current();
                Some(InputEvent::KeyUp { key, modifiers })
            }
            
            WM_CHAR => {
                if let Some(character) = char::from_u32(wparam.0 as u32) {
                    // 只处理可打印字符和空格/制表符
                    if !character.is_control() || character == ' ' || character == '\t' {
                        Some(InputEvent::TextInput { character })
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            
            WM_TIMER => {
                Some(InputEvent::Timer { id: wparam.0 as u32 })
            }
            
            WM_MOUSEWHEEL => {
                let (x, y) = extract_mouse_coords(lparam);
                let delta = ((wparam.0 >> 16) as i16) as i32;
                Some(InputEvent::MouseWheel { x, y, delta })
            }
            
            _ => None,
        }
    }
}
