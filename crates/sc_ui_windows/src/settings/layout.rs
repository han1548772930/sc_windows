use sc_platform::HostPlatform;
use sc_platform_windows::windows::{WindowsHostPlatform, window_id as to_window_id};
use windows::Win32::Foundation::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use super::window::SettingsWindowState;
use super::{
    BUTTON_HEIGHT, BUTTON_SPACING, BUTTON_WIDTH, CONTROL_HEIGHT, LABEL_CONTROL_GAP, LABEL_HEIGHT,
    LABEL_WIDTH, LABEL_Y_OFFSET, MARGIN, ROW_HEIGHT, ROW_SPACING, TAB_CONTENT_MARGIN,
    TAB_PAGE_HEIGHT_ADJUST, TAB_PAGE_WIDTH_ADJUST, TAB_PAGE_X, TAB_PAGE_Y,
};

impl SettingsWindowState {
    pub(super) fn layout_controls(&mut self) {
        unsafe {
            let mut client_rect = RECT::default();
            let _ = GetClientRect(self.hwnd, &mut client_rect);
            let window_width = client_rect.right - client_rect.left;
            let window_height = client_rect.bottom - client_rect.top;

            let tabs_height = window_height - BUTTON_HEIGHT - MARGIN * 3;
            let tabs_width = window_width - MARGIN * 2;
            if !self.tabs_container.is_invalid() {
                let _ = SetWindowPos(
                    self.tabs_container,
                    None,
                    MARGIN,
                    MARGIN,
                    tabs_width,
                    tabs_height,
                    SWP_NOZORDER,
                );

                let page_x = TAB_PAGE_X;
                let page_y = TAB_PAGE_Y;
                let page_width = tabs_width - TAB_PAGE_WIDTH_ADJUST;
                let page_height = tabs_height - TAB_PAGE_HEIGHT_ADJUST;

                if !self.tab_drawing.is_invalid() {
                    let _ = SetWindowPos(
                        self.tab_drawing,
                        None,
                        page_x,
                        page_y,
                        page_width,
                        page_height,
                        SWP_NOZORDER,
                    );
                    self.layout_drawing_tab(page_width);
                }

                if !self.tab_system.is_invalid() {
                    let _ = SetWindowPos(
                        self.tab_system,
                        None,
                        page_x,
                        page_y,
                        page_width,
                        page_height,
                        SWP_NOZORDER,
                    );
                    self.layout_system_tab(page_width);
                }
            }

            let button_spacing = BUTTON_SPACING;
            let buttons_total_width = BUTTON_WIDTH * 2 + button_spacing;
            let buttons_x = (window_width - buttons_total_width) / 2;
            let buttons_y = window_height - BUTTON_HEIGHT - MARGIN;

            let _ = SetWindowPos(
                self.ok_button,
                None,
                buttons_x,
                buttons_y,
                BUTTON_WIDTH,
                BUTTON_HEIGHT,
                SWP_NOZORDER,
            );
            let _ = SetWindowPos(
                self.cancel_button,
                None,
                buttons_x + BUTTON_WIDTH + button_spacing,
                buttons_y,
                BUTTON_WIDTH,
                BUTTON_HEIGHT,
                SWP_NOZORDER,
            );

            let platform = WindowsHostPlatform::new();
            let _ = platform.request_redraw_erase(to_window_id(self.hwnd));
        }
    }

