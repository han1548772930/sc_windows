use crate::InputEvent;

/// Opaque window identifier.
///
/// This is used to avoid leaking platform window handles (e.g. Win32 `HWND`) across crate
/// boundaries. Platform backends can convert to/from raw handles as needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowId(usize);

impl WindowId {
    pub const INVALID: WindowId = WindowId(0);

    #[inline]
    pub fn from_raw(raw: usize) -> Self {
        Self(raw)
    }

    #[inline]
    pub fn raw(self) -> usize {
        self.0
    }

    #[inline]
    pub fn is_valid(self) -> bool {
        self.0 != 0
    }
}

/// Minimal platform window-message handler.
///
/// This is intentionally low-level (raw message id + wparam/lparam) so the platform backend can own
/// the event loop and window procedure, while the host app remains responsible for dispatching
/// messages into higher-level events/commands.
///
/// Over time this can evolve into a more gpui-like typed event API.
pub trait WindowMessageHandler {
    type WindowHandle: Copy;
    type UserEvent: Send + 'static;

    /// Handle a platform-agnostic input event.
    ///
    /// Return `Some(result)` to mark the message as handled, or `None` to fall back to other message
    /// handling paths.
    fn handle_input_event(
        &mut self,
        window: Self::WindowHandle,
        event: InputEvent,
    ) -> Option<isize>;

    /// Handle a user-defined event delivered onto the window thread.
    ///
    /// This is the preferred way to bridge background threads into the UI/event-loop thread.
    fn handle_user_event(
        &mut self,
        _window: Self::WindowHandle,
        _event: Self::UserEvent,
    ) -> Option<isize> {
        None
    }

    /// Handle a raw platform window message.
    ///
    /// Return `Some(result)` to mark the message as handled, or `None` to fall back to the platform
    /// default procedure.
    fn handle_window_message(
        &mut self,
        window: Self::WindowHandle,
        msg: u32,
        wparam: usize,
        lparam: isize,
    ) -> Option<isize>;

    /// Handle a paint request.
    ///
    /// The platform runner owns the WM_PAINT BeginPaint/EndPaint cycle; this hook is for issuing
    /// rendering work only.
    fn handle_paint(&mut self, _window: Self::WindowHandle) -> Option<isize> {
        None
    }

    /// Handle a close request.
    ///
    /// If not overridden, the platform runner will fall back to the default window procedure.
    fn handle_close_requested(&mut self, _window: Self::WindowHandle) -> Option<isize> {
        None
    }
}
