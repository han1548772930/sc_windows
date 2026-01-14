use std::error::Error;
use std::ffi::c_void;
use std::fmt;

use sc_app::selection::RectI32;
use sc_platform::WindowId;
use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumChildWindows, EnumWindows, GetClassNameW, GetDlgCtrlID, GetParent, GetWindowRect,
    GetWindowTextW, IsIconic, IsWindowVisible,
};

pub mod auto_highlight;

pub use auto_highlight::{
    AutoHighlightMoveAction, AutoHighlightMoveArgs, AutoHighlighter, HighlightKind, HighlightTarget,
};

pub type Result<T> = std::result::Result<T, WindowDetectionError>;

/// Window detection errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowDetectionError {
    /// EnumWindows/EnumChildWindows failed.
    WindowEnumerationFailed,
}

impl fmt::Display for WindowDetectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WindowDetectionError::WindowEnumerationFailed => write!(f, "window enumeration failed"),
        }
    }
}

impl Error for WindowDetectionError {}

#[inline]
fn window_id_from_hwnd(hwnd: HWND) -> WindowId {
    WindowId::from_raw(hwnd.0 as usize)
}

#[inline]
fn hwnd_from_window_id(window: WindowId) -> HWND {
    HWND(window.raw() as *mut c_void)
}

/// Window info.
#[derive(Debug, Clone)]
pub struct WindowInfo {
    /// Opaque window handle.
    pub hwnd: WindowId,
    /// Window rect in screen coordinates.
    pub rect: RectI32,
    /// Window title.
    pub title: String,
    /// Window class name.
    pub class_name: String,
    /// Is visible.
    pub is_visible: bool,
    /// Is minimized.
    pub is_minimized: bool,
}

impl WindowInfo {
    /// Check whether a point is within the window rect.
    pub fn contains_point(&self, x: i32, y: i32) -> bool {
        x >= self.rect.left && x <= self.rect.right && y >= self.rect.top && y <= self.rect.bottom
    }
}

/// Child control info.
#[derive(Debug, Clone)]
pub struct ChildControlInfo {
    /// Opaque control handle.
    pub hwnd: WindowId,
    /// Control rect in screen coordinates.
    pub rect: RectI32,
    /// Control text/title.
    pub title: String,
    /// Control class name.
    pub class_name: String,
    /// Is visible.
    pub is_visible: bool,
    /// Parent window handle.
    pub parent_hwnd: WindowId,
    /// Control id.
    pub control_id: i32,
}

impl ChildControlInfo {
    /// Check whether a point is within the control rect.
    pub fn contains_point(&self, x: i32, y: i32) -> bool {
        x >= self.rect.left && x <= self.rect.right && y >= self.rect.top && y <= self.rect.bottom
    }
}

/// Window detector.
#[derive(Debug)]
pub struct WindowDetector {
    windows: Vec<WindowInfo>,
    current_highlighted_window: Option<usize>,
    child_controls: Vec<ChildControlInfo>,
    current_highlighted_control: Option<usize>,
    /// Last detected window handle (used to throttle child control refresh).
    last_detected_window: Option<WindowId>,
}

/// Window detection manager.
pub struct WindowDetectionManager {
    detector: WindowDetector,
    detection_enabled: bool,
}

impl Default for WindowDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowDetector {
    pub fn new() -> Self {
        Self {
            windows: Vec::new(),
            current_highlighted_window: None,
            child_controls: Vec::new(),
            current_highlighted_control: None,
            last_detected_window: None,
        }
    }

    /// Refresh all top-level windows.
    pub fn refresh_windows(&mut self) -> Result<()> {
        self.windows.clear();

        unsafe {
            // Enum all top-level windows (z-order from top to bottom).
            if EnumWindows(
                Some(enum_windows_proc),
                LPARAM(&mut self.windows as *mut _ as isize),
            )
            .is_err()
            {
                return Err(WindowDetectionError::WindowEnumerationFailed);
            }
        }

        // Filter out unwanted windows.
        self.windows.retain(|window| {
            window.is_visible
                && !window.is_minimized
                && !window.title.is_empty()
                && window.rect.right > window.rect.left
                && window.rect.bottom > window.rect.top
        });

        Ok(())
    }

    /// Refresh child controls of a window.
    pub fn refresh_child_controls(&mut self, parent_window: WindowId) -> Result<()> {
        self.child_controls.clear();

        let parent_hwnd = hwnd_from_window_id(parent_window);

        unsafe {
            let result = EnumChildWindows(
                Some(parent_hwnd),
                Some(enum_child_windows_proc),
                LPARAM(&mut self.child_controls as *mut _ as isize),
            );
            if !result.as_bool() {
                return Err(WindowDetectionError::WindowEnumerationFailed);
            }
        }

        self.child_controls.retain(|control| {
            control.is_visible
                && control.rect.right > control.rect.left
                && control.rect.bottom > control.rect.top
        });

        Ok(())
    }

    /// Get the child control under the point.
    /// Uses cached child controls and returns the smallest-area match.
    pub fn get_child_control_at_point(&mut self, x: i32, y: i32) -> Option<&ChildControlInfo> {
        let mut matching_controls = Vec::new();
        for (index, control) in self.child_controls.iter().enumerate() {
            if control.contains_point(x, y) {
                matching_controls.push((index, control));
            }
        }

        if matching_controls.is_empty() {
            self.current_highlighted_control = None;
            return None;
        }

        if let Some((index, control)) = matching_controls
            .iter()
            .min_by_key(|(_, c)| (c.rect.right - c.rect.left) * (c.rect.bottom - c.rect.top))
        {
            self.current_highlighted_control = Some(*index);
            Some(control)
        } else {
            self.current_highlighted_control = None;
            None
        }
    }

