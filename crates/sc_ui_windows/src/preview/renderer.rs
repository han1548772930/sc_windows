use anyhow::Result;
use sc_app::selection::RectI32;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::{
    D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, D2D1_COMPATIBLE_RENDER_TARGET_OPTIONS_GDI_COMPATIBLE,
    D2D1_DC_INITIALIZE_MODE_COPY, ID2D1Bitmap, ID2D1BitmapRenderTarget,
    ID2D1GdiInteropRenderTarget, ID2D1RenderTarget,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::{GetCurrentObject, HBITMAP, OBJ_BITMAP};
use windows::core::Interface;

use super::drawing::PreviewDrawingState;
use super::types::{D2DIconBitmaps, IconCache, SvgIcon};
use crate::constants::{
    BUTTON_WIDTH_OCR, CLOSE_BUTTON_HOVER_BG_COLOR, CONTENT_BG_COLOR, ICON_HOVER_BG_COLOR,
    ICON_HOVER_PADDING, ICON_HOVER_RADIUS, ICON_SIZE, OCR_CONTENT_PADDING_BOTTOM,
    OCR_CONTENT_PADDING_TOP, OCR_CONTENT_PADDING_X, OCR_IMAGE_START_Y_OFFSET, OCR_PANEL_GAP,
    OCR_TEXT_COLOR, OCR_TEXT_FONT_FAMILY, OCR_TEXT_FONT_SIZE, OCR_TEXT_PADDING_BOTTOM,
    OCR_TEXT_PANEL_WIDTH, OCR_TEXT_SELECTION_BG_COLOR, PIN_ACTIVE_COLOR, TITLE_BAR_BG_COLOR,
    TITLE_BAR_BUTTON_HOVER_BG_COLOR, TITLE_BAR_HEIGHT, TITLE_BAR_SEPARATOR_COLOR,
};
use sc_ui::preview_layout;
use crate::svg::{PixelFormat, apply_color_to_pixels, render_svg_pixels};
use sc_platform::{Color, DrawStyle, HostPlatform, Point, Rectangle, TextStyle, WindowId};
use sc_platform_windows::windows::{Direct2DRenderer, WindowsHostPlatform, bmp};

/// 预览窗口渲染器
pub struct PreviewRenderer {
    pub(super) d2d_renderer: Direct2DRenderer,
    pub(super) image_bitmap: Option<ID2D1Bitmap>,
    pub(super) icon_cache: IconCache,
    pub(super) icons_loaded: bool,
}

pub(super) struct PreviewRenderArgs<'a> {
    pub(super) text_lines: &'a [String],
    pub(super) text_rect: RectI32,
    pub(super) width: i32,
    pub(super) icons: &'a [SvgIcon],
    pub(super) is_pinned: bool,
    pub(super) scroll_offset: i32,
    pub(super) line_height: i32,
    pub(super) image_width: i32,
    pub(super) image_height: i32,
    pub(super) selection: Option<((usize, usize), (usize, usize))>,
    pub(super) show_text_area: bool,
    pub(super) drawing_state: Option<&'a mut PreviewDrawingState>,
}

impl PreviewRenderer {
    pub fn new() -> Result<Self> {
        // 优先使用全局共享的 Factory，避免重复创建重量级 COM 对象
        let d2d_renderer = Direct2DRenderer::new_with_shared_factories()
            .or_else(|_| Direct2DRenderer::new())
            .map_err(|e| anyhow::anyhow!("D2D Init Error: {:?}", e))?;

        Ok(Self {
            d2d_renderer,
            image_bitmap: None,
            icon_cache: IconCache::new(),
            icons_loaded: false,
        })
    }

