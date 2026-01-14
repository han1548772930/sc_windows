use windows::Win32::Foundation::RECT as WinRect;
use windows::Win32::Graphics::Direct2D::ID2D1RenderTarget;

use sc_drawing::Rect as DrawingRect;
use sc_drawing::windows::TextCursorState;
use sc_platform_windows::windows::Direct2DRenderer;

use super::{DrawingError, DrawingManager};

impl DrawingManager {
    /// 渲染绘图元素到指定的渲染目标（用于离屏合成/导出）
    pub fn render_elements_to_target(
        &self,
        render_target: &ID2D1RenderTarget,
        d2d_renderer: &mut Direct2DRenderer,
        selection_rect: &DrawingRect,
    ) -> Result<(), DrawingError> {
        let factory = d2d_renderer
            .d2d_factory
            .as_ref()
            .ok_or_else(|| DrawingError::RenderError("No D2D factory available".to_string()))?;
        let dwrite_factory = d2d_renderer.dwrite_factory.as_ref();

        // 计算偏移量：元素坐标是屏幕坐标，需要转换为离屏目标坐标
        let offset_x = -(selection_rect.left as f32);
        let offset_y = -(selection_rect.top as f32);

        self.win_renderer
            .render_elements_to_target_with_offset(
                factory,
                render_target,
                dwrite_factory,
                offset_x,
                offset_y,
                self.elements.get_elements(),
                self.current_element.as_ref(),
            )
            .map_err(|e| DrawingError::RenderError(e.to_string()))
    }

    /// 渲染绘图元素（支持裁剪区域 + 静态层缓存）
    pub fn render(
        &mut self,
        d2d_renderer: &mut Direct2DRenderer,
        selection_rect: Option<&DrawingRect>,
    ) -> Result<(), DrawingError> {
        let factory = d2d_renderer
            .d2d_factory
            .as_ref()
            .ok_or_else(|| DrawingError::RenderError("No D2D factory available".to_string()))?;
        let hwnd_rt = d2d_renderer
            .render_target
            .as_ref()
            .ok_or_else(|| DrawingError::RenderError("No render target available".to_string()))?;
        let render_target: &ID2D1RenderTarget = hwnd_rt;
        let dwrite_factory = d2d_renderer.dwrite_factory.as_ref();

        let screen_size = (
            d2d_renderer.screen_width.max(0) as u32,
            d2d_renderer.screen_height.max(0) as u32,
        );

        let needs_rebuild = self.static_layer_dirty;

        let win_selection_rect = selection_rect.map(|r| WinRect {
            left: r.left,
            top: r.top,
            right: r.right,
            bottom: r.bottom,
        });

        let rebuilt = self
            .win_renderer
            .render(
                factory,
                render_target,
                dwrite_factory,
                screen_size,
                self.elements.get_elements(),
                self.current_element.as_ref(),
                self.selected_element,
                self.cursor_state_for_renderer(),
                win_selection_rect.as_ref(),
                needs_rebuild,
            )
            .map_err(|e| DrawingError::RenderError(e.to_string()))?;

        if rebuilt {
            self.static_layer_dirty = false;
        }

        Ok(())
    }

    fn cursor_state_for_renderer(&self) -> Option<TextCursorState> {
        if !self.text_editing {
            return None;
        }

        let edit_idx = self.editing_element_index?;

        let element_id = self.elements.get_elements().get(edit_idx).map(|e| e.id)?;

        Some(TextCursorState {
            element_id,
            cursor_pos: self.text_cursor_pos,
            visible: self.text_cursor_visible,
        })
    }
}