    fn layout_drawing_tab(&self, _tab_width: i32) {
        unsafe {
            let margin = TAB_CONTENT_MARGIN;
            let control_x = margin + LABEL_WIDTH + LABEL_CONTROL_GAP;

            let mut y = margin;

            if !self.line_thickness_label.is_invalid() {
                let _ = SetWindowPos(
                    self.line_thickness_label,
                    None,
                    margin,
                    y + LABEL_Y_OFFSET,
                    LABEL_WIDTH,
                    LABEL_HEIGHT,
                    SWP_NOZORDER,
                );
            }
            let _ = SetWindowPos(
                self.line_thickness_edit,
                None,
                control_x,
                y,
                60,
                CONTROL_HEIGHT,
                SWP_NOZORDER,
            );
            y += ROW_HEIGHT + ROW_SPACING;

            if !self.font_label.is_invalid() {
                let _ = SetWindowPos(
                    self.font_label,
                    None,
                    margin,
                    y + LABEL_Y_OFFSET,
                    LABEL_WIDTH,
                    LABEL_HEIGHT,
                    SWP_NOZORDER,
                );
            }
            let _ = SetWindowPos(
                self.font_choose_button,
                None,
                control_x,
                y,
                110,
                BUTTON_HEIGHT,
                SWP_NOZORDER,
            );
            y += ROW_HEIGHT + ROW_SPACING;

            if !self.drawing_color_label.is_invalid() {
                let _ = SetWindowPos(
                    self.drawing_color_label,
                    None,
                    margin,
                    y + LABEL_Y_OFFSET,
                    LABEL_WIDTH,
                    LABEL_HEIGHT,
                    SWP_NOZORDER,
                );
            }
            let _ = SetWindowPos(
                self.drawing_color_preview,
                None,
                control_x,
                y + 2,
                24,
                20,
                SWP_NOZORDER,
            );
            let _ = SetWindowPos(
                self.drawing_color_button,
                None,
                control_x + 32,
                y,
                100,
                BUTTON_HEIGHT,
                SWP_NOZORDER,
            );
        }
    }

    fn layout_system_tab(&self, tab_width: i32) {
        unsafe {
            let margin = TAB_CONTENT_MARGIN;
            let control_x = margin + LABEL_WIDTH + LABEL_CONTROL_GAP;
            let available_width = tab_width - margin * 2;

            let mut y = margin;

            if !self.hotkey_label.is_invalid() {
                let _ = SetWindowPos(
                    self.hotkey_label,
                    None,
                    margin,
                    y + LABEL_Y_OFFSET,
                    LABEL_WIDTH,
                    LABEL_HEIGHT,
                    SWP_NOZORDER,
                );
            }
            let hotkey_width = available_width - LABEL_WIDTH - 20;
            let _ = SetWindowPos(
                self.hotkey_edit,
                None,
                control_x,
                y,
                hotkey_width,
                CONTROL_HEIGHT,
                SWP_NOZORDER,
            );
            y += ROW_HEIGHT + ROW_SPACING;

            if !self.config_path_label.is_invalid() {
                let _ = SetWindowPos(
                    self.config_path_label,
                    None,
                    margin,
                    y + LABEL_Y_OFFSET,
                    LABEL_WIDTH,
                    LABEL_HEIGHT,
                    SWP_NOZORDER,
                );
            }
            let browse_width = 80;
            let path_width = available_width - LABEL_WIDTH - browse_width - 30;
            let _ = SetWindowPos(
                self.config_path_edit,
                None,
                control_x,
                y,
                path_width,
                CONTROL_HEIGHT,
                SWP_NOZORDER,
            );
            let _ = SetWindowPos(
                self.config_path_browse_button,
                None,
                control_x + path_width + 8,
                y,
                browse_width,
                BUTTON_HEIGHT,
                SWP_NOZORDER,
            );
            y += ROW_HEIGHT + ROW_SPACING;

            if !self.ocr_language_label.is_invalid() {
                let _ = SetWindowPos(
                    self.ocr_language_label,
                    None,
                    margin,
                    y + LABEL_Y_OFFSET,
                    LABEL_WIDTH,
                    LABEL_HEIGHT,
                    SWP_NOZORDER,
                );
            }
            let _ = SetWindowPos(
                self.ocr_language_combo,
                None,
                control_x,
                y,
                160,
                200,
                SWP_NOZORDER,
            );
        }
    }
}
