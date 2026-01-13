use std::fmt;

use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, HICON, IDI_APPLICATION, IMAGE_ICON,
    LR_LOADFROMFILE, LoadIconW, LoadImageW, MF_SEPARATOR, MF_STRING, SetForegroundWindow,
    TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu,
};
use windows::core::PCWSTR;

use crate::win_api::to_wide_chars;

#[derive(Debug)]
pub enum TrayIconError {
    Io(std::io::Error),
    Windows(windows::core::Error),
}

impl fmt::Display for TrayIconError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TrayIconError::Io(e) => write!(f, "io error: {e}"),
            TrayIconError::Windows(e) => write!(f, "windows error: {e:?}"),
        }
    }
}

impl std::error::Error for TrayIconError {}

impl From<std::io::Error> for TrayIconError {
    fn from(value: std::io::Error) -> Self {
        TrayIconError::Io(value)
    }
}

impl From<windows::core::Error> for TrayIconError {
    fn from(value: windows::core::Error) -> Self {
        TrayIconError::Windows(value)
    }
}

pub type Result<T> = std::result::Result<T, TrayIconError>;

/// 创建默认图标（从嵌入数据加载；失败则回退到系统默认图标）
pub fn create_default_icon() -> Result<HICON> {
    // SAFETY: `include_bytes!` embeds the icon at compile time.
    const ICON_DATA: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../apps/sc_windows/icons/i.ico"
    ));

    match load_embedded_icon(ICON_DATA) {
        Ok(icon) => Ok(icon),
        Err(_e) => {
            // 如果嵌入图标加载失败，使用系统默认图标
            // SAFETY: LoadIconW is a Win32 API.
            unsafe { LoadIconW(None, IDI_APPLICATION) }.map_err(TrayIconError::from)
        }
    }
}

/// 从嵌入的字节数据直接加载图标（ICO 格式需要文件路径）
fn load_embedded_icon(icon_data: &[u8]) -> Result<HICON> {
    // 创建临时文件来存储图标数据
    let temp_path = std::env::temp_dir().join("temp_tray_icon.ico");
    std::fs::write(&temp_path, icon_data)?;

    // 直接加载 ICO 文件
    let path_wide = to_wide_chars(&temp_path.to_string_lossy());
    let result = unsafe {
        LoadImageW(
            None,
            PCWSTR(path_wide.as_ptr()),
            IMAGE_ICON,
            16, // 系统托盘图标标准大小
            16,
            LR_LOADFROMFILE,
        )
    }
    .map(|h| HICON(h.0))
    .map_err(TrayIconError::from);

    // 清理临时文件
    let _ = std::fs::remove_file(&temp_path);

    result
}

/// 添加托盘图标。
///
/// `callback_message` 应为 `WM_USER + N` 形式的自定义消息。
pub fn add_tray_icon(
    hwnd: HWND,
    icon_id: u32,
    callback_message: u32,
    tooltip: &str,
    icon: HICON,
) -> bool {
    unsafe {
        let tooltip_wide = to_wide_chars(tooltip);
        let mut tooltip_array = [0u16; 128];
        let copy_len = (tooltip_wide.len().saturating_sub(1)).min(tooltip_array.len() - 1);
        tooltip_array[..copy_len].copy_from_slice(&tooltip_wide[..copy_len]);

        let nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: icon_id,
            uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
            uCallbackMessage: callback_message,
            hIcon: icon,
            szTip: tooltip_array,
            ..Default::default()
        };

        Shell_NotifyIconW(NIM_ADD, &nid).as_bool()
    }
}

/// 删除托盘图标。
pub fn delete_tray_icon(hwnd: HWND, icon_id: u32) -> bool {
    unsafe {
        let nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: icon_id,
            uFlags: NIF_ICON,
            ..Default::default()
        };

        Shell_NotifyIconW(NIM_DELETE, &nid).as_bool()
    }
}

/// 显示默认的托盘右键菜单。
///
/// 返回选中的菜单项 ID；返回 0 表示用户取消/未选择。
pub fn show_default_context_menu(hwnd: HWND) -> u32 {
    unsafe {
        let hmenu = CreatePopupMenu().unwrap_or_default();
        if hmenu.is_invalid() {
            return 0;
        }

        // 添加菜单项（保持与 legacy host 逻辑一致）
        let _ = AppendMenuW(
            hmenu,
            MF_STRING,
            1001,
            windows::core::w!("截图(&S)\tCtrl+Alt+S"),
        );
        let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(hmenu, MF_STRING, 1002, windows::core::w!("设置(&T)"));
        let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(hmenu, MF_STRING, 1003, windows::core::w!("退出(&X)"));

        // 获取鼠标位置
        let mut cursor_pos = POINT::default();
        let _ = GetCursorPos(&mut cursor_pos);

        // 设置前台窗口以确保菜单正确显示
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

        let _ = DestroyMenu(hmenu);

        cmd.0 as u32
    }
}
