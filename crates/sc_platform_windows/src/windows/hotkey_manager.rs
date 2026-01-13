use sc_platform::WindowId;

use super::{SafeHwnd, hotkeys};

/// Stateful global hotkey manager.
///
/// This lives in the Windows platform backend (similar to Zed's `gpui::platform::windows`).
#[derive(Debug, Default)]
pub struct HotkeyManager {
    hwnd: SafeHwnd,
    registered_hotkeys: Vec<i32>,
}

impl HotkeyManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_hotkey(
        &mut self,
        window: WindowId,
        hotkey_id: i32,
        modifiers: u32,
        key: u32,
    ) -> windows::core::Result<()> {
        let hwnd = super::hwnd(window);
        self.hwnd.set(Some(hwnd));

        hotkeys::register_hotkey(hwnd, hotkey_id, modifiers, key)?;
        if !self.registered_hotkeys.contains(&hotkey_id) {
            self.registered_hotkeys.push(hotkey_id);
        }
        Ok(())
    }

    pub fn reregister_hotkey(
        &mut self,
        window: WindowId,
        hotkey_id: i32,
        modifiers: u32,
        key: u32,
    ) -> windows::core::Result<()> {
        self.cleanup();
        self.register_hotkey(window, hotkey_id, modifiers, key)
    }

    pub fn cleanup(&mut self) {
        let Some(hwnd) = self.hwnd.get() else {
            self.registered_hotkeys.clear();
            return;
        };

        for hotkey_id in self.registered_hotkeys.drain(..) {
            let _ = hotkeys::unregister_hotkey_for_window(hwnd, hotkey_id);
        }
    }
}
