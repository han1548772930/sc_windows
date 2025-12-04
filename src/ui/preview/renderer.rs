//! 预览窗口渲染器（负责所有 Direct2D 绘图）

use anyhow::Result;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap;

use crate::platform::traits::PlatformRenderer;
use crate::platform::windows::Direct2DRenderer;
use crate::constants::{
    BUTTON_WIDTH_OCR, CLOSE_BUTTON_HOVER_BG_COLOR_D2D, ICON_HOVER_BG_COLOR_D2D,
    ICON_HOVER_PADDING, ICON_HOVER_RADIUS, ICON_SIZE,
    TITLE_BAR_BG_COLOR_D2D, TITLE_BAR_BUTTON_HOVER_BG_COLOR_D2D, TITLE_BAR_HEIGHT,
};

use super::types::{D2DIconBitmaps, IconCache, SvgIcon};

/// 预览窗口渲染器
pub struct PreviewRenderer {
    pub(super) d2d_renderer: Direct2DRenderer,
    pub(super) image_bitmap: Option<ID2D1Bitmap>,
    pub(super) icon_cache: IconCache,
    pub(super) icons_loaded: bool,
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

    pub fn initialize(&mut self, hwnd: HWND, width: i32, height: i32) -> Result<()> {
        // 记录旧的 RenderTarget 指针，用于检测是否发生了重建
        use windows::core::Interface;
        let old_rt_ptr = self
            .d2d_renderer
            .render_target
            .as_ref()
            .map(|rt| rt.as_raw());

        self.d2d_renderer
            .initialize(hwnd, width, height)
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
            "pin",
            "window-close",
            "window-maximize",
            "window-minimize",
            "window-restore",
        ];

