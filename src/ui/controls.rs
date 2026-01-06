//! UI 控件封装模块
//!
//! 参考 native-windows-gui 的设计风格，封装常用的 Win32 控件。
//! 使用 Builder 模式创建控件，提供统一的接口。

use windows::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::LibraryLoader::*,
        UI::{Controls::*, WindowsAndMessaging::*},
    },
    core::*,
};

use crate::utils::to_wide_chars;

/// 创建 Win32 错误
fn win32_error(msg: &str) -> Error {
    Error::new(HRESULT(-1), msg)
}

// ═══════════════════════════════════════════════════════════════════════════════
// Font - 字体封装
// ═══════════════════════════════════════════════════════════════════════════════

/// 字体封装
pub struct Font {
    pub handle: HFONT,
}

impl Font {
    /// 创建 Segoe UI 字体（Windows 10+ 标准 UI 字体）
    pub fn segoe_ui(size: i32) -> Result<Self> {
        unsafe {
            let font = CreateFontW(
                -size,
                0,
                0,
                0,
                FW_NORMAL.0 as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET,
                OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                CLEARTYPE_QUALITY,
                (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
                PCWSTR(to_wide_chars("Segoe UI").as_ptr()),
            );
            if font.is_invalid() {
                Err(win32_error("Failed to create font"))
            } else {
                Ok(Self { handle: font })
            }
        }
    }

    /// 创建粗体字体
    pub fn segoe_ui_bold(size: i32) -> Result<Self> {
        unsafe {
            let font = CreateFontW(
                -size,
                0,
                0,
                0,
                FW_SEMIBOLD.0 as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET,
                OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                CLEARTYPE_QUALITY,
                (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
                PCWSTR(to_wide_chars("Segoe UI").as_ptr()),
            );
            if font.is_invalid() {
                Err(win32_error("Failed to create bold font"))
            } else {
                Ok(Self { handle: font })
            }
        }
    }
}

impl Drop for Font {
    fn drop(&mut self) {
        if !self.handle.is_invalid() {
            unsafe {
                let _ = DeleteObject(HGDIOBJ(self.handle.0));
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// TabsContainer - 选项卡容器
// ═══════════════════════════════════════════════════════════════════════════════

/// 选项卡容器控件
pub struct TabsContainer {
    pub handle: HWND,
}

impl TabsContainer {
    pub fn builder() -> TabsContainerBuilder {
        TabsContainerBuilder {
            size: (300, 300),
            position: (0, 0),
            parent: HWND::default(),
        }
    }

    /// 获取当前选中的 Tab 索引
    pub fn selected_tab(&self) -> usize {
        unsafe {
            SendMessageW(self.handle, TCM_GETCURSEL, Some(WPARAM(0)), Some(LPARAM(0))).0 as usize
        }
    }

    /// 设置当前选中的 Tab
    pub fn set_selected_tab(&self, index: usize) {
        unsafe {
            SendMessageW(
                self.handle,
                TCM_SETCURSEL,
                Some(WPARAM(index)),
                Some(LPARAM(0)),
            );
            // 通知切换 Tab
            self.update_tab_visibility(index as i32);
        }
    }

    /// 获取 Tab 数量
    pub fn tab_count(&self) -> usize {
        unsafe {
            SendMessageW(
                self.handle,
                TCM_GETITEMCOUNT,
                Some(WPARAM(0)),
                Some(LPARAM(0)),
            )
            .0 as usize
        }
    }

    /// 设置字体
    pub fn set_font(&self, font: &Font) {
        unsafe {
            SendMessageW(
                self.handle,
                WM_SETFONT,
                Some(WPARAM(font.handle.0 as usize)),
                Some(LPARAM(1)),
            );
        }
    }

    /// 获取 Tab 内容区域（排除 Tab 标题栏的区域）
    pub fn get_display_rect(&self) -> RECT {
        unsafe {
            let mut rect = RECT::default();
            let _ = GetClientRect(self.handle, &mut rect);
            // 调整为内容区域
            SendMessageW(
                self.handle,
                TCM_ADJUSTRECT,
                Some(WPARAM(0)),
                Some(LPARAM(&mut rect as *mut _ as isize)),
            );
            rect
        }
    }

    /// 更新 Tab 页面可见性
    pub fn update_tab_visibility(&self, selected_index: i32) {
        unsafe {
            // 枚举所有子窗口并更新可见性
            let data = (self.handle, selected_index);
            let _ = EnumChildWindows(
                Some(self.handle),
                Some(toggle_tab_children),
                LPARAM(&data as *const _ as isize),
            );
        }
    }
}

pub struct TabsContainerBuilder {
    size: (i32, i32),
    position: (i32, i32),
    parent: HWND,
}

impl TabsContainerBuilder {
    pub fn size(mut self, width: i32, height: i32) -> Self {
        self.size = (width, height);
        self
    }

    pub fn position(mut self, x: i32, y: i32) -> Self {
        self.position = (x, y);
        self
    }

    pub fn parent(mut self, parent: HWND) -> Self {
        self.parent = parent;
        self
    }

    pub fn build(self) -> Result<TabsContainer> {
        unsafe {
            let instance = GetModuleHandleW(None)?;

            let handle = CreateWindowExW(
                WS_EX_CONTROLPARENT,
                PCWSTR(to_wide_chars("SysTabControl32").as_ptr()),
                PCWSTR::null(),
                WS_VISIBLE | WS_CHILD | WS_CLIPCHILDREN | WS_CLIPSIBLINGS,
                self.position.0,
                self.position.1,
                self.size.0,
                self.size.1,
                Some(self.parent),
                None,
                Some(instance.into()),
                None,
            )?;

            Ok(TabsContainer { handle })
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tab - 选项卡页面
// ═══════════════════════════════════════════════════════════════════════════════

/// 选项卡页面控件
pub struct Tab {
    pub handle: HWND,
    index: usize,
}

impl Tab {
    pub fn builder() -> TabBuilder {
        TabBuilder {
            text: String::new(),
            parent: HWND::default(),
        }
    }

    /// 获取 Tab 索引
    pub fn index(&self) -> usize {
        self.index
    }

    /// 显示或隐藏 Tab 内容
    pub fn set_visible(&self, visible: bool) {
        unsafe {
            let _ = ShowWindow(self.handle, if visible { SW_SHOW } else { SW_HIDE });
        }
    }

    /// 设置 Tab 标题
    pub fn set_text(&self, text: &str, tabs_container: HWND) {
        unsafe {
            let text_wide = to_wide_chars(text);
            let item = TCITEMW {
                mask: TCIF_TEXT,
                dwState: TAB_CONTROL_ITEM_STATE(0),
                dwStateMask: TAB_CONTROL_ITEM_STATE(0),
                pszText: PWSTR(text_wide.as_ptr() as *mut _),
                cchTextMax: 0,
                iImage: -1,
                lParam: LPARAM(0),
            };
            SendMessageW(
                tabs_container,
                TCM_SETITEMW,
                Some(WPARAM(self.index)),
                Some(LPARAM(&item as *const _ as isize)),
            );
        }
    }
}

pub struct TabBuilder {
    text: String,
    parent: HWND,
}

impl TabBuilder {
    pub fn text(mut self, text: &str) -> Self {
        self.text = text.to_string();
        self
    }

    pub fn parent(mut self, parent: HWND) -> Self {
        self.parent = parent;
        self
    }

    pub fn build(self, tabs_container: &TabsContainer) -> Result<Tab> {
        unsafe {
            let instance = GetModuleHandleW(None)?;
            let index = tabs_container.tab_count();

            // 注册 Tab 页面窗口类（如果需要）
            let class_name = to_wide_chars("SC_TAB_PAGE");
            let window_class = WNDCLASSW {
                lpfnWndProc: Some(tab_page_proc),
                hInstance: instance.into(),
                lpszClassName: PCWSTR(class_name.as_ptr()),
                hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as *mut _), // 白色背景
                style: CS_HREDRAW | CS_VREDRAW,
                ..Default::default()
            };
            RegisterClassW(&window_class); // 忽略重复注册错误

            // 获取 Tab 控件大小
            let mut tab_rect = RECT::default();
            let _ = GetClientRect(tabs_container.handle, &mut tab_rect);
            let tab_width = tab_rect.right - tab_rect.left;
            let tab_height = tab_rect.bottom - tab_rect.top;

            // 计算 Tab 页面位置和大小（参考 NWG: x=5, y=25, w=width-11, h=height-33）
            let page_x = 5;
            let page_y = 25;
            let page_width = tab_width - 11;
            let page_height = tab_height - 33;

            // 创建 Tab 页面窗口
            let handle = CreateWindowExW(
                WS_EX_CONTROLPARENT,
                PCWSTR(class_name.as_ptr()),
                PCWSTR::null(),
                WS_CHILD | WS_CLIPCHILDREN | if index == 0 { WS_VISIBLE } else { WINDOW_STYLE(0) },
                page_x,
                page_y,
                page_width,
                page_height,
                Some(tabs_container.handle),
                None,
                Some(instance.into()),
                None,
            )?;

            // 添加 Tab 项到容器
            let text_wide = to_wide_chars(&self.text);
            let item = TCITEMW {
                mask: TCIF_TEXT,
                dwState: TAB_CONTROL_ITEM_STATE(0),
                dwStateMask: TAB_CONTROL_ITEM_STATE(0),
                pszText: PWSTR(text_wide.as_ptr() as *mut _),
                cchTextMax: 0,
                iImage: -1,
                lParam: LPARAM(0),
            };
            SendMessageW(
                tabs_container.handle,
                TCM_INSERTITEMW,
                Some(WPARAM(index)),
                Some(LPARAM(&item as *const _ as isize)),
            );

            // 存储索引到窗口数据 (index + 1)
            SetWindowLongPtrW(handle, GWLP_USERDATA, (index + 1) as isize);

            Ok(Tab { handle, index })
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// StatusBar - 状态栏
// ═══════════════════════════════════════════════════════════════════════════════

/// 状态栏控件
pub struct StatusBar {
    pub handle: HWND,
}

impl StatusBar {
    pub fn builder() -> StatusBarBuilder {
        StatusBarBuilder {
            text: String::new(),
            parent: HWND::default(),
        }
    }

    /// 设置状态栏文本
    pub fn set_text(&self, index: u8, text: &str) {
        unsafe {
            let text_wide = to_wide_chars(text);
            SendMessageW(
                self.handle,
                SB_SETTEXTW,
                Some(WPARAM(index as usize)),
                Some(LPARAM(text_wide.as_ptr() as isize)),
            );
        }
    }

    /// 获取状态栏文本
    pub fn text(&self, index: u8) -> String {
        unsafe {
            let len = SendMessageW(
                self.handle,
                SB_GETTEXTLENGTHW,
                Some(WPARAM(index as usize)),
                Some(LPARAM(0)),
            );
            let text_len = (len.0 as u32 & 0xFFFF) as usize + 1;
            let mut buffer: Vec<u16> = vec![0; text_len];
            SendMessageW(
                self.handle,
                SB_GETTEXTW,
                Some(WPARAM(index as usize)),
                Some(LPARAM(buffer.as_mut_ptr() as isize)),
            );
            String::from_utf16_lossy(&buffer)
                .trim_end_matches('\0')
                .to_string()
        }
    }

    /// 设置字体
    pub fn set_font(&self, font: &Font) {
        unsafe {
            SendMessageW(
                self.handle,
                WM_SETFONT,
                Some(WPARAM(font.handle.0 as usize)),
                Some(LPARAM(1)),
            );
        }
    }

    /// 设置最小高度
    pub fn set_min_height(&self, height: u32) {
        unsafe {
            SendMessageW(
                self.handle,
                SB_SETMINHEIGHT,
                Some(WPARAM(height as usize)),
                Some(LPARAM(0)),
            );
            SendMessageW(self.handle, WM_SIZE, Some(WPARAM(0)), Some(LPARAM(0)));
        }
    }
}

pub struct StatusBarBuilder {
    text: String,
    parent: HWND,
}

impl StatusBarBuilder {
    pub fn text(mut self, text: &str) -> Self {
        self.text = text.to_string();
        self
    }

    pub fn parent(mut self, parent: HWND) -> Self {
        self.parent = parent;
        self
    }

    pub fn build(self) -> Result<StatusBar> {
        unsafe {
            let instance = GetModuleHandleW(None)?;

            // CCS_BOTTOM = 0x0003 使状态栏自动定位到底部
            let handle = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(to_wide_chars("msctls_statusbar32").as_ptr()),
                PCWSTR::null(),
                WINDOW_STYLE(WS_VISIBLE.0 | WS_CHILD.0 | 0x0003),
                0,
                0,
                0,
                0,
                Some(self.parent),
                None,
                Some(instance.into()),
                None,
            )?;

            let status_bar = StatusBar { handle };
            if !self.text.is_empty() {
                status_bar.set_text(0, &self.text);
            }

            Ok(status_bar)
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 辅助函数
// ═══════════════════════════════════════════════════════════════════════════════

/// Tab 页面窗口过程 - 转发命令消息到主窗口
unsafe extern "system" fn tab_page_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match msg {
            // 转发 WM_COMMAND 到主窗口（跨越 TabsContainer）
            WM_COMMAND | WM_NOTIFY => {
                // 查找主窗口（Tab 页面 -> TabsContainer -> 主窗口）
                if let Ok(tabs_container) = GetParent(hwnd) {
                    if let Ok(main_window) = GetParent(tabs_container) {
                        return SendMessageW(main_window, msg, Some(wparam), Some(lparam));
                    }
                }
                LRESULT(0)
            }
            // 处理背景色
            WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORBTN => {
                // 转发给主窗口处理
                if let Ok(tabs_container) = GetParent(hwnd) {
                    if let Ok(main_window) = GetParent(tabs_container) {
                        return SendMessageW(main_window, msg, Some(wparam), Some(lparam));
                    }
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

/// 切换 Tab 子窗口的可见性
unsafe extern "system" fn toggle_tab_children(hwnd: HWND, lparam: LPARAM) -> BOOL {
    unsafe {
        let (parent, selected_index) = *(lparam.0 as *const (HWND, i32));

        // 检查是否是直接子窗口
        if let Ok(window_parent) = GetParent(hwnd) {
            if window_parent != parent {
                return TRUE;
            }
        } else {
            return TRUE;
        }

        // 获取窗口类名
        let mut class_name = [0u16; 64];
        let len = GetClassNameW(hwnd, &mut class_name);
        if len > 0 {
            let name = String::from_utf16_lossy(&class_name[..len as usize]);
            if name == "SC_TAB_PAGE" {
                let tab_index = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as i32;
                let visible = tab_index == selected_index + 1;
                let _ = ShowWindow(hwnd, if visible { SW_SHOW } else { SW_HIDE });
            }
        }

        TRUE
    }
}

/// 处理 Tab 切换通知
pub fn handle_tab_change(tabs: &TabsContainer) {
    let index = tabs.selected_tab();
    tabs.update_tab_visibility(index as i32);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 控件 ID 常量
// ═══════════════════════════════════════════════════════════════════════════════

/// Tab 切换通知码
pub const TCN_SELCHANGE: u32 = 0xFFFFFDDA_u32; // -550
