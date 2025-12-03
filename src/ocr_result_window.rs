use crate::ocr::OcrResult;
use crate::platform::traits::PlatformRenderer;
use crate::platform::windows::Direct2DRenderer;
use anyhow::Result;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap;
use windows::Win32::Graphics::Dwm::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::DataExchange::*;
use windows::Win32::System::Memory::*;
use windows::Win32::UI::HiDpi::{
    GetDpiForWindow, GetSystemMetricsForDpi, PROCESS_PER_MONITOR_DPI_AWARE, SetProcessDpiAwareness,
};
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

// 自定义标题栏/图标常量集中到 crate::constants
use crate::constants::{
    BUTTON_WIDTH_OCR, CLOSE_BUTTON_HOVER_BG_COLOR, CLOSE_BUTTON_HOVER_BG_COLOR_D2D,
    ICON_CLICK_PADDING, ICON_HOVER_BG_COLOR, ICON_HOVER_BG_COLOR_D2D, ICON_HOVER_PADDING,
    ICON_HOVER_RADIUS, ICON_SIZE, ICON_START_X, TITLE_BAR_BG_COLOR_D2D,
    TITLE_BAR_BUTTON_HOVER_BG_COLOR, TITLE_BAR_BUTTON_HOVER_BG_COLOR_D2D, TITLE_BAR_HEIGHT,
};

use std::collections::HashMap;

/// D2D 图标位图集合
struct D2DIconBitmaps {
    normal: ID2D1Bitmap,
    hover: ID2D1Bitmap,
    // active 状态可以共用 hover 或 normal，或者单独添加
    active_normal: Option<ID2D1Bitmap>,
    active_hover: Option<ID2D1Bitmap>,
}

/// OCR 结果渲染器（负责所有 Direct2D 绘图）
struct OcrResultRenderer {
    d2d_renderer: Direct2DRenderer,
    image_bitmap: Option<ID2D1Bitmap>,
    icon_cache: HashMap<String, D2DIconBitmaps>,
    icons_loaded: bool,
}

impl OcrResultRenderer {
    fn new() -> Result<Self> {
        Ok(Self {
            d2d_renderer: Direct2DRenderer::new()
                .map_err(|e| anyhow::anyhow!("D2D Init Error: {:?}", e))?,
            image_bitmap: None,
            icon_cache: HashMap::new(),
            icons_loaded: false,
        })
    }

