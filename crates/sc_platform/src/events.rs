/// Mouse button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Keyboard modifier state.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

impl Modifiers {
    pub const NONE: Modifiers = Modifiers {
        ctrl: false,
        shift: false,
        alt: false,
    };
}

/// System tray event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayEvent {
    DoubleClick,
    MenuCommand(u32),
}

/// Virtual key code (platform-agnostic key identifier).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyCode(pub u32);

impl KeyCode {
    // Common key constants.
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

/// Platform-agnostic input event.
#[derive(Debug, Clone)]
pub enum InputEvent {
    /// Mouse moved.
    MouseMove { x: i32, y: i32 },
    /// Mouse button pressed.
    MouseDown { x: i32, y: i32, button: MouseButton },
    /// Mouse button released.
    MouseUp { x: i32, y: i32, button: MouseButton },
    /// Mouse double click.
    DoubleClick { x: i32, y: i32, button: MouseButton },
    /// Key pressed.
    KeyDown { key: KeyCode, modifiers: Modifiers },
    /// Key released.
    KeyUp { key: KeyCode, modifiers: Modifiers },
    /// Text input (IME-processed character).
    TextInput { character: char },
    /// System tray event.
    Tray(TrayEvent),
    /// Global hotkey.
    Hotkey { id: u32 },
    /// Timer event.
    Timer { id: u32 },
    /// Mouse wheel.
    MouseWheel { x: i32, y: i32, delta: i32 },
}
