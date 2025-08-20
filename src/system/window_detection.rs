// 窗口检测管理
//
// 负责检测和高亮窗口

use super::SystemError;
use crate::message::Command;
use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumChildWindows, EnumWindows, GetClassNameW, GetParent, GetWindowRect, GetWindowTextW,
    IsWindowVisible, WindowFromPoint,
};

/// 窗口信息（从原始代码迁移）
#[derive(Debug, Clone)]
pub struct WindowInfo {
    /// 窗口句柄
    pub hwnd: HWND,
    /// 窗口矩形
    pub rect: RECT,
    /// 窗口标题
    pub title: String,
    /// 窗口类名
    pub class_name: String,
    /// 是否可见
    pub is_visible: bool,
    /// 是否最小化
    pub is_minimized: bool,
}

impl WindowInfo {
    /// 检查点是否在窗口内
    pub fn contains_point(&self, x: i32, y: i32) -> bool {
        x >= self.rect.left && x <= self.rect.right && y >= self.rect.top && y <= self.rect.bottom
    }
}

/// 子控件信息（从原始代码迁移）
#[derive(Debug, Clone)]
pub struct ChildControlInfo {
    /// 控件句柄
    pub hwnd: HWND,
    /// 控件矩形
    pub rect: RECT,
    /// 控件标题/文本
    pub title: String,
    /// 控件类名
    pub class_name: String,
    /// 是否可见
    pub is_visible: bool,
    /// 父窗口句柄
    pub parent_hwnd: HWND,
    /// 控件ID
    pub control_id: i32,
}

impl ChildControlInfo {
    /// 检查点是否在子控件内
    pub fn contains_point(&self, x: i32, y: i32) -> bool {
        x >= self.rect.left && x <= self.rect.right && y >= self.rect.top && y <= self.rect.bottom
    }
}

/// 窗口检测器（从原始代码迁移）
#[derive(Debug)]
pub struct WindowDetector {
    windows: Vec<WindowInfo>,
    current_highlighted_window: Option<usize>,
    child_controls: Vec<ChildControlInfo>,
    current_highlighted_control: Option<usize>,
}

/// 窗口检测管理器
pub struct WindowDetectionManager {
    /// 窗口检测器
    detector: WindowDetector,
    /// 是否启用检测
    detection_enabled: bool,
}

impl WindowDetector {
    /// 创建新的窗口检测器
    pub fn new() -> Self {
        Self {
            windows: Vec::new(),
            current_highlighted_window: None,
            child_controls: Vec::new(),
            current_highlighted_control: None,
        }
    }

    /// 获取所有活动窗口（从原始代码迁移）
    pub fn refresh_windows(&mut self) -> Result<(), SystemError> {
        self.windows.clear();

        unsafe {
            // 使用EnumWindows枚举所有顶级窗口
            if EnumWindows(
                Some(enum_windows_proc),
                LPARAM(&mut self.windows as *mut _ as isize),
            )
            .is_err()
            {
                return Err(SystemError::WindowEnumerationFailed);
            }
        }

        // 过滤掉不需要的窗口（从原始代码迁移）
        self.windows.retain(|window| {
            window.is_visible
                && !window.is_minimized
                && !window.title.is_empty()
                && window.rect.right > window.rect.left
                && window.rect.bottom > window.rect.top
            // && !is_system_window(&window.class_name) // 启用系统窗口过滤
        });

        Ok(())
    }

    /// 刷新指定窗口的子控件（从原始代码迁移）
    pub fn refresh_child_controls(&mut self, parent_hwnd: HWND) -> Result<(), SystemError> {
        self.child_controls.clear();

        unsafe {
            // 枚举子窗口
            let result = EnumChildWindows(
                Some(parent_hwnd),
                Some(enum_child_windows_proc),
                LPARAM(&mut self.child_controls as *mut _ as isize),
            );
            if !result.as_bool() {
                return Err(SystemError::WindowEnumerationFailed);
            }
        }

        // 过滤掉不需要的子控件（从原始代码迁移）
        self.child_controls.retain(|control| {
            control.is_visible
                && control.rect.right > control.rect.left
                && control.rect.bottom > control.rect.top
        });

        Ok(())
    }

