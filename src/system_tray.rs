use windows::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        UI::{Shell::*, WindowsAndMessaging::*},
    },
    core::*,
};

use crate::utils::to_wide_chars;

/// 系统托盘管理器
#[derive(Debug)]
pub struct SystemTray {
    hwnd: HWND,
    icon_id: u32,
    is_added: bool,
}

impl SystemTray {
    /// 创建新的系统托盘实例
    pub fn new(hwnd: HWND, icon_id: u32) -> Self {
        Self {
            hwnd,
            icon_id,
            is_added: false,
        }
    }

    /// 添加托盘图标
    pub fn add_icon(&mut self, tooltip: &str, icon: HICON) -> Result<()> {
        if self.is_added {
            return Ok(());
        }

        unsafe {
            let tooltip_wide = to_wide_chars(tooltip);
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
                Err(Error::from_win32())
            }
        }
    }

    /// 更新托盘图标
    pub fn update_icon(&self, tooltip: &str, icon: HICON) -> Result<()> {
        if !self.is_added {
            return Err(Error::from(E_FAIL));
        }

        unsafe {
            let tooltip_wide = to_wide_chars(tooltip);
            let mut tooltip_array = [0u16; 128];
            let copy_len = (tooltip_wide.len() - 1).min(tooltip_array.len() - 1);
            tooltip_array[..copy_len].copy_from_slice(&tooltip_wide[..copy_len]);

            let mut nid = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: self.hwnd,
                uID: self.icon_id,
                uFlags: NIF_ICON | NIF_TIP,
                hIcon: icon,
                szTip: tooltip_array,
                ..Default::default()
            };

            let result = Shell_NotifyIconW(NIM_MODIFY, &mut nid);
            if result.as_bool() {
                Ok(())
            } else {
                Err(Error::from_win32())
            }
        }
    }

    /// 显示气球提示
    pub fn show_balloon(&self, title: &str, text: &str, icon_type: BalloonIconType) -> Result<()> {
        if !self.is_added {
            return Err(Error::from(E_FAIL));
        }

        unsafe {
            let title_wide = to_wide_chars(title);
            let text_wide = to_wide_chars(text);

            let mut title_array = [0u16; 64];
            let mut text_array = [0u16; 256];

            let title_len = (title_wide.len() - 1).min(title_array.len() - 1);
            let text_len = (text_wide.len() - 1).min(text_array.len() - 1);

            title_array[..title_len].copy_from_slice(&title_wide[..title_len]);
            text_array[..text_len].copy_from_slice(&text_wide[..text_len]);

            let mut nid = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: self.hwnd,
                uID: self.icon_id,
                uFlags: NIF_INFO,
                szInfoTitle: title_array,
                szInfo: text_array,
                Anonymous: NOTIFYICONDATAW_0 {
                    uTimeout: 5000, // 5秒超时
                },
                dwInfoFlags: match icon_type {
                    BalloonIconType::None => NIIF_NONE,
                    BalloonIconType::Info => NIIF_INFO,
                    BalloonIconType::Warning => NIIF_WARNING,
                    BalloonIconType::Error => NIIF_ERROR,
                },
                ..Default::default()
            };

            let result = Shell_NotifyIconW(NIM_MODIFY, &mut nid);
            if result.as_bool() {
                Ok(())
            } else {
                Err(Error::from_win32())
            }
        }
    }

    /// 移除托盘图标
    pub fn remove_icon(&mut self) -> Result<()> {
        if !self.is_added {
            return Ok(());
        }

        unsafe {
            let mut nid = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: self.hwnd,
                uID: self.icon_id,
                ..Default::default()
            };

            let result = Shell_NotifyIconW(NIM_DELETE, &mut nid);
            if result.as_bool() {
                self.is_added = false;
                Ok(())
            } else {
                Err(Error::from_win32())
            }
        }
    }

    /// 检查托盘图标是否已添加
    pub fn is_added(&self) -> bool {
        self.is_added
    }
}

impl Drop for SystemTray {
    fn drop(&mut self) {
        let _ = self.remove_icon();
    }
}

/// 气球提示图标类型
#[derive(Debug, Clone, Copy)]
pub enum BalloonIconType {
    None,
    Info,
    Warning,
    Error,
}

/// 托盘消息处理
pub fn handle_tray_message(wparam: WPARAM, lparam: LPARAM) -> TrayMessage {
    let icon_id = wparam.0 as u32;
    let message = (lparam.0 & 0xFFFF) as u32; // 等同于LOWORD

    match message {
        val if val == WM_LBUTTONDOWN => TrayMessage::LeftClick(icon_id),
        val if val == WM_RBUTTONDOWN => TrayMessage::RightClick(icon_id),
        val if val == WM_LBUTTONDBLCLK => TrayMessage::DoubleClick(icon_id),
        val if val == WM_MOUSEMOVE => TrayMessage::MouseMove(icon_id),
        _ => TrayMessage::Unknown(icon_id, message),
    }
}