    fn initialize(&mut self, hwnd: HWND, width: i32, height: i32) -> Result<()> {
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
        // 如果只是 Resize，则保留资源
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

    fn set_image_from_pixels(&mut self, pixels: &[u8], width: i32, height: i32) -> Result<()> {
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
        // 定义要加载的图标列表
        let icons = [
            "pin",
            "window-close",
            "window-maximize",
            "window-minimize",
            "window-restore",
        ];

        for name in icons.iter() {
            // 1. 加载普通状态
            let normal_pixels = Self::load_svg_pixels(name, ICON_SIZE, None)?;
            let normal_bitmap = self
                .d2d_renderer
                .create_bitmap_from_pixels(
                    &normal_pixels,
                    (ICON_SIZE * 2) as u32,
                    (ICON_SIZE * 2) as u32,
                )
                .map_err(|e| anyhow::anyhow!("Failed to create bitmap for {}: {:?}", name, e))?;

            // 2. 加载悬停状态 (对于图标，悬停通常是改变背景，图标本身可能不变，或者变色)
            // 这里我们简单复用普通位图，或者如果有特定颜色需求（如关闭按钮变白）则重新生成
            let hover_pixels = if *name == "window-close" {
                // 关闭按钮悬停时变白
                Self::load_svg_pixels(name, ICON_SIZE, Some((255, 255, 255)))?
            } else {
                // 其他图标悬停时保持原色（背景色由 draw_custom_title_bar 绘制）
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

            // 3. 加载激活状态 (Pin)
            let (active_normal, active_hover) = if *name == "pin" {
                let green = (0, 128, 0);
                let an_pixels = Self::load_svg_pixels(name, ICON_SIZE, Some(green))?;
                let ah_pixels = Self::load_svg_pixels(name, ICON_SIZE, Some(green))?; // 悬停时也保持绿色

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

        // 使用 2x 超采样
        let scale = 2.0;
        let render_size = (size as f32 * scale) as u32;
        let mut pixmap = tiny_skia::Pixmap::new(render_size, render_size)
            .ok_or_else(|| anyhow::anyhow!("Failed to create pixmap"))?;

        // 透明背景
        pixmap.fill(tiny_skia::Color::TRANSPARENT);

        let svg_size = tree.size();
        let render_ts = tiny_skia::Transform::from_scale(
            size as f32 * scale / svg_size.width(),
            size as f32 * scale / svg_size.height(),
        );

        resvg::render(&tree, render_ts, &mut pixmap.as_mut());

        // 如果需要颜色覆盖 (简单粗暴地将非透明像素染成指定颜色)
        if let Some((r, g, b)) = color_override {
            let pixels = pixmap.data_mut();
            for i in (0..pixels.len()).step_by(4) {
                // pixels is RGBA
                let alpha = pixels[i + 3];
                if alpha > 0 {
                    pixels[i] = r;
                    pixels[i + 1] = g;
                    pixels[i + 2] = b;
                    // Alpha 保持不变
                }
            }
        }

        Ok(pixmap.data().to_vec())
    }

    /// 将文本按指定宽度分行
    fn split_text_into_lines(&self, text: &str, width: f32) -> Vec<String> {
        self.d2d_renderer
            .split_text_into_lines(text, width, "Microsoft YaHei", 18.0)
            .unwrap_or_else(|_| vec![text.to_string()])
    }

    /// 获取点击位置的字符索引
    fn get_text_position_from_point(&self, text: &str, x: f32) -> usize {
        self.d2d_renderer
            .get_text_position_from_point(text, x, "Microsoft YaHei", 18.0)
            .unwrap_or(0)
    }

    fn begin_frame(&mut self) -> Result<()> {
        use crate::platform::traits::PlatformRenderer;
        self.d2d_renderer
            .begin_frame()
            .map_err(|e| anyhow::anyhow!("BeginFrame Error: {:?}", e))
    }

    fn end_frame(&mut self) -> Result<()> {
        use crate::platform::traits::PlatformRenderer;
        self.d2d_renderer
            .end_frame()
            .map_err(|e| anyhow::anyhow!("EndFrame Error: {:?}", e))
    }

    fn clear(&mut self, r: f32, g: f32, b: f32, a: f32) -> Result<()> {
        use crate::platform::traits::PlatformRenderer;
        self.d2d_renderer
            .clear(crate::platform::Color { r, g, b, a })
            .map_err(|e| anyhow::anyhow!("Clear Error: {:?}", e))
    }

    /// 绘制自定义标题栏
    fn draw_custom_title_bar(
        &mut self,
        width: i32,
        icons: &[SvgIcon],
        is_pinned: bool,
    ) -> Result<()> {
        use crate::platform::traits::{Color, DrawStyle, PlatformRenderer, Rectangle, RendererExt};

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
            // 跳过不可见的图标
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
                    // 普通图标：圆角背景
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
                    // 标题栏按钮：矩形背景
                    let mut button_rect = icon_rect;
                    // 扩展到标准按钮宽度
                    let button_width = BUTTON_WIDTH_OCR as f32;
                    let center_x = icon_rect.x + icon_rect.width / 2.0;
                    button_rect.x = center_x - button_width / 2.0;
                    button_rect.width = button_width;
                    button_rect.y = 0.0; // 铺满高度
                    button_rect.height = TITLE_BAR_HEIGHT as f32;

                    // 关闭按钮延伸到边缘
                    if icon.name == "window-close" {
                        button_rect.width = (width as f32) - button_rect.x;
                    }

                    self.d2d_renderer
                        .draw_rectangle(button_rect, &hover_style)
                        .map_err(|e| anyhow::anyhow!("Failed to draw button hover: {:?}", e))?;
                }
            } else if icon.name == "pin" && is_pinned {
                // Pin 激活状态背景
                // TODO: Differentiate active vs hover state visual if needed
            }

            // 绘制图标本身 (SVG -> D2D Bitmap)
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

                let _icon_padding = ICON_CLICK_PADDING as f32; // 使用适当的 padding
                let icon_width = ICON_SIZE as f32;
                let icon_height = ICON_SIZE as f32;

                // 居中计算
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

    fn render(
        &mut self,
        text_lines: &[String],
        text_rect: RECT,
        width: i32,
        icons: &[SvgIcon],
        is_pinned: bool,
        is_maximized: bool,
        scroll_offset: i32,
        line_height: i32,
        image_width: i32,
        image_height: i32,
        selection: Option<((usize, usize), (usize, usize))>,
    ) -> Result<()> {
        self.begin_frame()?;

        // 清除背景 - 使用浅灰色背景
        self.clear(0.93, 0.93, 0.93, 1.0)?;

        // 1. 绘制标题栏（包括按钮背景）
        self.draw_custom_title_bar(width, icons, is_pinned)?;

        // 2. 绘制图片（如果存在）
        // 图片应该在左侧区域，不覆盖标题栏和文字区域
        if let Some(bitmap) = &self.image_bitmap {
            // 计算可用区域
            let available_width = (width - 350 - 40) as f32; // 减去文字区域宽度和边距
            let available_height = (text_rect.bottom - text_rect.top) as f32; // 使用文本区域的高度作为参考（实际上是窗口高度减去标题栏和padding）
            let start_y = TITLE_BAR_HEIGHT as f32 + 10.0;

            // 限制最大可用高度
            let max_height =
                (crate::platform::windows::system::get_screen_size().1 as f32) - start_y - 20.0;
            let effective_available_height = available_height.min(max_height);

            // 原始尺寸
            let original_width = image_width as f32;
            let original_height = image_height as f32;

            // 计算缩放比例 (保持长宽比，不放大超过原图)
            let scale_x = available_width / original_width;
            let scale_y = effective_available_height / original_height;
            let scale = scale_x.min(scale_y).min(1.0); // 最多缩放到1.0，即原图大小

            // 计算缩放后的尺寸
            let display_width = original_width * scale;
            let display_height = original_height * scale;

            // 居中显示在左侧区域
            // 左侧区域中心点X
            let left_area_center_x = 20.0 + available_width / 2.0;
            let image_x = left_area_center_x - display_width / 2.0;
            let image_y = start_y; // 顶部对齐

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

        // 4. 绘制窗口边框 - 仅在非最大化时绘制
        if !is_pinned && !is_maximized {
            let border_color = crate::platform::traits::Color {
                r: 0.8,
                g: 0.8,
                b: 0.8,
                a: 1.0,
            }; // 浅灰色边框
            let border_style = crate::platform::traits::DrawStyle {
                stroke_color: border_color,
                fill_color: None,
                stroke_width: 1.0,
            };
            let border_rect = crate::platform::traits::Rectangle {
                x: 0.5, // 0.5 偏移以获得清晰的线条
                y: 0.5,
                width: width as f32 - 1.0,
                height: (text_rect.bottom + 15) as f32 - 1.0, // 到底部
            };
            let _ = self.d2d_renderer.draw_rectangle(border_rect, &border_style);
        }

        // 3. 绘制文本和UI元素
        // 使用平台抽象的颜色和文本样式
        let text_color = crate::platform::traits::Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };
        // 字体大小 18.0 约等于 GDI height 24
        let text_style = crate::platform::traits::TextStyle {
            font_size: 18.0,
            color: text_color,
            font_family: "Microsoft YaHei".to_string(),
        };

        // 剪裁文本区域，防止绘制到标题栏或图片区域外
        let _clip_rect = crate::platform::traits::Rectangle {
            x: text_rect.left as f32,
            y: text_rect.top as f32,
            width: (text_rect.right - text_rect.left) as f32,
            height: (text_rect.bottom - text_rect.top) as f32,
        };

        // 设置裁剪
        // 注意：d2d_renderer 需要暴露 push_clip_rect/pop_clip_rect 或类似的接口
        // 这里暂时不裁剪，依靠y坐标判断是否绘制

        let start_y = text_rect.top as f32 - scroll_offset as f32;

        // 预计算选择范围
        let (start_sel, end_sel) = if let Some((s, e)) = selection {
            if s <= e { (s, e) } else { (e, s) }
        } else {
            ((usize::MAX, 0), (usize::MAX, 0))
        };

        for (i, line) in text_lines.iter().enumerate() {
            let line_y = start_y + (i as f32 * line_height as f32);

            // 优化：只绘制可见区域内的行
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

                // 计算高亮行的起始和结束字符索引
                let start_char = if i == start_sel.0 { start_sel.1 } else { 0 };
                let end_char = if i == end_sel.0 {
                    end_sel.1
                } else {
                    line.chars().count()
                };

                if start_char < end_char {
                    // 测量前缀宽度 (start_char之前)
                    let prefix = line.chars().take(start_char).collect::<String>();
                    let (prefix_width, _) = self
                        .d2d_renderer
                        .measure_text_layout_size(&prefix, 10000.0, &text_style)
                        .unwrap_or((0.0, 0.0));

                    // 测量选中部分宽度
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

                    // 绘制高亮矩形
                    let highlight_color = crate::platform::traits::Color {
                        r: 0.78,
                        g: 0.97,
                        b: 0.77,
                        a: 1.0,
                    }; // #C8F7C5
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

        self.end_frame()?;
        Ok(())
    }
}

// SVG 图标结构 - 简化版，只保存位置和状态信息
#[derive(Clone)]
struct SvgIcon {
    name: String,
    rect: RECT,
    hovered: bool,
    is_title_bar_button: bool, // 是否是标题栏按钮
}

// DWM边距结构
#[repr(C)]
#[derive(Clone, Copy)]
#[allow(non_snake_case)]
struct MARGINS {
    cxLeftWidth: i32,
    cxRightWidth: i32,
    cyTopHeight: i32,
    cyBottomHeight: i32,
}

/// OCR 结果显示窗口
pub struct OcrResultWindow {
    hwnd: HWND,
    // 原始图像数据，用于D2D位图创建
    image_pixels: Vec<u8>,
    image_width: i32,
    image_height: i32,
    text_area_rect: RECT,    // 文字显示区域
    window_width: i32,       // 窗口宽度
    window_height: i32,      // 窗口高度
    is_maximized: bool,      // 是否最大化
    svg_icons: Vec<SvgIcon>, // SVG 图标列表（包括左侧图标和标题栏按钮）

    // 自绘文本相关
    text_content: String,    // 文本内容
    scroll_offset: i32,      // 垂直滚动偏移量
    line_height: i32,        // 行高
    text_lines: Vec<String>, // 分行后的文本

    // 文本选择相关
    is_selecting: bool,                        // 是否正在选择文本
    selection_start: Option<(usize, usize)>,   // 选择开始位置 (行号, 字符位置)
    selection_end: Option<(usize, usize)>,     // 选择结束位置 (行号, 字符位置)
    selection_start_pixel: Option<(i32, i32)>, // 选择开始的像素位置
    selection_end_pixel: Option<(i32, i32)>,   // 选择结束的像素位置
    last_click_time: std::time::Instant,       // 上次点击时间，用于双击检测
    last_click_pos: Option<(i32, i32)>,        // 上次点击位置

    // 置顶/Pin 状态
    is_pinned: bool, // 是否置顶

    // Direct2D 渲染器
    renderer: Option<OcrResultRenderer>,
}
impl OcrResultWindow {
    // 获取窗口边框厚度
    fn get_frame_thickness(hwnd: HWND) -> i32 {
        unsafe {
            let dpi = GetDpiForWindow(hwnd);
            let resize_frame = GetSystemMetricsForDpi(SM_CXSIZEFRAME, dpi);
            let padding = GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi);
            resize_frame + padding
        }
    }

    /// 清理所有资源
    fn cleanup_all_resources(&mut self) {
        // 清理SVG图标列表
        self.svg_icons.clear();
    }

    /// 创建左侧图标（仅位置和名称）
    fn create_left_icons() -> Vec<SvgIcon> {
        let mut icons = Vec::new();

        let icon_x = ICON_START_X;
        let icon_y = (TITLE_BAR_HEIGHT - ICON_SIZE) / 2;

        icons.push(SvgIcon {
            name: "pin".to_string(),
            rect: RECT {
                left: icon_x,
                top: icon_y,
                right: icon_x + ICON_SIZE,
                bottom: icon_y + ICON_SIZE,
            },
            hovered: false,
            is_title_bar_button: false,
        });

        icons
    }

    /// 调整图标位置使其居中（简化版本，参考test.rs）
    fn center_icons(&mut self) {
        // 重新计算图标位置，确保在标题栏中居中
        for icon in &mut self.svg_icons {
            if !icon.is_title_bar_button {
                // 对于左侧图标，重新计算Y位置使其居中
                let icon_y = (TITLE_BAR_HEIGHT - ICON_SIZE) / 2;
                let icon_height = icon.rect.bottom - icon.rect.top;
                icon.rect.top = icon_y;
                icon.rect.bottom = icon.rect.top + icon_height;
            } else {
                // 对于标题栏按钮，也重新计算Y位置
                let icon_y = (TITLE_BAR_HEIGHT - ICON_SIZE) / 2;
                let icon_height = icon.rect.bottom - icon.rect.top;
                icon.rect.top = icon_y;
                icon.rect.bottom = icon.rect.top + icon_height;
            }
        }
    }

    /// 更新标题栏按钮状态
    fn update_title_bar_buttons(&mut self) {
        let window_width = self.window_width;

        // 移除旧的标题栏按钮，保留左侧图标
        self.svg_icons.retain(|icon| !icon.is_title_bar_button);

        // 确保有左侧图标并更新它们的位置
        let has_left_icons = self.svg_icons.iter().any(|icon| !icon.is_title_bar_button);
        if !has_left_icons {
            let mut left_icons = Self::create_left_icons();
            self.svg_icons.append(&mut left_icons);
        }

        // 更新所有左侧图标的位置
        for icon in &mut self.svg_icons {
            if !icon.is_title_bar_button {
                // 左侧图标在标题栏内垂直居中
                let new_y = (TITLE_BAR_HEIGHT - ICON_SIZE) / 2;
                let icon_height = icon.rect.bottom - icon.rect.top;
                icon.rect.top = new_y;
                icon.rect.bottom = new_y + icon_height;
            }
        }

        // 创建新的标题栏按钮
        let mut title_bar_buttons = self.create_title_bar_buttons(window_width, self.is_maximized);
        self.svg_icons.append(&mut title_bar_buttons);
    }

    /// 创建标题栏按钮
    fn create_title_bar_buttons(&self, window_width: i32, is_maximized: bool) -> Vec<SvgIcon> {
        let mut buttons = Vec::new();

        // 根据窗口状态选择按钮配置
        let button_names = if is_maximized {
            // 最大化状态：关闭、还原、最小化（从右到左）
            vec!["window-close", "window-restore", "window-minimize"]
        } else {
            // 普通状态：关闭、最大化、最小化（从右到左）
            vec!["window-close", "window-maximize", "window-minimize"]
        };

        // 从右到左创建按钮
        for (i, name) in button_names.iter().enumerate() {
            // 按钮位置计算
            let button_x = window_width - (i as i32 + 1) * BUTTON_WIDTH_OCR;
            let icon_x = button_x + (BUTTON_WIDTH_OCR - ICON_SIZE) / 2;
            let icon_y = (TITLE_BAR_HEIGHT - ICON_SIZE) / 2;

            buttons.push(SvgIcon {
                name: name.to_string(),
                rect: RECT {
                    left: icon_x,
                    top: icon_y,
                    right: icon_x + ICON_SIZE,
                    bottom: icon_y + ICON_SIZE,
                },
                hovered: false,
                is_title_bar_button: true,
            });
        }

        buttons
    }

    /// 重新计算窗口布局
    fn recalculate_layout(&mut self) {
        // 右边文字区域宽度（固定350像素）
        let text_area_width = 350;

        // 左边图像区域宽度
        // 注意：这里的image_area_width是为图像预留的区域宽度，不是图像实际显示宽度
        let image_area_width = self.window_width - text_area_width - 20; // 减去中间分隔20像素

        // 计算文字显示区域（简化版本）
        let title_bar_height = TITLE_BAR_HEIGHT;
        let text_padding_left = 20;
        let text_padding_right = 20;
        let text_padding_top = title_bar_height + 15;
        let text_padding_bottom = 15;

        let new_text_area_rect = RECT {
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
                // 确保渲染器已初始化
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
            unsafe {
                let _ = InvalidateRect(Some(self.hwnd), Some(&self.text_area_rect), false);
            }
        }
    }

    pub fn show(
        image_data: Vec<u8>,
        ocr_results: Vec<OcrResult>,
        selection_rect: RECT,
    ) -> Result<()> {
        unsafe {
            // 1. DPI 设置
            let _ = SetProcessDpiAwareness(PROCESS_PER_MONITOR_DPI_AWARE);

            // 2. 注册窗口类
            let class_name = windows::core::w!("OcrResultWindow");
            let instance = windows::Win32::System::LibraryLoader::GetModuleHandleW(None)?;

            let window_class = WNDCLASSW {
                lpfnWndProc: Some(Self::window_proc),
                hInstance: instance.into(),
                lpszClassName: class_name,
                hCursor: LoadCursorW(None, IDC_ARROW)?,
                // 关键：使用黑色背景刷，防止调整大小时出现白色闪烁
                hbrBackground: CreateSolidBrush(COLORREF(0)),
                style: CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS,
                hIcon: HICON::default(),
                ..Default::default()
            };

            RegisterClassW(&window_class);

            // 3. 解析图片与计算尺寸
            let (image_pixels, actual_width, actual_height) = Self::parse_bmp_data(&image_data)?;

            // 布局计算：完全不考虑系统边框高度，只计算我们需要的高度
            let text_area_width = 350;
            let image_area_width = actual_width + 40;
            let window_width = image_area_width + text_area_width + 20;

            let content_padding_top = 20;
            let content_padding_bottom = 20;
            // 窗口高度 = 自定义标题栏 + 边距 + 内容
            let window_height =
                TITLE_BAR_HEIGHT + content_padding_top + actual_height + content_padding_bottom;

            // 4. 计算位置
            let (screen_width, screen_height) = crate::platform::windows::system::get_screen_size();
            let mut window_x = selection_rect.right + 20;
            let mut window_y = selection_rect.top;

            if window_x + window_width > screen_width {
                window_x = selection_rect.left - window_width - 20;
                if window_x < 0 {
                    window_x = 50;
                }
            }
            if window_y + window_height > screen_height {
                window_y = screen_height - window_height - 50;
                if window_y < 0 {
                    window_y = 50;
                }
            }

            // 5. 创建窗口 [Zed 风格核心样式]
            // WS_POPUP 是最纯净的无边框，但 WS_THICKFRAME 让我们可以利用系统的拖拽调整大小
            // 这里我们手动组合样式，确保没有 WS_CAPTION
            let dw_style = WS_THICKFRAME
                | WS_SYSMENU
                | WS_MAXIMIZEBOX
                | WS_MINIMIZEBOX
                | WS_VISIBLE
                | WS_CLIPCHILDREN;
            let dw_ex_style = WS_EX_APPWINDOW; // 不使用 NOREDIRECTIONBITMAP，以兼容你的 D2D 写法

            let hwnd = CreateWindowExW(
                dw_ex_style,
                class_name,
                windows::core::w!("识别结果"),
                dw_style,
                window_x,
                window_y,
                window_width,
                window_height,
                None,
                None,
                Some(instance.into()),
                None,
            )?;

            // 6. DWM 设置 [修复圆角和边框的关键]

            // 启用沉浸式暗色模式 (让窗口阴影变暗)
            let dark_mode = 1 as i32;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWINDOWATTRIBUTE(20), // DWMWA_USE_IMMERSIVE_DARK_MODE
                &dark_mode as *const _ as *const _,
                std::mem::size_of::<i32>() as u32,
            );

            // [修复圆角] 显式开启 Windows 11 圆角
            // DWMWA_WINDOW_CORNER_PREFERENCE = 33, DWMWCP_ROUND = 2
            let round_preference = 2 as i32;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWINDOWATTRIBUTE(33),
                &round_preference as *const _ as *const _,
                std::mem::size_of::<i32>() as u32,
            );

            // [修复白色边框] 扩展 Frame，让 DWM 绘制阴影，但内容区覆盖边框
            // 只要 WM_NCCALCSIZE 返回 0，这里设为 -1 就会产生完美的无边框阴影效果
            let margins = MARGINS {
                cxLeftWidth: -1,
                cxRightWidth: -1,
                cyTopHeight: -1,
                cyBottomHeight: -1,
            };
            let _ = DwmExtendFrameIntoClientArea(hwnd, &margins as *const MARGINS as *const _);

            // 触发一次 Frame 改变，强制系统重新计算非客户区
            SetWindowPos(
                hwnd,
                None,
                0,
                0,
                0,
                0,
                SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            )?;

            // 7. 初始化逻辑 (保持不变)
            let text_padding_left = 20;
            let text_padding_top = TITLE_BAR_HEIGHT + 15;
            let text_padding_right = 20;
            let text_padding_bottom = 15;

            let text_area_rect = RECT {
                left: image_area_width + text_padding_left,
                top: text_padding_top,
                right: window_width - text_padding_right,
                bottom: window_height - text_padding_bottom,
            };

            let mut all_text = String::new();
            for (i, result) in ocr_results.iter().enumerate() {
                if i > 0 {
                    all_text.push_str("\r\n");
                }
                if result.text == "未识别到任何文字" && result.confidence == 0.0 {
                    all_text.push_str("未识别到任何文字");
                } else {
                    all_text.push_str(&result.text);
                }
            }
            if all_text.trim().is_empty() {
                all_text = "未识别到文本内容".to_string();
            }

            let text_lines: Vec<String> = all_text.lines().map(|s| s.to_string()).collect();

            let svg_icons = Self::create_left_icons();
            let mut window = Self {
                hwnd,
                image_pixels,
                image_width: actual_width,
                image_height: actual_height,
                text_area_rect,
                window_width,
                window_height,
                is_maximized: false,
                svg_icons,
                is_pinned: false,
                text_content: all_text,
                scroll_offset: 0,
                line_height: 24,
                text_lines,
                is_selecting: false,
                selection_start: None,
                selection_end: None,
                selection_start_pixel: None,
                selection_end_pixel: None,
                last_click_time: std::time::Instant::now(),
                last_click_pos: None,
                renderer: OcrResultRenderer::new().ok(),
            };

            let mut title_bar_buttons = window.create_title_bar_buttons(window_width, false);
            window.svg_icons.append(&mut title_bar_buttons);

            let window_ptr = Box::into_raw(Box::new(window));
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, window_ptr as isize);

            let window = &mut *window_ptr;
            let mut client_rect = RECT::default();
            let _ = GetClientRect(hwnd, &mut client_rect);
            let actual_client_width = client_rect.right - client_rect.left;
            let actual_client_height = client_rect.bottom - client_rect.top;

            // 安全检查：如果获取失败（极少见），回退到 window_width
            let safe_width = if actual_client_width > 0 {
                actual_client_width
            } else {
                window_width
            };
            let safe_height = if actual_client_height > 0 {
                actual_client_height
            } else {
                window_height
            };

            // 初始化渲染器
            if let Some(renderer) = &mut window.renderer {
                let _ = renderer.initialize(hwnd, safe_width, safe_height);
            }

            window.window_width = actual_client_width;
            window.window_height = actual_client_height;
            window.recalculate_layout();
            window.update_title_bar_buttons();

            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = UpdateWindow(hwnd);

            Ok(())
        }
    }
    /// 从 BMP 数据解析像素数据 (RGBA)
    fn parse_bmp_data(bmp_data: &[u8]) -> Result<(Vec<u8>, i32, i32)> {
        if bmp_data.len() < 54 {
            return Err(anyhow::anyhow!("BMP 数据太小"));
        }

        // 检查BMP文件签名
        if bmp_data[0] != b'B' || bmp_data[1] != b'M' {
            return Err(anyhow::anyhow!("不是有效的BMP文件"));
        }

        // 读取数据偏移量
        let data_offset =
            u32::from_le_bytes([bmp_data[10], bmp_data[11], bmp_data[12], bmp_data[13]]) as usize;

        // 解析 BMP 头部获取尺寸信息
        let width = i32::from_le_bytes([bmp_data[18], bmp_data[19], bmp_data[20], bmp_data[21]]);
        let height_raw =
            i32::from_le_bytes([bmp_data[22], bmp_data[23], bmp_data[24], bmp_data[25]]);
        let height = height_raw.abs(); // 取绝对值
        let is_top_down = height_raw < 0; // 负值表示自顶向下

        // 读取位深度
        let bit_count = u16::from_le_bytes([bmp_data[28], bmp_data[29]]);

        // 检查数据偏移量是否有效
        if data_offset >= bmp_data.len() {
            return Err(anyhow::anyhow!("BMP数据偏移量无效"));
        }

        // 获取像素数据
        let pixel_data = &bmp_data[data_offset..];

        // 计算每行的字节数（考虑4字节对齐）
        let bytes_per_pixel = (bit_count / 8) as usize;
        let row_size = (width as usize * bytes_per_pixel).div_ceil(4) * 4; // 4字节对齐

        let mut rgba_pixels = vec![0u8; (width * height * 4) as usize];

        // 逐行复制和转换像素数据
        for y in 0..height {
            // 计算源数据中的实际行索引
            let src_y = if is_top_down {
                y // 自顶向下，行索引不变
            } else {
                height - 1 - y // 自底向上，需要翻转行索引
            };

            let src_row_start = src_y as usize * row_size;

            for x in 0..width {
                let src_idx = src_row_start + x as usize * bytes_per_pixel;
                let dst_idx = (y * width + x) as usize * 4;

                if src_idx + bytes_per_pixel <= pixel_data.len() && dst_idx + 3 < rgba_pixels.len()
                {
                    match bit_count {
                        24 => {
                            // 24位BMP: BGR -> RGBA
                            rgba_pixels[dst_idx] = pixel_data[src_idx + 2]; // R
                            rgba_pixels[dst_idx + 1] = pixel_data[src_idx + 1]; // G
                            rgba_pixels[dst_idx + 2] = pixel_data[src_idx]; // B
                            rgba_pixels[dst_idx + 3] = 255; // A - 不透明
                        }
                        32 => {
                            // 32位BMP: BGRA -> RGBA
                            rgba_pixels[dst_idx] = pixel_data[src_idx + 2]; // R
                            rgba_pixels[dst_idx + 1] = pixel_data[src_idx + 1]; // G
                            rgba_pixels[dst_idx + 2] = pixel_data[src_idx]; // B
                            rgba_pixels[dst_idx + 3] = 255; // A
                        }
                        _ => {
                            // 其他格式，设置为白色
                            rgba_pixels[dst_idx] = 255; // R
                            rgba_pixels[dst_idx + 1] = 255; // G
                            rgba_pixels[dst_idx + 2] = 255; // B
                            rgba_pixels[dst_idx + 3] = 255; // A
                        }
                    }
                }
            }
        }

        Ok((rgba_pixels, width, height))
    }

    /// 处理窗口大小变化，使用缓冲机制减少闪烁
    fn handle_size_change(&mut self, new_width: i32, new_height: i32) {
        // 直接更新窗口尺寸
        self.window_width = new_width;
        self.window_height = new_height;

        // 重新计算布局（包括文本编辑控件的重新定位）
        self.recalculate_layout();
    }

    /// 自定义标题栏处理函数（根据官方文档重构）
    fn custom_caption_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        unsafe {
            match msg {
                WM_ERASEBKGND => {
                    // 阻止默认的背景擦除，减少闪烁
                    // 我们在 WM_PAINT 中自己处理背景绘制
                    LRESULT(1) // 返回非零值表示已处理
                }
                WM_SIZE => {
                    // 处理窗口大小变化
                    let new_width = (lparam.0 & 0xFFFF) as i32;
                    let new_height = ((lparam.0 >> 16) & 0xFFFF) as i32;

                    let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Self;
                    if !window_ptr.is_null() {
                        let window = &mut *window_ptr;

                        // 更新最大化状态
                        if wparam.0 == SIZE_MAXIMIZED as usize {
                            window.is_maximized = true;
                        } else if wparam.0 == SIZE_RESTORED as usize {
                            window.is_maximized = false;
                        }

                        window.handle_size_change(new_width, new_height);

                        // 状态改变后更新按钮
                        window.update_title_bar_buttons();
                    }
                    LRESULT(0)
                }
                WM_PAINT => {
                    // 如果有 Direct2D 渲染器，尝试使用它
                    let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Self;

                    if !window_ptr.is_null() {
                        let window = &mut *window_ptr;
                        if let Some(renderer) = &mut window.renderer {
                            // 尝试初始化渲染器（如果尚未初始化）
                            let mut rect = RECT::default();
                            let _ = GetClientRect(hwnd, &mut rect);
                            let width = rect.right - rect.left;
                            let height = rect.bottom - rect.top;

                            if width > 0 && height > 0 {
                                // 确保初始化
                                let _ = renderer.initialize(hwnd, width, height);

                                // 确保图片已加载
                                let _ = renderer.set_image_from_pixels(
                                    &window.image_pixels,
                                    window.image_width,
                                    window.image_height,
                                );

                                // 尝试渲染
                                let selection = if let (Some(start), Some(end)) =
                                    (window.selection_start, window.selection_end)
                                {
                                    Some((start, end))
                                } else {
                                    None
                                };

                                if renderer
                                    .render(
                                        &window.text_lines,
                                        window.text_area_rect,
                                        width,
                                        &window.svg_icons,
                                        window.is_pinned,
                                        window.is_maximized,
                                        window.scroll_offset,
                                        window.line_height,
                                        window.image_width,
                                        window.image_height,
                                        selection,
                                    )
                                    .is_ok()
                                {
                                    // 验证更新区域，防止无限重绘
                                    let _ = ValidateRect(Some(hwnd), None);
                                }
                            }
                        }
                    }
                    LRESULT(0)
                }
                WM_NCCALCSIZE => {
                    if wparam.0 == 1 {
                        let params = lparam.0 as *mut NCCALCSIZE_PARAMS;
                        if !params.is_null() {
                            let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
                            let is_maximized = (style & WS_MAXIMIZE.0) != 0;

                            if is_maximized {
                                // 最大化时，手动减去边框，防止内容被切掉
                                let frame_thickness = Self::get_frame_thickness(hwnd);
                                let rgrc = &mut (*params).rgrc;
                                rgrc[0].top += frame_thickness;
                                rgrc[0].bottom -= frame_thickness;
                                rgrc[0].left += frame_thickness;
                                rgrc[0].right -= frame_thickness;
                            }
                            // 窗口模式下不做任何修改，让 客户区 = 窗口区
                        }
                        return LRESULT(0);
                    }
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }
                WM_NCHITTEST => Self::hit_test_nca(hwnd, wparam, lparam),
                _ => {
                    // 其他消息交给应用程序处理
                    Self::app_window_proc(hwnd, msg, wparam, lparam)
                }
            }
        }
    }

