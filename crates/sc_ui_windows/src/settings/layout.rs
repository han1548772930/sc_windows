use sc_platform::HostPlatform;
use sc_platform_windows::windows::{WindowsHostPlatform, window_id as to_window_id};
use windows::Win32::Foundation::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use super::window::SettingsWindowState;
use super::{
    BUTTON_HEIGHT, BUTTON_SPACING, BUTTON_WIDTH, COLOR_BUTTON_WIDTH, COLOR_PREVIEW_HEIGHT,
    COLOR_PREVIEW_WIDTH, CONTROL_HEIGHT, FONT_BUTTON_WIDTH, LABEL_CONTROL_GAP, LABEL_HEIGHT,
    LABEL_WIDTH, LABEL_Y_OFFSET, MARGIN, OCR_LANGUAGE_DROPDOWN_HEIGHT, OCR_LANGUAGE_WIDTH,
    PATH_BROWSE_BUTTON_WIDTH, PATH_BUTTON_GAP, ROW_HEIGHT, ROW_SPACING, SHORT_EDIT_WIDTH,
    TAB_CONTENT_MARGIN, TAB_PAGE_HEIGHT_ADJUST, TAB_PAGE_WIDTH_ADJUST, TAB_PAGE_X, TAB_PAGE_Y,
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

    fn layout_drawing_tab(&self, tab_width: i32) {
        unsafe {
            let margin = TAB_CONTENT_MARGIN;
            let metrics = RowMetrics::new(tab_width, margin);

            let mut y = margin;

            Self::position_label(self.line_thickness_label, &metrics, y);
            Self::position_control(
                self.line_thickness_edit,
                metrics.control_x,
                y,
                SHORT_EDIT_WIDTH,
            );
            y += ROW_HEIGHT + ROW_SPACING;

            Self::position_label(self.font_label, &metrics, y);
            Self::position_control(
                self.font_choose_button,
                metrics.control_x,
                y,
                FONT_BUTTON_WIDTH.min(metrics.control_width),
            );
            y += ROW_HEIGHT + ROW_SPACING;

            Self::position_label(self.drawing_color_label, &metrics, y);
            let _ = SetWindowPos(
                self.drawing_color_preview,
                None,
                metrics.control_x,
                y + (CONTROL_HEIGHT - COLOR_PREVIEW_HEIGHT) / 2,
                COLOR_PREVIEW_WIDTH,
                COLOR_PREVIEW_HEIGHT,
                SWP_NOZORDER,
            );
            Self::position_control(
                self.drawing_color_button,
                metrics.control_x + COLOR_PREVIEW_WIDTH + PATH_BUTTON_GAP,
                y,
                COLOR_BUTTON_WIDTH.min(
                    metrics
                        .control_width
                        .saturating_sub(COLOR_PREVIEW_WIDTH + PATH_BUTTON_GAP),
                ),
            );
        }
    }

    fn layout_system_tab(&self, tab_width: i32) {
        unsafe {
            let metrics = RowMetrics::new(tab_width, TAB_CONTENT_MARGIN);

            let mut y = TAB_CONTENT_MARGIN;

            Self::position_label(self.hotkey_label, &metrics, y);
            Self::position_control(
                self.hotkey_edit,
                metrics.control_x,
                y,
                metrics.control_width,
            );
            y += ROW_HEIGHT + ROW_SPACING;

            Self::position_label(self.config_path_label, &metrics, y);
            let path_width = metrics
                .control_width
                .saturating_sub(PATH_BROWSE_BUTTON_WIDTH + PATH_BUTTON_GAP);
            Self::position_control(self.config_path_edit, metrics.control_x, y, path_width);
            Self::position_control(
                self.config_path_browse_button,
                metrics.control_x + path_width + PATH_BUTTON_GAP,
                y,
                PATH_BROWSE_BUTTON_WIDTH,
            );
            y += ROW_HEIGHT + ROW_SPACING;

            Self::position_label(self.ocr_language_label, &metrics, y);
            let _ = SetWindowPos(
                self.ocr_language_combo,
                None,
                metrics.control_x,
                y,
                OCR_LANGUAGE_WIDTH.min(metrics.control_width),
                OCR_LANGUAGE_DROPDOWN_HEIGHT,
                SWP_NOZORDER,
            );
        }
    }

    fn position_label(label: HWND, metrics: &RowMetrics, y: i32) {
        unsafe {
            if !label.is_invalid() {
                let _ = SetWindowPos(
                    label,
                    None,
                    metrics.label_x,
                    y + LABEL_Y_OFFSET,
                    LABEL_WIDTH,
                    LABEL_HEIGHT,
                    SWP_NOZORDER,
                );
            }
        }
    }

    fn position_control(control: HWND, x: i32, y: i32, width: i32) {
        unsafe {
            let _ = SetWindowPos(
                control,
                None,
                x,
                y,
                width.max(CONTROL_HEIGHT),
                CONTROL_HEIGHT,
                SWP_NOZORDER,
            );
        }
    }
}

struct RowMetrics {
    label_x: i32,
    control_x: i32,
    control_width: i32,
}

impl RowMetrics {
    fn new(tab_width: i32, margin: i32) -> Self {
        let available_width = (tab_width - margin * 2).max(0);
        let control_x = margin + LABEL_WIDTH + LABEL_CONTROL_GAP;
        let control_width = available_width
            .saturating_sub(LABEL_WIDTH + LABEL_CONTROL_GAP)
            .max(CONTROL_HEIGHT);

        Self {
            label_x: margin,
            control_x,
            control_width,
        }
    }
}
