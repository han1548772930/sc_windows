use crate::ocr::OcrResult;
use anyhow::Result;
use windows::Win32::Foundation::*;
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
    BUTTON_WIDTH_OCR, CLOSE_BUTTON_HOVER_BG_COLOR, ICON_CLICK_PADDING, ICON_HOVER_BG_COLOR,
    ICON_HOVER_PADDING, ICON_HOVER_RADIUS, ICON_SIZE, ICON_START_X,
    TITLE_BAR_BUTTON_HOVER_BG_COLOR, TITLE_BAR_HEIGHT,
};

// 图标缓存结构体 - 一次性加载所有图标，避免重复加载
struct IconCache {
    // 左侧图标
    pin_normal: HBITMAP,
    pin_hover: HBITMAP,
    pin_active_normal: HBITMAP, // 绿色激活状态
    pin_active_hover: HBITMAP,  // 绿色激活悬停状态

    // 标题栏按钮 - 普通状态
    close_normal: HBITMAP,
    close_hover: HBITMAP,
    maximize_normal: HBITMAP,
    maximize_hover: HBITMAP,
    minimize_normal: HBITMAP,
    minimize_hover: HBITMAP,
    restore_normal: HBITMAP,
    restore_hover: HBITMAP,
}

impl IconCache {
    /// 创建图标缓存，一次性加载所有图标
    fn new() -> Option<Self> {
        // 加载左侧图标
        let (pin_normal, pin_hover) = Self::load_svg_icon_from_file("pin.svg", ICON_SIZE)?;
        let (pin_active_normal, pin_active_hover) = Self::load_colored_pin_bitmaps(
            "pin.svg",
            ICON_SIZE,
            (0, 128, 0), // 绿色
        )?;

        // 加载标题栏按钮
        let (close_normal, close_hover, _) = Self::load_title_bar_button_from_file("x.svg")?;
        let (maximize_normal, maximize_hover, _) =
            Self::load_title_bar_button_from_file("square.svg")?;
        let (minimize_normal, minimize_hover, _) =
            Self::load_title_bar_button_from_file("minus.svg")?;
        let (restore_normal, restore_hover, _) =
            Self::load_title_bar_button_from_file("reduction.svg")?;

        Some(IconCache {
            pin_normal,
            pin_hover,
            pin_active_normal,
            pin_active_hover,
            close_normal,
            close_hover,
            maximize_normal,
            maximize_hover,
            minimize_normal,
            minimize_hover,
            restore_normal,
            restore_hover,
        })
    }

    // 复用现有的图标加载方法
    fn load_svg_icon_from_file(filename: &str, size: i32) -> Option<(HBITMAP, HBITMAP)> {
        OcrResultWindow::load_svg_icon_from_file(filename, size)
    }

    fn load_title_bar_button_from_file(filename: &str) -> Option<(HBITMAP, HBITMAP, HBITMAP)> {
        OcrResultWindow::load_title_bar_button_from_file(filename)
    }

    fn load_colored_pin_bitmaps(
        filename: &str,
        size: i32,
        icon_rgb: (u8, u8, u8),
    ) -> Option<(HBITMAP, HBITMAP)> {
        OcrResultWindow::load_colored_pin_bitmaps(filename, size, icon_rgb)
    }
}

impl Drop for IconCache {
    fn drop(&mut self) {
        unsafe {
            // 清理所有位图资源
            let bitmaps = [
                self.pin_normal,
                self.pin_hover,
                self.pin_active_normal,
                self.pin_active_hover,
                self.close_normal,
                self.close_hover,
                self.maximize_normal,
                self.maximize_hover,
                self.minimize_normal,
                self.minimize_hover,
                self.restore_normal,
                self.restore_hover,
            ];

            for bitmap in bitmaps.iter() {
                if !bitmap.is_invalid() {
                    let _ = DeleteObject((*bitmap).into());
                }
            }
        }
    }
}

// SVG 图标结构 - 简化版，只保存位置和状态信息
#[derive(Clone)]
struct SvgIcon {
    name: String,
    normal_bitmap: HBITMAP, // 引用IconCache中的位图
    hover_bitmap: HBITMAP,  // 引用IconCache中的位图
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
    image_bitmap: Option<HBITMAP>,
    image_width: i32,
    image_height: i32,
    font: HFONT,
    text_area_rect: RECT,      // 文字显示区域
    window_width: i32,         // 窗口宽度
    window_height: i32,        // 窗口高度
    is_no_text_detected: bool, // 是否是"未识别到文字"状态
    is_maximized: bool,        // 是否最大化
    svg_icons: Vec<SvgIcon>,   // SVG 图标列表（包括左侧图标和标题栏按钮）

    // 简化的缓冲相关字段
    content_buffer: Option<HBITMAP>, // 内容区域缓冲位图
    buffer_width: i32,               // 缓冲区宽度
    buffer_height: i32,              // 缓冲区高度
    buffer_valid: bool,              // 缓冲区是否有效（只有第一次创建后就一直有效）

    // 图标缓存 - 一次性加载所有图标，避免重复加载
    icon_cache: IconCache,

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

    /// 清理所有资源（简化版本 - IconCache会自动清理）
    fn cleanup_all_resources(&mut self) {
        unsafe {
            // 清理内容缓冲区
            self.cleanup_content_buffer();

            // 清理图像位图
            if let Some(bitmap) = self.image_bitmap.take() {
                let _ = DeleteObject(bitmap.into());
            }

            // 清理字体
            if !self.font.is_invalid() {
                let _ = DeleteObject(self.font.into());
            }

            // 清理SVG图标列表（不需要清理位图，因为它们来自IconCache）
            self.svg_icons.clear();

            // IconCache会在Drop时自动清理所有位图资源
        }
    }

    /// 检查并创建内容缓冲区（仅在尺寸变化时重建）
    fn ensure_content_buffer(&mut self, width: i32, height: i32) -> Result<()> {
        unsafe {
            // 只有在缓冲区不存在或尺寸确实变化时才重新创建
            if self.content_buffer.is_none()
                || self.buffer_width != width
                || self.buffer_height != height
            {
                // 清理旧缓冲区
                self.cleanup_content_buffer();

                // 创建新缓冲区
                let screen_dc = GetDC(None);
                if screen_dc.is_invalid() {
                    return Err(anyhow::anyhow!("获取屏幕DC失败"));
                }

                let new_buffer = CreateCompatibleBitmap(screen_dc, width, height);
                let _ = ReleaseDC(None, screen_dc);

                if new_buffer.is_invalid() {
                    return Err(anyhow::anyhow!("创建内容缓冲区失败"));
                }

                self.content_buffer = Some(new_buffer);
                self.buffer_width = width;
                self.buffer_height = height;
                self.buffer_valid = false; // 标记需要重绘
            }

            Ok(())
        }
    }

    /// 清理内容缓冲区
    fn cleanup_content_buffer(&mut self) {
        unsafe {
            if let Some(old_buffer) = self.content_buffer.take() {
                let _ = DeleteObject(old_buffer.into());
            }
            self.buffer_width = 0;
            self.buffer_height = 0;
            self.buffer_valid = false;
        }
    }

    /// 渲染内容到缓冲区
    fn render_to_buffer(&mut self, screen_hdc: HDC) -> Result<()> {
        unsafe {
            if let Some(buffer_bitmap) = self.content_buffer {
                let buffer_dc = CreateCompatibleDC(Some(screen_hdc));
                let old_bitmap = SelectObject(buffer_dc, buffer_bitmap.into());

                // 渲染内容到缓冲区
                let result = self.paint_content_to_dc(buffer_dc);

                SelectObject(buffer_dc, old_bitmap);
                let _ = DeleteDC(buffer_dc);

                if result.is_ok() {
                    self.buffer_valid = true;
                }

                result
            } else {
                Err(anyhow::anyhow!("缓冲区不存在"))
            }
        }
    }