    /// 根据鼠标位置获取当前应该高亮的子控件（从原始代码迁移）
    pub fn get_child_control_at_point(&mut self, x: i32, y: i32) -> Option<&ChildControlInfo> {
        // 找到鼠标位置下的所有子控件
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

        // 使用ChildWindowFromPoint来获取最顶层的子控件（从原始代码迁移）
        unsafe {
            let point = POINT { x, y };
            // 先找到父窗口
            let parent_hwnd = WindowFromPoint(point);
            if !parent_hwnd.0.is_null() {
                use windows::Win32::UI::WindowsAndMessaging::ChildWindowFromPoint;
                let child_hwnd = ChildWindowFromPoint(parent_hwnd, point);
                if !child_hwnd.0.is_null() && child_hwnd != parent_hwnd {
                    // 在匹配的控件中查找对应的控件
                    for (index, control) in &matching_controls {
                        if control.hwnd == child_hwnd {
                            self.current_highlighted_control = Some(*index);
                            return Some(control);
                        }
                    }
                }
            }
        }

        // 如果ChildWindowFromPoint失败，返回最小的匹配控件（从原始代码迁移）
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

    /// 综合检测：根据鼠标位置同时检测窗口和子控件（从原始代码迁移）
    /// 返回 (窗口信息, 子控件信息)
    pub fn detect_at_point(
        &mut self,
        x: i32,
        y: i32,
    ) -> (Option<WindowInfo>, Option<ChildControlInfo>) {
        // 首先检测窗口（从原始代码迁移）
        let window = self.get_window_at_point(x, y);

        // 如果找到了窗口，再检测其子控件（从原始代码迁移）
        let control = if let Some(ref window_info) = window {
            // 刷新该窗口的子控件
            if self.refresh_child_controls(window_info.hwnd).is_ok() {
                self.get_child_control_at_point(x, y).cloned()
            } else {
                None
            }
        } else {
            None
        };

        (window, control)
    }

    /// 根据鼠标位置获取当前应该高亮的窗口（从原始代码迁移）
    fn get_window_at_point(&mut self, x: i32, y: i32) -> Option<WindowInfo> {
        // 找到鼠标位置下的所有窗口
        let mut matching_windows = Vec::new();
        for (index, window) in self.windows.iter().enumerate() {
            if window.contains_point(x, y) {
                matching_windows.push((index, window));
            }
        }

        if matching_windows.is_empty() {
            self.current_highlighted_window = None;
            return None;
        }

        // 如果只有一个窗口，直接返回
        if matching_windows.len() == 1 {
            let (index, window) = matching_windows[0];
            self.current_highlighted_window = Some(index);
            return Some(window.clone());
        }

        // 如果有多个窗口，使用WindowFromPoint来确定最顶层的窗口
        unsafe {
            let point = POINT { x, y };
            let hwnd_at_point = WindowFromPoint(point);

            if !hwnd_at_point.0.is_null() {
                // 获取顶级窗口（从原始代码迁移）
                let mut top_level_hwnd = hwnd_at_point;
                loop {
                    match GetParent(top_level_hwnd) {
                        Ok(parent) => {
                            if parent.0.is_null() {
                                break;
                            }
                            top_level_hwnd = parent;
                        }
                        Err(_) => break,
                    }
                }

                // 在匹配的窗口中查找对应的窗口
                for (index, window) in &matching_windows {
                    if window.hwnd == top_level_hwnd {
                        self.current_highlighted_window = Some(*index);
                        return Some((*window).clone());
                    }
                }
            }
        }

        // 如果没有找到精确匹配，返回第一个匹配的窗口
        if let Some((index, window)) = matching_windows.first() {
            self.current_highlighted_window = Some(*index);
            Some((*window).clone())
        } else {
            None
        }
    }
}

impl WindowDetectionManager {
    /// 创建新的窗口检测管理器
    pub fn new() -> Result<Self, SystemError> {
        Ok(Self {
            detector: WindowDetector::new(),
            detection_enabled: false,
        })
    }

    /// 启动窗口检测
    pub fn start_detection(&mut self) -> Result<(), SystemError> {
        // 刷新窗口列表以启动检测
        self.detector.refresh_windows()?;
        self.detection_enabled = true;
        Ok(())
    }

    /// 停止窗口检测
    pub fn stop_detection(&mut self) {
        self.detection_enabled = false;
    }

    /// 刷新窗口列表（从原始代码迁移）
    pub fn refresh_windows(&mut self) -> Result<(), SystemError> {
        self.detector.refresh_windows()
    }

    /// 检测指定点的窗口（从原始代码迁移）
    pub fn detect_window_at_point(
        &mut self,
        x: i32,
        y: i32,
    ) -> Result<Option<WindowInfo>, SystemError> {
        if !self.detection_enabled {
            return Ok(None);
        }

        let (window_info, _) = self.detector.detect_at_point(x, y);
        Ok(window_info)
    }