    /// 检查点是否在文本区域内
    fn is_point_in_text_area(&self, x: i32, y: i32) -> bool {
        x >= self.text_area_rect.left
            && x <= self.text_area_rect.right
            && y >= self.text_area_rect.top
            && y <= self.text_area_rect.bottom
    }

    /// 获取调整后的文本区域重绘矩形，避免全屏时的闪烁
    fn get_adjusted_text_rect_for_redraw(&self) -> RECT {
        unsafe {
            // 检查是否全屏
            let _is_fullscreen = {
                let mut window_rect = RECT::default();
                let _ = GetWindowRect(self.hwnd, &mut window_rect);
                let (screen_width, screen_height) =
                    crate::platform::windows::system::get_screen_size();

                window_rect.left <= 0
                    && window_rect.top <= 0
                    && window_rect.right >= screen_width
                    && window_rect.bottom >= screen_height
            };

            // 简化版本（参考test.rs，不使用偏移）
            let actual_content_start = TITLE_BAR_HEIGHT;

            // 调整重绘区域，确保从实际内容开始位置开始，避免包含标题栏和间隙区域
            RECT {
                left: self.text_area_rect.left,
                top: self.text_area_rect.top.max(actual_content_start),
                right: self.text_area_rect.right,
                bottom: self.text_area_rect.bottom,
            }
        }
    }