    /// 将内容绘制到指定的DC（用于缓冲和直接绘制）
    fn paint_content_to_dc(&self, hdc: HDC) -> Result<()> {
        unsafe {
            let rect = RECT {
                left: 0,
                top: 0,
                right: self.buffer_width,
                bottom: self.buffer_height,
            };

            // 设置背景色为白色（仅内容区域）
            let white_brush = CreateSolidBrush(COLORREF(0x00FFFFFF));
            FillRect(hdc, &rect, white_brush);

            // 绘制窗口边框
            let border_pen = CreatePen(PS_SOLID, 2, COLORREF(0x00CCCCCC));
            let old_pen = SelectObject(hdc, border_pen.into());
            let old_brush = SelectObject(hdc, GetStockObject(NULL_BRUSH));

            let _ = Rectangle(hdc, rect.left, rect.top, rect.right, rect.bottom);

            SelectObject(hdc, old_pen);
            SelectObject(hdc, old_brush);
            let _ = DeleteObject(border_pen.into());

            // 设置文本颜色为黑色
            SetTextColor(hdc, COLORREF(0x00000000));
            SetBkMode(hdc, TRANSPARENT);

            // 选择微软雅黑字体
            let old_font = SelectObject(hdc, self.font.into());

            // 使用预计算的布局
            let image_area_width = self.text_area_rect.left - 10;

            // 绘制图像区域边框
            let image_rect = RECT {
                left: 10,
                top: 10,
                right: image_area_width - 10,
                bottom: self.buffer_height - 10,
            };

            let border_pen = CreatePen(PS_SOLID, 1, COLORREF(0x00CCCCCC));
            let old_pen = SelectObject(hdc, border_pen.into());
            let old_brush = SelectObject(hdc, GetStockObject(NULL_BRUSH));

            let _ = Rectangle(
                hdc,
                image_rect.left,
                image_rect.top,
                image_rect.right,
                image_rect.bottom,
            );

            SelectObject(hdc, old_pen);
            SelectObject(hdc, old_brush);
            let _ = DeleteObject(border_pen.into());

            // 绘制实际的截图图像
            if let Some(bitmap) = self.image_bitmap {
                // 创建内存 DC 来绘制位图
                let mem_dc = CreateCompatibleDC(Some(hdc));
                let old_bitmap = SelectObject(mem_dc, bitmap.into());

                // 计算图像显示区域（保持宽高比，居中显示）
                let available_width = image_area_width - 40;
                let available_height = self.buffer_height - 60;

                let scale_x = available_width as f32 / self.image_width as f32;
                let scale_y = available_height as f32 / self.image_height as f32;
                let scale = scale_x.min(scale_y).min(1.0); // 不放大

                let scaled_width = (self.image_width as f32 * scale) as i32;
                let scaled_height = (self.image_height as f32 * scale) as i32;

                let x_offset = 20 + (available_width - scaled_width) / 2;
                let y_offset = 30 + (available_height - scaled_height) / 2;

                // 使用 StretchBlt 绘制缩放的图像
                let _ = StretchBlt(
                    hdc,
                    x_offset,
                    y_offset,
                    scaled_width,
                    scaled_height,
                    Some(mem_dc),
                    0,
                    0,
                    self.image_width,
                    self.image_height,
                    SRCCOPY,
                );

                SelectObject(mem_dc, old_bitmap);
                let _ = DeleteDC(mem_dc);
            } else {
                // 如果没有位图，显示提示文字
                let image_text = "截图图像\n(加载失败)";
                let mut image_text_rect = RECT {
                    left: 20,
                    top: 30,
                    right: image_area_width - 20,
                    bottom: 100,
                };

                let mut image_text_wide: Vec<u16> = image_text
                    .encode_utf16()
                    .chain(std::iter::once(0))
                    .collect();
                DrawTextW(
                    hdc,
                    &mut image_text_wide,
                    &mut image_text_rect,
                    DT_LEFT | DT_TOP | DT_WORDBREAK,
                );
            }

            // 绘制文本区域
            self.draw_text_area_to_dc(hdc);

            // 恢复原来的字体
            SelectObject(hdc, old_font);

            let _ = DeleteObject(white_brush.into());

            Ok(())
        }
    }