    /// 综合检测：根据鼠标位置同时检测窗口和子控件（从原始代码迁移）
    /// 返回 (窗口信息, 子控件信息)
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

/// EnumWindows的回调函数（从原始代码迁移）
unsafe extern "system" fn enum_windows_proc(hwnd: HWND, lparam: LPARAM) -> windows::core::BOOL {
    unsafe {
        let windows = &mut *(lparam.0 as *mut Vec<WindowInfo>);

        // 获取窗口矩形
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return windows::core::BOOL::from(true); // 继续枚举
        }

        // 修正全屏窗口的矩形坐标，确保不超出屏幕边界（从原始代码迁移）
        let (screen_width, screen_height) = crate::platform::windows::system::get_screen_size();

        // 限制窗口矩形在屏幕范围内
        rect.left = rect.left.max(0);
        rect.top = rect.top.max(0);
        rect.right = rect.right.min(screen_width);
        rect.bottom = rect.bottom.min(screen_height);

        // 获取窗口标题
        let mut title_buffer = [0u16; 256];
        let title_len = GetWindowTextW(hwnd, &mut title_buffer);
        let title = if title_len > 0 {
            String::from_utf16_lossy(&title_buffer[..title_len as usize])
        } else {
            String::new()
        };

        // 获取窗口类名（从原始代码迁移）
        let mut class_buffer = [0u16; 256];
        let class_len = GetClassNameW(hwnd, &mut class_buffer);
        let class_name = if class_len > 0 {
            String::from_utf16_lossy(&class_buffer[..class_len as usize])
        } else {
            String::new()
        };

        // 检查窗口是否可见和最小化状态（从原始代码迁移）
        let is_visible = IsWindowVisible(hwnd).as_bool();
        use windows::Win32::UI::WindowsAndMessaging::IsIconic;
        let is_minimized = IsIconic(hwnd).as_bool();

        // 创建窗口信息
        let window_info = WindowInfo {
            hwnd,
            rect,
            title,
            class_name,
            is_visible,
            is_minimized,
        };

        windows.push(window_info);
        windows::core::BOOL::from(true) // 继续枚举
    }
}

/// 检查是否为系统窗口（需要过滤掉的窗口）（从原始代码迁移）
#[allow(dead_code)]
fn is_system_window(class_name: &str) -> bool {
    const SYSTEM_CLASSES: &[&str] = &[
        "Shell_TrayWnd",              // 任务栏
        "DV2ControlHost",             // 系统控件
        "MsgrIMEWindowClass",         // 输入法
        "SysShadow",                  // 系统阴影
        "Button",                     // 系统按钮
        "Progman",                    // 桌面
        "WorkerW",                    // 桌面工作区
        "Windows.UI.Core.CoreWindow", // UWP应用核心窗口
        "ApplicationFrameWindow",     // UWP应用框架
        "ForegroundStaging",          // 前台暂存
        "MultitaskingViewFrame",      // 多任务视图
        "EdgeUiInputTopWndClass",     // Edge UI
        "NativeHWNDHost",             // 原生HWND主机
        "Chrome_WidgetWin_0",         // Chrome内部窗口（某些版本）
    ];

    SYSTEM_CLASSES
        .iter()
        .any(|&sys_class| class_name.contains(sys_class))
}

/// EnumChildWindows的回调函数（从原始代码迁移）
unsafe extern "system" fn enum_child_windows_proc(
    hwnd: HWND,
    lparam: LPARAM,
) -> windows::core::BOOL {
    unsafe {
        let child_controls = &mut *(lparam.0 as *mut Vec<ChildControlInfo>);

        // 获取子控件矩形（相对于屏幕坐标）
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return windows::core::BOOL::from(true); // 继续枚举
        }

        // 获取父窗口句柄
        let parent_hwnd = match GetParent(hwnd) {
            Ok(parent) => parent,
            Err(_) => return windows::core::BOOL::from(true),
        };

        // 获取控件标题/文本
        let mut title_buffer = [0u16; 256];
        let title_len = GetWindowTextW(hwnd, &mut title_buffer);
        let title = if title_len > 0 {
            String::from_utf16_lossy(&title_buffer[..title_len as usize])
        } else {
            String::new()
        };

        // 获取控件类名
        let mut class_buffer = [0u16; 256];
        let class_len = GetClassNameW(hwnd, &mut class_buffer);
        let class_name = if class_len > 0 {
            String::from_utf16_lossy(&class_buffer[..class_len as usize])
        } else {
            String::new()
        };

        // 获取控件ID
        use windows::Win32::UI::WindowsAndMessaging::GetDlgCtrlID;
        let control_id = GetDlgCtrlID(hwnd);

        // 检查控件是否可见
        let is_visible = IsWindowVisible(hwnd).as_bool();

        let control_info = ChildControlInfo {
            hwnd,
            rect,
            title,
            class_name,
            is_visible,
            parent_hwnd,
            control_id,
        };

        child_controls.push(control_info);
        windows::core::BOOL::from(true) // 继续枚举
    }
}