    /// 检测是否为双击
    fn is_double_click(&mut self, x: i32, y: i32) -> bool {
        let now = std::time::Instant::now();
        let double_click_time = std::time::Duration::from_millis(500); // 500ms内算双击
        let double_click_distance = 5; // 5像素内算同一位置

        if let Some((last_x, last_y)) = self.last_click_pos {
            let time_diff = now.duration_since(self.last_click_time);
            let distance = ((x - last_x).pow(2) + (y - last_y).pow(2)) as f32;
            let distance = distance.sqrt() as i32;

            if time_diff <= double_click_time && distance <= double_click_distance {
                return true;
            }
        }

        self.last_click_time = now;
        self.last_click_pos = Some((x, y));
        false
    }

    /// 全选所有文本
    fn select_all_text(&mut self) {
        if self.text_lines.is_empty() {
            return;
        }

        // 选择从第一行第一个字符到最后一行最后一个字符
        self.selection_start = Some((0, 0));
        let last_line_idx = self.text_lines.len() - 1;
        let last_char_idx = self.text_lines[last_line_idx].chars().count();
        self.selection_end = Some((last_line_idx, last_char_idx));

        // 设置像素坐标（可选，主要用于显示）
        self.selection_start_pixel = Some((self.text_area_rect.left, self.text_area_rect.top));
        self.selection_end_pixel = Some((self.text_area_rect.right, self.text_area_rect.bottom));

        // 使缓冲区失效并重绘
        unsafe {
            let adjusted_rect = self.get_adjusted_text_rect_for_redraw();
            let _ = InvalidateRect(Some(self.hwnd), Some(&adjusted_rect), false);
            let _ = UpdateWindow(self.hwnd);
        }
    }