    pub fn initialize(&mut self, window: WindowId, width: i32, height: i32) -> Result<()> {
        // 记录旧的 RenderTarget 指针，用于检测是否发生了重建
        let old_rt_ptr = self
            .d2d_renderer
            .render_target
            .as_ref()
            .map(|rt| rt.as_raw());

        self.d2d_renderer
            .initialize(window, width, height)
            .map_err(|e| anyhow::anyhow!("D2D Initialize Error: {:?}", e))?;

        // 检查 RenderTarget 是否改变
        let new_rt_ptr = self
            .d2d_renderer
            .render_target
            .as_ref()
            .map(|rt| rt.as_raw());

        let rt_changed = match (old_rt_ptr, new_rt_ptr) {
            (Some(p1), Some(p2)) => p1 != p2,
            (None, None) => false,
            _ => true,
        };

        // 只有在 RenderTarget 真正改变（重建）时才清理资源
        if rt_changed {
            self.image_bitmap = None;
            self.icon_cache.clear();
            self.icons_loaded = false;
        }

        // 初始化后加载图标（如果尚未加载）
        if !self.icons_loaded {
            self.load_icons()?;
            self.icons_loaded = true;
        }
        Ok(())
    }

    pub fn set_image_from_pixels(&mut self, pixels: &[u8], width: i32, height: i32) -> Result<()> {
        if self.image_bitmap.is_some() {
            return Ok(());
        }

        let d2d_bitmap = self
            .d2d_renderer
            .create_bitmap_from_pixels(pixels, width as u32, height as u32)
            .map_err(|e| anyhow::anyhow!("Failed to create D2D bitmap from pixels: {:?}", e))?;

        self.image_bitmap = Some(d2d_bitmap);
        Ok(())
    }

    /// 加载所有图标到 D2D 位图
    fn load_icons(&mut self) -> Result<()> {
        let icons = [
            preview_layout::ICON_PIN,
            preview_layout::ICON_OCR,
            preview_layout::ICON_SAVE,
            preview_layout::ICON_WINDOW_CLOSE,
            preview_layout::ICON_WINDOW_MAXIMIZE,
            preview_layout::ICON_WINDOW_MINIMIZE,
            preview_layout::ICON_WINDOW_RESTORE,
            // 绘图工具图标
            preview_layout::ICON_TOOL_SQUARE,
            preview_layout::ICON_TOOL_CIRCLE,
            preview_layout::ICON_TOOL_ARROW,
            preview_layout::ICON_TOOL_PEN,
            preview_layout::ICON_TOOL_TEXT,
        ];

        for name in icons.iter() {
            let normal_pixels = Self::load_svg_pixels(name, ICON_SIZE, None)?;
            let normal_bitmap = self
                .d2d_renderer
                .create_bitmap_from_pixels(&normal_pixels, ICON_SIZE as u32, ICON_SIZE as u32)
                .map_err(|e| anyhow::anyhow!("Failed to create bitmap for {}: {:?}", name, e))?;

            let hover_pixels = if *name == preview_layout::ICON_WINDOW_CLOSE {
                Self::load_svg_pixels(name, ICON_SIZE, Some((255, 255, 255)))?
            } else {
                normal_pixels.clone()
            };

            let hover_bitmap = self
                .d2d_renderer
                .create_bitmap_from_pixels(&hover_pixels, ICON_SIZE as u32, ICON_SIZE as u32)
                .map_err(|e| {
                    anyhow::anyhow!("Failed to create hover bitmap for {}: {:?}", name, e)
                })?;

            // Selected/active (green) icons.
            // We currently use the same active color for pin and drawing tools.
            let supports_active_color = matches!(
                *name,
                preview_layout::ICON_PIN
                    | preview_layout::ICON_OCR
                    | preview_layout::ICON_TOOL_SQUARE
                    | preview_layout::ICON_TOOL_CIRCLE
                    | preview_layout::ICON_TOOL_ARROW
                    | preview_layout::ICON_TOOL_PEN
                    | preview_layout::ICON_TOOL_TEXT
            );

            let (active_normal, active_hover) = if supports_active_color {
                let an_pixels = Self::load_svg_pixels(name, ICON_SIZE, Some(PIN_ACTIVE_COLOR))?;
                let ah_pixels = Self::load_svg_pixels(name, ICON_SIZE, Some(PIN_ACTIVE_COLOR))?;

                let an_bmp = self
                    .d2d_renderer
                    .create_bitmap_from_pixels(&an_pixels, ICON_SIZE as u32, ICON_SIZE as u32)
                    .ok();
                let ah_bmp = self
                    .d2d_renderer
                    .create_bitmap_from_pixels(&ah_pixels, ICON_SIZE as u32, ICON_SIZE as u32)
                    .ok();
                (an_bmp, ah_bmp)
            } else {
                (None, None)
            };

            self.icon_cache.insert(
                name.to_string(),
                D2DIconBitmaps {
                    normal: normal_bitmap,
                    hover: hover_bitmap,
                    active_normal,
                    active_hover,
                },
            );
        }

        Ok(())
    }