    /// 创建左侧图标（使用缓存的位图）
    fn create_left_icons(icon_cache: &IconCache) -> Vec<SvgIcon> {
        let mut icons = Vec::new();

        let icon_x = ICON_START_X;
        let icon_y = (TITLE_BAR_HEIGHT - ICON_SIZE) / 2;

        icons.push(SvgIcon {
            name: "pin".to_string(),
            normal_bitmap: icon_cache.pin_normal,
            hover_bitmap: icon_cache.pin_hover,
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

    /// 更新标题栏按钮状态（优化版本 - 不重新加载图标）
    fn update_title_bar_buttons(&mut self) {
        let window_width = self.window_width;

        // 移除旧的标题栏按钮，保留左侧图标（不需要释放位图，因为它们来自缓存）
        self.svg_icons.retain(|icon| !icon.is_title_bar_button);

        // 确保有左侧图标并更新它们的位置
        let has_left_icons = self.svg_icons.iter().any(|icon| !icon.is_title_bar_button);
        if !has_left_icons {
            // 从缓存创建左侧图标
            let mut left_icons = Self::create_left_icons(&self.icon_cache);
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

        // 创建新的标题栏按钮（使用缓存的位图）
        let mut title_bar_buttons =
            self.create_title_bar_buttons_from_cache(window_width, self.is_maximized);
        self.svg_icons.append(&mut title_bar_buttons);
    }

    /// 创建标题栏按钮（使用缓存的位图）
    fn create_title_bar_buttons_from_cache(
        &self,
        window_width: i32,
        is_maximized: bool,
    ) -> Vec<SvgIcon> {
        let mut buttons = Vec::new();

        // 根据窗口状态选择按钮配置
        let button_configs = if is_maximized {
            // 最大化状态：关闭、还原、最小化（从右到左）
            vec![
                (
                    "x",
                    self.icon_cache.close_normal,
                    self.icon_cache.close_hover,
                ),
                (
                    "reduction",
                    self.icon_cache.restore_normal,
                    self.icon_cache.restore_hover,
                ),
                (
                    "minus",
                    self.icon_cache.minimize_normal,
                    self.icon_cache.minimize_hover,
                ),
            ]
        } else {
            // 普通状态：关闭、最大化、最小化（从右到左）
            vec![
                (
                    "x",
                    self.icon_cache.close_normal,
                    self.icon_cache.close_hover,
                ),
                (
                    "square",
                    self.icon_cache.maximize_normal,
                    self.icon_cache.maximize_hover,
                ),
                (
                    "minus",
                    self.icon_cache.minimize_normal,
                    self.icon_cache.minimize_hover,
                ),
            ]
        };

        // 从右到左创建按钮
        for (i, (name, normal_bitmap, hover_bitmap)) in button_configs.iter().enumerate() {
            // 按钮位置计算
            let button_x = window_width - (i as i32 + 1) * BUTTON_WIDTH_OCR;
            let icon_x = button_x + (BUTTON_WIDTH_OCR - ICON_SIZE) / 2;
            let icon_y = (TITLE_BAR_HEIGHT - ICON_SIZE) / 2;

            buttons.push(SvgIcon {
                name: name.to_string(),
                normal_bitmap: *normal_bitmap,
                hover_bitmap: *hover_bitmap,
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

    /// 文本换行处理
    fn wrap_text_lines(text: &str, text_rect: &RECT, font: HFONT, hwnd: HWND) -> Vec<String> {
        unsafe {
            let hdc = GetDC(Some(hwnd));
            let old_font = SelectObject(hdc, font.into());

            let text_width = text_rect.right - text_rect.left - 20; // 减去左右边距
            let mut lines = Vec::new();

            for paragraph in text.split('\n') {
                if paragraph.trim().is_empty() {
                    lines.push(String::new());
                    continue;
                }

                let words: Vec<&str> = paragraph.split_whitespace().collect();
                let mut current_line = String::new();

                for word in words {
                    let test_line = if current_line.is_empty() {
                        word.to_string()
                    } else {
                        format!("{current_line} {word}")
                    };

                    // 测量文本宽度
                    let test_wide: Vec<u16> = test_line.encode_utf16().collect();
                    let mut size = SIZE::default();
                    let _ = GetTextExtentPoint32W(hdc, &test_wide, &mut size);

                    if size.cx <= text_width || current_line.is_empty() {
                        current_line = test_line;
                    } else {
                        lines.push(current_line);
                        current_line = word.to_string();
                    }
                }

                if !current_line.is_empty() {
                    lines.push(current_line);
                }
            }

            let _ = SelectObject(hdc, old_font);
            let _ = ReleaseDC(Some(hwnd), hdc);
            lines
        }
    }

    /// 重新计算窗口布局
    fn recalculate_layout(&mut self) {
        // 右边文字区域宽度（固定350像素）
        let text_area_width = 350;

        // 左边图像区域宽度
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
            self.text_lines = Self::wrap_text_lines(
                &self.text_content,
                &self.text_area_rect,
                self.font,
                self.hwnd,
            );

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

    /// 从文件加载标题栏按钮，创建专门的悬停效果
    fn load_title_bar_button_from_file(filename: &str) -> Option<(HBITMAP, HBITMAP, HBITMAP)> {
        unsafe {
            // 读取SVG文件
            let svg_path = format!("icons/{filename}");
            let svg_data = match std::fs::read_to_string(&svg_path) {
                Ok(data) => data,
                Err(_) => return None,
            };

            // 解析SVG - 使用默认选项
            let tree = match usvg::Tree::from_str(&svg_data, &usvg::Options::default()) {
                Ok(tree) => tree,
                Err(_) => return None,
            };

            // 创建pixmap
            let mut pixmap = tiny_skia::Pixmap::new(ICON_SIZE as u32, ICON_SIZE as u32)?;
            pixmap.fill(tiny_skia::Color::TRANSPARENT);

            // 获取SVG的尺寸并计算缩放
            let svg_size = tree.size();

            // 渲染SVG到pixmap
            let render_ts = tiny_skia::Transform::from_scale(
                ICON_SIZE as f32 / svg_size.width(),
                ICON_SIZE as f32 / svg_size.height(),
            );

            resvg::render(&tree, render_ts, &mut pixmap.as_mut());

            // 获取屏幕DC
            let screen_dc = GetDC(None);

            // 创建普通状态的位图（透明背景）
            let normal_bitmap =
                Self::create_transparent_icon_bitmap(&screen_dc, &pixmap, ICON_SIZE)?;

            // 创建悬停状态的位图（矩形背景，铺满标题栏高度）
            let hover_bitmap = if filename == "x.svg" {
                // 关闭按钮：红色背景，矩形，铺满高度
                Self::create_title_bar_button_rect_hover_bitmap(
                    &screen_dc,
                    &pixmap,
                    ICON_SIZE,
                    CLOSE_BUTTON_HOVER_BG_COLOR,
                    true, // 是关闭按钮
                )?
            } else {
                // 其他按钮：灰色背景，矩形，铺满高度
                Self::create_title_bar_button_rect_hover_bitmap(
                    &screen_dc,
                    &pixmap,
                    ICON_SIZE,
                    TITLE_BAR_BUTTON_HOVER_BG_COLOR,
                    false, // 不是关闭按钮
                )?
            };

            // 最大化状态下的悬停位图（使用相同的hover位图）
            let hover_bitmap_maximized = hover_bitmap;

            let _ = ReleaseDC(None, screen_dc);

            Some((normal_bitmap, hover_bitmap, hover_bitmap_maximized))
        }
    }

    /// 创建标题栏按钮的矩形hover背景（撑满标题栏高度）
    fn create_title_bar_button_rect_hover_bitmap(
        screen_dc: &HDC,
        pixmap: &tiny_skia::Pixmap,
        size: i32,
        bg_color: (u8, u8, u8),
        is_close_button: bool,
    ) -> Option<HBITMAP> {
        unsafe {
            // 使用标题栏按钮的实际宽度和标题栏高度，撑满标题栏
            // 关闭按钮使用更宽的宽度以便延伸到窗口右边缘
            let button_width = if is_close_button {
                BUTTON_WIDTH_OCR * 2 // 关闭按钮使用更宽的位图
            } else {
                BUTTON_WIDTH_OCR
            };
            let button_height = TITLE_BAR_HEIGHT; // 使用标题栏高度

            // 创建DIB段位图以支持Alpha通道
            let bitmap_info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: button_width,
                    biHeight: -button_height, // 负值表示自顶向下
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [RGBQUAD::default(); 1],
            };

            let mut bits_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
            let bitmap = CreateDIBSection(
                Some(*screen_dc),
                &bitmap_info,
                DIB_RGB_COLORS,
                &mut bits_ptr,
                None,
                0,
            )
            .ok()?;

            // 处理像素数据
            let pixel_data = pixmap.data();
            let bits_slice = std::slice::from_raw_parts_mut(
                bits_ptr as *mut u8,
                (button_width * button_height * 4) as usize,
            );

            // 填充整个区域为背景色（矩形，撑满标题栏高度）
            for y in 0..button_height {
                for x in 0..button_width {
                    let dst_idx = (y * button_width + x) as usize * 4;
                    if dst_idx + 3 < bits_slice.len() {
                        bits_slice[dst_idx] = bg_color.2; // B
                        bits_slice[dst_idx + 1] = bg_color.1; // G
                        bits_slice[dst_idx + 2] = bg_color.0; // R
                        bits_slice[dst_idx + 3] = 255; // A
                    }
                }
            }

            // 绘制图标：居中显示
            let icon_x_offset = if is_close_button {
                // 关闭按钮：在左边部分居中（因为位图比按钮宽）
                (BUTTON_WIDTH_OCR - size) / 2
            } else {
                // 其他按钮：正常居中
                (button_width - size) / 2
            };
            let icon_y_offset = (button_height - size) / 2; // 在标题栏高度中垂直居中

            for y in 0..size {
                for x in 0..size {
                    let src_idx = (y * size + x) as usize * 4;
                    let dst_x = x + icon_x_offset;
                    let dst_y = y + icon_y_offset;
                    let dst_idx = (dst_y * button_width + dst_x) as usize * 4;

                    if src_idx + 3 < pixel_data.len() && dst_idx + 3 < bits_slice.len() {
                        let src_r = pixel_data[src_idx + 2];
                        let src_g = pixel_data[src_idx + 1];
                        let src_b = pixel_data[src_idx];
                        let alpha = pixel_data[src_idx + 3];

                        if alpha > 0 {
                            if is_close_button {
                                // 关闭按钮：检查是否为白色或接近白色的像素（描边）
                                let is_white_ish = src_r > 200 && src_g > 200 && src_b > 200;

                                if !is_white_ish {
                                    // 非白色像素设置为白色
                                    bits_slice[dst_idx] = 255; // B - 白色
                                    bits_slice[dst_idx + 1] = 255; // G - 白色
                                    bits_slice[dst_idx + 2] = 255; // R - 白色
                                    bits_slice[dst_idx + 3] = alpha; // 保持原始透明度
                                }
                            } else {
                                // 其他按钮：保持原始颜色
                                bits_slice[dst_idx] = src_b;
                                bits_slice[dst_idx + 1] = src_g;
                                bits_slice[dst_idx + 2] = src_r;
                                bits_slice[dst_idx + 3] = alpha;
                            }
                        }
                    }
                }
            }

            Some(bitmap)
        }
    }

    /// 从文件加载单个SVG图标，返回普通状态和悬停状态的位图
    fn load_svg_icon_from_file(filename: &str, size: i32) -> Option<(HBITMAP, HBITMAP)> {
        unsafe {
            // 读取SVG文件
            let svg_path = format!("icons/{filename}");
            let svg_data = match std::fs::read_to_string(&svg_path) {
                Ok(data) => data,
                Err(_) => return None,
            };

            // 解析SVG - 使用默认选项
            let tree = match usvg::Tree::from_str(&svg_data, &usvg::Options::default()) {
                Ok(tree) => tree,
                Err(_) => return None,
            };

            // 创建pixmap
            let mut pixmap = tiny_skia::Pixmap::new(size as u32, size as u32)?;
            pixmap.fill(tiny_skia::Color::TRANSPARENT);

            // 获取SVG的尺寸并计算缩放
            let svg_size = tree.size();

            // 渲染SVG到pixmap
            let render_ts = tiny_skia::Transform::from_scale(
                size as f32 / svg_size.width(),
                size as f32 / svg_size.height(),
            );

            resvg::render(&tree, render_ts, &mut pixmap.as_mut());

            // 创建Windows位图
            let screen_dc = GetDC(None);

            // 创建普通状态的位图（完全透明背景，与标题栏无缝融合）
            let normal_bitmap = Self::create_transparent_icon_bitmap(&screen_dc, &pixmap, size)?;

            // 创建悬停状态的位图（浅蓝色背景）
            let hover_bitmap =
                Self::create_icon_bitmap(&screen_dc, &pixmap, size, ICON_HOVER_BG_COLOR)?;

            let _ = ReleaseDC(None, screen_dc);

            Some((normal_bitmap, hover_bitmap))
        }
    }

    // 移除了未使用的create_title_bar_button_bitmap函数

    fn is_point_in_rounded_rect(x: f32, y: f32, width: f32, height: f32, radius: f32) -> bool {
        // 计算到各个角的距离
        let left = radius;
        let right = width - radius;
        let top = radius;
        let bottom = height - radius;

        // 如果点在中央矩形区域，直接返回true
        if x >= left && x <= right {
            return true;
        }
        if y >= top && y <= bottom {
            return true;
        }

        // 检查四个圆角区域
        let mut in_corner = false;

        // 左上角
        if x < left && y < top {
            let dx = left - x;
            let dy = top - y;
            in_corner = dx * dx + dy * dy <= radius * radius;
        }
        // 右上角
        else if x > right && y < top {
            let dx = x - right;
            let dy = top - y;
            in_corner = dx * dx + dy * dy <= radius * radius;
        }
        // 左下角
        else if x < left && y > bottom {
            let dx = left - x;
            let dy = y - bottom;
            in_corner = dx * dx + dy * dy <= radius * radius;
        }
        // 右下角
        else if x > right && y > bottom {
            let dx = x - right;
            let dy = y - bottom;
            in_corner = dx * dx + dy * dy <= radius * radius;
        }

        in_corner
    }

    /// 创建带指定背景色的图标位图
    fn create_icon_bitmap(
        screen_dc: &HDC,
        pixmap: &tiny_skia::Pixmap,
        size: i32,
        bg_color: (u8, u8, u8),
    ) -> Option<HBITMAP> {
        unsafe {
            // 创建更大的位图来包含悬停背景
            let padding = ICON_HOVER_PADDING; // 使用常量
            let total_size = size + padding * 2; // 总尺寸包含padding

            // 创建DIB段位图以支持Alpha通道
            let bitmap_info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: total_size,
                    biHeight: -total_size, // 负值表示自顶向下
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [RGBQUAD::default(); 1],
            };

            let mut bits_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
            let bitmap = CreateDIBSection(
                Some(*screen_dc),
                &bitmap_info,
                DIB_RGB_COLORS,
                &mut bits_ptr,
                None,
                0,
            )
            .ok()?;

            // 处理像素数据
            let pixel_data = pixmap.data();
            let bits_slice = std::slice::from_raw_parts_mut(
                bits_ptr as *mut u8,
                (total_size * total_size * 4) as usize,
            );

            // 首先填充整个区域为背景色（带圆角）
            let radius = ICON_HOVER_RADIUS;
            for y in 0..total_size {
                for x in 0..total_size {
                    let dst_idx = (y * total_size + x) as usize * 4;
                    if dst_idx + 3 < bits_slice.len() {
                        // 检查是否在圆角矩形内
                        let in_rounded_rect = Self::is_point_in_rounded_rect(
                            x as f32,
                            y as f32,
                            total_size as f32,
                            total_size as f32,
                            radius,
                        );

                        if in_rounded_rect {
                            bits_slice[dst_idx] = bg_color.2; // B
                            bits_slice[dst_idx + 1] = bg_color.1; // G
                            bits_slice[dst_idx + 2] = bg_color.0; // R
                            bits_slice[dst_idx + 3] = 255; // A
                        } else {
                            // 圆角外的区域使用标题栏背景色，避免白色边缘
                            let title_bg_r = 0xED;
                            let title_bg_g = 0xED;
                            let title_bg_b = 0xED;
                            let title_bg_a = 255;
                            let alpha_factor = title_bg_a as f32 / 255.0;
                            bits_slice[dst_idx] = (title_bg_b as f32 * alpha_factor) as u8; // B
                            bits_slice[dst_idx + 1] = (title_bg_g as f32 * alpha_factor) as u8; // G
                            bits_slice[dst_idx + 2] = (title_bg_r as f32 * alpha_factor) as u8; // R
                            bits_slice[dst_idx + 3] = title_bg_a; // A
                        }
                    }
                }
            }

            // 然后在中心绘制图标
            for y in 0..size {
                for x in 0..size {
                    let src_idx = (y * size + x) as usize * 4;
                    let dst_x = x + padding;
                    let dst_y = y + padding;
                    let dst_idx = (dst_y * total_size + dst_x) as usize * 4;

                    if src_idx + 3 < pixel_data.len() && dst_idx + 3 < bits_slice.len() {
                        let alpha = pixel_data[src_idx + 3];
                        if alpha > 0 {
                            // 有内容的像素，使用预乘Alpha格式设置为黑色
                            let alpha_f = alpha as f32 / 255.0;
                            let icon_r = 0; // 图标颜色：黑色
                            let icon_g = 0;
                            let icon_b = 0;

                            // 预乘Alpha：RGB值需要乘以Alpha值
                            bits_slice[dst_idx] = (icon_b as f32 * alpha_f) as u8; // B
                            bits_slice[dst_idx + 1] = (icon_g as f32 * alpha_f) as u8; // G
                            bits_slice[dst_idx + 2] = (icon_r as f32 * alpha_f) as u8; // R
                            bits_slice[dst_idx + 3] = alpha; // A
                        }
                    }
                }
            }

            Some(bitmap)
        }
    }

    /// 创建带标题栏背景色的图标位图
    fn create_transparent_icon_bitmap(
        screen_dc: &HDC,
        pixmap: &tiny_skia::Pixmap,
        size: i32,
    ) -> Option<HBITMAP> {
        unsafe {
            // 创建DIB段位图以支持Alpha通道
            let bitmap_info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: size,
                    biHeight: -size, // 负值表示自顶向下
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [RGBQUAD::default(); 1],
            };

            let mut bits_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
            let bitmap = CreateDIBSection(
                Some(*screen_dc),
                &bitmap_info,
                DIB_RGB_COLORS,
                &mut bits_ptr,
                None,
                0,
            )
            .ok()?;

            // 处理像素数据，图标部分为黑色，背景使用标题栏背景色
            let pixel_data = pixmap.data();
            let bits_slice =
                std::slice::from_raw_parts_mut(bits_ptr as *mut u8, (size * size * 4) as usize);

            // 标题栏背景色 - 与 paint_custom_caption 中的颜色保持一致
            let bg_r = 0xED;
            let bg_g = 0xED;
            let bg_b = 0xED;
            let bg_a = 255;

            for i in 0..(size * size) as usize {
                let src_idx = i * 4;
                let dst_idx = i * 4;

                if src_idx + 3 < pixel_data.len() && dst_idx + 3 < bits_slice.len() {
                    let _src_r = pixel_data[src_idx + 2];
                    let _src_g = pixel_data[src_idx + 1];
                    let _src_b = pixel_data[src_idx];
                    let alpha = pixel_data[src_idx + 3];

                    if alpha == 0 {
                        // 完全透明的像素，使用标题栏背景色（预乘Alpha）
                        bits_slice[dst_idx] = bg_b; // B
                        bits_slice[dst_idx + 1] = bg_g; // G
                        bits_slice[dst_idx + 2] = bg_r; // R
                        bits_slice[dst_idx + 3] = bg_a; // A
                    } else {
                        // 有内容的像素，使用预乘Alpha格式
                        let alpha_f = alpha as f32 / 255.0;
                        let icon_r = 0; // 图标颜色：黑色
                        let icon_g = 0;
                        let icon_b = 0;

                        // 预乘Alpha：RGB值需要乘以Alpha值
                        bits_slice[dst_idx] = (icon_b as f32 * alpha_f) as u8; // B
                        bits_slice[dst_idx + 1] = (icon_g as f32 * alpha_f) as u8; // G
                        bits_slice[dst_idx + 2] = (icon_r as f32 * alpha_f) as u8; // R
                        bits_slice[dst_idx + 3] = alpha; // A
                    }
                }
            }

            Some(bitmap)
        }
    }

    /// 加载带指定颜色的SVG位图（用于Pin激活为绿色）
    fn load_colored_pin_bitmaps(
        filename: &str,
        size: i32,
        icon_rgb: (u8, u8, u8),
    ) -> Option<(HBITMAP, HBITMAP)> {
        unsafe {
            let svg_path = format!("icons/{filename}");
            let svg_data = std::fs::read_to_string(&svg_path).ok()?;
            let tree = usvg::Tree::from_str(&svg_data, &usvg::Options::default()).ok()?;

            let mut pixmap = tiny_skia::Pixmap::new(size as u32, size as u32)?;
            pixmap.fill(tiny_skia::Color::TRANSPARENT);
            let svg_size = tree.size();
            let render_ts = tiny_skia::Transform::from_scale(
                size as f32 / svg_size.width(),
                size as f32 / svg_size.height(),
            );
            resvg::render(&tree, render_ts, &mut pixmap.as_mut());

            let screen_dc = GetDC(None);

            // 普通状态（透明背景） + 绿色图标
            let normal_bitmap = OcrResultWindow::create_transparent_icon_bitmap_colored(
                &screen_dc, &pixmap, size, icon_rgb,
            )?;

            // 悬停状态（浅蓝背景） + 绿色图标
            let hover_bitmap = OcrResultWindow::create_icon_bitmap_colored(
                &screen_dc,
                &pixmap,
                size,
                ICON_HOVER_BG_COLOR,
                icon_rgb,
            )?;

            let _ = ReleaseDC(None, screen_dc);

            Some((normal_bitmap, hover_bitmap))
        }
    }

    /// 创建带指定背景色且图标为指定颜色的位图（用于hover）
    fn create_icon_bitmap_colored(
        screen_dc: &HDC,
        pixmap: &tiny_skia::Pixmap,
        size: i32,
        bg_color: (u8, u8, u8),
        icon_rgb: (u8, u8, u8),
    ) -> Option<HBITMAP> {
        unsafe {
            let padding = ICON_HOVER_PADDING;
            let total_size = size + padding * 2;
            let bitmap_info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: total_size,
                    biHeight: -total_size,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [RGBQUAD::default(); 1],
            };
            let mut bits_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
            let bitmap = CreateDIBSection(
                Some(*screen_dc),
                &bitmap_info,
                DIB_RGB_COLORS,
                &mut bits_ptr,
                None,
                0,
            )
            .ok()?;

            let pixel_data = pixmap.data();
            let bits_slice = std::slice::from_raw_parts_mut(
                bits_ptr as *mut u8,
                (total_size * total_size * 4) as usize,
            );

            // 背景（带圆角）
            let radius = ICON_HOVER_RADIUS;
            for y in 0..total_size {
                for x in 0..total_size {
                    let dst_idx = (y * total_size + x) as usize * 4;
                    let in_rounded_rect = OcrResultWindow::is_point_in_rounded_rect(
                        x as f32,
                        y as f32,
                        total_size as f32,
                        total_size as f32,
                        radius,
                    );
                    if in_rounded_rect {
                        bits_slice[dst_idx] = bg_color.2;
                        bits_slice[dst_idx + 1] = bg_color.1;
                        bits_slice[dst_idx + 2] = bg_color.0;
                        bits_slice[dst_idx + 3] = 255;
                    } else {
                        // 标题栏背景
                        bits_slice[dst_idx] = 0xED;
                        bits_slice[dst_idx + 1] = 0xED;
                        bits_slice[dst_idx + 2] = 0xED;
                        bits_slice[dst_idx + 3] = 255;
                    }
                }
            }

            // 图标（居中，使用指定颜色）
            for y in 0..size {
                for x in 0..size {
                    let src_idx = (y * size + x) as usize * 4;
                    let dst_x = x + padding;
                    let dst_y = y + padding;
                    let dst_idx = (dst_y * total_size + dst_x) as usize * 4;
                    if src_idx + 3 < pixel_data.len() && dst_idx + 3 < bits_slice.len() {
                        let alpha = pixel_data[src_idx + 3];
                        if alpha > 0 {
                            // 使用预乘Alpha格式
                            let alpha_f = alpha as f32 / 255.0;
                            bits_slice[dst_idx] = (icon_rgb.2 as f32 * alpha_f) as u8; // B
                            bits_slice[dst_idx + 1] = (icon_rgb.1 as f32 * alpha_f) as u8; // G
                            bits_slice[dst_idx + 2] = (icon_rgb.0 as f32 * alpha_f) as u8; // R
                            bits_slice[dst_idx + 3] = alpha; // A
                        }
                    }
                }
            }

            Some(bitmap)
        }
    }

    /// 创建透明背景且图标为指定颜色的位图（用于普通状态）
    fn create_transparent_icon_bitmap_colored(
        screen_dc: &HDC,
        pixmap: &tiny_skia::Pixmap,
        size: i32,
        icon_rgb: (u8, u8, u8),
    ) -> Option<HBITMAP> {
        unsafe {
            let bitmap_info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: size,
                    biHeight: -size,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [RGBQUAD::default(); 1],
            };
            let mut bits_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
            let bitmap = CreateDIBSection(
                Some(*screen_dc),
                &bitmap_info,
                DIB_RGB_COLORS,
                &mut bits_ptr,
                None,
                0,
            )
            .ok()?;

            let pixel_data = pixmap.data();
            let bits_slice =
                std::slice::from_raw_parts_mut(bits_ptr as *mut u8, (size * size * 4) as usize);

            // 填充标题栏背景色
            for i in 0..(size * size) as usize {
                let dst_idx = i * 4;
                bits_slice[dst_idx] = 0xED;
                bits_slice[dst_idx + 1] = 0xED;
                bits_slice[dst_idx + 2] = 0xED;
                bits_slice[dst_idx + 3] = 255;
            }

            // 绘制图标像素为指定颜色
            for i in 0..(size * size) as usize {
                let src_idx = i * 4;
                let dst_idx = i * 4;
                if src_idx + 3 < pixel_data.len() && dst_idx + 3 < bits_slice.len() {
                    let alpha = pixel_data[src_idx + 3];

                    if alpha > 0 {
                        // 使用预乘Alpha格式
                        let alpha_f = alpha as f32 / 255.0;
                        bits_slice[dst_idx] = (icon_rgb.2 as f32 * alpha_f) as u8; // B
                        bits_slice[dst_idx + 1] = (icon_rgb.1 as f32 * alpha_f) as u8; // G
                        bits_slice[dst_idx + 2] = (icon_rgb.0 as f32 * alpha_f) as u8; // R
                        bits_slice[dst_idx + 3] = alpha; // A
                    }
                }
            }

            Some(bitmap)
        }
    }

    /// 创建并显示 OCR 结果窗口
    pub fn show(
        image_data: Vec<u8>,
        ocr_results: Vec<OcrResult>,
        selection_rect: RECT,
    ) -> Result<()> {
        unsafe {
            let _ = SetProcessDpiAwareness(PROCESS_PER_MONITOR_DPI_AWARE);
            // 注册窗口类
            let class_name = windows::core::w!("OcrResultWindow");
            let instance = windows::Win32::System::LibraryLoader::GetModuleHandleW(None)?;

            // 使用默认应用程序图标
            let _icon = LoadIconW(None, IDI_APPLICATION).unwrap_or_default();

            let window_class = WNDCLASSW {
                lpfnWndProc: Some(Self::window_proc),
                hInstance: instance.into(),
                lpszClassName: class_name,
                hCursor: LoadCursorW(None, IDC_ARROW)?,
                hbrBackground: HBRUSH(GetStockObject(BLACK_BRUSH).0), // 使用黑色背景以支持DWM扩展
                style: CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS,          // 添加双缓冲支持
                hIcon: HICON::default(),                              // 不使用图标
                ..Default::default()
            };

            RegisterClassW(&window_class);

            // 从 BMP 数据获取实际图片尺寸
            let (bitmap, actual_width, actual_height) = Self::create_bitmap_from_data(&image_data)?;

            // 获取屏幕尺寸
            let (screen_width, screen_height) = crate::platform::windows::system::get_screen_size();

            // 右边文字区域宽度（固定350像素）
            let text_area_width = 350;

            // 图像保持原始尺寸，不进行缩放
            let display_image_width = actual_width;
            let display_image_height = actual_height;

            // 左边图像区域宽度（实际显示宽度 + 边距，比图片大一圈）
            let image_area_width = display_image_width + 40; // 左右各20像素边距
            // 总窗口宽度
            let window_width = image_area_width + text_area_width + 20; // 中间分隔20像素

            // 使用平台封装获取准确的窗口装饰尺寸
            let caption_height = crate::platform::windows::system::get_caption_height();
            let border_height = crate::platform::windows::system::get_border_height();
            let frame_height = crate::platform::windows::system::get_frame_height();

            // 计算窗口装饰的总高度
            let window_decoration_height =
                caption_height + (border_height * 2) + (frame_height * 2);

            // 增加更多的内容边距，确保有足够空间
            let content_padding = 120; // 上下各60像素边距，增加空间

            // 窗口总高度 = 图像高度 + 窗口装饰高度 + 内容边距
            // 再额外增加一些空间以确保不被截断
            let extra_space = 50;
            let window_height =
                display_image_height + window_decoration_height + content_padding + extra_space;

            // 计算窗口位置（在截图区域附近显示，避免超出屏幕）

            let mut window_x = selection_rect.right + 20; // 在截图区域右侧
            let mut window_y = selection_rect.top;

            // 确保窗口不超出屏幕边界
            if window_x + window_width > screen_width {
                window_x = selection_rect.left - window_width - 20; // 放在左侧
                if window_x < 0 {
                    window_x = 50; // 如果左侧也放不下，就放在屏幕左边
                }
            }
            if window_y + window_height > screen_height {
                window_y = screen_height - window_height - 50;
                if window_y < 0 {
                    window_y = 50;
                }
            }

            // 创建窗口 - 使用 WS_OVERLAPPEDWINDOW
            let hwnd = CreateWindowExW(
                WS_EX_APPWINDOW, 
                class_name,
                windows::core::w!("识别结果"),
                WS_OVERLAPPEDWINDOW | WS_VISIBLE | WS_CLIPCHILDREN, 
                window_x,
                window_y,
                window_width,
                window_height,
                None,
                None,
                Some(instance.into()),
                None,
            )?;

            // 扩展 DWM 边框以显示阴影
            let margins = MARGINS {
                cxLeftWidth: 0,
                cxRightWidth: 0,
                cyTopHeight: 1, 
                cyBottomHeight: 0,
            };
            let _ = DwmExtendFrameIntoClientArea(hwnd, &margins as *const MARGINS as *const _);

            // 触发一次 WM_NCCALCSIZE 以去除标准边框
            SetWindowPos(
                hwnd, 
                None, 
                0, 0, 0, 0, 
                SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE
            )?;

            // 位图已经在上面创建了
            let width = actual_width;
            let height = actual_height;

            // 创建微软雅黑字体
            let font_name: Vec<u16> = "微软雅黑"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let font = CreateFontW(
                24,                                        // 字体高度（增大字体）
                0,                                         // 字体宽度（0表示自动）
                0,                                         // 文本角度
                0,                                         // 基线角度
                FW_NORMAL.0 as i32,                        // 字体粗细
                0,                                         // 斜体
                0,                                         // 下划线
                0,                                         // 删除线
                DEFAULT_CHARSET,                           // 字符集
                OUT_DEFAULT_PRECIS,                        // 输出精度
                CLIP_DEFAULT_PRECIS,                       // 裁剪精度
                DEFAULT_QUALITY,                           // 输出质量
                (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,  // 字体间距和族
                windows::core::PCWSTR(font_name.as_ptr()), // 字体名称
            );

            // 计算文字显示区域，适应新的标题栏高度
            let title_bar_height = TITLE_BAR_HEIGHT; // 使用实际的标题栏高度
            let text_padding_left = 20; // 左侧padding
            let text_padding_right = 20; // 右侧padding
            let text_padding_top = title_bar_height + 15; // 顶部padding（包含标题栏高度）
            let text_padding_bottom = 15; // 底部padding

            let text_area_rect = RECT {
                left: image_area_width + text_padding_left,
                top: text_padding_top,
                right: window_width - text_padding_right,
                bottom: window_height - text_padding_bottom,
            };

            // 计算行高（基于字体）
            let line_height = {
                let hdc = GetDC(Some(hwnd));
                let old_font = SelectObject(hdc, font.into());
                let mut tm = TEXTMETRICW::default();
                let _ = GetTextMetricsW(hdc, &mut tm);
                let height = tm.tmHeight + tm.tmExternalLeading + 4; // 添加行间距
                let _ = SelectObject(hdc, old_font);
                let _ = ReleaseDC(Some(hwnd), hdc);
                height
            };

            // 合并所有OCR结果为文本
            let mut all_text = String::new();
            let mut is_no_text_detected = false;

            for (i, result) in ocr_results.iter().enumerate() {
                if i > 0 {
                    all_text.push_str("\r\n"); // Windows换行符
                }

                // 检查是否是"未识别到文字"的特殊情况
                if result.text == "未识别到任何文字" && result.confidence == 0.0 {
                    is_no_text_detected = true;
                    all_text.push_str("未识别到任何文字");
                } else {
                    all_text.push_str(&result.text);
                }
            }

            if all_text.trim().is_empty() {
                all_text = "未识别到文本内容".to_string();
                is_no_text_detected = true;
            }

            // 分行处理文本
            let text_lines = Self::wrap_text_lines(&all_text, &text_area_rect, font, hwnd);

            // 创建图标缓存
            let icon_cache = IconCache::new().ok_or_else(|| anyhow::anyhow!("无法创建图标缓存"))?;

            // 创建左侧图标
            let svg_icons = Self::create_left_icons(&icon_cache);

            // 创建窗口实例
            let mut window = Self {
                hwnd,
                image_bitmap: Some(bitmap),
                image_width: width,
                image_height: height,
                font,
                text_area_rect,
                window_width,
                window_height,
                is_no_text_detected,
                is_maximized: false,
                svg_icons,

                // 图标缓存
                icon_cache,

                // 置顶状态
                is_pinned: false,

                // 初始化缓冲相关字段
                content_buffer: None,
                buffer_width: 0,
                buffer_height: 0,
                buffer_valid: false,

                // 初始化自绘文本字段
                text_content: all_text,
                scroll_offset: 0,
                line_height,
                text_lines,

                // 初始化文本选择字段
                is_selecting: false,
                selection_start: None,
                selection_end: None,
                selection_start_pixel: None,
                selection_end_pixel: None,
                last_click_time: std::time::Instant::now(),
                last_click_pos: None,
            };

            // 添加标题栏按钮
            let mut title_bar_buttons =
                window.create_title_bar_buttons_from_cache(window_width, false);
            window.svg_icons.append(&mut title_bar_buttons);

            // 将窗口实例指针存储到窗口数据中
            let window_ptr = Box::into_raw(Box::new(window));
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, window_ptr as isize);

            // 获取窗口的实际客户端区域尺寸，重新计算布局
            let window = &mut *window_ptr;
            let mut client_rect = RECT::default();
            let _ = GetClientRect(hwnd, &mut client_rect);
            let actual_width = client_rect.right - client_rect.left;
            let actual_height = client_rect.bottom - client_rect.top;

            // 更新窗口尺寸并重新计算标题栏按钮位置
            window.window_width = actual_width;
            window.window_height = actual_height;
            window.recalculate_layout();
            window.update_title_bar_buttons();

            // 显示窗口
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = UpdateWindow(hwnd);

            Ok(())
        }
    }

    /// 从 BMP 数据创建位图
    fn create_bitmap_from_data(bmp_data: &[u8]) -> Result<(HBITMAP, i32, i32)> {
        unsafe {
            if bmp_data.len() < 54 {
                return Err(anyhow::anyhow!("BMP 数据太小"));
            }

            // 检查BMP文件签名
            if bmp_data[0] != b'B' || bmp_data[1] != b'M' {
                return Err(anyhow::anyhow!("不是有效的BMP文件"));
            }

            // 读取数据偏移量（像素数据开始位置）
            let data_offset =
                u32::from_le_bytes([bmp_data[10], bmp_data[11], bmp_data[12], bmp_data[13]])
                    as usize;

            // 解析 BMP 头部获取尺寸信息
            let width =
                i32::from_le_bytes([bmp_data[18], bmp_data[19], bmp_data[20], bmp_data[21]]);
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

            // 获取屏幕 DC
            let screen_dc = GetDC(None);

            // 创建DIB段位图以支持更好的像素数据处理
            let bitmap_info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    biHeight: -height, // 负值表示自顶向下
                    biPlanes: 1,
                    biBitCount: 32, // 强制使用32位以确保兼容性
                    biCompression: BI_RGB.0,
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [RGBQUAD::default(); 1],
            };

            let mut bits_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
            let bitmap = CreateDIBSection(
                Some(screen_dc),
                &bitmap_info,
                DIB_RGB_COLORS,
                &mut bits_ptr,
                None,
                0,
            )
            .map_err(|e| anyhow::anyhow!("创建DIB段失败: {:?}", e))?;

            // 获取像素数据（使用正确的偏移量）
            let pixel_data = &bmp_data[data_offset..];

            // 计算每行的字节数（考虑4字节对齐）
            let bytes_per_pixel = (bit_count / 8) as usize;
            let row_size = (width as usize * bytes_per_pixel).div_ceil(4) * 4; // 4字节对齐

            if !bits_ptr.is_null() {
                let bits_slice = std::slice::from_raw_parts_mut(
                    bits_ptr as *mut u8,
                    (width * height * 4) as usize, // 输出总是32位
                );

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

                        if src_idx + bytes_per_pixel <= pixel_data.len()
                            && dst_idx + 3 < bits_slice.len()
                        {
                            match bit_count {
                                24 => {
                                    // 24位BMP: BGR -> BGRA
                                    bits_slice[dst_idx] = pixel_data[src_idx]; // B
                                    bits_slice[dst_idx + 1] = pixel_data[src_idx + 1]; // G
                                    bits_slice[dst_idx + 2] = pixel_data[src_idx + 2]; // R
                                    bits_slice[dst_idx + 3] = 255; // A - 不透明
                                }
                                32 => {
                                    // 32位BMP: BGRA -> BGRA，确保Alpha通道不透明
                                    bits_slice[dst_idx] = pixel_data[src_idx]; // B
                                    bits_slice[dst_idx + 1] = pixel_data[src_idx + 1]; // G
                                    bits_slice[dst_idx + 2] = pixel_data[src_idx + 2]; // R
                                    bits_slice[dst_idx + 3] = 255; // A - 强制设置为不透明
                                }
                                _ => {
                                    // 其他格式，设置为白色
                                    bits_slice[dst_idx] = 255; // B
                                    bits_slice[dst_idx + 1] = 255; // G
                                    bits_slice[dst_idx + 2] = 255; // R
                                    bits_slice[dst_idx + 3] = 255; // A
                                }
                            }
                        }
                    }
                }
            }

            let _ = ReleaseDC(None, screen_dc);

            Ok((bitmap, width, height))
        }
    }

    /// 绘制窗口内容（不包含BeginPaint/EndPaint调用）- 使用静态缓冲机制
    fn paint_content_only(&mut self, hdc: HDC) -> Result<()> {
        unsafe {
            let mut rect = RECT::default();
            GetClientRect(self.hwnd, &mut rect)?;

            // 检查是否全屏，计算实际的内容开始位置
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

            // 简化的标题栏高度（参考test.rs，不使用偏移）
            let title_bar_height = TITLE_BAR_HEIGHT;
            let actual_content_start = title_bar_height;

            // 去掉全屏间隙填充逻辑（参考test.rs的简洁方式）

            // 计算内容区域尺寸（去除标题栏和偏移量）
            let content_width = rect.right - rect.left;
            let content_height = rect.bottom - actual_content_start;

            // 检查尺寸是否合理
            if content_width <= 0 || content_height <= 0 {
                return Ok(()); // 无效尺寸，跳过绘制
            }

            // 确保缓冲区存在且尺寸正确
            self.ensure_content_buffer(content_width, content_height)?;

            // 如果缓冲区无效（第一次创建或尺寸变化），重新渲染静态内容
            if !self.buffer_valid {
                self.render_to_buffer(hdc)?;
            }

            // 将缓冲区内容复制到屏幕（这是唯一每次都要做的操作）
            if let Some(buffer_bitmap) = self.content_buffer {
                let buffer_dc = CreateCompatibleDC(Some(hdc));
                if buffer_dc.is_invalid() {
                    return Err(anyhow::anyhow!("创建缓冲DC失败"));
                }

                let old_bitmap = SelectObject(buffer_dc, buffer_bitmap.into());

                // 从实际的内容开始位置绘制
                let _ = BitBlt(
                    hdc,
                    0,
                    actual_content_start, // 考虑全屏偏移量
                    content_width,
                    content_height,
                    Some(buffer_dc),
                    0,
                    0,
                    SRCCOPY,
                );

                SelectObject(buffer_dc, old_bitmap);
                let _ = DeleteDC(buffer_dc);
            }

            Ok(())
        }
    }

    /// 处理窗口大小变化，使用缓冲机制减少闪烁
    fn handle_size_change(&mut self, new_width: i32, new_height: i32) {
        // 直接更新窗口尺寸
        self.window_width = new_width;
        self.window_height = new_height;

        // 重新计算布局（包括文本编辑控件的重新定位）
        self.recalculate_layout();

        // 计算缓冲区变化幅度，只有在尺寸变化较大时才重建缓冲区
        let buffer_width_diff = (self.buffer_width - new_width).abs();
        let buffer_height_diff = (self.buffer_height - new_height).abs();

        // 如果缓冲区尺寸变化较大，则使其无效
        if buffer_width_diff >= 20 || buffer_height_diff >= 20 {
            self.buffer_valid = false;
        }
    }

    /// 自定义标题栏处理函数（根据官方文档重构）
    fn custom_caption_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        unsafe {
            // 记录鼠标事件
            if msg == WM_LBUTTONDOWN || msg == WM_LBUTTONUP || msg == WM_MOUSEMOVE {
                let x = (lparam.0 as i16) as i32;
                let y = ((lparam.0 >> 16) as i16) as i32;

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
                            // 尝试直接处理文本选择
                            let window_mut = &mut *(window_ptr as *mut OcrResultWindow);
                            window_mut.start_text_selection(x, y);
                            let _ = InvalidateRect(Some(hwnd), None, false);
                            return LRESULT(0);
                        }
                    }
                } else if msg == WM_MOUSEMOVE {
                    // 检查鼠标是否在文本区域内移动
                    let window_ptr =
                        GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const OcrResultWindow;
                    if !window_ptr.is_null() {
                        let window = &*window_ptr;
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
                    }
                }
            }

            match msg {
                WM_CREATE => {
                    let mut rect = RECT::default();
                    let _ = GetWindowRect(hwnd, &mut rect);

                    // 通知应用程序框架变化
                    let _ = SetWindowPos(
                        hwnd,
                        None,
                        rect.left,
                        rect.top,
                        rect.right - rect.left,
                        rect.bottom - rect.top,
                        SWP_FRAMECHANGED,
                    );

                    LRESULT(0)
                }
                WM_ACTIVATE => {
                    // 扩展框架到客户端区域（简化版本）
                    let margins = MARGINS {
                        cxLeftWidth: 0,
                        cxRightWidth: 0,
                        cyTopHeight: TITLE_BAR_HEIGHT,
                        cyBottomHeight: 0,
                    };

                    let _ =
                        DwmExtendFrameIntoClientArea(hwnd, &margins as *const MARGINS as *const _);
                    LRESULT(0)
                }
                WM_ERASEBKGND => {
                    // 阻止默认的背景擦除，减少闪烁
                    // 我们在 WM_PAINT 中自己处理背景绘制
                    LRESULT(1) // 返回非零值表示已处理
                }
                WM_PAINT => {
                    // 使用 GetDC 替代 BeginPaint 以避免最大化时的绘制限制
                    let hdc = GetDC(Some(hwnd));

                    if !hdc.is_invalid() {
                        // 绘制自定义标题栏
                        Self::paint_custom_caption(hwnd, hdc);

                        // 同时绘制窗口内容，避免重复绘制
                        let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Self;
                        if !window_ptr.is_null() {
                            let window = &mut *window_ptr;
                            let _ = window.paint_content_only(hdc);
                        }

                        let _ = ReleaseDC(Some(hwnd), hdc);

                        // 手动验证更新区域，防止无限重绘
                        let _ = ValidateRect(Some(hwnd), None);
                    }

                    LRESULT(0)
                }
                WM_NCCALCSIZE => {
                    if wparam.0 != 0 {
                        let params = lparam.0 as *mut NCCALCSIZE_PARAMS;
                        if !params.is_null() {
                            let rgrc = &mut (*params).rgrc;
                            
                            // 获取最大化状态
                            let is_maximized = {
                                let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Self;
                                if !window_ptr.is_null() {
                                    (*window_ptr).is_maximized
                                } else {
                                    let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
                                    (style & WS_MAXIMIZE.0) != 0
                                }
                            };

                            if is_maximized {
                                let frame_thickness = Self::get_frame_thickness(hwnd);
                                rgrc[0].left += frame_thickness;
                                rgrc[0].top += frame_thickness;
                                rgrc[0].right -= frame_thickness;
                                rgrc[0].bottom -= frame_thickness;
                            }
                        }
                        LRESULT(0)
                    } else {
                        DefWindowProcW(hwnd, msg, wparam, lparam)
                    }
                }
                WM_NCHITTEST => {
                    Self::hit_test_nca(hwnd, wparam, lparam)
                }
                _ => {
                    // 其他消息交给应用程序处理
                    Self::app_window_proc(hwnd, msg, wparam, lparam)
                }
            }
        }
    }
    /// 绘制自定义标题栏（静态函数）- 使用双缓冲减少闪烁
    fn paint_custom_caption(hwnd: HWND, hdc: HDC) {
        unsafe {
            let mut rect = RECT::default();
            let _ = GetClientRect(hwnd, &mut rect);

            // 创建支持Alpha通道的DIB位图（简化版本）
            let buffer_width = rect.right;
            
            // 如果宽度无效，直接返回
            if buffer_width <= 0 { return; }

            let bitmap_info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: buffer_width,
                    biHeight: -TITLE_BAR_HEIGHT, // 使用调整后的高度
                    biPlanes: 1,
                    biBitCount: 32, // 32位支持Alpha通道
                    biCompression: BI_RGB.0,
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [RGBQUAD::default(); 1],
            };

            let mut bits_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
            let buffer_bitmap = CreateDIBSection(
                Some(hdc),
                &bitmap_info,
                DIB_RGB_COLORS,
                &mut bits_ptr,
                None,
                0,
            );

            let buffer_bitmap = match buffer_bitmap {
                Ok(bitmap) => {
                    if bitmap.is_invalid() || bits_ptr.is_null() {
                        return; // 创建失败，直接返回
                    }
                    bitmap
                }
                Err(_) => return, // 创建失败，直接返回
            };

            let mem_dc = CreateCompatibleDC(Some(hdc));
            let old_bitmap = SelectObject(mem_dc, buffer_bitmap.into());

            // 获取窗口实例来访问标题栏按钮和图标
            let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Self;
            if !window_ptr.is_null() {
                let window = &*window_ptr;

                // 手动填充像素数据，使用预乘Alpha格式
                let pixel_count = (buffer_width * TITLE_BAR_HEIGHT) as usize;
                let bits_slice =
                    std::slice::from_raw_parts_mut(bits_ptr as *mut u8, pixel_count * 4);

                // 填充自定义标题栏背景色，使用预乘Alpha格式
                // 使用浅灰色 (#EDEDED) 作为标题栏背景
                let bg_r = 0xED;
                let bg_g = 0xED;
                let bg_b = 0xED;
                let bg_a = 255; // 完全不透明

                for i in 0..pixel_count {
                    let idx = i * 4;
                    if idx + 3 < bits_slice.len() {
                        // 使用预乘Alpha格式：RGB值需要乘以Alpha值
                        let alpha_factor = bg_a as f32 / 255.0;
                        bits_slice[idx] = (bg_b as f32 * alpha_factor) as u8; // B
                        bits_slice[idx + 1] = (bg_g as f32 * alpha_factor) as u8; // G
                        bits_slice[idx + 2] = (bg_r as f32 * alpha_factor) as u8; // R
                        bits_slice[idx + 3] = bg_a; // A
                    }
                }

                // 绘制所有SVG图标到内存DC（包括标题栏按钮）。Pin 置顶时为绿色
                for icon in &window.svg_icons {
                    // 略过不需要绘制的图标（例如如果窗口太小）
                    if icon.rect.right > buffer_width { continue; }
                    
                    let icon_size = icon.rect.right - icon.rect.left;

                    // 根据悬停状态和是否置顶选择正确的位图（Pin使用绿色激活位图）
                    let bitmap_to_use = if icon.name == "pin" && window.is_pinned {
                        if icon.hovered {
                            window.icon_cache.pin_active_hover
                        } else {
                            window.icon_cache.pin_active_normal
                        }
                    } else if icon.hovered {
                        icon.hover_bitmap
                    } else {
                        icon.normal_bitmap
                    };

                    // 绘制图标到内存DC
                    let icon_mem_dc = CreateCompatibleDC(Some(mem_dc));
                    let old_icon_bitmap = SelectObject(icon_mem_dc, bitmap_to_use.into());

                    // 对于标题栏按钮，处理 Hover 状态的背景
                    if icon.hovered && icon.is_title_bar_button {
                        // 计算按钮区域的左边界
                        // 对于 Hover 状态，我们可能使用了更宽的位图（如关闭按钮）
                        let button_left = icon.rect.left - (BUTTON_WIDTH_OCR - icon_size) / 2;

                        // 关闭按钮延伸到窗口右边缘，去掉右边间隙
                        let (draw_x, draw_width) = if icon.name == "x" {
                            // 关闭按钮：从按钮左边界延伸到窗口右边缘
                            let window_right = window.window_width;
                            (button_left, window_right - button_left)
                        } else {
                            // 其他按钮：正常的按钮宽度
                            (button_left, BUTTON_WIDTH_OCR)
                        };

                        // 绘制hover位图
                        let _ = BitBlt(
                            mem_dc,
                            draw_x,
                            0, // 从顶部开始绘制
                            draw_width,
                            TITLE_BAR_HEIGHT, // 使用标题栏高度
                            Some(icon_mem_dc),
                            0,
                            0,
                            SRCCOPY,
                        );
                    } else if icon.hovered && !icon.is_title_bar_button {
                        // 普通图标 Hover
                        let padding = ICON_HOVER_PADDING;
                        let total_size = icon_size + padding * 2;

                        let _ = BitBlt(
                            mem_dc,
                            icon.rect.left - padding,
                            icon.rect.top - padding,
                            total_size,
                            total_size,
                            Some(icon_mem_dc),
                            0,
                            0,
                            SRCCOPY,
                        );
                    } else {
                        // 普通图标正常状态
                        let _ = BitBlt(
                            mem_dc,
                            icon.rect.left,
                            icon.rect.top,
                            icon_size,
                            icon_size,
                            Some(icon_mem_dc),
                            0,
                            0,
                            SRCCOPY,
                        );
                    }

                    SelectObject(icon_mem_dc, old_icon_bitmap);
                    let _ = DeleteDC(icon_mem_dc);
                }
            }

            // 简化的绘制参数
            let draw_height = std::cmp::min(TITLE_BAR_HEIGHT, rect.bottom);
            let draw_width = rect.right;

            // 使用 AlphaBlend 绘制到目标 DC，确保透明通道正确处理
            let blend_function = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };

            let _ = AlphaBlend(
                hdc,
                0,
                0,
                draw_width,
                draw_height,
                mem_dc,
                0,
                0,
                draw_width,
                draw_height,
                blend_function,
            );

            SelectObject(mem_dc, old_bitmap);
            let _ = DeleteDC(mem_dc);
            let _ = DeleteObject(buffer_bitmap.into());
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
        self.buffer_valid = false;
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
            self.buffer_valid = false; // 强制重新渲染缓冲区
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
                self.buffer_valid = false; // 强制重新渲染缓冲区
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
        self.buffer_valid = false;
        unsafe {
            let adjusted_rect = self.get_adjusted_text_rect_for_redraw();
            let _ = InvalidateRect(Some(self.hwnd), Some(&adjusted_rect), false);
            let _ = UpdateWindow(self.hwnd);
        }
    }

    /// 将像素坐标转换为文本位置 (行号, 字符位置)
    fn pixel_to_text_position(&self, x: i32, y: i32) -> Option<(usize, usize)> {
        unsafe {
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

            // 使用实际的字体度量来计算字符位置
            let hdc = GetDC(Some(self.hwnd));
            let old_font = SelectObject(hdc, self.font.into());

            let chars: Vec<char> = line.chars().collect();
            let mut best_char_index = 0;
            let mut min_distance = i32::MAX;

            // 逐个字符测量，找到最接近点击位置的字符
            for i in 0..=chars.len() {
                let text_to_measure: String = chars.iter().take(i).collect();
                let text_wide: Vec<u16> = text_to_measure.encode_utf16().collect();

                let mut size = SIZE::default();
                let _ = GetTextExtentPoint32W(hdc, &text_wide, &mut size);

                let char_x = size.cx;
                let distance = (char_x - relative_x).abs();

                if distance < min_distance {
                    min_distance = distance;
                    best_char_index = i;
                }

                // 如果已经超过了点击位置，可以提前退出
                if char_x > relative_x {
                    break;
                }
            }

            let _ = SelectObject(hdc, old_font);
            let _ = ReleaseDC(Some(self.hwnd), hdc);

            Some((line_index, best_char_index))
        }
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

    /// 绘制选择高亮背景
    fn draw_selection_highlight(
        &self,
        hdc: HDC,
        line_index: usize,
        text_rect: &RECT,
        y: i32,
        line_height: i32,
    ) {
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

            // 检查当前行是否在选择范围内
            if line_index >= start_line && line_index <= end_line {
                unsafe {
                    let selection_brush = CreateSolidBrush(COLORREF(0x00C8F7C5)); // 淡绿色高亮

                    let line_text = &self.text_lines[line_index];

                    let (highlight_start_char, highlight_end_char) =
                        if line_index == start_line && line_index == end_line {
                            // 同一行内的选择
                            (start_char, end_char)
                        } else if line_index == start_line {
                            // 选择的第一行
                            (start_char, line_text.chars().count())
                        } else if line_index == end_line {
                            // 选择的最后一行
                            (0, end_char)
                        } else {
                            // 中间行，全选
                            (0, line_text.chars().count())
                        };

                    // 计算实际的字符位置
                    let chars: Vec<char> = line_text.chars().collect();

                    // 测量到开始字符的宽度
                    let start_text: String = chars.iter().take(highlight_start_char).collect();
                    let start_text_wide: Vec<u16> = start_text.encode_utf16().collect();
                    let mut start_size = SIZE { cx: 0, cy: 0 };
                    let _ = GetTextExtentPoint32W(hdc, &start_text_wide, &mut start_size);

                    // 测量到结束字符的宽度
                    let end_text: String = chars.iter().take(highlight_end_char).collect();
                    let end_text_wide: Vec<u16> = end_text.encode_utf16().collect();
                    let mut end_size = SIZE { cx: 0, cy: 0 };
                    let _ = GetTextExtentPoint32W(hdc, &end_text_wide, &mut end_size);

                    let highlight_start_x = text_rect.left + 10 + start_size.cx;
                    let highlight_end_x = text_rect.left + 10 + end_size.cx;

                    let highlight_rect = RECT {
                        left: highlight_start_x,
                        top: y,
                        right: highlight_end_x,
                        bottom: y + line_height,
                    };

                    let _ = FillRect(hdc, &highlight_rect, selection_brush);
                    let _ = DeleteObject(selection_brush.into());
                }
            }
        }
    }

    /// 绘制文本区域到指定的DC（用于缓冲区绘制）
    fn draw_text_area_to_dc(&self, hdc: HDC) {
        unsafe {
            // 计算文本区域在缓冲区中的位置（简化版本）
            let title_bar_height = TITLE_BAR_HEIGHT;
            let text_rect = RECT {
                left: self.text_area_rect.left,
                top: self.text_area_rect.top - title_bar_height, // 减去实际的标题栏高度
                right: self.text_area_rect.right,
                bottom: self.text_area_rect.bottom - title_bar_height,
            };

            // 绘制文本区域背景（白色）
            let white_brush = CreateSolidBrush(COLORREF(0x00FFFFFF));
            let _ = FillRect(hdc, &text_rect, white_brush);
            let _ = DeleteObject(white_brush.into());
            // 设置文本绘制属性
            let old_font = SelectObject(hdc, self.font.into());
            let _text_color_result = SetTextColor(hdc, COLORREF(0x00000000)); // 黑色文字
            let _bk_mode_result = SetBkMode(hdc, TRANSPARENT);

            let line_height = self.line_height;
            let scroll_offset = self.scroll_offset;

            // 如果没有文本内容，显示提示信息
            if self.text_lines.is_empty() || self.text_content.is_empty() {
                let hint_text = if self.is_no_text_detected {
                    "未识别到文字"
                } else {
                    "正在加载文本..."
                };
                let hint_wide: Vec<u16> = hint_text.encode_utf16().collect();
                let _ = TextOutW(hdc, text_rect.left + 10, text_rect.top + 10, &hint_wide);
            } else {
                // 计算可见区域
                let visible_height = text_rect.bottom - text_rect.top;
                let start_line = (scroll_offset / line_height).max(0) as usize;
                let end_line = ((scroll_offset + visible_height) / line_height + 1)
                    .min(self.text_lines.len() as i32) as usize;

                // 绘制可见的文本行
                for (i, line) in self
                    .text_lines
                    .iter()
                    .enumerate()
                    .skip(start_line)
                    .take(end_line - start_line)
                {
                    let y = text_rect.top + (i as i32 * line_height) - scroll_offset + 10; // 添加上边距

                    if y + line_height > text_rect.top && y < text_rect.bottom {
                        // 绘制选择高亮背景
                        self.draw_selection_highlight(hdc, i, &text_rect, y, line_height);

                        let line_wide: Vec<u16> = line.encode_utf16().collect();
                        let _ = TextOutW(
                            hdc,
                            text_rect.left + 10, // 添加左边距
                            y,
                            &line_wide,
                        );
                    }
                }
            }

            let _ = SelectObject(hdc, old_font);
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
                                    "minus" => {
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
                                    "square" => {
                                        // 执行最大化
                                        let _ = ShowWindow(hwnd, SW_MAXIMIZE);
                                        // 立即更新状态
                                        window.is_maximized = true;
                                        window.update_title_bar_buttons();
                                        // 最大化完成后调用图标居中函数
                                        window.center_icons();
                                        let _ = InvalidateRect(Some(hwnd), None, false);
                                        return LRESULT(0);
                                    }
                                    "reduction" => {
                                        // 执行还原
                                        let _ = ShowWindow(hwnd, SW_RESTORE);
                                        // 立即更新状态
                                        window.is_maximized = false;
                                        window.update_title_bar_buttons();
                                        // 还原完成后也调用图标居中函数
                                        window.center_icons();
                                        let _ = InvalidateRect(Some(hwnd), None, false);
                                        return LRESULT(0);
                                    }
                                    "x" => {
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