    /// 开始文本选择
    fn start_text_selection(&mut self, x: i32, y: i32) {
        // 检测双击
        if self.is_double_click(x, y) {
            self.select_all_text();
            return;
        }

        // 清除之前的选择
        self.selection_start = None;
        self.selection_end = None;
        self.selection_start_pixel = Some((x, y));
        self.selection_end_pixel = Some((x, y));
        self.is_selecting = true;

        // 将像素坐标转换为文本位置
        if let Some(text_pos) = self.pixel_to_text_position(x, y) {
            self.selection_start = Some(text_pos);
            self.selection_end = Some(text_pos);

            // 使缓冲区失效并立即触发重绘以显示选择高亮
            unsafe {
                let adjusted_rect = self.get_adjusted_text_rect_for_redraw();
                let _ = InvalidateRect(Some(self.hwnd), Some(&adjusted_rect), false);
                let _ = UpdateWindow(self.hwnd); // 强制立即重绘
            }
        }
    }

    /// 更新文本选择
    fn update_text_selection(&mut self, x: i32, y: i32) {
        if !self.is_selecting {
            return;
        }

        self.selection_end_pixel = Some((x, y));

        // 将像素坐标转换为文本位置
        if let Some(text_pos) = self.pixel_to_text_position(x, y) {
            // 只有当选择位置真正改变时才更新和重绘
            if self.selection_end != Some(text_pos) {
                self.selection_end = Some(text_pos);

                // 使缓冲区失效并立即重绘文本区域以显示选择更新
                unsafe {
                    let adjusted_rect = self.get_adjusted_text_rect_for_redraw();
                    let _ = InvalidateRect(Some(self.hwnd), Some(&adjusted_rect), false);
                    let _ = UpdateWindow(self.hwnd); // 强制立即重绘
                }
            }
        }
    }

    /// 复制选中的文本到剪贴板
    fn copy_selected_text_to_clipboard(&self) {
        let selected_text = match self.get_selected_text() {
            Some(text) if !text.is_empty() => text,
            _ => return,
        };

        unsafe {
            // 打开剪贴板
            if OpenClipboard(Some(self.hwnd)).is_ok() {
                // 清空剪贴板
                let _ = EmptyClipboard();

                // 将文本转换为UTF-16
                let wide_text: Vec<u16> = selected_text
                    .encode_utf16()
                    .chain(std::iter::once(0))
                    .collect();

                // 分配全局内存
                if let Ok(h_mem) = GlobalAlloc(GLOBAL_ALLOC_FLAGS(0x0002), wide_text.len() * 2) {
                    let p_mem = GlobalLock(h_mem);
                    if !p_mem.is_null() {
                        // 复制文本到全局内存
                        std::ptr::copy_nonoverlapping(
                            wide_text.as_ptr(),
                            p_mem as *mut u16,
                            wide_text.len(),
                        );
                        let _ = GlobalUnlock(h_mem);

                        // 设置剪贴板数据
                        let _ = SetClipboardData(13u32, Some(HANDLE(h_mem.0))); // CF_UNICODETEXT = 13
                    }
                }

                // 关闭剪贴板
                let _ = CloseClipboard();
            }
        }
    }