    /// 加载SVG并渲染为像素数据(RGBA)
    fn load_svg_pixels(
        filename: &str,
        size: i32,
        color_override: Option<(u8, u8, u8)>,
    ) -> Result<Vec<u8>> {
        // 使用嵌入式 SVG 内容，而非文件系统读取
        let svg_content = crate::icon_assets::preview_icon_svg(filename)
            .ok_or_else(|| anyhow::anyhow!("Unknown embedded icon: {}", filename))?;
        let (mut pixels, _, _) =
            render_svg_pixels(svg_content, size as u32, PixelFormat::Rgba, None)?;

        // 如果需要颜色覆盖，在像素级别应用
        if let Some(color) = color_override {
            apply_color_to_pixels(&mut pixels, color, PixelFormat::Rgba);
        }

        Ok(pixels)
    }

    /// 将文本按指定宽度分行
    pub fn split_text_into_lines(&self, text: &str, width: f32) -> Vec<String> {
        self.d2d_renderer
            .split_text_into_lines(text, width, OCR_TEXT_FONT_FAMILY, OCR_TEXT_FONT_SIZE)
            .unwrap_or_else(|_| vec![text.to_string()])
    }

    /// 获取点击位置的字符索引
    pub fn get_text_position_from_point(&self, text: &str, x: f32) -> usize {
        self.d2d_renderer
            .get_text_position_from_point(text, x, OCR_TEXT_FONT_FAMILY, OCR_TEXT_FONT_SIZE)
            .unwrap_or(0)
    }

    pub fn begin_frame(&mut self) -> Result<()> {
        self.d2d_renderer
            .begin_frame()
            .map_err(|e| anyhow::anyhow!("BeginFrame Error: {:?}", e))
    }

    pub fn end_frame(&mut self) -> Result<()> {
        self.d2d_renderer
            .end_frame()
            .map_err(|e| anyhow::anyhow!("EndFrame Error: {:?}", e))
    }

    pub fn clear(&mut self, r: f32, g: f32, b: f32, a: f32) -> Result<()> {
        self.d2d_renderer
            .clear(Color { r, g, b, a })
            .map_err(|e| anyhow::anyhow!("Clear Error: {:?}", e))
    }

