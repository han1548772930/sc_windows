use windows::Win32::{
    Foundation::*,
    Graphics::{Direct2D::Common::*, Gdi::InvalidateRect},
    UI::WindowsAndMessaging::*,
};

use crate::*;

impl Toolbar {
    pub fn update_position(
        &mut self,
        selection_rect: &RECT,
        screen_width: i32,
        screen_height: i32,
    ) {
        let toolbar_width = BUTTON_WIDTH * BUTTON_COUNT as f32
            + BUTTON_SPACING * (BUTTON_COUNT - 1) as f32
            + TOOLBAR_PADDING * 2.0;

        let mut toolbar_x = selection_rect.left as f32
            + (selection_rect.right - selection_rect.left) as f32 / 2.0
            - toolbar_width / 2.0;
        let mut toolbar_y = selection_rect.bottom as f32 + TOOLBAR_MARGIN;

        if toolbar_y + TOOLBAR_HEIGHT > screen_height as f32 {
            toolbar_y = selection_rect.top as f32 - TOOLBAR_HEIGHT - TOOLBAR_MARGIN;
        }

        toolbar_x = toolbar_x.max(0.0).min(screen_width as f32 - toolbar_width);
        toolbar_y = toolbar_y
            .max(0.0)
            .min(screen_height as f32 - TOOLBAR_HEIGHT);

        self.rect = D2D_RECT_F {
            left: toolbar_x,
            top: toolbar_y,
            right: toolbar_x + toolbar_width,
            bottom: toolbar_y + TOOLBAR_HEIGHT,
        };

        self.buttons.clear();

        // 修改：让按钮垂直居中，形成正方形
        let button_y = toolbar_y + (TOOLBAR_HEIGHT - BUTTON_HEIGHT) / 2.0; // 垂直居中
        let mut button_x = toolbar_x + TOOLBAR_PADDING;

        let buttons_data = [
            (ToolbarButton::Rectangle, IconData::from_text(RECT_ICON)),
            (ToolbarButton::Circle, IconData::from_text(CIRCLE_ICON)),
            (ToolbarButton::Arrow, IconData::from_text(ARROW_ICON)),
            (ToolbarButton::Pen, IconData::from_text(PEN_ICON)),
            (ToolbarButton::Text, IconData::from_text(TEXT_ICON)),
            (ToolbarButton::Undo, IconData::from_text(UNDO_ICON)),
            (ToolbarButton::Save, IconData::from_text(SAVE_ICON)),
            (ToolbarButton::Pin, IconData::from_text(PIN_ICON)),
            (ToolbarButton::Copy, IconData::from_text(COPY_ICON)),
            (ToolbarButton::Confirm, IconData::from_text(CONFIRM_ICON)),
            (ToolbarButton::Cancel, IconData::from_text(CANCEL_ICON)),
        ];

        for (button_type, icon_data) in buttons_data.iter() {
            let button_rect = D2D_RECT_F {
                left: button_x,
                top: button_y,
                right: button_x + BUTTON_WIDTH,
                bottom: button_y + BUTTON_HEIGHT, // 使用BUTTON_HEIGHT而不是toolbar高度
            };
            self.buttons
                .push((button_rect, *button_type, icon_data.clone()));
            button_x += BUTTON_WIDTH + BUTTON_SPACING;
        }

        self.visible = true;
    }
    pub fn get_button_at_position(&self, x: i32, y: i32) -> ToolbarButton {
        for (rect, button_type, _) in &self.buttons {
            if x as f32 >= rect.left
                && x as f32 <= rect.right
                && y as f32 >= rect.top
                && y as f32 <= rect.bottom
            {
                return *button_type;
            }
        }
        ToolbarButton::None
    }

    pub fn set_hovered_button(&mut self, button: ToolbarButton) {
        self.hovered_button = button;
    }

    pub fn set_clicked_button(&mut self, button: ToolbarButton) {
        self.clicked_button = button;
    }

    pub fn clear_clicked_button(&mut self) {
        self.clicked_button = ToolbarButton::None;
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.hovered_button = ToolbarButton::None;
    }
}

impl WindowState {
    pub fn handle_toolbar_click(&mut self, button: ToolbarButton, hwnd: HWND) {
        match button {
            ToolbarButton::Rectangle => {
                self.current_tool = DrawingTool::Rectangle;
                self.selected_element = None;
                for element in &mut self.drawing_elements {
                    element.selected = false;
                }
            }
            ToolbarButton::Circle => {
                self.current_tool = DrawingTool::Circle;
                self.selected_element = None;
                for element in &mut self.drawing_elements {
                    element.selected = false;
                }
            }
            ToolbarButton::Arrow => {
                self.current_tool = DrawingTool::Arrow;
                self.selected_element = None;
                for element in &mut self.drawing_elements {
                    element.selected = false;
                }
            }
            ToolbarButton::Pen => {
                self.current_tool = DrawingTool::Pen;
                self.selected_element = None;
                for element in &mut self.drawing_elements {
                    element.selected = false;
                }
            }
            ToolbarButton::Text => {
                self.current_tool = DrawingTool::Text;
                self.selected_element = None;
                for element in &mut self.drawing_elements {
                    element.selected = false;
                }
            }
            ToolbarButton::Undo => {
                // 检查是否可以撤销
                if self.can_undo() {
                    self.undo();
                    unsafe {
                        InvalidateRect(hwnd, None, FALSE);
                    }
                }
            }
            ToolbarButton::Save => {
                let _ = self.save_selection();
            }
            ToolbarButton::Copy => {
                let _ = self.save_selection();
            }
            ToolbarButton::Pin => {
                self.pin_selection(hwnd);
            }
            ToolbarButton::Confirm => {
                let _ = self.save_selection();
                unsafe {
                    PostQuitMessage(0);
                }
            }
            ToolbarButton::Cancel => {
                self.current_tool = DrawingTool::None;
                self.selected_element = None;
                self.current_element = None;
                for element in &mut self.drawing_elements {
                    element.selected = false;
                }
                unsafe {
                    PostQuitMessage(0);
                }
            }
            _ => {}
        }

        unsafe {
            InvalidateRect(hwnd, None, FALSE);
        }
    }
}
