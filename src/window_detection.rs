use windows::{
    Win32::{Foundation::*, UI::WindowsAndMessaging::*},
    core::*,
};

/// 窗口信息结构体
#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub hwnd: HWND,
    pub rect: RECT,
    pub title: String,
    pub class_name: String,
    pub is_visible: bool,
    pub is_minimized: bool,
}

/// 子控件信息结构体
#[derive(Debug, Clone)]
pub struct ChildControlInfo {
    pub hwnd: HWND,
    pub rect: RECT,
    pub title: String,
    pub class_name: String,
    pub is_visible: bool,
    pub parent_hwnd: HWND,
    pub control_id: i32,
}

impl WindowInfo {
    /// 检查点是否在窗口内
    pub fn contains_point(&self, x: i32, y: i32) -> bool {
        x >= self.rect.left && x <= self.rect.right && y >= self.rect.top && y <= self.rect.bottom
    }
}

impl ChildControlInfo {
    /// 检查点是否在子控件内
    pub fn contains_point(&self, x: i32, y: i32) -> bool {
        x >= self.rect.left && x <= self.rect.right && y >= self.rect.top && y <= self.rect.bottom
    }
}

/// 窗口检测器
#[derive(Debug)]
pub struct WindowDetector {
    windows: Vec<WindowInfo>,
    current_highlighted_window: Option<usize>,
    child_controls: Vec<ChildControlInfo>,
    current_highlighted_control: Option<usize>,
}

impl WindowDetector {
    pub fn new() -> Self {
        Self {
            windows: Vec::new(),
            current_highlighted_window: None,
            child_controls: Vec::new(),
            current_highlighted_control: None,
        }
    }

    /// 获取所有活动窗口
    pub fn refresh_windows(&mut self) -> Result<()> {
        self.windows.clear();

        unsafe {
            // 使用EnumWindows枚举所有顶级窗口
            EnumWindows(
                Some(enum_windows_proc),
                LPARAM(&mut self.windows as *mut _ as isize),
            )?;
        }

        // 过滤掉不需要的窗口
        self.windows.retain(|window| {
            window.is_visible
                && !window.is_minimized
                && !window.title.is_empty()
                && window.rect.right > window.rect.left
                && window.rect.bottom > window.rect.top
            // && !is_system_window(&window.class_name)
        });

        Ok(())
    }

    /// 根据鼠标位置获取当前应该高亮的窗口（考虑Z-order层级）
    pub fn get_window_at_point(&mut self, x: i32, y: i32) -> Option<&WindowInfo> {
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
            return Some(window);
        }