    /// 绘制自定义标题栏
    pub fn draw_custom_title_bar(
        &mut self,
        width: i32,
        icons: &[SvgIcon],
        is_pinned: bool,
    ) -> Result<()> {
        // 绘制标题栏背景
        let title_bar_rect = Rectangle::new(0.0, 0.0, width as f32, TITLE_BAR_HEIGHT as f32);
        let bg_color = TITLE_BAR_BG_COLOR;

        let bg_style = DrawStyle {
            stroke_color: bg_color,
            fill_color: Some(bg_color),
            stroke_width: 0.0,
        };

        self.d2d_renderer
            .draw_rectangle(title_bar_rect, &bg_style)
            .map_err(|e| anyhow::anyhow!("Failed to draw title bar bg: {:?}", e))?;

        // 绘图工具图标是一组，在两边加上分割竖线。
        {
            let is_tool_icon = |name: &str| {
                matches!(
                    name,
                    preview_layout::ICON_TOOL_SQUARE
                        | preview_layout::ICON_TOOL_CIRCLE
                        | preview_layout::ICON_TOOL_ARROW
                        | preview_layout::ICON_TOOL_PEN
                        | preview_layout::ICON_TOOL_TEXT
                )
            };

            let mut tool_left: Option<i32> = None;
            let mut tool_right: Option<i32> = None;
            for icon in icons
                .iter()
                .filter(|i| !i.is_title_bar_button && is_tool_icon(&i.name))
            {
                tool_left = Some(tool_left.map_or(icon.rect.left, |v| v.min(icon.rect.left)));
                tool_right = Some(tool_right.map_or(icon.rect.right, |v| v.max(icon.rect.right)));
            }

            if let (Some(tool_left), Some(tool_right)) = (tool_left, tool_right) {
                let left_neighbor_right = icons
                    .iter()
                    .filter(|i| !i.is_title_bar_button && i.rect.right <= tool_left)
                    .map(|i| i.rect.right)
                    .max();
                let right_neighbor_left = icons
                    .iter()
                    .filter(|i| !i.is_title_bar_button && i.rect.left >= tool_right)
                    .map(|i| i.rect.left)
                    .min();

                if let (Some(left_r), Some(right_l)) = (left_neighbor_right, right_neighbor_left) {
                    let sep_left_x = (left_r + tool_left) as f32 / 2.0;
                    let sep_right_x = (tool_right + right_l) as f32 / 2.0;

                    let icon_y = (TITLE_BAR_HEIGHT - ICON_SIZE) / 2;
                    let line_top = (icon_y - 4).max(0) as f32;
                    let line_bottom = (icon_y + ICON_SIZE + 4).min(TITLE_BAR_HEIGHT) as f32;

                    let style = DrawStyle {
                        stroke_color: TITLE_BAR_SEPARATOR_COLOR,
                        fill_color: None,
                        stroke_width: 1.0,
                    };

                    let _ = self.d2d_renderer.draw_line(
                        Point {
                            x: sep_left_x,
                            y: line_top,
                        },
                        Point {
                            x: sep_left_x,
                            y: line_bottom,
                        },
                        &style,
                    );
                    let _ = self.d2d_renderer.draw_line(
                        Point {
                            x: sep_right_x,
                            y: line_top,
                        },
                        Point {
                            x: sep_right_x,
                            y: line_bottom,
                        },
                        &style,
                    );
                }
            }
        }

        // 绘制标题栏按钮和图标
        for icon in icons {
            if icon.rect.right > width {
                continue;
            }

            let icon_rect = Rectangle::from_bounds(
                icon.rect.left as f32,
                icon.rect.top as f32,
                icon.rect.right as f32,
                icon.rect.bottom as f32,
            );

            // 绘制悬停/激活背景
            if icon.hovered {
                let (hover_color, use_rounded) = if icon.is_title_bar_button {
                    if icon.name == preview_layout::ICON_WINDOW_CLOSE {
                        (CLOSE_BUTTON_HOVER_BG_COLOR, false)
                    } else {
                        (TITLE_BAR_BUTTON_HOVER_BG_COLOR, false)
                    }
                } else {
                    (ICON_HOVER_BG_COLOR, true)
                };

                let hover_style = DrawStyle {
                    stroke_color: hover_color,
                    fill_color: Some(hover_color),
                    stroke_width: 0.0,
                };

                if use_rounded {
                    let padding = ICON_HOVER_PADDING as f32;
                    let hover_rect = Rectangle::new(
                        icon_rect.x - padding,
                        icon_rect.y - padding,
                        icon_rect.width + padding * 2.0,
                        icon_rect.height + padding * 2.0,
                    );

                    self.d2d_renderer
                        .draw_rounded_rectangle(hover_rect, ICON_HOVER_RADIUS, &hover_style)
                        .map_err(|e| anyhow::anyhow!("Failed to draw icon hover: {:?}", e))?;
                } else {
                    let mut button_rect = icon_rect;
                    let button_width = BUTTON_WIDTH_OCR as f32;
                    let center_x = icon_rect.x + icon_rect.width / 2.0;
                    button_rect.x = center_x - button_width / 2.0;
                    button_rect.width = button_width;
                    button_rect.y = 0.0;
                    button_rect.height = TITLE_BAR_HEIGHT as f32;

                    if icon.name == preview_layout::ICON_WINDOW_CLOSE {
                        button_rect.width = (width as f32) - button_rect.x;
                    }

                    self.d2d_renderer
                        .draw_rectangle(button_rect, &hover_style)
                        .map_err(|e| anyhow::anyhow!("Failed to draw button hover: {:?}", e))?;
                }
            }

            // 绘制图标本身
            if let Some(bitmaps) = self.icon_cache.get(&icon.name) {
                let bitmap_to_use = if icon.name == preview_layout::ICON_PIN && is_pinned {
                    if icon.hovered {
                        bitmaps.active_hover.as_ref().unwrap_or(&bitmaps.hover)
                    } else {
                        bitmaps.active_normal.as_ref().unwrap_or(&bitmaps.normal)
                    }
                } else if icon.selected {
                    if icon.hovered {
                        bitmaps.active_hover.as_ref().unwrap_or(&bitmaps.hover)
                    } else {
                        bitmaps.active_normal.as_ref().unwrap_or(&bitmaps.normal)
                    }
                } else if icon.hovered {
                    &bitmaps.hover
                } else {
                    &bitmaps.normal
                };

                let icon_width = ICON_SIZE as f32;
                let icon_height = ICON_SIZE as f32;

                let center_x = icon_rect.x + icon_rect.width / 2.0;
                let center_y = icon_rect.y + icon_rect.height / 2.0;

                let draw_rect = D2D_RECT_F {
                    left: center_x - icon_width / 2.0,
                    top: center_y - icon_height / 2.0,
                    right: center_x + icon_width / 2.0,
                    bottom: center_y + icon_height / 2.0,
                };

                self.d2d_renderer
                    .draw_d2d_bitmap(bitmap_to_use, Some(draw_rect), 1.0, None)
                    .map_err(|e| anyhow::anyhow!("Failed to draw icon {}: {:?}", icon.name, e))?;
            }
        }

        Ok(())
    }

