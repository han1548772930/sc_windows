// 覆盖层管理
//
// 负责管理屏幕覆盖层的显示和交互

use super::UIError;
use crate::platform::{PlatformError, PlatformRenderer};

/// 覆盖层管理器
pub struct OverlayManager {
    /// 是否显示覆盖层
    visible: bool,
    /// 屏幕尺寸
    screen_width: f32,
    screen_height: f32,
    /// 当前选择区域
    selection_rect: Option<SelectionRect>,
}

/// 选择矩形
#[derive(Debug, Clone)]
struct SelectionRect {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl OverlayManager {
    /// 创建新的覆盖层管理器
    pub fn new() -> Result<Self, UIError> {
        Ok(Self {
            visible: false,
            screen_width: 1920.0, // 默认屏幕尺寸，实际使用时应该获取真实尺寸
            screen_height: 1080.0,
            selection_rect: None,
        })
    }

    /// 设置屏幕尺寸
    pub fn set_screen_size(&mut self, width: f32, height: f32) {
        self.screen_width = width;
        self.screen_height = height;
    }

    /// 设置选择区域
    pub fn set_selection(&mut self, x: f32, y: f32, width: f32, height: f32) {
        self.selection_rect = Some(SelectionRect {
            x,
            y,
            width,
            height,
        });
    }

    /// 清除选择区域
    pub fn clear_selection(&mut self) {
        self.selection_rect = None;
    }

    /// 获取选择区域
    pub fn get_selection(&self) -> Option<(f32, f32, f32, f32)> {
        self.selection_rect
            .as_ref()
            .map(|rect| (rect.x, rect.y, rect.width, rect.height))
    }

    /// 显示覆盖层
    pub fn show(&mut self) {
        self.visible = true;
    }

    /// 隐藏覆盖层
    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// 渲染覆盖层
    pub fn render(
        &self,
        renderer: &mut dyn PlatformRenderer<Error = PlatformError>,
    ) -> Result<(), UIError> {
        if self.visible {
            if let Some(ref selection) = self.selection_rect {
                // 有选择区域时，绘制遮罩和选择框
                self.render_dimmed_overlay(renderer, selection)?;
                self.render_selection_border(renderer, selection)?;
                self.render_selection_handles(renderer, selection)?;
            } else {
                // 无选择区域时，绘制全屏遮罩
                self.render_full_screen_mask(renderer)?;
            }
        }
        Ok(())
    }

    /// 渲染半透明遮罩（选择区域外的部分）
    fn render_dimmed_overlay(
        &self,
        renderer: &mut dyn PlatformRenderer<Error = PlatformError>,
        selection: &SelectionRect,
    ) -> Result<(), UIError> {
        use crate::platform::traits::{Color, DrawStyle, Rectangle};

        let mask_color = Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.6, // 60%透明度
        };

        let mask_style = DrawStyle {
            stroke_color: Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            },
            fill_color: Some(mask_color),
            stroke_width: 0.0,
        };

        // 绘制四个遮罩矩形（选择区域外的部分）

        // 上方遮罩
        if selection.y > 0.0 {
            let top_rect = Rectangle {
                x: 0.0,
                y: 0.0,
                width: self.screen_width,
                height: selection.y,
            };
            renderer
                .draw_rectangle(top_rect, &mask_style)
                .map_err(|e| UIError::RenderError(e.to_string()))?;
        }

        // 下方遮罩
        let selection_bottom = selection.y + selection.height;
        if selection_bottom < self.screen_height {
            let bottom_rect = Rectangle {
                x: 0.0,
                y: selection_bottom,
                width: self.screen_width,
                height: self.screen_height - selection_bottom,
            };
            renderer
                .draw_rectangle(bottom_rect, &mask_style)
                .map_err(|e| UIError::RenderError(e.to_string()))?;
        }

        // 左侧遮罩
        if selection.x > 0.0 {
            let left_rect = Rectangle {
                x: 0.0,
                y: selection.y,
                width: selection.x,
                height: selection.height,
            };
            renderer
                .draw_rectangle(left_rect, &mask_style)
                .map_err(|e| UIError::RenderError(e.to_string()))?;
        }