    /// 结束文本选择
    fn end_text_selection(&mut self) {
        self.is_selecting = false;

        // 自动复制选中的文本到剪贴板
        self.copy_selected_text_to_clipboard();

        // 使缓冲区失效并重绘以清除选择高亮
        unsafe {
            let adjusted_rect = self.get_adjusted_text_rect_for_redraw();
            let _ = InvalidateRect(Some(self.hwnd), Some(&adjusted_rect), false);
            let _ = UpdateWindow(self.hwnd);
        }
    }

    /// 将像素坐标转换为文本位置 (行号, 字符位置)
    fn pixel_to_text_position(&self, x: i32, y: i32) -> Option<(usize, usize)> {
        // 计算相对于文本区域的坐标，使用与绘制时相同的坐标系
        let relative_x = x - self.text_area_rect.left - 10; // 减去左边距
        let relative_y = y - self.text_area_rect.top + self.scroll_offset - 10; // 减去上边距，加上滚动偏移

        // 计算行号
        let line_index = (relative_y / self.line_height).max(0) as usize;

        if line_index >= self.text_lines.len() {
            // 超出文本范围，返回最后一行的末尾
            if let Some(last_line) = self.text_lines.last() {
                return Some((self.text_lines.len() - 1, last_line.chars().count()));
            }
            return None;
        }

        // 获取当前行的文本
        let line = &self.text_lines[line_index];
        if line.is_empty() {
            return Some((line_index, 0));
        }

        let best_char_index = if let Some(renderer) = &self.renderer {
            renderer.get_text_position_from_point(line, relative_x as f32)
        } else {
            0
        };

        Some((line_index, best_char_index))
    }

    /// 获取选中的文本
    fn get_selected_text(&self) -> Option<String> {
        if let (Some(start), Some(end)) = (self.selection_start, self.selection_end) {
            let (start_line, start_char) = start;
            let (end_line, end_char) = end;

            // 确保开始位置在结束位置之前
            let (start_line, start_char, end_line, end_char) =
                if start_line > end_line || (start_line == end_line && start_char > end_char) {
                    (end_line, end_char, start_line, start_char)
                } else {
                    (start_line, start_char, end_line, end_char)
                };

            let mut selected_text = String::new();

            if start_line == end_line {
                // 同一行内的选择
                if let Some(line) = self.text_lines.get(start_line) {
                    let chars: Vec<char> = line.chars().collect();
                    let start_idx = start_char.min(chars.len());
                    let end_idx = end_char.min(chars.len());
                    selected_text = chars[start_idx..end_idx].iter().collect();
                }
            } else {
                // 跨行选择
                for line_idx in start_line..=end_line {
                    if let Some(line) = self.text_lines.get(line_idx) {
                        let chars: Vec<char> = line.chars().collect();

                        if line_idx == start_line {
                            // 第一行：从开始字符到行尾
                            let start_idx = start_char.min(chars.len());
                            let line_part: String = chars[start_idx..].iter().collect();
                            selected_text.push_str(&line_part);
                        } else if line_idx == end_line {
                            // 最后一行：从行首到结束字符
                            let end_idx = end_char.min(chars.len());
                            let line_part: String = chars[..end_idx].iter().collect();
                            selected_text.push_str(&line_part);
                        } else {
                            // 中间行：整行
                            selected_text.push_str(line);
                        }

                        // 除了最后一行，都添加换行符
                        if line_idx < end_line {
                            selected_text.push('\n');
                        }
                    }
                }
            }

            if !selected_text.is_empty() {
                Some(selected_text)
            } else {
                None
            }
        } else {
            None
        }
    }

    /// 自定义点击测试
    fn hit_test_nca(hwnd: HWND, _wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        unsafe {
            // 获取鼠标坐标
            let pt_mouse_x = (lparam.0 as i16) as i32;
            let pt_mouse_y = ((lparam.0 >> 16) as i16) as i32;

            // 获取窗口矩形
            let mut rc_window = RECT::default();
            let _ = GetWindowRect(hwnd, &mut rc_window);

            // 检查窗口是否真正全屏
            let _is_fullscreen = {
                let (screen_width, screen_height) =
                    crate::platform::windows::system::get_screen_size();

                rc_window.left <= 0
                    && rc_window.top <= 0
                    && rc_window.right >= screen_width
                    && rc_window.bottom >= screen_height
            };

            // 简化版本（参考test.rs，不使用偏移）

            // 转换为客户端坐标
            let client_x = pt_mouse_x - rc_window.left;
            let client_y = pt_mouse_y - rc_window.top;

            // 获取窗口实例来检查按钮和图标区域
            let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Self;
            if !window_ptr.is_null() {
                let window = &*window_ptr;

                // 检查是否在标题栏区域（参考test.rs的简洁方式）
                if (0..TITLE_BAR_HEIGHT).contains(&client_y) {
                    for icon in &window.svg_icons {
                        let in_click_area = if icon.is_title_bar_button {
                            // 标题栏按钮：使用按钮的完整宽度区域进行点击检测
                            let button_width = BUTTON_WIDTH_OCR;
                            let button_left = icon.rect.left - (button_width - ICON_SIZE) / 2;
                            let button_right = button_left + button_width;
                            client_x >= button_left
                                && client_x <= button_right
                                && (0..=TITLE_BAR_HEIGHT).contains(&client_y)
                        } else {
                            // 普通图标：使用图标矩形区域加padding
                            let click_padding = ICON_CLICK_PADDING;
                            client_x >= (icon.rect.left - click_padding)
                                && client_x <= (icon.rect.right + click_padding)
                                && client_y >= (icon.rect.top - click_padding)
                                && client_y <= (icon.rect.bottom + click_padding)
                        };

                        if in_click_area {
                            return LRESULT(HTCLIENT as isize); // 让图标可以响应点击和悬停
                        }
                    }
                }
            }

            // 获取框架矩形，调整为没有标题栏的样式
            let mut rc_frame = RECT::default();
            let _ = AdjustWindowRectEx(
                &mut rc_frame,
                WS_OVERLAPPEDWINDOW & !WS_CAPTION,
                false,
                WS_EX_OVERLAPPEDWINDOW,
            );

            // 确定是否在调整大小边框上，默认中间(1,1)
            let mut u_row = 1;
            let mut u_col = 1;
            let mut f_on_resize_border = false;

            // 确定点是否在窗口的顶部或底部
            if pt_mouse_y >= rc_window.top && pt_mouse_y < rc_window.top + TITLE_BAR_HEIGHT {
                f_on_resize_border = pt_mouse_y < (rc_window.top - rc_frame.top);
                u_row = 0;
            } else if pt_mouse_y < rc_window.bottom && pt_mouse_y >= rc_window.bottom - 5
            // 使用固定的5像素边框
            {
                u_row = 2;
            }

            // 确定点是否在窗口的左侧或右侧 - 使用固定的5像素边框
            if pt_mouse_x >= rc_window.left && pt_mouse_x < rc_window.left + 5 {
                u_col = 0; // 左侧
            } else if pt_mouse_x < rc_window.right && pt_mouse_x >= rc_window.right - 5 {
                u_col = 2; // 右侧
            }

            // 点击测试矩阵
            let hit_tests = [
                [
                    HTTOPLEFT as isize,
                    if f_on_resize_border {
                        HTTOP as isize
                    } else {
                        HTCAPTION as isize
                    },
                    HTTOPRIGHT as isize,
                ],
                [HTLEFT as isize, HTCLIENT as isize, HTRIGHT as isize],
                [
                    HTBOTTOMLEFT as isize,
                    HTBOTTOM as isize,
                    HTBOTTOMRIGHT as isize,
                ],
            ];

            let result = hit_tests[u_row][u_col];

            LRESULT(result)
        }
    }