        // 如果有多个窗口，使用WindowFromPoint来确定最顶层的窗口
        unsafe {
            let point = POINT { x, y };
            let hwnd_at_point = WindowFromPoint(point);

            if !hwnd_at_point.0.is_null() {
                // 获取顶级窗口
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
                        return Some(window);
                    }
                }
            }
        }

        // 如果WindowFromPoint失败，返回第一个匹配的窗口（通常是最小的）
        let (index, window) = matching_windows[0];
        self.current_highlighted_window = Some(index);
        Some(window)
    }

    /// 获取当前高亮的窗口
    pub fn get_current_highlighted_window(&self) -> Option<&WindowInfo> {
        if let Some(index) = self.current_highlighted_window {
            self.windows.get(index)
        } else {
            None
        }
    }

    /// 获取所有窗口
    pub fn get_all_windows(&self) -> &Vec<WindowInfo> {
        &self.windows
    }

    /// 刷新指定窗口的子控件
    pub fn refresh_child_controls(&mut self, parent_hwnd: HWND) -> Result<()> {
        self.child_controls.clear();

        unsafe {
            // 枚举子窗口
            let result = EnumChildWindows(
                Some(parent_hwnd),
                Some(enum_child_windows_proc),
                LPARAM(&mut self.child_controls as *mut _ as isize),
            );
            if !result.as_bool() {
                return Err(Error::from_win32());
            }
        }

        // 过滤掉不需要的子控件
        self.child_controls.retain(|control| {
            control.is_visible
                && control.rect.right > control.rect.left
                && control.rect.bottom > control.rect.top
        });

        Ok(())
    }

    /// 根据鼠标位置获取当前应该高亮的子控件
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

        // 使用ChildWindowFromPoint来获取最顶层的子控件
        unsafe {
            let point = POINT { x, y };
            // 先找到父窗口
            let parent_hwnd = WindowFromPoint(point);
            if !parent_hwnd.0.is_null() {
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

        // 如果ChildWindowFromPoint失败，返回最小的匹配控件
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

    /// 获取当前高亮的子控件
    pub fn get_current_highlighted_control(&self) -> Option<&ChildControlInfo> {
        if let Some(index) = self.current_highlighted_control {
            self.child_controls.get(index)
        } else {
            None
        }
    }

    /// 获取所有子控件
    pub fn get_all_child_controls(&self) -> &Vec<ChildControlInfo> {
        &self.child_controls
    }

    /// 清除子控件选择
    pub fn clear_child_control_selection(&mut self) {
        self.current_highlighted_control = None;
    }

    /// 综合检测：根据鼠标位置同时检测窗口和子控件
    /// 返回 (窗口信息, 子控件信息)
    pub fn detect_at_point(
        &mut self,
        x: i32,
        y: i32,
    ) -> (Option<WindowInfo>, Option<ChildControlInfo>) {
        // 首先检测窗口
        let window = self.get_window_at_point(x, y).cloned();

        // 如果找到了窗口，再检测其子控件
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

    /// 检查是否应该过滤掉指定的子控件
    pub fn should_filter_child_control(&self, control: &ChildControlInfo) -> bool {
        // 过滤掉一些不需要的控件类型
        matches!(
            control.class_name.as_str(),
            "ScrollBar"
                | "Static"
                | "#32770"
                | "SysTabControl32"
                | "msctls_progress32"
                | "msctls_trackbar32"
                | "ToolbarWindow32"
        )
    }
}

/// EnumWindows的回调函数
unsafe extern "system" fn enum_windows_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    unsafe {
        let windows = &mut *(lparam.0 as *mut Vec<WindowInfo>);

        // 获取窗口矩形
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return TRUE; // 继续枚举
        }

        // 修正全屏窗口的矩形坐标，确保不超出屏幕边界
        let screen_width = GetSystemMetrics(SM_CXSCREEN);
        let screen_height = GetSystemMetrics(SM_CYSCREEN);

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

        // 获取窗口类名
        let mut class_buffer = [0u16; 256];
        let class_len = GetClassNameW(hwnd, &mut class_buffer);
        let class_name = if class_len > 0 {
            String::from_utf16_lossy(&class_buffer[..class_len as usize])
        } else {
            String::new()
        };

        // 检查窗口是否可见和最小化状态
        let is_visible = IsWindowVisible(hwnd).as_bool();
        let is_minimized = IsIconic(hwnd).as_bool();

        let window_info = WindowInfo {
            hwnd,
            rect,
            title,
            class_name,
            is_visible,
            is_minimized,
        };

        windows.push(window_info);
        TRUE // 继续枚举
    }
}

/// EnumChildWindows的回调函数
unsafe extern "system" fn enum_child_windows_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    unsafe {
        let child_controls = &mut *(lparam.0 as *mut Vec<ChildControlInfo>);

        // 获取子控件矩形（相对于屏幕坐标）
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return TRUE; // 继续枚举
        }

        // 获取父窗口句柄
        let parent_hwnd = match GetParent(hwnd) {
            Ok(parent) => parent,
            Err(_) => return TRUE,
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
        TRUE // 继续枚举
    }
}

/// 检查是否为系统窗口（需要过滤掉的窗口）
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

/// 获取窗口在屏幕上的实际可见区域（考虑被其他窗口遮挡的情况）
pub fn get_visible_window_region(hwnd: HWND) -> Result<Vec<RECT>> {
    unsafe {
        let mut window_rect = RECT::default();
        GetWindowRect(hwnd, &mut window_rect)?;

        // 简化版本：直接返回窗口矩形
        // 在实际应用中，可以使用更复杂的算法来计算可见区域
        Ok(vec![window_rect])
    }
}