    pub fn render(&mut self, args: PreviewRenderArgs<'_>) -> Result<()> {
        let PreviewRenderArgs {
            text_lines,
            text_rect,
            width,
            icons,
            is_pinned,
            scroll_offset,
            line_height,
            image_width,
            image_height,
            selection,
            show_text_area,
            drawing_state,
        } = args;

        self.begin_frame()?;

        self.clear(
            CONTENT_BG_COLOR.r,
            CONTENT_BG_COLOR.g,
            CONTENT_BG_COLOR.b,
            CONTENT_BG_COLOR.a,
        )?;

        // 1. 绘制标题栏
        self.draw_custom_title_bar(width, icons, is_pinned)?;

        // 2. 绘制图片
        if let Some(bitmap) = &self.image_bitmap {
            let original_width = image_width as f32;
            let original_height = image_height as f32;

            let (image_x, image_y, display_width, display_height) = if !show_text_area {
                let x = 0.0;
                let y = TITLE_BAR_HEIGHT as f32;
                (x, y, original_width, original_height)
            } else {
                let image_area_width = width - OCR_TEXT_PANEL_WIDTH - OCR_PANEL_GAP;
                let available_width = (image_area_width - 2 * OCR_CONTENT_PADDING_X) as f32;

                // Derive window height from the text rect so image layout stays consistent when the
                // window is resized/maximized.
                let window_height = text_rect.bottom + OCR_TEXT_PADDING_BOTTOM;
                let available_height = (window_height
                    - TITLE_BAR_HEIGHT
                    - OCR_CONTENT_PADDING_TOP
                    - OCR_CONTENT_PADDING_BOTTOM) as f32;

                let start_y = TITLE_BAR_HEIGHT as f32 + OCR_IMAGE_START_Y_OFFSET as f32;

                let (_, screen_height) = WindowsHostPlatform::new().screen_size();
                let max_height =
                    (screen_height as f32) - start_y - OCR_CONTENT_PADDING_BOTTOM as f32;
                let effective_available_height = available_height.min(max_height);

                let scale_x = available_width / original_width;
                let scale_y = effective_available_height / original_height;
                let scale = scale_x.min(scale_y).min(1.0);

                let display_w = original_width * scale;
                let display_h = original_height * scale;

                let left_area_center_x = OCR_CONTENT_PADDING_X as f32 + available_width / 2.0;
                let x = left_area_center_x - display_w / 2.0;
                let y = start_y + (effective_available_height - display_h) / 2.0;

                (x, y, display_w, display_h)
            };

            let image_dest_rect = D2D_RECT_F {
                left: image_x,
                top: image_y,
                right: image_x + display_width,
                bottom: image_y + display_height,
            };

            self.d2d_renderer
                .draw_d2d_bitmap(bitmap, Some(image_dest_rect), 1.0, None)
                .map_err(|e| anyhow::anyhow!("Failed to draw bitmap: {:?}", e))?;
        }

        // 3. 绘制绘图元素（在图片上方）
        if let Some(ds) = drawing_state {
            let image_area_rect = ds.image_area_rect;
            let image_area_rect_drawing = sc_drawing::Rect {
                left: image_area_rect.left,
                top: image_area_rect.top,
                right: image_area_rect.right,
                bottom: image_area_rect.bottom,
            };
            ds.manager
                .render(&mut self.d2d_renderer, Some(&image_area_rect_drawing))
                .map_err(|e| anyhow::anyhow!("Failed to render drawing elements: {:?}", e))?;
        }

        // 4. 绘制文本区域
        if show_text_area {
            self.render_text_area(text_lines, text_rect, scroll_offset, line_height, selection)?;
        }

        self.end_frame()?;
        Ok(())
    }

