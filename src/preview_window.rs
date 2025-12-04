use crate::ocr::OcrResult;
use crate::platform::traits::PlatformRenderer;
use crate::platform::windows::Direct2DRenderer;
use anyhow::Result;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap;
use windows::Win32::Graphics::Dwm::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::HiDpi::{
    GetDpiForWindow, GetSystemMetricsForDpi, PROCESS_PER_MONITOR_DPI_AWARE, SetProcessDpiAwareness,
};
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::constants::{
    BUTTON_WIDTH_OCR, CLOSE_BUTTON_HOVER_BG_COLOR_D2D, ICON_CLICK_PADDING, ICON_HOVER_BG_COLOR_D2D,
    ICON_HOVER_PADDING, ICON_HOVER_RADIUS, ICON_SIZE, ICON_START_X, TITLE_BAR_BG_COLOR_D2D,
    TITLE_BAR_BUTTON_HOVER_BG_COLOR_D2D, TITLE_BAR_HEIGHT,
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

/// 预览窗口渲染器（负责所有 Direct2D 绘图）
struct PreviewRenderer {
    d2d_renderer: Direct2DRenderer,
    image_bitmap: Option<ID2D1Bitmap>,
    icon_cache: HashMap<String, D2DIconBitmaps>,
    icons_loaded: bool,
}

impl PreviewRenderer {
    fn new() -> Result<Self> {
        // 优先使用全局共享的 Factory，避免重复创建重量级 COM 对象
        let d2d_renderer = Direct2DRenderer::new_with_shared_factories()
            .or_else(|_| Direct2DRenderer::new())
            .map_err(|e| anyhow::anyhow!("D2D Init Error: {:?}", e))?;

        Ok(Self {
            d2d_renderer,
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
        use crate::platform::traits::{Color, DrawStyle, PlatformRenderer, Rectangle};

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

        // 1. 绘制标题栏（包括按钮背景）
        self.draw_custom_title_bar(width, icons, is_pinned)?;

        // 2. 绘制图片（如果存在）
        if let Some(bitmap) = &self.image_bitmap {
            let original_width = image_width as f32;
            let original_height = image_height as f32;

            // Pin 模式：图片紧贴标题栏下方，原始大小显示
            // OCR 模式：图片在左侧区域居中显示
            let (image_x, image_y, display_width, display_height) = if !show_text_area {
                // Pin 模式：直接显示原始大小，紧贴标题栏
                let x = 0.0;
                let y = TITLE_BAR_HEIGHT as f32;
                (x, y, original_width, original_height)
            } else {
                // OCR 模式：在左侧区域居中显示
                let available_width = (width - 350 - 40) as f32;
                let available_height = (text_rect.bottom - text_rect.top) as f32;
                let start_y = TITLE_BAR_HEIGHT as f32 + 10.0;

                // 限制最大可用高度
                let max_height =
                    (crate::platform::windows::system::get_screen_size().1 as f32) - start_y - 20.0;
                let effective_available_height = available_height.min(max_height);

                // 计算缩放比例 (保持长宽比，不放大超过原图)
                let scale_x = available_width / original_width;
                let scale_y = effective_available_height / original_height;
                let scale = scale_x.min(scale_y).min(1.0);

                let display_w = original_width * scale;
                let display_h = original_height * scale;

                // 居中显示
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

        if show_text_area {
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

            // 剪裁文本区域...
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

/// 预览显示窗口 (支持 OCR 结果和 Pin 模式)
pub struct PreviewWindow {
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
    is_selecting: bool,                      // 是否正在选择文本
    selection_start: Option<(usize, usize)>, // 选择开始位置 (行号, 字符位置)
    selection_end: Option<(usize, usize)>,   // 选择结束位置 (行号, 字符位置)

    // 置顶/Pin 状态
    is_pinned: bool,      // 是否置顶
    show_text_area: bool, // 是否显示文本区域

    // Direct2D 渲染器
    renderer: Option<PreviewRenderer>,
}
impl PreviewWindow {
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
        // 右边文字区域宽度
        let text_area_width = if self.show_text_area { 350 } else { 0 };

        // 左边图像区域宽度
        // 注意：这里的image_area_width是为图像预留的区域宽度，不是图像实际显示宽度
        let margin = if self.show_text_area { 20 } else { 0 };
        let image_area_width = self.window_width - text_area_width - margin;

        if self.show_text_area {
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
    }

    pub fn show(
        image_data: Vec<u8>,
        ocr_results: Vec<OcrResult>,
        selection_rect: RECT,
        is_pin_mode: bool,
    ) -> Result<()> {
        unsafe {
            // 1. DPI 设置
            let _ = SetProcessDpiAwareness(PROCESS_PER_MONITOR_DPI_AWARE);

            // 2. 注册窗口类
            let class_name = windows::core::w!("PreviewWindow");
            let instance = windows::Win32::System::LibraryLoader::GetModuleHandleW(None)?;

            // 使用新的类名注册
            let bg_brush = CreateSolidBrush(COLORREF(0));
            let cursor = LoadCursorW(None, IDC_ARROW)?;

            let window_class = WNDCLASSW {
                lpfnWndProc: Some(Self::window_proc),
                hInstance: instance.into(),
                lpszClassName: class_name,
                hCursor: cursor,
                // 关键：使用黑色背景刷，防止调整大小时出现白色闪烁
                hbrBackground: bg_brush,
                style: CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS,
                hIcon: HICON::default(),
                ..Default::default()
            };

            let atom = RegisterClassW(&window_class);
            if atom == 0 {
                let err_code = GetLastError();
                // ERROR_CLASS_ALREADY_EXISTS = 1410
                if err_code.0 != 1410 {
                    eprintln!("RegisterClassW failed: {:?}", err_code);
                    // Fallback or error?
                }
            }

            // 3. 解析图片与计算尺寸
            let (image_pixels, actual_width, actual_height) = Self::parse_bmp_data(&image_data)?;

            // 布局计算
            let (window_width, window_height) = if is_pin_mode {
                // Pin 模式：窗口紧凑图片大小，只有标题栏
                (actual_width, TITLE_BAR_HEIGHT + actual_height)
            } else {
                // OCR 模式：有文本区域和边距
                let text_area_width = 350;
                let image_area_width = actual_width + 40;
                let margin = 20;
                let content_padding_top = 20;
                let content_padding_bottom = 20;
                (
                    image_area_width + text_area_width + margin,
                    TITLE_BAR_HEIGHT + content_padding_top + actual_height + content_padding_bottom,
                )
            };

            // 4. 计算位置
            let (screen_width, screen_height) = crate::platform::windows::system::get_screen_size();
            let mut window_x = selection_rect.right + 20;
            let mut window_y = selection_rect.top;

            // Pin 模式使用选区位置，OCR 模式在右侧弹出
            if is_pin_mode {
                window_x = selection_rect.left;
                window_y = selection_rect.top;
            } else {
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
            }

            // 边界检查
            if window_x + window_width > screen_width {
                window_x = screen_width - window_width;
            }
            if window_y + window_height > screen_height {
                window_y = screen_height - window_height;
            }
            window_x = window_x.max(0);
            window_y = window_y.max(0);

            // 5. 创建窗口 [Zed 风格核心样式]
            let dw_style = WS_THICKFRAME
                | WS_SYSMENU
                | WS_MAXIMIZEBOX
                | WS_MINIMIZEBOX
                | WS_VISIBLE
                | WS_CLIPCHILDREN;

            let mut dw_ex_style = WS_EX_APPWINDOW;
            if is_pin_mode {
                dw_ex_style |= WS_EX_TOPMOST;
            }

            let hwnd = CreateWindowExW(
                dw_ex_style,
                class_name,
                windows::core::w!("预览"),
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

            // 6. DWM 设置
            let dark_mode = 1_i32;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWINDOWATTRIBUTE(20),
                &dark_mode as *const _ as *const _,
                std::mem::size_of::<i32>() as u32,
            );

            let round_preference = 2_i32;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWINDOWATTRIBUTE(33),
                &round_preference as *const _ as *const _,
                std::mem::size_of::<i32>() as u32,
            );

            let margins = MARGINS {
                cxLeftWidth: -1,
                cxRightWidth: -1,
                cyTopHeight: -1,
                cyBottomHeight: -1,
            };
            let _ = DwmExtendFrameIntoClientArea(hwnd, &margins as *const MARGINS as *const _);

            // 触发一次 Frame 改变
            SetWindowPos(
                hwnd,
                None,
                0,
                0,
                0,
                0,
                SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            )?;

            let svg_icons = Self::create_left_icons();

            // 准备文本
            let text_content = ocr_results
                .iter()
                .map(|r| r.text.clone())
                .collect::<Vec<_>>()
                .join("\n");

            let mut window = Self {
                hwnd,
                image_pixels,
                image_width: actual_width,
                image_height: actual_height,
                text_area_rect: RECT::default(), // 将在 recalculate_layout 中计算
                window_width,
                window_height,
                is_maximized: false,
                svg_icons,
                text_content,
                scroll_offset: 0,
                line_height: 24, // 估算值，后面可能会更精确
                text_lines: Vec::new(),
                is_selecting: false,
                selection_start: None,
                selection_end: None,
                is_pinned: is_pin_mode,
                show_text_area: !is_pin_mode,
                renderer: PreviewRenderer::new().ok(),
            };

            let mut title_bar_buttons = window.create_title_bar_buttons(window_width, false);
            window.svg_icons.append(&mut title_bar_buttons);

            // 存储窗口指针
            let window_ptr = Box::into_raw(Box::new(window));
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, window_ptr as isize);

            // 初始化布局和渲染器
            let window = &mut *window_ptr;
            window.center_icons();
            window.recalculate_layout(); // 计算初始布局

            // 初始化渲染器
            if let Some(renderer) = &mut window.renderer {
                let _ = renderer.initialize(hwnd, window_width, window_height);
                let _ = renderer.set_image_from_pixels(
                    &window.image_pixels,
                    window.image_width,
                    window.image_height,
                );
                // 初始化文本分行
                if window.show_text_area {
                    let width = (window.text_area_rect.right - window.text_area_rect.left) as f32;
                    window.text_lines = renderer.split_text_into_lines(&window.text_content, width);
                }
            }

            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = UpdateWindow(hwnd);

            Ok(())
        }
    }

    fn parse_bmp_data(bmp_data: &[u8]) -> Result<(Vec<u8>, i32, i32)> {
        // 复用 BMP 解析逻辑
        if bmp_data.len() < 54 {
            return Err(anyhow::anyhow!("BMP 数据太小"));
        }
        if bmp_data[0] != b'B' || bmp_data[1] != b'M' {
            return Err(anyhow::anyhow!("不是有效的BMP文件"));
        }
        let data_offset =
            u32::from_le_bytes([bmp_data[10], bmp_data[11], bmp_data[12], bmp_data[13]]) as usize;
        let width = i32::from_le_bytes([bmp_data[18], bmp_data[19], bmp_data[20], bmp_data[21]]);
        let height_raw =
            i32::from_le_bytes([bmp_data[22], bmp_data[23], bmp_data[24], bmp_data[25]]);
        let height = height_raw.abs();
        let is_top_down = height_raw < 0;
        let bit_count = u16::from_le_bytes([bmp_data[28], bmp_data[29]]);

        if data_offset >= bmp_data.len() {
            return Err(anyhow::anyhow!("BMP数据偏移量无效"));
        }

        let pixel_data = &bmp_data[data_offset..];
        let bytes_per_pixel = (bit_count / 8) as usize;
        let row_size = (width as usize * bytes_per_pixel).div_ceil(4) * 4;

        let mut rgba_pixels = vec![0u8; (width * height * 4) as usize];

        for y in 0..height {
            let src_y = if is_top_down { y } else { height - 1 - y };
            let src_row_start = src_y as usize * row_size;
            for x in 0..width {
                let src_idx = src_row_start + x as usize * bytes_per_pixel;
                let dst_idx = (y * width + x) as usize * 4;
                if src_idx + bytes_per_pixel <= pixel_data.len() && dst_idx + 3 < rgba_pixels.len()
                {
                    match bit_count {
                        24 => {
                            rgba_pixels[dst_idx] = pixel_data[src_idx + 2];
                            rgba_pixels[dst_idx + 1] = pixel_data[src_idx + 1];
                            rgba_pixels[dst_idx + 2] = pixel_data[src_idx];
                            rgba_pixels[dst_idx + 3] = 255;
                        }
                        32 => {
                            rgba_pixels[dst_idx] = pixel_data[src_idx + 2];
                            rgba_pixels[dst_idx + 1] = pixel_data[src_idx + 1];
                            rgba_pixels[dst_idx + 2] = pixel_data[src_idx];
                            rgba_pixels[dst_idx + 3] = 255;
                        }
                        _ => {
                            rgba_pixels[dst_idx] = 255;
                            rgba_pixels[dst_idx + 1] = 255;
                            rgba_pixels[dst_idx + 2] = 255;
                            rgba_pixels[dst_idx + 3] = 255;
                        }
                    }
                }
            }
        }
        Ok((rgba_pixels, width, height))
    }

    fn custom_caption_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        unsafe {
            match msg {
                WM_ERASEBKGND => LRESULT(1),
                WM_SIZE => {
                    let new_width = (lparam.0 & 0xFFFF) as i32;
                    let new_height = ((lparam.0 >> 16) & 0xFFFF) as i32;
                    let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Self;
                    if !window_ptr.is_null() {
                        let window = &mut *window_ptr;
                        let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
                        window.is_maximized = (style & WS_MAXIMIZE.0) != 0;
                        window.window_width = new_width;
                        window.window_height = new_height;
                        window.update_title_bar_buttons();
                        window.recalculate_layout();
                    }
                    LRESULT(0)
                }
                WM_PAINT => {
                    let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Self;
                    if !window_ptr.is_null() {
                        let window = &mut *window_ptr;
                        if let Some(renderer) = &mut window.renderer {
                            let mut rect = RECT::default();
                            let _ = GetClientRect(hwnd, &mut rect);
                            let width = rect.right - rect.left;
                            let height = rect.bottom - rect.top;

                            if width > 0 && height > 0 {
                                if let Err(e) = renderer.initialize(hwnd, width, height) {
                                    eprintln!("PreviewWindow: renderer.initialize failed: {:?}", e);
                                }
                                if let Err(e) = renderer.set_image_from_pixels(
                                    &window.image_pixels,
                                    window.image_width,
                                    window.image_height,
                                ) {
                                    eprintln!(
                                        "PreviewWindow: set_image_from_pixels failed: {:?}",
                                        e
                                    );
                                }

                                // 传递 show_text_area 参数
                                if let Err(e) = renderer.render(
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
                                    window
                                        .selection_start
                                        .and_then(|s| window.selection_end.map(|e| (s, e))),
                                    window.show_text_area,
                                ) {
                                    eprintln!("PreviewWindow: render failed: {:?}", e);
                                } else {
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
                                let frame_thickness = Self::get_frame_thickness(hwnd);
                                let rgrc = &mut (*params).rgrc;
                                rgrc[0].top += frame_thickness;
                                rgrc[0].bottom -= frame_thickness;
                                rgrc[0].left += frame_thickness;
                                rgrc[0].right -= frame_thickness;
                            }
                        }
                        return LRESULT(0);
                    }
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }
                WM_NCHITTEST => Self::hit_test_nca(hwnd, wparam, lparam),
                _ => Self::app_window_proc(hwnd, msg, wparam, lparam),
            }
        }
    }

    fn hit_test_nca(hwnd: HWND, _wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        unsafe {
            let pt_mouse_x = (lparam.0 as i16) as i32;
            let pt_mouse_y = ((lparam.0 >> 16) as i16) as i32;

            let mut rc_window = RECT::default();
            let _ = GetWindowRect(hwnd, &mut rc_window);

            let client_x = pt_mouse_x - rc_window.left;
            let client_y = pt_mouse_y - rc_window.top;

            let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Self;
            if !window_ptr.is_null() {
                let window = &*window_ptr;
                if (0..TITLE_BAR_HEIGHT).contains(&client_y) {
                    for icon in &window.svg_icons {
                        let in_click_area = if icon.is_title_bar_button {
                            let button_width = BUTTON_WIDTH_OCR;
                            let button_left = icon.rect.left - (button_width - ICON_SIZE) / 2;
                            let button_right = button_left + button_width;
                            client_x >= button_left
                                && client_x <= button_right
                                && (0..=TITLE_BAR_HEIGHT).contains(&client_y)
                        } else {
                            let click_padding = ICON_CLICK_PADDING;
                            client_x >= (icon.rect.left - click_padding)
                                && client_x <= (icon.rect.right + click_padding)
                                && client_y >= (icon.rect.top - click_padding)
                                && client_y <= (icon.rect.bottom + click_padding)
                        };
                        if in_click_area {
                            return LRESULT(HTCLIENT as isize);
                        }
                    }
                }
            }

            let mut rc_frame = RECT::default();
            let _ = AdjustWindowRectEx(
                &mut rc_frame,
                WS_OVERLAPPEDWINDOW & !WS_CAPTION,
                false,
                WS_EX_OVERLAPPEDWINDOW,
            );

            let mut u_row = 1;
            let mut u_col = 1;
            let mut f_on_resize_border = false;

            if pt_mouse_y >= rc_window.top && pt_mouse_y < rc_window.top + TITLE_BAR_HEIGHT {
                f_on_resize_border = pt_mouse_y < (rc_window.top - rc_frame.top);
                u_row = 0;
            } else if pt_mouse_y < rc_window.bottom && pt_mouse_y >= rc_window.bottom - 5 {
                u_row = 2;
            }

            if pt_mouse_x >= rc_window.left && pt_mouse_x < rc_window.left + 5 {
                u_col = 0;
            } else if pt_mouse_x < rc_window.right && pt_mouse_x >= rc_window.right - 5 {
                u_col = 2;
            }

            let hit_tests = [
                [
                    HTTOPLEFT,
                    if f_on_resize_border { HTTOP } else { HTCAPTION },
                    HTTOPRIGHT,
                ],
                [HTLEFT, HTCLIENT, HTRIGHT],
                [HTBOTTOMLEFT, HTBOTTOM, HTBOTTOMRIGHT],
            ];

            LRESULT(hit_tests[u_row][u_col] as isize)
        }
    }

    unsafe extern "system" fn window_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
            if msg == WM_LBUTTONDOWN || msg == WM_LBUTTONUP || msg == WM_MOUSEMOVE {
                let result = Self::custom_caption_proc(hwnd, msg, wparam, lparam);
                if result.0 == 0 && msg == WM_LBUTTONDOWN {
                    return result;
                }
            }

            let dwm_enabled = DwmIsCompositionEnabled().unwrap_or(FALSE);
            if dwm_enabled.as_bool() {
                let mut lret = LRESULT(0);
                let call_dwp = !DwmDefWindowProc(hwnd, msg, wparam, lparam, &mut lret).as_bool();
                if call_dwp {
                    Self::custom_caption_proc(hwnd, msg, wparam, lparam)
                } else {
                    lret
                }
            } else {
                Self::app_window_proc(hwnd, msg, wparam, lparam)
            }
        }));

        match result {
            Ok(lresult) => lresult,
            Err(_) => {
                eprintln!("Panic in window_proc! msg={}", msg);
                unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
        }
    }

    fn app_window_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        unsafe {
            match msg {
                WM_NCCALCSIZE => {
                    if wparam.0 == 1 {
                        return LRESULT(0);
                    }
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }
                WM_NCPAINT => LRESULT(0),
                WM_NCACTIVATE => LRESULT(1),
                WM_GETMINMAXINFO => {
                    let minmax_info = lparam.0 as *mut MINMAXINFO;
                    if !minmax_info.is_null() {
                        let info = &mut *minmax_info;
                        info.ptMinTrackSize.x = 300;
                        info.ptMinTrackSize.y = 200;
                    }
                    LRESULT(0)
                }
                WM_LBUTTONDOWN => {
                    let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Self;
                    if !window_ptr.is_null() {
                        let window = &mut *window_ptr;
                        let x = (lparam.0 as i16) as i32;
                        let y = ((lparam.0 >> 16) as i16) as i32;

                        // 点击图标处理
                        for icon in &mut window.svg_icons {
                            let in_click_area = if icon.is_title_bar_button {
                                let button_width = BUTTON_WIDTH_OCR;
                                let button_left = icon.rect.left - (button_width - ICON_SIZE) / 2;
                                let button_right = button_left + button_width;
                                x >= button_left
                                    && x <= button_right
                                    && (0..TITLE_BAR_HEIGHT).contains(&y)
                            } else {
                                let click_padding = ICON_CLICK_PADDING;
                                x >= (icon.rect.left - click_padding)
                                    && x <= (icon.rect.right + click_padding)
                                    && y >= (icon.rect.top - click_padding)
                                    && y <= (icon.rect.bottom + click_padding)
                            };

                            if in_click_area {
                                match icon.name.as_str() {
                                    "window-minimize" => {
                                        let _ = ShowWindow(hwnd, SW_MINIMIZE);
                                        return LRESULT(0);
                                    }
                                    "pin" => {
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
                                        let _ = ShowWindow(hwnd, SW_MAXIMIZE);
                                        return LRESULT(0);
                                    }
                                    "window-restore" => {
                                        let _ = ShowWindow(hwnd, SW_RESTORE);
                                        return LRESULT(0);
                                    }
                                    "window-close" => {
                                        let _ = PostMessageW(
                                            Some(hwnd),
                                            WM_CLOSE,
                                            WPARAM(0),
                                            LPARAM(0),
                                        );
                                        return LRESULT(0);
                                    }
                                    _ => {}
                                }
                            }
                        }

                        // 文本选择逻辑 (仅当显示文本区域时)
                        if window.show_text_area {
                            let text_rect = window.text_area_rect;
                            if x >= text_rect.left && x <= text_rect.right
                               && y >= text_rect.top && y <= text_rect.bottom {
                                // 计算点击位置对应的行和字符
                                let relative_y = y - text_rect.top + window.scroll_offset;
                                let line_index = (relative_y / window.line_height) as usize;

                                if line_index < window.text_lines.len() {
                                    let relative_x = (x - text_rect.left) as f32;
                                    let line = &window.text_lines[line_index];
                                    let char_index = if let Some(renderer) = &window.renderer {
                                        renderer.get_text_position_from_point(line, relative_x)
                                    } else {
                                        0
                                    };

                                    window.is_selecting = true;
                                    window.selection_start = Some((line_index, char_index));
                                    window.selection_end = Some((line_index, char_index));
                                    let _ = SetCapture(hwnd);
                                    let _ = InvalidateRect(Some(hwnd), None, false);
                                }
                            }
                        }
                    }
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }
                WM_LBUTTONUP => {
                    let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Self;
                    if !window_ptr.is_null() {
                        let window = &mut *window_ptr;
                        if window.is_selecting {
                            window.is_selecting = false;
                            let _ = ReleaseCapture();
                        }
                    }
                    let _ = InvalidateRect(Some(hwnd), None, false);
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }
                WM_MOUSEMOVE => {
                    let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Self;
                    if !window_ptr.is_null() {
                        let window = &mut *window_ptr;
                        let x = (lparam.0 as i16) as i32;
                        let y = ((lparam.0 >> 16) as i16) as i32;

                        let mut tme = TRACKMOUSEEVENT {
                            cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                            dwFlags: TME_LEAVE,
                            hwndTrack: hwnd,
                            dwHoverTime: 0,
                        };
                        let _ = TrackMouseEvent(&mut tme);

                        let mut needs_repaint = false;
                        if (0..=TITLE_BAR_HEIGHT).contains(&y) {
                            for icon in &mut window.svg_icons {
                                let hovered = if icon.is_title_bar_button {
                                    let hover_padding = ICON_HOVER_PADDING;
                                    x >= (icon.rect.left - hover_padding)
                                        && x <= (icon.rect.right + hover_padding)
                                        && y >= (icon.rect.top - hover_padding)
                                        && y <= (icon.rect.bottom + hover_padding)
                                } else {
                                    let hover_padding = ICON_HOVER_PADDING;
                                    x >= (icon.rect.left - hover_padding)
                                        && x <= (icon.rect.right + hover_padding)
                                        && y >= (icon.rect.top - hover_padding)
                                        && y <= (icon.rect.bottom + hover_padding)
                                };
                                if icon.hovered != hovered {
                                    icon.hovered = hovered;
                                    needs_repaint = true;
                                }
                            }
                        } else {
                            for icon in &mut window.svg_icons {
                                if icon.hovered {
                                    icon.hovered = false;
                                    needs_repaint = true;
                                }
                            }
                        }

                        // 文本选择移动逻辑
                        if window.is_selecting && window.show_text_area {
                            let text_rect = window.text_area_rect;
                            let clamped_x = x.max(text_rect.left).min(text_rect.right);
                            let clamped_y = y.max(text_rect.top).min(text_rect.bottom);

                            let relative_y = clamped_y - text_rect.top + window.scroll_offset;
                            let line_index = ((relative_y / window.line_height) as usize)
                                .min(window.text_lines.len().saturating_sub(1));

                            let relative_x = (clamped_x - text_rect.left) as f32;
                            let line = &window.text_lines[line_index];
                            let char_index = if let Some(renderer) = &window.renderer {
                                renderer.get_text_position_from_point(line, relative_x)
                            } else {
                                0
                            };

                            window.selection_end = Some((line_index, char_index));
                            needs_repaint = true;
                        }

                        if needs_repaint {
                            let _ = InvalidateRect(Some(hwnd), None, false);
                        }
                    }
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }
                WM_MOUSEWHEEL => {
                    let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Self;
                    if !window_ptr.is_null() {
                         let window = &mut *window_ptr;
                         if window.show_text_area {
                             let delta = ((wparam.0 >> 16) as i16) as i32;
                             // 滚动...
                             let scroll_amount = (delta / 120) * window.line_height * 3;
                             window.scroll_offset -= scroll_amount;

                             // Clamping
                             let max_scroll = (window.text_lines.len() as i32 * window.line_height)
                                - (window.text_area_rect.bottom - window.text_area_rect.top);
                             window.scroll_offset = window.scroll_offset.clamp(0, max_scroll.max(0));

                             let _ = InvalidateRect(Some(hwnd), Some(&window.text_area_rect), false);
                         }
                    }
                    LRESULT(0)
                }
                0x02A3 /* WM_MOUSELEAVE */ => {
                    let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Self;
                    if !window_ptr.is_null() {
                        let window = &mut *window_ptr;
                        let mut needs_repaint = false;
                        for icon in &mut window.svg_icons {
                            if icon.hovered {
                                icon.hovered = false;
                                needs_repaint = true;
                            }
                        }
                        if needs_repaint {
                            let _ = InvalidateRect(Some(hwnd), None, false);
                        }
                    }
                    LRESULT(0)
                }
                WM_KEYDOWN => {
                    let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Self;
                    if !window_ptr.is_null() {
                        let window = &mut *window_ptr;
                        let vk = wparam.0 as u16;
                        let ctrl_pressed = (GetKeyState(0x11 /* VK_CONTROL */) as u16 & 0x8000) != 0;

                        if ctrl_pressed && window.show_text_area {
                            match vk {
                                0x41 /* VK_A */ => {
                                    // Ctrl+A: 全选
                                    if !window.text_lines.is_empty() {
                                        window.selection_start = Some((0, 0));
                                        let last_line = window.text_lines.len() - 1;
                                        let last_char = window.text_lines[last_line].chars().count();
                                        window.selection_end = Some((last_line, last_char));
                                        let _ = InvalidateRect(Some(hwnd), None, false);
                                    }
                                    return LRESULT(0);
                                }
                                0x43 /* VK_C */ => {
                                    // Ctrl+C: 复制选中文本
                                    if let (Some(start), Some(end)) = (window.selection_start, window.selection_end) {
                                        let (start, end) = if start <= end { (start, end) } else { (end, start) };
                                        let mut selected_text = String::new();

                                        for i in start.0..=end.0 {
                                            if i >= window.text_lines.len() { break; }
                                            let line = &window.text_lines[i];
                                            let chars: Vec<char> = line.chars().collect();

                                            let start_char = if i == start.0 { start.1 } else { 0 };
                                            let end_char = if i == end.0 { end.1 } else { chars.len() };

                                            if start_char < chars.len() {
                                                let slice: String = chars[start_char..end_char.min(chars.len())].iter().collect();
                                                selected_text.push_str(&slice);
                                            }
                                            if i < end.0 {
                                                selected_text.push('\n');
                                            }
                                        }

                                        if !selected_text.is_empty() {
                                            let _ = crate::screenshot::save::copy_text_to_clipboard(&selected_text);
                                        }
                                    }
                                    return LRESULT(0);
                                }
                                _ => {}
                            }
                        }
                    }
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }
                WM_DESTROY => {
                    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Self;
                    if !ptr.is_null() {
                        let mut window = Box::from_raw(ptr);
                        window.cleanup_all_resources();
                    }
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                    LRESULT(0)
                }
                _ => DefWindowProcW(hwnd, msg, wparam, lparam),
            }
        }
    }
}
