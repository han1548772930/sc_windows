use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::UI::{Shell::*, WindowsAndMessaging::*};

use super::SystemError;
use crate::message::Command;
use crate::platform::windows::SafeHwnd;
use crate::settings::show_settings_window;
use crate::utils::win_api::close_all_app_windows;
use crate::utils::to_wide_chars;

/// 系统托盘管理器
#[derive(Debug)]
pub struct TrayManager {
    hwnd: SafeHwnd,
    icon_id: u32,
    is_added: bool,
}

impl TrayManager {
    /// 创建新的托盘管理器
    pub fn new() -> std::result::Result<Self, SystemError> {
        Ok(Self {
            hwnd: SafeHwnd::default(),
            icon_id: 1001,
            is_added: false,
        })
    }

    /// 初始化系统托盘
    pub fn initialize(&mut self, hwnd: HWND) -> std::result::Result<(), SystemError> {
        self.hwnd.set(Some(hwnd));

        // 创建托盘图标
        let icon = create_default_icon()?;

        // 添加托盘图标
        self.add_icon("截图工具 - Ctrl+Alt+S 截图，右键查看菜单", icon)?;

        Ok(())
    }

    /// 添加托盘图标
    pub fn add_icon(&mut self, tooltip: &str, icon: HICON) -> std::result::Result<(), SystemError> {
        if self.is_added {
            return Ok(());
        }

        unsafe {
            let tooltip_wide = to_wide_chars(tooltip);
            let mut tooltip_array = [0u16; 128];
            let copy_len = (tooltip_wide.len() - 1).min(tooltip_array.len() - 1);
            tooltip_array[..copy_len].copy_from_slice(&tooltip_wide[..copy_len]);

            let hwnd = self.hwnd.get().unwrap_or(HWND(std::ptr::null_mut()));
            let nid = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: hwnd,
                uID: self.icon_id,
                uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
                uCallbackMessage: WM_USER + 1, // 自定义消息
                hIcon: icon,
                szTip: tooltip_array,
                ..Default::default()
            };

            let result = Shell_NotifyIconW(NIM_ADD, &nid);
            if result.as_bool() {
                self.is_added = true;
                Ok(())
            } else {
                Err(SystemError::TrayError(
                    "Failed to add tray icon".to_string(),
                ))
            }
        }
    }

    /// 处理托盘消息
    pub fn handle_message(&mut self, _wparam: u32, lparam: u32) -> Vec<Command> {
        match lparam {
            WM_RBUTTONUP => {
                // 右键菜单
                self.show_context_menu();
                vec![]
            }
            WM_LBUTTONDBLCLK => {
                // 双击托盘图标，显示设置窗口
                vec![Command::ShowSettings]
            }
            _ => vec![],
        }
    }

    /// 显示右键菜单
    fn show_context_menu(&self) {
        unsafe {
            let hmenu = CreatePopupMenu().unwrap_or_default();
            if hmenu.is_invalid() {
                return;
            }

            // 添加菜单项
            let _ = AppendMenuW(
                hmenu,
                MF_STRING,
                1001,
                windows::core::w!("截图(&S)\tCtrl+Alt+S"),
            );
            let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, windows::core::PCWSTR::null());
            let _ = AppendMenuW(hmenu, MF_STRING, 1002, windows::core::w!("设置(&T)"));
            let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, windows::core::PCWSTR::null());
            let _ = AppendMenuW(hmenu, MF_STRING, 1003, windows::core::w!("退出(&X)"));

            // 获取鼠标位置
            let mut cursor_pos = POINT::default();
            let _ = GetCursorPos(&mut cursor_pos);

            // 设置前台窗口以确保菜单正确显示
            let hwnd = self.hwnd.get().unwrap_or(HWND(std::ptr::null_mut()));
            let _ = SetForegroundWindow(hwnd);

            // 显示菜单
            let cmd = TrackPopupMenu(
                hmenu,
                TPM_RIGHTBUTTON | TPM_RETURNCMD,
                cursor_pos.x,
                cursor_pos.y,
                Some(0),
                hwnd,
                None,
            );

            // 处理菜单选择
            match cmd.0 {
                1001 => {
                    // 截图
                    let hwnd = self.hwnd.get();
                    let _ = PostMessageW(hwnd, WM_HOTKEY, WPARAM(1001), LPARAM(0));
                }
                1002 => {
                    // 设置
                    let _ = show_settings_window();
                }
                1003 => {
                    // 退出 - 优雅关闭所有属于本进程的窗口
                    close_all_app_windows();
                }
                _ => {}
            }

            let _ = DestroyMenu(hmenu);
        }
    }

    /// 重新加载设置
    pub fn reload_settings(&mut self) {
        // 托盘设置通常不需要重新加载，但可以在这里添加相关逻辑
        // 例如更新托盘图标或提示文本
    }

    /// 清理托盘资源
    pub fn cleanup(&mut self) {
        if self.is_added {
            unsafe {
                let hwnd = self.hwnd.get().unwrap_or(HWND(std::ptr::null_mut()));
                let nid = NOTIFYICONDATAW {
                    cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                    hWnd: hwnd,
                    uID: self.icon_id,
                    uFlags: NIF_ICON,
                    ..Default::default()
                };

                let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
                self.is_added = false;
            }
        }
    }
}

/// 创建默认图标（简化版本，直接从嵌入数据加载）
pub fn create_default_icon() -> std::result::Result<HICON, SystemError> {
    unsafe {
        // 使用 include_bytes! 在编译时嵌入图标文件
        const ICON_DATA: &[u8] = include_bytes!("../../icons/i.ico");

        // 直接从嵌入的字节数据加载图标，避免复杂的文件操作
        match load_embedded_icon(ICON_DATA) {
            Ok(icon) => Ok(icon),
            Err(_e) => {
                // 如果嵌入图标加载失败，使用系统默认图标
                LoadIconW(None, IDI_APPLICATION).map_err(|e| {
                    SystemError::TrayError(format!("Failed to load default icon: {e:?}"))
                })
            }
        }
    }
}

/// 从嵌入的字节数据直接加载图标（简化版本）
fn load_embedded_icon(icon_data: &[u8]) -> std::result::Result<HICON, SystemError> {
    unsafe {
        // 创建临时文件来存储图标数据（ICO格式需要文件路径）
        let temp_path = std::env::temp_dir().join("temp_tray_icon.ico");
        std::fs::write(&temp_path, icon_data).map_err(|e| {
            SystemError::TrayError(format!("Failed to write temp icon file: {e:?}"))
        })?;

        // 直接加载ICO文件
        let path_wide = to_wide_chars(&temp_path.to_string_lossy());
        let result = LoadImageW(
            None,
            PCWSTR(path_wide.as_ptr()),
            IMAGE_ICON,
            16, // 系统托盘图标标准大小
            16,
            LR_LOADFROMFILE,
        )
        .map(|h| HICON(h.0))
        .map_err(|e| SystemError::TrayError(format!("Failed to load embedded icon: {e:?}")));

        // 清理临时文件
        let _ = std::fs::remove_file(&temp_path);

        result
    }
}