    /// Detect at point and return (window, child_control).
    /// Refreshes child controls only when the window changes.
    pub fn detect_at_point(
        &mut self,
        x: i32,
        y: i32,
    ) -> (Option<WindowInfo>, Option<ChildControlInfo>) {
        let window = self.get_window_at_point(x, y);

        let control = if let Some(ref window_info) = window {
            let current_window = window_info.hwnd;

            if self.last_detected_window != Some(current_window) {
                self.last_detected_window = Some(current_window);
                let _ = self.refresh_child_controls(window_info.hwnd);
            }

            self.get_child_control_at_point(x, y).cloned()
        } else {
            self.last_detected_window = None;
            self.child_controls.clear();
            None
        };

        (window, control)
    }

    fn get_window_at_point(&mut self, x: i32, y: i32) -> Option<WindowInfo> {
        let mut matching_windows = Vec::new();

        // self.windows comes from EnumWindows, which preserves z-order (top to bottom).
        for (index, window) in self.windows.iter().enumerate() {
            if window.contains_point(x, y) {
                matching_windows.push((index, window));
            }
        }

        if matching_windows.is_empty() {
            self.current_highlighted_window = None;
            return None;
        }

        // Return the first match (topmost window under the point).
        if let Some((index, window)) = matching_windows.first() {
            self.current_highlighted_window = Some(*index);
            return Some((*window).clone());
        }

        None
    }
}

impl Default for WindowDetectionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowDetectionManager {
    pub fn new() -> Self {
        Self {
            detector: WindowDetector::new(),
            detection_enabled: false,
        }
    }

    /// Start detection (refreshes window list).
    pub fn start_detection(&mut self) -> Result<()> {
        self.detector.refresh_windows()?;
        self.detection_enabled = true;
        Ok(())
    }

    pub fn stop_detection(&mut self) {
        self.detection_enabled = false;
    }

    pub fn refresh_windows(&mut self) -> Result<()> {
        self.detector.refresh_windows()
    }

    pub fn detect_window_at_point(&mut self, x: i32, y: i32) -> Result<Option<WindowInfo>> {
        if !self.detection_enabled {
            return Ok(None);
        }

        let (window_info, _) = self.detector.detect_at_point(x, y);
        Ok(window_info)
    }

    pub fn detect_at_point(
        &mut self,
        x: i32,
        y: i32,
    ) -> (Option<WindowInfo>, Option<ChildControlInfo>) {
        if !self.detection_enabled {
            return (None, None);
        }

        self.detector.detect_at_point(x, y)
    }
}

unsafe extern "system" fn enum_windows_proc(hwnd: HWND, lparam: LPARAM) -> windows::core::BOOL {
    unsafe {
        let windows = &mut *(lparam.0 as *mut Vec<WindowInfo>);

        let hwnd_id = window_id_from_hwnd(hwnd);

        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return windows::core::BOOL::from(true);
        }
        let rect = RectI32 {
            left: rect.left,
            top: rect.top,
            right: rect.right,
            bottom: rect.bottom,
        };

        let mut title_buffer = [0u16; 256];
        let title_len = GetWindowTextW(hwnd, &mut title_buffer);
        let title = if title_len > 0 {
            String::from_utf16_lossy(&title_buffer[..title_len as usize])
        } else {
            String::new()
        };

        let mut class_buffer = [0u16; 256];
        let class_len = GetClassNameW(hwnd, &mut class_buffer);
        let class_name = if class_len > 0 {
            String::from_utf16_lossy(&class_buffer[..class_len as usize])
        } else {
            String::new()
        };

        let is_visible = IsWindowVisible(hwnd).as_bool();
        let is_minimized = IsIconic(hwnd).as_bool();

        windows.push(WindowInfo {
            hwnd: hwnd_id,
            rect,
            title,
            class_name,
            is_visible,
            is_minimized,
        });

        windows::core::BOOL::from(true)
    }
}

unsafe extern "system" fn enum_child_windows_proc(
    hwnd: HWND,
    lparam: LPARAM,
) -> windows::core::BOOL {
    unsafe {
        let child_controls = &mut *(lparam.0 as *mut Vec<ChildControlInfo>);

        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return windows::core::BOOL::from(true);
        }
        let rect = RectI32 {
            left: rect.left,
            top: rect.top,
            right: rect.right,
            bottom: rect.bottom,
        };

        let parent_hwnd = match GetParent(hwnd) {
            Ok(parent) => parent,
            Err(_) => return windows::core::BOOL::from(true),
        };

        let mut title_buffer = [0u16; 256];
        let title_len = GetWindowTextW(hwnd, &mut title_buffer);
        let title = if title_len > 0 {
            String::from_utf16_lossy(&title_buffer[..title_len as usize])
        } else {
            String::new()
        };

        let mut class_buffer = [0u16; 256];
        let class_len = GetClassNameW(hwnd, &mut class_buffer);
        let class_name = if class_len > 0 {
            String::from_utf16_lossy(&class_buffer[..class_len as usize])
        } else {
            String::new()
        };

        let control_id = GetDlgCtrlID(hwnd);
        let is_visible = IsWindowVisible(hwnd).as_bool();

        child_controls.push(ChildControlInfo {
            hwnd: window_id_from_hwnd(hwnd),
            rect,
            title,
            class_name,
            is_visible,
            parent_hwnd: window_id_from_hwnd(parent_hwnd),
            control_id,
        });

        windows::core::BOOL::from(true)
    }
}
