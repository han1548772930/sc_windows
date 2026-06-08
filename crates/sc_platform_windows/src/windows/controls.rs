use windows::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::LibraryLoader::*,
        UI::{Controls::*, WindowsAndMessaging::*},
    },
    core::*,
};

use crate::win_api::to_wide_chars;

fn win32_error(msg: &str) -> Error {
    Error::new(HRESULT(-1), msg)
}

pub struct Font {
    pub handle: HFONT,
}

impl Font {
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

    pub fn selected_tab(&self) -> usize {
        unsafe {
            SendMessageW(self.handle, TCM_GETCURSEL, Some(WPARAM(0)), Some(LPARAM(0))).0 as usize
        }
    }

    pub fn set_selected_tab(&self, index: usize) {
        unsafe {
            SendMessageW(
                self.handle,
                TCM_SETCURSEL,
                Some(WPARAM(index)),
                Some(LPARAM(0)),
            );
            self.update_tab_visibility(index as i32);
        }
    }

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

    pub fn get_display_rect(&self) -> RECT {
        unsafe {
            let mut rect = RECT::default();
            let _ = GetClientRect(self.handle, &mut rect);
            SendMessageW(
                self.handle,
                TCM_ADJUSTRECT,
                Some(WPARAM(0)),
                Some(LPARAM(&mut rect as *mut _ as isize)),
            );
            rect
        }
    }

    pub fn update_tab_visibility(&self, selected_index: i32) {
        unsafe {
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

    pub fn index(&self) -> usize {
        self.index
    }

    pub fn set_visible(&self, visible: bool) {
        unsafe {
            let _ = ShowWindow(self.handle, if visible { SW_SHOW } else { SW_HIDE });
        }
    }

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

            let class_name = to_wide_chars("SC_TAB_PAGE");
            let window_class = WNDCLASSW {
                lpfnWndProc: Some(tab_page_proc),
                hInstance: instance.into(),
                lpszClassName: PCWSTR(class_name.as_ptr()),
                hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as *mut _),
                style: CS_HREDRAW | CS_VREDRAW,
                ..Default::default()
            };
            RegisterClassW(&window_class);

            let mut tab_rect = RECT::default();
            let _ = GetClientRect(tabs_container.handle, &mut tab_rect);
            let tab_width = tab_rect.right - tab_rect.left;
            let tab_height = tab_rect.bottom - tab_rect.top;

            let page_x = 5;
            let page_y = 25;
            let page_width = tab_width - 11;
            let page_height = tab_height - 33;

            let handle = CreateWindowExW(
                WS_EX_CONTROLPARENT,
                PCWSTR(class_name.as_ptr()),
                PCWSTR::null(),
                WS_CHILD
                    | WS_CLIPCHILDREN
                    | if index == 0 {
                        WS_VISIBLE
                    } else {
                        WINDOW_STYLE(0)
                    },
                page_x,
                page_y,
                page_width,
                page_height,
                Some(tabs_container.handle),
                None,
                Some(instance.into()),
                None,
            )?;

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

            SetWindowLongPtrW(handle, GWLP_USERDATA, (index + 1) as isize);

            Ok(Tab { handle, index })
        }
    }
}

unsafe extern "system" fn tab_page_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match msg {
            WM_COMMAND | WM_NOTIFY => {
                if let Ok(tabs_container) = GetParent(hwnd)
                    && let Ok(main_window) = GetParent(tabs_container)
                {
                    return SendMessageW(main_window, msg, Some(wparam), Some(lparam));
                }
                LRESULT(0)
            }
            WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORBTN => {
                if let Ok(tabs_container) = GetParent(hwnd)
                    && let Ok(main_window) = GetParent(tabs_container)
                {
                    return SendMessageW(main_window, msg, Some(wparam), Some(lparam));
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

unsafe extern "system" fn toggle_tab_children(hwnd: HWND, lparam: LPARAM) -> BOOL {
    unsafe {
        let (parent, selected_index) = *(lparam.0 as *const (HWND, i32));

        if let Ok(window_parent) = GetParent(hwnd) {
            if window_parent != parent {
                return TRUE;
            }
        } else {
            return TRUE;
        }

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

pub fn handle_tab_change(tabs: &TabsContainer) {
    let index = tabs.selected_tab();
    tabs.update_tab_visibility(index as i32);
}

pub const TCN_SELCHANGE: u32 = windows::Win32::UI::Controls::TCN_SELCHANGE;
