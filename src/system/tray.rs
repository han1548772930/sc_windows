// 系统托盘管理（从原始代码完整迁移）
//
// 负责系统托盘的创建、消息处理和菜单管理

use super::SystemError;
use crate::message::Command;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::{Shell::*, WindowsAndMessaging::*};
use windows::core::*;

/// 系统托盘管理器（从原始代码迁移）
#[derive(Debug)]
pub struct TrayManager {
    hwnd: HWND,
    icon_id: u32,
    is_added: bool,
}

impl TrayManager {
    /// 创建新的托盘管理器
    pub fn new() -> std::result::Result<Self, SystemError> {
        Ok(Self {
            hwnd: HWND(std::ptr::null_mut()),
            icon_id: 1001,
            is_added: false,
        })
    }

    /// 初始化系统托盘（从原始代码迁移）
    pub fn initialize(&mut self, hwnd: HWND) -> std::result::Result<(), SystemError> {
        self.hwnd = hwnd;

        // 创建托盘图标
        let icon = create_default_icon()?;

        // 添加托盘图标
        self.add_icon("截图工具 - Ctrl+Alt+S 截图，右键查看菜单", icon)?;

        Ok(())
    }

    /// 添加托盘图标（从原始代码迁移）
    pub fn add_icon(&mut self, tooltip: &str, icon: HICON) -> std::result::Result<(), SystemError> {
        if self.is_added {
            return Ok(());
        }

        unsafe {
            let tooltip_wide = crate::utils::to_wide_chars(tooltip);
            let mut tooltip_array = [0u16; 128];
            let copy_len = (tooltip_wide.len() - 1).min(tooltip_array.len() - 1);
            tooltip_array[..copy_len].copy_from_slice(&tooltip_wide[..copy_len]);

            let mut nid = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: self.hwnd,
                uID: self.icon_id,
                uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
                uCallbackMessage: WM_USER + 1, // 自定义消息
                hIcon: icon,
                szTip: tooltip_array,
                ..Default::default()
            };

            let result = Shell_NotifyIconW(NIM_ADD, &mut nid);
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

    /// 处理托盘消息（从原始代码迁移）
    pub fn handle_message(&mut self, _wparam: u32, lparam: u32) -> Vec<Command> {
        match lparam as u32 {
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

    /// 显示右键菜单（从原始代码迁移）
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
            let _ = SetForegroundWindow(self.hwnd);

            // 显示菜单
            let cmd = TrackPopupMenu(
                hmenu,
                TPM_RIGHTBUTTON | TPM_RETURNCMD,
                cursor_pos.x,
                cursor_pos.y,
                Some(0),
                self.hwnd,
                None,
            );

            // 处理菜单选择
            match cmd.0 {
                1001 => {
                    // 截图
                    let _ = PostMessageW(Some(self.hwnd), WM_HOTKEY, WPARAM(1001), LPARAM(0));
                }
                1002 => {
                    // 设置
                    let _ = crate::simple_settings::show_settings_window();
                }
                1003 => {
                    // 退出
                    let _ = PostMessageW(Some(self.hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
                }
                _ => {}
            }

            let _ = DestroyMenu(hmenu);
        }
    }

    /// 重新加载设置（从原始代码迁移）
    pub fn reload_settings(&mut self) {
        // 托盘设置通常不需要重新加载，但可以在这里添加相关逻辑
        // 例如更新托盘图标或提示文本
    }

    /// 清理托盘资源（从原始代码迁移）
    pub fn cleanup(&mut self) {
        if self.is_added {
            unsafe {
                let mut nid = NOTIFYICONDATAW {
                    cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                    hWnd: self.hwnd,
                    uID: self.icon_id,
                    uFlags: NIF_ICON,
                    ..Default::default()
                };

                let _ = Shell_NotifyIconW(NIM_DELETE, &mut nid);
                self.is_added = false;
            }
        }
    }
}

/// 创建默认图标（从原始代码完整迁移）
pub fn create_default_icon() -> std::result::Result<HICON, SystemError> {
    unsafe {
        // 使用 include_bytes! 在编译时嵌入图标文件（与原始代码一致）
        const ICON_DATA: &[u8] = include_bytes!("../../icons/i.ico");

        match load_icon_from_bytes(ICON_DATA) {
            Ok(icon) => Ok(icon),
            Err(_e) => {
                // 如果嵌入图标加载失败，使用系统默认图标
                LoadIconW(None, IDI_APPLICATION).map_err(|e| {
                    SystemError::TrayError(format!("Failed to load default icon: {:?}", e))
                })
            }
        }
    }
}

/// 从字节数据加载图标（从原始代码迁移）
pub fn load_icon_from_bytes(image_data: &[u8]) -> std::result::Result<HICON, SystemError> {
    unsafe {
        // 创建临时文件来存储图像数据
        let temp_path = std::env::temp_dir().join("temp_icon.ico");
        std::fs::write(&temp_path, image_data).map_err(|e| {
            SystemError::TrayError(format!("Failed to write temp icon file: {:?}", e))
        })?;

        // 使用临时文件加载图标
        let result = load_icon_from_file(&temp_path.to_string_lossy());

        // 清理临时文件
        let _ = std::fs::remove_file(&temp_path);

        result
    }
}

/// 从文件加载图标（从原始代码迁移）
pub fn load_icon_from_file(file_path: &str) -> std::result::Result<HICON, SystemError> {
    unsafe {
        let path_wide = crate::utils::to_wide_chars(file_path);

        // 如果是 ICO 文件，直接加载图标
        if file_path.to_lowercase().ends_with(".ico") {
            LoadImageW(
                None,
                PCWSTR(path_wide.as_ptr()),
                IMAGE_ICON,
                0,
                0,
                LR_LOADFROMFILE,
            )
            .map(|h| HICON(h.0))
            .map_err(|e| SystemError::TrayError(format!("Failed to load icon from file: {:?}", e)))
        } else {
            // 对于其他格式，先加载为位图然后转换为图标
            let hbitmap = LoadImageW(
                None,
                PCWSTR(path_wide.as_ptr()),
                IMAGE_BITMAP,
                0,
                0,
                LR_LOADFROMFILE,
            )
            .map_err(|e| SystemError::TrayError(format!("Failed to load bitmap: {:?}", e)))?;

            // 将位图转换为图标
            let bitmap = HBITMAP(hbitmap.0);
            create_icon_from_bitmap(bitmap)
        }
    }
}

/// 从位图创建图标（从原始代码迁移）
fn create_icon_from_bitmap(bitmap: HBITMAP) -> std::result::Result<HICON, SystemError> {
    unsafe {
        // 获取位图信息
        let mut bm = BITMAP::default();
        if GetObjectW(
            bitmap.into(),
            std::mem::size_of::<BITMAP>() as i32,
            Some(&mut bm as *mut _ as *mut _),
        ) == 0
        {
            return Err(SystemError::TrayError(
                "Failed to get bitmap info".to_string(),
            ));
        }

        // 创建掩码位图
        let hdc = GetDC(Some(HWND(std::ptr::null_mut())));
        let mask_bitmap = CreateCompatibleBitmap(hdc, bm.bmWidth, bm.bmHeight);
        if mask_bitmap.is_invalid() {
            return Err(SystemError::TrayError(
                "Failed to create mask bitmap".to_string(),
            ));
        }
        let _ = ReleaseDC(Some(HWND(std::ptr::null_mut())), hdc);

        // 创建图标信息结构
        let icon_info = ICONINFO {
            fIcon: TRUE,
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask_bitmap,
            hbmColor: bitmap,
        };

        // 创建图标
        let icon = CreateIconIndirect(&icon_info)
            .map_err(|e| SystemError::TrayError(format!("Failed to create icon: {:?}", e)))?;

        // 清理资源
        let _ = DeleteObject(mask_bitmap.into());

        Ok(icon)
    }
}