    /// 渲染文本区域
    fn render_text_area(
        &mut self,
        text_lines: &[String],
        text_rect: RectI32,
        scroll_offset: i32,
        line_height: i32,
        selection: Option<((usize, usize), (usize, usize))>,
    ) -> Result<()> {
        let text_color = OCR_TEXT_COLOR;
        let text_style = TextStyle {
            font_size: OCR_TEXT_FONT_SIZE,
            color: text_color,
            font_family: OCR_TEXT_FONT_FAMILY.to_string(),
        };

        let start_y = text_rect.top as f32 - scroll_offset as f32;

        let (start_sel, end_sel) = if let Some((s, e)) = selection {
            if s <= e { (s, e) } else { (e, s) }
        } else {
            ((usize::MAX, 0), (usize::MAX, 0))
        };

        for (i, line) in text_lines.iter().enumerate() {
            let line_y = start_y + (i as f32 * line_height as f32);

            if line_y + (line_height as f32) < text_rect.top as f32 {
                continue;
            }
            if line_y > text_rect.bottom as f32 {
                break;
            }

            let pos = Point {
                x: text_rect.left as f32,
                y: line_y,
            };

            // 绘制选择高亮
            if i >= start_sel.0 && i <= end_sel.0 {
                let mut sel_rect = Rectangle {
                    x: pos.x,
                    y: pos.y,
                    width: 0.0,
                    height: line_height as f32,
                };

                let start_char = if i == start_sel.0 { start_sel.1 } else { 0 };
                let end_char = if i == end_sel.0 {
                    end_sel.1
                } else {
                    line.chars().count()
                };

                if start_char < end_char {
                    let prefix = line.chars().take(start_char).collect::<String>();
                    let (prefix_width, _) = self
                        .d2d_renderer
                        .measure_text_layout_size(&prefix, 10000.0, &text_style)
                        .unwrap_or((0.0, 0.0));

                    let selected_text = line
                        .chars()
                        .skip(start_char)
                        .take(end_char - start_char)
                        .collect::<String>();
                    let (sel_width, _) = self
                        .d2d_renderer
                        .measure_text_layout_size(&selected_text, 10000.0, &text_style)
                        .unwrap_or((0.0, 0.0));

                    sel_rect.x += prefix_width;
                    sel_rect.width = sel_width;

                    let highlight_color = OCR_TEXT_SELECTION_BG_COLOR;
                    let highlight_style = DrawStyle {
                        stroke_color: highlight_color,
                        fill_color: Some(highlight_color),
                        stroke_width: 0.0,
                    };

                    let _ = self.d2d_renderer.draw_rectangle(sel_rect, &highlight_style);
                }
            }

            // 绘制文本行
            self.d2d_renderer
                .draw_text(line, pos, &text_style)
                .map_err(|e| anyhow::anyhow!("Failed to draw text: {:?}", e))?;
        }

        Ok(())
    }

