use sc_app::selection::RectI32;
use sc_platform::HostPlatform;
use sc_platform_windows::windows::WindowsHostPlatform;

use super::window::PreviewWindowState;
use crate::constants::{
    OCR_CONTENT_PADDING_BOTTOM, OCR_CONTENT_PADDING_TOP, OCR_CONTENT_PADDING_X,
    OCR_IMAGE_START_Y_OFFSET, OCR_PANEL_GAP, OCR_TEXT_PADDING_BOTTOM, OCR_TEXT_PADDING_LEFT,
    OCR_TEXT_PADDING_RIGHT, OCR_TEXT_PADDING_TOP, OCR_TEXT_PANEL_WIDTH, TITLE_BAR_HEIGHT,
};

impl PreviewWindowState {
    pub(super) fn recalculate_layout(&mut self) {
        // 右边文字区域宽度
        let text_area_width = if self.show_text_area {
            OCR_TEXT_PANEL_WIDTH
        } else {
            0
        };

        // 左边图像区域宽度
        // 注意：这里的 image_area_width 是为图像预留的区域宽度，不是图像实际显示宽度
        let margin = if self.show_text_area {
            OCR_PANEL_GAP
        } else {
            0
        };
        let image_area_width = self.window_width - text_area_width - margin;

        // 计算图片显示区域（用于绘图限制）
        let image_area_rect = if !self.show_text_area {
            // Pin 模式：图片占满标题栏下方
            RectI32 {
                left: 0,
                top: TITLE_BAR_HEIGHT,
                right: self.image_width,
                bottom: TITLE_BAR_HEIGHT + self.image_height,
            }
        } else {
            // OCR 模式：图片在左侧区域居中
            let available_width = (image_area_width - 2 * OCR_CONTENT_PADDING_X) as f32;
            let available_height = (self.window_height
                - TITLE_BAR_HEIGHT
                - OCR_CONTENT_PADDING_TOP
                - OCR_CONTENT_PADDING_BOTTOM) as f32;
            let start_y = TITLE_BAR_HEIGHT + OCR_IMAGE_START_Y_OFFSET;

            let scale_x = available_width / self.image_width as f32;
            let scale_y = available_height / self.image_height as f32;
            let scale = scale_x.min(scale_y).min(1.0);

            let display_w = (self.image_width as f32 * scale) as i32;
            let display_h = (self.image_height as f32 * scale) as i32;

            let left_area_center_x = OCR_CONTENT_PADDING_X + (available_width as i32) / 2;
            let x = left_area_center_x - display_w / 2;
            let y = start_y + ((available_height as i32) - display_h) / 2;

            RectI32 {
                left: x,
                top: y,
                right: x + display_w,
                bottom: y + display_h,
            }
        };

        // 更新绘图状态的图片区域
        if let Some(ref mut ds) = self.drawing_state {
            ds.set_image_area(image_area_rect);
        }

        if self.show_text_area {
            // 计算文字显示区域
            let text_padding_left = OCR_TEXT_PADDING_LEFT;
            let text_padding_right = OCR_TEXT_PADDING_RIGHT;
            let text_padding_top = TITLE_BAR_HEIGHT + OCR_TEXT_PADDING_TOP;
            let text_padding_bottom = OCR_TEXT_PADDING_BOTTOM;

            let new_text_area_rect = RectI32 {
                left: image_area_width + text_padding_left,
                top: text_padding_top,
                right: self.window_width - text_padding_right,
                bottom: self.window_height - text_padding_bottom,
            };

            // 只有在文本区域真正改变时才重新计算文本布局
            if new_text_area_rect.left != self.text_area_rect.left
                || new_text_area_rect.top != self.text_area_rect.top
                || new_text_area_rect.right != self.text_area_rect.right
                || new_text_area_rect.bottom != self.text_area_rect.bottom
            {
                self.text_area_rect = new_text_area_rect;

                // 重新计算文本换行
                if let Some(renderer) = &mut self.renderer {
                    let width = (self.text_area_rect.right - self.text_area_rect.left) as f32;
                    self.text_lines = renderer.split_text_into_lines(&self.text_content, width);
                } else {
                    // Fallback if renderer not available
                    self.text_lines = vec![self.text_content.clone()];
                }

                // 调整滚动偏移量，确保不超出范围
                let max_scroll = (self.text_lines.len() as i32 * self.line_height)
                    - (self.text_area_rect.bottom - self.text_area_rect.top);
                if self.scroll_offset > max_scroll.max(0) {
                    self.scroll_offset = max_scroll.max(0);
                }

                // 标记需要重绘
                let window_id = self.window_id();
                let platform = WindowsHostPlatform::new();
                let _ = platform.request_redraw_rect(
                    window_id,
                    self.text_area_rect.left,
                    self.text_area_rect.top,
                    self.text_area_rect.right,
                    self.text_area_rect.bottom,
                );
            }
        }
    }
}