    /// 窗口过程
    unsafe extern "system" fn window_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        unsafe {
            // 先处理鼠标消息，不让DWM拦截
            if msg == WM_LBUTTONDOWN || msg == WM_LBUTTONUP || msg == WM_MOUSEMOVE {
                let x = (lparam.0 as i16) as i32;
                let y = ((lparam.0 >> 16) as i16) as i32;

                // 处理点击事件
                if msg == WM_LBUTTONDOWN {
                    // 获取窗口信息
                    let window_ptr =
                        GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const OcrResultWindow;
                    if !window_ptr.is_null() {
                        let window = &*window_ptr;

                        // 检查点击是否在文本区域内
                        if x >= window.text_area_rect.left
                            && x <= window.text_area_rect.right
                            && y >= window.text_area_rect.top
                            && y <= window.text_area_rect.bottom
                        {
                            // 在文本区域内点击，处理文本选择
                        }
                    }
                }

                // 直接处理鼠标消息，不经过DWM
                let result = Self::custom_caption_proc(hwnd, msg, wparam, lparam);

                // 如果我们处理了消息，直接返回
                if result.0 == 0 && msg == WM_LBUTTONDOWN {
                    return result;
                }
            }

            // 检查DWM是否启用
            let dwm_enabled = DwmIsCompositionEnabled().unwrap_or(FALSE);

            if dwm_enabled.as_bool() {
                // 先让DWM处理消息
                let mut lret = LRESULT(0);
                let call_dwp = !DwmDefWindowProc(hwnd, msg, wparam, lparam, &mut lret).as_bool();

                // 如果DWM没有处理，我们来处理
                if call_dwp {
                    Self::custom_caption_proc(hwnd, msg, wparam, lparam)
                } else {
                    lret
                }
            } else {
                Self::app_window_proc(hwnd, msg, wparam, lparam)
            }
        }
    }

    /// 应用程序窗口过程
    fn app_window_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        unsafe {
            match msg {
                // 自绘无框：告诉系统整个区域是客户区，从而隐藏原生标题栏与边框（参考test.rs）
                WM_NCCALCSIZE => {
                    if wparam.0 == 1 {
                        return LRESULT(0);
                    }
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }
                // 自绘无框：拦截非客户区绘制/激活，避免系统重绘（参考test.rs）
                WM_NCPAINT => LRESULT(0),
                WM_NCACTIVATE => LRESULT(1),
                WM_GETMINMAXINFO => {
                    // 参考test.rs的完整实现，确保最大化时正确显示
                    let minmax_info = lparam.0 as *mut MINMAXINFO;
                    if !minmax_info.is_null() {
                        let info = &mut *minmax_info;

                        // 设置窗口最小尺寸为800*500
                        info.ptMinTrackSize.x = 800;
                        info.ptMinTrackSize.y = 500;

                        // 参考test.rs：设置最大化时的位置和大小为监视器的工作区
                        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
                        let mut monitor_info = MONITORINFO {
                            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                            ..Default::default()
                        };
                        let _ = GetMonitorInfoW(monitor, &mut monitor_info);

                        // 设置最大化时的位置和大小为监视器的工作区 (不包括任务栏)
                        info.ptMaxPosition = POINT {
                            x: monitor_info.rcWork.left,
                            y: monitor_info.rcWork.top,
                        };
                        info.ptMaxSize.x = monitor_info.rcWork.right - monitor_info.rcWork.left;
                        info.ptMaxSize.y = monitor_info.rcWork.bottom - monitor_info.rcWork.top;
                    }
                    LRESULT(0)
                }
                WM_LBUTTONDOWN => {
                    // 处理标题栏按钮和图标点击
                    let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Self;
                    if !window_ptr.is_null() {
                        let window = &mut *window_ptr;
                        let x = (lparam.0 as i16) as i32;
                        let y = ((lparam.0 >> 16) as i16) as i32;

                        // 检查窗口是否真正全屏
                        let _is_fullscreen = {
                            let mut window_rect = RECT::default();
                            let _ = GetWindowRect(hwnd, &mut window_rect);
                            let (screen_width, screen_height) =
                                crate::platform::windows::system::get_screen_size();

                            window_rect.left <= 0
                                && window_rect.top <= 0
                                && window_rect.right >= screen_width
                                && window_rect.bottom >= screen_height
                        };

                        // 简化版本（参考test.rs，不使用偏移）

                        // 处理SVG图标点击（包括标题栏按钮）
                        for icon in &mut window.svg_icons {
                            let in_click_area = if icon.is_title_bar_button {
                                // 标题栏按钮：使用按钮的完整宽度区域进行点击检测
                                let button_width = BUTTON_WIDTH_OCR;
                                let button_left = icon.rect.left - (button_width - ICON_SIZE) / 2;
                                let button_right = button_left + button_width;
                                x >= button_left
                                    && x <= button_right
                                    && (0..TITLE_BAR_HEIGHT).contains(&y)
                            } else {
                                // 普通图标：使用图标矩形区域加padding
                                let click_padding = ICON_CLICK_PADDING;
                                x >= (icon.rect.left - click_padding)
                                    && x <= (icon.rect.right + click_padding)
                                    && y >= (icon.rect.top - click_padding)
                                    && y <= (icon.rect.bottom + click_padding)
                            };

                            if in_click_area {
                                // 处理标题栏按钮点击
                                match icon.name.as_str() {
                                    "window-minimize" => {
                                        // 执行最小化
                                        let _ = ShowWindow(hwnd, SW_MINIMIZE);
                                        return LRESULT(0);
                                    }
                                    "pin" => {
                                        // 切换置顶
                                        window.is_pinned = !window.is_pinned;
                                        let _ = SetWindowPos(
                                            hwnd,
                                            if window.is_pinned {
                                                Some(HWND_TOPMOST)
                                            } else {
                                                Some(HWND_NOTOPMOST)
                                            },
                                            0,
                                            0,
                                            0,
                                            0,
                                            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                                        );
                                        let _ = InvalidateRect(Some(hwnd), None, false);
                                        return LRESULT(0);
                                    }
                                    "window-maximize" => {
                                        // 执行最大化
                                        let _ = ShowWindow(hwnd, SW_MAXIMIZE);
                                        // 状态更新由 WM_SIZE 处理
                                        return LRESULT(0);
                                    }
                                    "window-restore" => {
                                        // 执行还原
                                        let _ = ShowWindow(hwnd, SW_RESTORE);
                                        // 状态更新由 WM_SIZE 处理
                                        return LRESULT(0);
                                    }
                                    "window-close" => {
                                        // 执行关闭
                                        let _ = PostMessageW(
                                            Some(hwnd),
                                            WM_CLOSE,
                                            WPARAM(0),
                                            LPARAM(0),
                                        );
                                        return LRESULT(0);
                                    }
                                    _ => {
                                        // 处理其他SVG图标点击
                                        return LRESULT(0);
                                    }
                                }
                            }
                        }

                        // 如果没有点击图标，检查是否点击了文本区域
                        if window.is_point_in_text_area(x, y) {
                            // 需要获取可变引用来修改窗口状态
                            let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Self;
                            if !window_ptr.is_null() {
                                let window_mut = &mut *window_ptr;
                                window_mut.start_text_selection(x, y);
                                let _ = InvalidateRect(Some(hwnd), None, false);
                                return LRESULT(0);
                            }
                        }
                    }
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }
                WM_LBUTTONUP => {
                    // 处理SVG图标点击释放（包括标题栏按钮）
                    let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Self;
                    if !window_ptr.is_null() {
                        let window = &mut *window_ptr;
                        let x = (lparam.0 as i16) as i32;
                        let y = ((lparam.0 >> 16) as i16) as i32;

                        for icon in &mut window.svg_icons {
                            if x >= icon.rect.left
                                && x <= icon.rect.right
                                && y >= icon.rect.top
                                && y <= icon.rect.bottom
                            {
                                // 执行按钮操作
                                match icon.name.as_str() {
                                    _ => {
                                        // 处理其他图标点击
                                    }
                                }
                                let _ = InvalidateRect(Some(hwnd), None, false);
                                return LRESULT(0);
                            }
                        }

                        // 处理文本选择结束
                        if window.is_selecting {
                            // 需要获取可变引用来修改窗口状态
                            let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Self;
                            if !window_ptr.is_null() {
                                let window_mut = &mut *window_ptr;
                                window_mut.end_text_selection();

                                // 如果有选中的文本，复制到剪贴板
                                if let Some(_selected_text) = window_mut.get_selected_text() {
                                    // 文本已选中，可以进行复制操作
                                }

                                let _ = InvalidateRect(Some(hwnd), None, false);
                                return LRESULT(0);
                            }
                        }
                    }
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }
                WM_MOUSEMOVE => {
                    // 处理鼠标悬停效果
                    let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Self;
                    if !window_ptr.is_null() {
                        let window = &mut *window_ptr;
                        let x = (lparam.0 as i16) as i32;
                        let y = ((lparam.0 >> 16) as i16) as i32;

                        // 检查窗口是否真正全屏
                        let _is_fullscreen = {
                            let mut window_rect = RECT::default();
                            let _ = GetWindowRect(hwnd, &mut window_rect);
                            let (screen_width, screen_height) =
                                crate::platform::windows::system::get_screen_size();

                            window_rect.left <= 0
                                && window_rect.top <= 0
                                && window_rect.right >= screen_width
                                && window_rect.bottom >= screen_height
                        };

                        // 启用鼠标跟踪以确保接收到 WM_MOUSELEAVE 消息
                        let mut tme = TRACKMOUSEEVENT {
                            cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                            dwFlags: TME_LEAVE,
                            hwndTrack: hwnd,
                            dwHoverTime: 0,
                        };
                        let _ = TrackMouseEvent(&mut tme);

                        let mut needs_repaint = false;

                        // 处理SVG图标悬停（简化版本，参考test.rs）
                        if (0..=TITLE_BAR_HEIGHT).contains(&y) {
                            for icon in &mut window.svg_icons {
                                let hovered = if icon.is_title_bar_button {
                                    // 标题栏按钮：简化版本（参考test.rs）
                                    let hover_padding = ICON_HOVER_PADDING;
                                    let offset_x = 0;
                                    let offset_y = 0;

                                    x >= (icon.rect.left - hover_padding + offset_x)
                                        && x <= (icon.rect.right + hover_padding + offset_x)
                                        && y >= (icon.rect.top - hover_padding + offset_y)
                                        && y <= (icon.rect.bottom + hover_padding + offset_y)
                                } else {
                                    // 普通图标：简化版本（参考test.rs）
                                    let hover_padding = ICON_HOVER_PADDING;
                                    let offset_x = 0;
                                    let offset_y = 0;

                                    x >= (icon.rect.left - hover_padding + offset_x)
                                        && x <= (icon.rect.right + hover_padding + offset_x)
                                        && y >= (icon.rect.top - hover_padding + offset_y)
                                        && y <= (icon.rect.bottom + hover_padding + offset_y)
                                };

                                if icon.hovered != hovered {
                                    icon.hovered = hovered;
                                    needs_repaint = true;
                                }
                            }
                        } else {
                            // 如果鼠标不在标题栏区域，清除所有图标的悬停状态
                            for icon in &mut window.svg_icons {
                                if icon.hovered {
                                    icon.hovered = false;
                                    needs_repaint = true;
                                }
                            }
                        }

                        // 检查是否在文本区域内，设置相应的鼠标指针
                        if x >= window.text_area_rect.left
                            && x <= window.text_area_rect.right
                            && y >= window.text_area_rect.top
                            && y <= window.text_area_rect.bottom
                        {
                            // 设置文本选择鼠标指针
                            if let Ok(cursor) = LoadCursorW(None, IDC_IBEAM) {
                                SetCursor(Some(cursor));
                            }
                        } else {
                            // 恢复默认鼠标指针
                            if let Ok(cursor) = LoadCursorW(None, IDC_ARROW) {
                                SetCursor(Some(cursor));
                            }
                        }

                        // 处理文本选择拖拽
                        if window.is_selecting {
                            // 需要获取可变引用来修改窗口状态
                            let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Self;
                            if !window_ptr.is_null() {
                                let window_mut = &mut *window_ptr;
                                window_mut.update_text_selection(x, y);
                                let _ = InvalidateRect(Some(hwnd), None, false);
                            }
                        }

                        if needs_repaint {
                            // 只标记需要重绘，不立即更新，让系统批量处理
                            let title_bar_rect = RECT {
                                left: 0,
                                top: 0,
                                right: window.window_width,
                                bottom: TITLE_BAR_HEIGHT,
                            };
                            let _ = InvalidateRect(Some(hwnd), Some(&title_bar_rect), false);
                            // 移除 UpdateWindow 调用，避免强制立即重绘
                        }
                    }
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }
                0x02A3 => {
                    // WM_MOUSELEAVE
                    // 鼠标离开窗口时，清除所有悬停状态
                    let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Self;
                    if !window_ptr.is_null() {
                        let window = &mut *window_ptr;
                        let mut needs_repaint = false;

                        // 清除SVG图标悬停状态
                        for icon in &mut window.svg_icons {
                            if icon.hovered {
                                icon.hovered = false;
                                needs_repaint = true;
                            }
                        }

                        if needs_repaint {
                            let title_bar_rect = RECT {
                                left: 0,
                                top: 0,
                                right: window.window_width,
                                bottom: TITLE_BAR_HEIGHT,
                            };
                            let _ = InvalidateRect(Some(hwnd), Some(&title_bar_rect), false);
                            // 移除强制更新，让系统优化重绘时机
                        }
                    }
                    LRESULT(0)
                }
                WM_RBUTTONUP => {
                    // 右键点击关闭窗口
                    let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
                    LRESULT(0)
                }
                WM_KEYDOWN => {
                    // 处理键盘按键
                    if wparam.0 == VK_ESCAPE.0 as usize {
                        // ESC 键关闭窗口
                        let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
                    } else if wparam.0 == 0x41
                        && (GetKeyState(VK_CONTROL.0 as i32) & 0x8000u16 as i16) != 0
                    {
                        // Ctrl+A 全选文本
                        let window_ptr =
                            GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut OcrResultWindow;
                        if !window_ptr.is_null() {
                            let window = &mut *window_ptr;
                            window.select_all_text();
                        }
                    }
                    LRESULT(0)
                }
                // WM_CTLCOLOREDIT 不再需要，因为我们使用自绘文本
                WM_MOUSEWHEEL => {
                    // 处理文本区域的滚轮滚动
                    let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Self;
                    if !window_ptr.is_null() {
                        let window = &mut *window_ptr;

                        // 获取滚轮增量
                        let delta = ((wparam.0 >> 16) as i16) as i32;
                        let scroll_lines = 3; // 每次滚动3行
                        let scroll_amount = if delta > 0 {
                            -scroll_lines
                        } else {
                            scroll_lines
                        } * window.line_height;

                        // 计算新的滚动偏移量
                        let old_offset = window.scroll_offset;
                        window.scroll_offset += scroll_amount;

                        // 限制滚动范围
                        let max_scroll = (window.text_lines.len() as i32 * window.line_height)
                            - (window.text_area_rect.bottom - window.text_area_rect.top);
                        window.scroll_offset = window.scroll_offset.clamp(0, max_scroll.max(0));

                        // 如果滚动位置改变了，重绘文本区域
                        if window.scroll_offset != old_offset {
                            let _ = InvalidateRect(Some(hwnd), Some(&window.text_area_rect), false);
                        }
                    }
                    LRESULT(0)
                }
                WM_CLOSE => {
                    let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Self;
                    if !window_ptr.is_null() {
                        let mut window = Box::from_raw(window_ptr);
                        // 使用统一的清理方法
                        window.cleanup_all_resources();
                        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                    }
                    let _ = DestroyWindow(hwnd);
                    LRESULT(0)
                }
                WM_SIZE => {
                    // 检测窗口状态变化
                    let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Self;
                    if !window_ptr.is_null() {
                        let window = &mut *window_ptr;
                        let state = wparam.0 as u32;
                        let new_width = (lparam.0 as i16) as i32;
                        let new_height = ((lparam.0 >> 16) as i16) as i32;

                        let new_maximized = state == 2; // SIZE_MAXIMIZED
                        let size_changed =
                            window.window_width != new_width || window.window_height != new_height;
                        let maximized_changed = window.is_maximized != new_maximized;

                        if maximized_changed || size_changed {
                            let old_width = window.window_width;
                            window.is_maximized = new_maximized;

                            // 使用新的方法处理大小变化
                            window.handle_size_change(new_width, new_height);

                            // 当最大化状态改变或窗口宽度发生任何变化时都要更新标题栏按钮
                            // 因为标题栏按钮的位置依赖于窗口宽度
                            if maximized_changed || old_width != new_width {
                                window.update_title_bar_buttons();
                            }

                            let _ = InvalidateRect(Some(hwnd), None, false);
                        }
                    }
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }
                _ => DefWindowProcW(hwnd, msg, wparam, lparam),
            }
        }
    }
}

impl Drop for OcrResultWindow {
    fn drop(&mut self) {
        // 使用RAII模式自动清理资源
        self.cleanup_all_resources();
    }
}