        // 右侧遮罩
        let selection_right = selection.x + selection.width;
        if selection_right < self.screen_width {
            let right_rect = Rectangle {
                x: selection_right,
                y: selection.y,
                width: self.screen_width - selection_right,
                height: selection.height,
            };
            renderer
                .draw_rectangle(right_rect, &mask_style)
                .map_err(|e| UIError::RenderError(e.to_string()))?;
        }

        Ok(())
    }

    /// 渲染选择框边框
    fn render_selection_border(
        &self,
        renderer: &mut dyn PlatformRenderer<Error = PlatformError>,
        selection: &SelectionRect,
    ) -> Result<(), UIError> {
        use crate::platform::traits::{Color, DrawStyle, Rectangle};

        let border_rect = Rectangle {
            x: selection.x,
            y: selection.y,
            width: selection.width,
            height: selection.height,
        };

        let border_style = DrawStyle {
            stroke_color: Color {
                r: 0.0,
                g: 0.5,
                b: 1.0,
                a: 1.0,
            }, // 蓝色边框
            fill_color: None,
            stroke_width: 2.0,
        };

        renderer
            .draw_rectangle(border_rect, &border_style)
            .map_err(|e| UIError::RenderError(e.to_string()))?;

        Ok(())
    }

    /// 渲染选择框手柄
    fn render_selection_handles(
        &self,
        renderer: &mut dyn PlatformRenderer<Error = PlatformError>,
        selection: &SelectionRect,
    ) -> Result<(), UIError> {
        use crate::platform::traits::{Color, DrawStyle, Rectangle};

        let handle_size = 8.0;
        let half_handle = handle_size / 2.0;

        // 计算8个手柄的位置
        let handles = [
            // 四个角
            (selection.x, selection.y),                    // 左上
            (selection.x + selection.width, selection.y),  // 右上
            (selection.x, selection.y + selection.height), // 左下
            (
                selection.x + selection.width,
                selection.y + selection.height,
            ), // 右下
            // 四个边的中点
            (selection.x + selection.width / 2.0, selection.y), // 上中
            (
                selection.x + selection.width / 2.0,
                selection.y + selection.height,
            ), // 下中
            (selection.x, selection.y + selection.height / 2.0), // 左中
            (
                selection.x + selection.width,
                selection.y + selection.height / 2.0,
            ), // 右中
        ];

        let handle_fill_style = DrawStyle {
            stroke_color: Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            },
            fill_color: Some(Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            }), // 白色填充
            stroke_width: 0.0,
        };

        let handle_border_style = DrawStyle {
            stroke_color: Color {
                r: 0.0,
                g: 0.5,
                b: 1.0,
                a: 1.0,
            }, // 蓝色边框
            fill_color: None,
            stroke_width: 1.0,
        };

        for (hx, hy) in handles.iter() {
            let handle_rect = Rectangle {
                x: hx - half_handle,
                y: hy - half_handle,
                width: handle_size,
                height: handle_size,
            };

            // 绘制手柄填充
            renderer
                .draw_rectangle(handle_rect, &handle_fill_style)
                .map_err(|e| UIError::RenderError(e.to_string()))?;

            // 绘制手柄边框
            renderer
                .draw_rectangle(handle_rect, &handle_border_style)
                .map_err(|e| UIError::RenderError(e.to_string()))?;
        }

        Ok(())
    }

    /// 渲染全屏遮罩
    fn render_full_screen_mask(
        &self,
        renderer: &mut dyn PlatformRenderer<Error = PlatformError>,
    ) -> Result<(), UIError> {
        use crate::platform::traits::{Color, DrawStyle, Rectangle};

        let screen_rect = Rectangle {
            x: 0.0,
            y: 0.0,
            width: self.screen_width,
            height: self.screen_height,
        };

        let mask_style = DrawStyle {
            stroke_color: Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            },
            fill_color: Some(Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.6, // 60%透明度
            }),
            stroke_width: 0.0,
        };

        renderer
            .draw_rectangle(screen_rect, &mask_style)
            .map_err(|e| UIError::RenderError(e.to_string()))?;

        Ok(())
    }
}