        for name in icons.iter() {
            let normal_pixels = Self::load_svg_pixels(name, ICON_SIZE, None)?;
            let normal_bitmap = self
                .d2d_renderer
                .create_bitmap_from_pixels(
                    &normal_pixels,
                    (ICON_SIZE * 2) as u32,
                    (ICON_SIZE * 2) as u32,
                )
                .map_err(|e| anyhow::anyhow!("Failed to create bitmap for {}: {:?}", name, e))?;

            let hover_pixels = if *name == "window-close" {
                Self::load_svg_pixels(name, ICON_SIZE, Some((255, 255, 255)))?
            } else {
                normal_pixels.clone()
            };

            let hover_bitmap = self
                .d2d_renderer
                .create_bitmap_from_pixels(
                    &hover_pixels,
                    (ICON_SIZE * 2) as u32,
                    (ICON_SIZE * 2) as u32,
                )
                .map_err(|e| {
                    anyhow::anyhow!("Failed to create hover bitmap for {}: {:?}", name, e)
                })?;

            let (active_normal, active_hover) = if *name == "pin" {
                let an_pixels = Self::load_svg_pixels(name, ICON_SIZE, Some(crate::constants::PIN_ACTIVE_COLOR))?;
                let ah_pixels = Self::load_svg_pixels(name, ICON_SIZE, Some(crate::constants::PIN_ACTIVE_COLOR))?;

                let an_bmp = self
                    .d2d_renderer
                    .create_bitmap_from_pixels(
                        &an_pixels,
                        (ICON_SIZE * 2) as u32,
                        (ICON_SIZE * 2) as u32,
                    )
                    .ok();
                let ah_bmp = self
                    .d2d_renderer
                    .create_bitmap_from_pixels(
                        &ah_pixels,
                        (ICON_SIZE * 2) as u32,
                        (ICON_SIZE * 2) as u32,
                    )
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

    /// 加载 SVG 并渲染为像素数据 (RGBA)
    fn load_svg_pixels(
        filename: &str,
        size: i32,
        color_override: Option<(u8, u8, u8)>,
    ) -> Result<Vec<u8>> {
        let svg_path = format!("icons/{}.svg", filename);
        let svg_data = std::fs::read_to_string(&svg_path)?;
        let tree = usvg::Tree::from_str(&svg_data, &usvg::Options::default())?;

        let scale = 2.0;
        let render_size = (size as f32 * scale) as u32;
        let mut pixmap = tiny_skia::Pixmap::new(render_size, render_size)
            .ok_or_else(|| anyhow::anyhow!("Failed to create pixmap"))?;

        pixmap.fill(tiny_skia::Color::TRANSPARENT);

        let svg_size = tree.size();
        let render_ts = tiny_skia::Transform::from_scale(
            size as f32 * scale / svg_size.width(),
            size as f32 * scale / svg_size.height(),
        );

        resvg::render(&tree, render_ts, &mut pixmap.as_mut());

        if let Some((r, g, b)) = color_override {
            let pixels = pixmap.data_mut();
            for i in (0..pixels.len()).step_by(4) {
                let alpha = pixels[i + 3];
                if alpha > 0 {
                    pixels[i] = r;
                    pixels[i + 1] = g;
                    pixels[i + 2] = b;
                }
            }
        }

        Ok(pixmap.data().to_vec())
    }

    /// 将文本按指定宽度分行
    pub fn split_text_into_lines(&self, text: &str, width: f32) -> Vec<String> {
        self.d2d_renderer
            .split_text_into_lines(text, width, "Microsoft YaHei", 18.0)
            .unwrap_or_else(|_| vec![text.to_string()])
    }

    /// 获取点击位置的字符索引
    pub fn get_text_position_from_point(&self, text: &str, x: f32) -> usize {
        self.d2d_renderer
            .get_text_position_from_point(text, x, "Microsoft YaHei", 18.0)
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
            .clear(crate::platform::Color { r, g, b, a })
            .map_err(|e| anyhow::anyhow!("Clear Error: {:?}", e))
    }

    /// 绘制自定义标题栏
    pub fn draw_custom_title_bar(
        &mut self,
        width: i32,
        icons: &[SvgIcon],
        is_pinned: bool,
    ) -> Result<()> {
        use crate::platform::traits::{Color, DrawStyle, Rectangle};

        // 绘制标题栏背景
        let title_bar_rect = Rectangle::new(0.0, 0.0, width as f32, TITLE_BAR_HEIGHT as f32);
        let bg_color = Color {
            r: TITLE_BAR_BG_COLOR_D2D.r,
            g: TITLE_BAR_BG_COLOR_D2D.g,
            b: TITLE_BAR_BG_COLOR_D2D.b,
            a: TITLE_BAR_BG_COLOR_D2D.a,
        };

        let bg_style = DrawStyle {
            stroke_color: bg_color,
            fill_color: Some(bg_color),
            stroke_width: 0.0,
        };

        self.d2d_renderer
            .draw_rectangle(title_bar_rect, &bg_style)
            .map_err(|e| anyhow::anyhow!("Failed to draw title bar bg: {:?}", e))?;

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
                let (bg_color_d2d, use_rounded) = if icon.is_title_bar_button {
                    if icon.name == "window-close" {
                        (CLOSE_BUTTON_HOVER_BG_COLOR_D2D, false)
                    } else {
                        (TITLE_BAR_BUTTON_HOVER_BG_COLOR_D2D, false)
                    }
                } else {
                    (ICON_HOVER_BG_COLOR_D2D, true)
                };

                let hover_color = Color {
                    r: bg_color_d2d.r,
                    g: bg_color_d2d.g,
                    b: bg_color_d2d.b,
                    a: bg_color_d2d.a,
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

                    if icon.name == "window-close" {
                        button_rect.width = (width as f32) - button_rect.x;
                    }

                    self.d2d_renderer
                        .draw_rectangle(button_rect, &hover_style)
                        .map_err(|e| anyhow::anyhow!("Failed to draw button hover: {:?}", e))?;
                }
            }

            // 绘制图标本身
            if let Some(bitmaps) = self.icon_cache.get(&icon.name) {
                let bitmap_to_use = if icon.name == "pin" && is_pinned {
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

    pub fn render(
        &mut self,
        text_lines: &[String],
        text_rect: RECT,
        width: i32,
        icons: &[SvgIcon],
        is_pinned: bool,
        _is_maximized: bool,
        scroll_offset: i32,
        line_height: i32,
        image_width: i32,
        image_height: i32,
        selection: Option<((usize, usize), (usize, usize))>,
        show_text_area: bool,
    ) -> Result<()> {
        self.begin_frame()?;

        self.clear(
            crate::constants::CONTENT_BG_COLOR_D2D.r,
            crate::constants::CONTENT_BG_COLOR_D2D.g,
            crate::constants::CONTENT_BG_COLOR_D2D.b,
            crate::constants::CONTENT_BG_COLOR_D2D.a,
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
                let available_width = (width - 350 - 40) as f32;
                let available_height = (text_rect.bottom - text_rect.top) as f32;
                let start_y = TITLE_BAR_HEIGHT as f32 + 10.0;

                let max_height =
                    (crate::platform::windows::system::get_screen_size().1 as f32) - start_y - 20.0;
                let effective_available_height = available_height.min(max_height);

                let scale_x = available_width / original_width;
                let scale_y = effective_available_height / original_height;
                let scale = scale_x.min(scale_y).min(1.0);

                let display_w = original_width * scale;
                let display_h = original_height * scale;

                let left_area_center_x = 20.0 + available_width / 2.0;
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

        // 3. 绘制文本区域
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
        text_rect: RECT,
        scroll_offset: i32,
        line_height: i32,
        selection: Option<((usize, usize), (usize, usize))>,
    ) -> Result<()> {
        let text_color = crate::platform::traits::Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };
        let text_style = crate::platform::traits::TextStyle {
            font_size: 18.0,
            color: text_color,
            font_family: "Microsoft YaHei".to_string(),
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

            let pos = crate::platform::traits::Point {
                x: text_rect.left as f32,
                y: line_y,
            };

            // 绘制选择高亮
            if i >= start_sel.0 && i <= end_sel.0 {
                let mut sel_rect = crate::platform::traits::Rectangle {
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

                    let highlight_color = crate::platform::traits::Color {
                        r: 0.78,
                        g: 0.97,
                        b: 0.77,
                        a: 1.0,
                    };
                    let highlight_style = crate::platform::traits::DrawStyle {
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
}