    /// Render the current image (and optional drawings) into BMP file bytes.
    ///
    /// We render only the image area (excluding the custom title bar / text area).
    pub fn render_image_area_to_bmp(
        &mut self,
        image_area_rect: RectI32,
        drawing_state: Option<&mut PreviewDrawingState>,
    ) -> Result<Vec<u8>> {
        let Some(source_bitmap) = self.image_bitmap.as_ref() else {
            return Err(anyhow::anyhow!("image bitmap not initialized"));
        };
        let Some(render_target) = self.d2d_renderer.render_target.as_ref() else {
            return Err(anyhow::anyhow!("no render target available"));
        };

        let width = (image_area_rect.right - image_area_rect.left).max(0) as u32;
        let height = (image_area_rect.bottom - image_area_rect.top).max(0) as u32;
        if width == 0 || height == 0 {
            return Err(anyhow::anyhow!("invalid image area size"));
        }

        // Create a GDI-compatible offscreen target so we can extract pixels via GetDC/GetDIBits.
        let size = D2D_SIZE_F {
            width: width as f32,
            height: height as f32,
        };
        let pixel_size = D2D_SIZE_U { width, height };
        let pixel_format = D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
        };

        let offscreen_target: ID2D1BitmapRenderTarget = unsafe {
            render_target.CreateCompatibleRenderTarget(
                Some(&size),
                Some(&pixel_size),
                Some(&pixel_format),
                D2D1_COMPATIBLE_RENDER_TARGET_OPTIONS_GDI_COMPATIBLE,
            )
        }
        .map_err(|e| anyhow::anyhow!("Failed to create offscreen target: {e:?}"))?;

        let gdi_target: ID2D1GdiInteropRenderTarget = offscreen_target
            .cast()
            .map_err(|e| anyhow::anyhow!("Failed to cast to GDI interop: {e:?}"))?;

        unsafe {
            offscreen_target.BeginDraw();

            // White background.
            let clear_color = D2D1_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            };
            offscreen_target.Clear(Some(&clear_color));

            // Draw the screenshot image scaled to the current image area size.
            let dest_rect = D2D_RECT_F {
                left: 0.0,
                top: 0.0,
                right: width as f32,
                bottom: height as f32,
            };
            offscreen_target.DrawBitmap(
                source_bitmap,
                Some(&dest_rect),
                1.0,
                D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                None,
            );
        }

        // Overlay drawings (if present).
        if let Some(ds) = drawing_state {
            let selection_rect = sc_drawing::Rect {
                left: image_area_rect.left,
                top: image_area_rect.top,
                right: image_area_rect.right,
                bottom: image_area_rect.bottom,
            };
            let offscreen_rt: &ID2D1RenderTarget = &offscreen_target;
            ds.manager
                .render_elements_to_target(offscreen_rt, &mut self.d2d_renderer, &selection_rect)
                .map_err(|e| anyhow::anyhow!("Failed to render drawing elements: {e}"))?;
        }

        // Extract pixel data into BMP bytes.
        let bmp_bytes = unsafe {
            let hdc = gdi_target
                .GetDC(D2D1_DC_INITIALIZE_MODE_COPY)
                .map_err(|e| anyhow::anyhow!("Failed to get DC: {e:?}"))?;

            let current_obj = GetCurrentObject(hdc, OBJ_BITMAP);
            let hbitmap = HBITMAP(current_obj.0);

            let bmp = bmp::bitmap_to_bmp_data(hdc, hbitmap, width as i32, height as i32)
                .map_err(|e| anyhow::anyhow!("Failed to read bitmap pixels: {e:?}"))?;

            let _ = gdi_target.ReleaseDC(None);
            bmp
        };

        unsafe {
            offscreen_target
                .EndDraw(None, None)
                .map_err(|e| anyhow::anyhow!("Offscreen EndDraw failed: {e:?}"))?;
        }

        Ok(bmp_bytes)
    }
}