/// 托盘消息类型
#[derive(Debug, Clone)]
pub enum TrayMessage {
    LeftClick(u32),
    RightClick(u32),
    DoubleClick(u32),
    MouseMove(u32),
    Unknown(u32, u32),
}

/// 创建默认应用程序图标
pub fn create_default_icon() -> Result<HICON> {
    unsafe {
        // 使用 include_bytes! 在编译时嵌入图标文件
        const ICON_DATA: &[u8] = include_bytes!("../icons/i.ico");

        match load_icon_from_bytes(ICON_DATA) {
            Ok(icon) => Ok(icon),
            Err(_e) => LoadIconW(None, IDI_APPLICATION),
        }
    }
}

/// 从字节数据加载图标
pub fn load_icon_from_bytes(image_data: &[u8]) -> Result<HICON> {
    unsafe {
        // 创建临时文件来存储图像数据
        let temp_path = std::env::temp_dir().join("temp_icon.ico");
        std::fs::write(&temp_path, image_data)?;

        // 使用临时文件加载图标
        let result = load_icon_from_file(&temp_path.to_string_lossy());

        // 清理临时文件
        let _ = std::fs::remove_file(&temp_path);

        result
    }
}

/// 从资源加载图标
pub fn load_icon_from_resource(instance: HINSTANCE, resource_id: u16) -> Result<HICON> {
    unsafe { LoadIconW(Some(instance), PCWSTR(resource_id as *const u16)) }
}

/// 从文件加载图标
pub fn load_icon_from_file(file_path: &str) -> Result<HICON> {
    unsafe {
        let path_wide = to_wide_chars(file_path);

        // 如果是 ICO 文件，直接加载图标
        if file_path.to_lowercase().ends_with(".ico") {
            // 直接加载图标文件
            LoadImageW(
                None,
                PCWSTR(path_wide.as_ptr()),
                IMAGE_ICON,
                16, // 小图标大小，适合系统托盘
                16,
                LR_LOADFROMFILE,
            )
            .map(|handle| HICON(handle.0))
        } else if file_path.to_lowercase().ends_with(".jpg")
            || file_path.to_lowercase().ends_with(".jpeg")
        {
            // 加载为位图
            let hbitmap = LoadImageW(
                None,
                PCWSTR(path_wide.as_ptr()),
                IMAGE_BITMAP,
                16, // 图标大小
                16,
                LR_LOADFROMFILE,
            )?;

            // 将位图转换为图标
            let bitmap = HBITMAP(hbitmap.0);
            create_icon_from_bitmap(bitmap)
        } else {
            // 直接加载图标文件
            LoadImageW(
                None,
                PCWSTR(path_wide.as_ptr()),
                IMAGE_ICON,
                16,
                16,
                LR_LOADFROMFILE,
            )
            .map(|handle| HICON(handle.0))
        }
    }
}

/// 从位图创建图标
fn create_icon_from_bitmap(bitmap: HBITMAP) -> Result<HICON> {
    unsafe {
        // 获取位图信息
        let mut bm = BITMAP::default();
        if GetObjectW(
            bitmap.into(),
            std::mem::size_of::<BITMAP>() as i32,
            Some(&mut bm as *mut _ as *mut _),
        ) == 0
        {
            return Err(Error::from_win32());
        }

        // 创建掩码位图（全白色）
        let hdc = GetDC(None);
        let mask_bitmap = CreateCompatibleBitmap(hdc, bm.bmWidth, bm.bmHeight);
        let mask_dc = CreateCompatibleDC(Some(hdc));
        let old_mask_bitmap = SelectObject(mask_dc, mask_bitmap.into());

        // 填充掩码为白色（透明）
        let white_brush = CreateSolidBrush(COLORREF(0x00FFFFFF));
        let mask_rect = windows::Win32::Foundation::RECT {
            left: 0,
            top: 0,
            right: bm.bmWidth,
            bottom: bm.bmHeight,
        };
        FillRect(mask_dc, &mask_rect, white_brush);

        SelectObject(mask_dc, old_mask_bitmap);
        let _ = DeleteDC(mask_dc);
        let _ = DeleteObject(white_brush.into());
        let _ = ReleaseDC(None, hdc);

        // 创建图标信息
        let icon_info = ICONINFO {
            fIcon: TRUE,
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask_bitmap,
            hbmColor: bitmap,
        };

        // 创建图标
        let icon = CreateIconIndirect(&icon_info)?;

        // 清理资源
        let _ = DeleteObject(mask_bitmap.into());

        Ok(icon)
    }
}
