// 这是一个展示如何在你的截图应用中使用系统托盘的示例

use windows::{
    Win32::{
        Foundation::*,
        UI::WindowsAndMessaging::*,
    },
};

use crate::system_tray::*;

/// 在WindowState中添加托盘支持的示例
impl crate::WindowState {
    /// 初始化系统托盘
    pub fn init_system_tray(&mut self, hwnd: HWND) -> windows::core::Result<()> {
        // 创建托盘图标
        let icon = create_default_icon()?;
        
        // 创建托盘实例
        let mut tray = SystemTray::new(hwnd, 1001); // 使用ID 1001
        
        // 添加托盘图标
        tray.add_icon("截图工具 - 点击右键查看菜单", icon)?;
        
        // 可以保存到WindowState中
        // self.system_tray = Some(tray);
        
        Ok(())
    }

    /// 处理托盘消息的示例
    pub fn handle_tray_message(&mut self, hwnd: HWND, wparam: WPARAM, lparam: LPARAM) {
        let tray_msg = handle_tray_message(wparam, lparam);
        
        match tray_msg {
            TrayMessage::LeftClick(_) => {
                // 左键点击 - 显示/隐藏窗口
                unsafe {
                    if IsWindowVisible(hwnd).as_bool() {
                        ShowWindow(hwnd, SW_HIDE);
                    } else {
                        ShowWindow(hwnd, SW_SHOW);
                        SetForegroundWindow(hwnd);
                    }
                }
            }
            TrayMessage::RightClick(_) => {
                // 右键点击 - 显示上下文菜单
                self.show_tray_context_menu(hwnd);
            }
            TrayMessage::DoubleClick(_) => {
                // 双击 - 开始截图
                unsafe {
                    ShowWindow(hwnd, SW_SHOW);
                    SetForegroundWindow(hwnd);
                }
                // 这里可以触发截图功能
            }
            _ => {}
        }
    }

    /// 显示托盘右键菜单
    fn show_tray_context_menu(&self, hwnd: HWND) {
        unsafe {
            // 创建弹出菜单
            let hmenu = CreatePopupMenu().unwrap();
            
            // 添加菜单项
            let show_text = crate::utils::to_wide_chars("显示窗口");
            let screenshot_text = crate::utils::to_wide_chars("开始截图");
            let exit_text = crate::utils::to_wide_chars("退出");
            
            AppendMenuW(hmenu, MF_STRING, 1001, windows::core::PCWSTR(show_text.as_ptr()));
            AppendMenuW(hmenu, MF_STRING, 1002, windows::core::PCWSTR(screenshot_text.as_ptr()));
            AppendMenuW(hmenu, MF_SEPARATOR, 0, windows::core::PCWSTR::null());
            AppendMenuW(hmenu, MF_STRING, 1003, windows::core::PCWSTR(exit_text.as_ptr()));
            
            // 获取鼠标位置
            let mut cursor_pos = POINT::default();
            GetCursorPos(&mut cursor_pos);
            
            // 显示菜单
            SetForegroundWindow(hwnd); // 确保菜单能正确显示
            let cmd = TrackPopupMenu(
                hmenu,
                TPM_RIGHTBUTTON | TPM_RETURNCMD,
                cursor_pos.x,
                cursor_pos.y,
                0,
                hwnd,
                None,
            );
            
            // 处理菜单选择
            match cmd {
                1001 => {
                    // 显示窗口
                    ShowWindow(hwnd, SW_SHOW);
                    SetForegroundWindow(hwnd);
                }
                1002 => {
                    // 开始截图
                    ShowWindow(hwnd, SW_SHOW);
                    SetForegroundWindow(hwnd);
                    // 触发截图逻辑
                }
                1003 => {
                    // 退出程序
                    PostQuitMessage(0);
                }
                _ => {}
            }
            
            // 清理菜单
            DestroyMenu(hmenu);
        }
    }

    /// 显示托盘通知
    pub fn show_tray_notification(&self, title: &str, message: &str) {
        // 如果有托盘实例，显示气球提示
        // if let Some(ref tray) = self.system_tray {
        //     let _ = tray.show_balloon(title, message, BalloonIconType::Info);
        // }
    }
}

/// 在窗口过程中处理托盘消息的示例
pub fn handle_tray_in_window_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
    const WM_TRAYICON: u32 = WM_USER + 1;
    
    match msg {
        val if val == WM_TRAYICON => {
            // 获取WindowState并处理托盘消息
            unsafe {
                let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut crate::WindowState;
                if !state_ptr.is_null() {
                    let state = &mut *state_ptr;
                    state.handle_tray_message(hwnd, wparam, lparam);
                }
            }
            Some(LRESULT(0))
        }
        _ => None
    }
}

/// 托盘使用的完整示例
pub fn example_usage() {
    /*
    // 在WindowState中添加字段：
    pub struct WindowState {
        // ... 其他字段
        pub system_tray: Option<SystemTray>,
    }

    // 在窗口初始化时：
    let mut state = WindowState::new(hwnd)?;
    state.init_system_tray(hwnd)?;

    // 在窗口过程中：
    unsafe extern "system" fn window_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        // 首先检查托盘消息
        if let Some(result) = handle_tray_in_window_proc(hwnd, msg, wparam, lparam) {
            return result;
        }

        // 处理其他消息...
        match msg {
            // ... 其他消息处理
        }
    }
    */
}
