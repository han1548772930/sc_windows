use crate::ocr::OcrResult;
use anyhow::Result;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Dwm::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::HiDpi::PROCESS_PER_MONITOR_DPI_AWARE;
use windows::Win32::UI::HiDpi::SetProcessDpiAwareness;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

// 自定义标题栏常量
const LEFTEXTENDWIDTH: i32 = 0;
const RIGHTEXTENDWIDTH: i32 = 0;
const BOTTOMEXTENDWIDTH: i32 = 0;
const TOPEXTENDWIDTH: i32 = 50; // 增加标题栏高度

// SVG 图标常量
const ICON_SIZE: i32 = 24; // 图标大小
const ICON_SPACING: i32 = 20; // 图标间距
const ICON_START_X: i32 = 12; // 图标起始位置 - 左对齐
const ICON_HOVER_PADDING: i32 = 8; // 图标悬停背景padding
const ICON_CLICK_PADDING: i32 = 16; // 图标点击检测区域padding
const ICON_HOVER_BG_COLOR: (u8, u8, u8) = (0xE1, 0xF3, 0xFF); // 悬停背景颜色（浅蓝色）
const ICON_HOVER_RADIUS: f32 = 6.0; // 悬停背景圆角半径

// 标题栏按钮常量
const TITLE_BAR_BUTTON_WIDTH: i32 = 70; // 标题栏按钮宽度

// SVG 图标结构
#[derive(Clone)]
struct SvgIcon {
    name: String,
    bitmap: HBITMAP,
    hover_bitmap: HBITMAP, // 悬停状态的位图
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
    text_edit: HWND, // 可选择的文本编辑控件
    ocr_results: Vec<OcrResult>,
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

    // 图标缓存 - 避免重复创建相同图标
    left_icons_cache: Option<Vec<SvgIcon>>, // 左侧图标缓存（只创建一次）
}

impl OcrResultWindow {
    /// 清理所有资源
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

            // 清理SVG图标
            for icon in &self.svg_icons {
                let _ = DeleteObject(icon.bitmap.into());
                let _ = DeleteObject(icon.hover_bitmap.into());
            }
            self.svg_icons.clear();

            // 清理缓存的左侧图标
            if let Some(ref cached_icons) = self.left_icons_cache {
                for icon in cached_icons {
                    let _ = DeleteObject(icon.bitmap.into());
                    let _ = DeleteObject(icon.hover_bitmap.into());
                }
            }
            self.left_icons_cache = None;

            println!("已清理所有窗口资源");
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
                let result = DeleteObject(old_buffer.into());
                if !result.as_bool() {
                    println!("警告: 删除缓冲区失败");
                }
            }
            self.buffer_width = 0;
            self.buffer_height = 0;
            self.buffer_valid = false;
        }
    }

    /// 使缓冲区无效，强制下次重绘
    fn invalidate_content_buffer(&mut self) {
        self.buffer_valid = false;
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

            // 恢复原来的字体
            SelectObject(hdc, old_font);

            let _ = DeleteObject(white_brush.into());

            Ok(())
        }
    }

    /// 加载所有SVG图标并转换为位图
    fn load_all_svg_icons() -> Vec<SvgIcon> {
        let svg_files = vec![
            "pin.svg",
            "circle.svg",
            "move-up-right.svg",
            "pen.svg",
            "extracttext.svg",
            "languages.svg",
            "download.svg",
            "type.svg",
            "undo-2.svg",
            "check.svg",
        ];

        let mut icons = Vec::new();

        // 创建左侧的常规图标
        for (i, filename) in svg_files.iter().enumerate() {
            if let Some((normal_bitmap, hover_bitmap)) =
                Self::load_svg_icon_from_file(filename, ICON_SIZE)
            {
                let icon_x = ICON_START_X + i as i32 * (ICON_SIZE + ICON_SPACING);
                let icon_y = (TOPEXTENDWIDTH - ICON_SIZE) / 2;

                icons.push(SvgIcon {
                    name: filename.replace(".svg", "").to_string(),
                    bitmap: normal_bitmap,
                    hover_bitmap,
                    rect: RECT {
                        left: icon_x,
                        top: icon_y,
                        right: icon_x + ICON_SIZE,
                        bottom: icon_y + ICON_SIZE,
                    },
                    hovered: false,
                    is_title_bar_button: false, // 普通图标不是标题栏按钮
                });
            }
        }

        icons
    }

    /// 更新标题栏按钮状态
    fn update_title_bar_buttons(&mut self) {
        let window_width = self.window_width;

        // 先释放旧的标题栏按钮的位图资源，避免内存泄漏
        unsafe {
            for icon in &self.svg_icons {
                if icon.is_title_bar_button {
                    let _ = DeleteObject(icon.bitmap.into());
                    let _ = DeleteObject(icon.hover_bitmap.into());
                }
            }
        }

        // 移除旧的标题栏按钮，保留左侧图标
        self.svg_icons.retain(|icon| !icon.is_title_bar_button);

        // 确保有左侧图标（使用缓存）
        let has_left_icons = self.svg_icons.iter().any(|icon| !icon.is_title_bar_button);
        if !has_left_icons {
            // 获取缓存的左侧图标并克隆到svg_icons中
            if self.left_icons_cache.is_none() {
                self.left_icons_cache = Some(Self::load_all_svg_icons());
                println!("首次创建左侧图标缓存");
            }

            if let Some(ref left_icons) = self.left_icons_cache {
                for icon in left_icons {
                    self.svg_icons.push(SvgIcon {
                        name: icon.name.clone(),
                        bitmap: icon.bitmap,
                        hover_bitmap: icon.hover_bitmap,
                        rect: icon.rect,
                        hovered: false, // 重置悬停状态
                        is_title_bar_button: false,
                    });
                }
            }
        }

        // 创建新的标题栏按钮
        let mut title_bar_buttons = if self.is_maximized {
            Self::create_title_bar_buttons_maximized(window_width)
        } else {
            Self::create_title_bar_buttons(window_width)
        };

        self.svg_icons.append(&mut title_bar_buttons);
    }

    /// 创建最大化状态的标题栏按钮
    fn create_title_bar_buttons_maximized(window_width: i32) -> Vec<SvgIcon> {
        let mut buttons = Vec::new();

        // 标题栏按钮列表：关闭、还原、最小化（从右到左的正确顺序）
        let button_files = vec![
            "x.svg",         // 关闭（最右边）
            "reduction.svg", // 还原
            "minus.svg",     // 最小化（最左边）
        ];

        // 从右到左创建按钮
        for (i, filename) in button_files.iter().enumerate() {
            if let Some((normal_bitmap, hover_bitmap)) =
                Self::load_title_bar_button_from_file(filename)
            {
                // 按钮位置计算（从右边开始，关闭按钮贴边）
                let button_x = window_width - (i as i32 + 1) * TITLE_BAR_BUTTON_WIDTH;

                // 图标在按钮中心
                let icon_x = button_x + (TITLE_BAR_BUTTON_WIDTH - ICON_SIZE) / 2;
                let icon_y = (TOPEXTENDWIDTH - ICON_SIZE) / 2;

                buttons.push(SvgIcon {
                    name: filename.replace(".svg", "").to_string(),
                    bitmap: normal_bitmap,
                    hover_bitmap,
                    rect: RECT {
                        left: icon_x,
                        top: icon_y,
                        right: icon_x + ICON_SIZE,
                        bottom: icon_y + ICON_SIZE,
                    },
                    hovered: false,
                    is_title_bar_button: true, // 标记为标题栏按钮
                });
            }
        }

        buttons
    }

    /// 创建标题栏按钮
    fn create_title_bar_buttons(window_width: i32) -> Vec<SvgIcon> {
        let mut buttons = Vec::new();

        // 标题栏按钮列表：关闭、最大化、最小化（从右到左的正确顺序）
        let button_files = vec![
            "x.svg",      // 关闭（最右边）
            "square.svg", // 最大化
            "minus.svg",  // 最小化（最左边）
        ];

        // 从右到左创建按钮
        for (i, filename) in button_files.iter().enumerate() {
            if let Some((normal_bitmap, hover_bitmap)) =
                Self::load_title_bar_button_from_file(filename)
            {
                // 按钮位置计算（从右边开始，关闭按钮贴边）
                let button_x = window_width - (i as i32 + 1) * TITLE_BAR_BUTTON_WIDTH;

                // 图标在按钮中心
                let icon_x = button_x + (TITLE_BAR_BUTTON_WIDTH - ICON_SIZE) / 2;
                let icon_y = (TOPEXTENDWIDTH - ICON_SIZE) / 2;

                buttons.push(SvgIcon {
                    name: filename.replace(".svg", "").to_string(),
                    bitmap: normal_bitmap,
                    hover_bitmap,
                    rect: RECT {
                        left: icon_x,
                        top: icon_y,
                        right: icon_x + ICON_SIZE,
                        bottom: icon_y + ICON_SIZE,
                    },
                    hovered: false,
                    is_title_bar_button: true, // 标记为标题栏按钮
                });
            }
        }

        buttons
    }

    /// 重新计算窗口布局
    fn recalculate_layout(&mut self) {
        unsafe {
            // 右边文字区域宽度（固定350像素）
            let text_area_width = 350;

            // 左边图像区域宽度
            let image_area_width = self.window_width - text_area_width - 20; // 减去中间分隔20像素

            // 计算文字显示区域
            let title_bar_height = TOPEXTENDWIDTH;
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

            // 只有在文本区域真正改变时才重新定位文本编辑控件，减少闪烁
            if new_text_area_rect.left != self.text_area_rect.left
                || new_text_area_rect.top != self.text_area_rect.top
                || new_text_area_rect.right != self.text_area_rect.right
                || new_text_area_rect.bottom != self.text_area_rect.bottom
            {
                self.text_area_rect = new_text_area_rect;

                // 使用 BeginDeferWindowPos 和 EndDeferWindowPos 进行批量窗口操作，减少闪烁
                if let Ok(hdwp) = BeginDeferWindowPos(1) {
                    if let Ok(hdwp) = DeferWindowPos(
                        hdwp,
                        self.text_edit,
                        None,
                        self.text_area_rect.left,
                        self.text_area_rect.top,
                        self.text_area_rect.right - self.text_area_rect.left,
                        self.text_area_rect.bottom - self.text_area_rect.top,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    ) {
                        let _ = EndDeferWindowPos(hdwp);
                    }
                } else {
                    // 如果批量操作失败，回退到单个操作
                    let _ = SetWindowPos(
                        self.text_edit,
                        None,
                        self.text_area_rect.left,
                        self.text_area_rect.top,
                        self.text_area_rect.right - self.text_area_rect.left,
                        self.text_area_rect.bottom - self.text_area_rect.top,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                }

          
            }
        }
    }

    /// 从文件加载标题栏按钮，创建专门的悬停效果
    fn load_title_bar_button_from_file(filename: &str) -> Option<(HBITMAP, HBITMAP)> {
        unsafe {
            // 读取SVG文件
            let svg_path = format!("icons/{}", filename);
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

            // 创建悬停状态的位图（根据按钮类型选择背景色）
            let hover_bitmap = if filename == "x.svg" {
                // 关闭按钮使用红色背景
                Self::create_title_bar_button_bitmap_with_color(
                    &screen_dc,
                    &pixmap,
                    (0xE8, 0x4A, 0x4A),
                )?
            } else {
                // 其他按钮使用默认灰色背景
                Self::create_title_bar_button_bitmap(&screen_dc, &pixmap)?
            };

            let _ = ReleaseDC(None, screen_dc);

            Some((normal_bitmap, hover_bitmap))
        }
    }

    /// 创建标题栏按钮悬停位图（满宽背景）
    fn create_title_bar_button_bitmap(
        screen_dc: &HDC,
        pixmap: &tiny_skia::Pixmap,
    ) -> Option<HBITMAP> {
        Self::create_title_bar_button_bitmap_with_color(screen_dc, pixmap, (0xE0, 0xE0, 0xE0))
    }

    /// 创建带自定义颜色的标题栏按钮悬停位图
    fn create_title_bar_button_bitmap_with_color(
        screen_dc: &HDC,
        pixmap: &tiny_skia::Pixmap,
        hover_color: (u8, u8, u8),
    ) -> Option<HBITMAP> {
        unsafe {
            // 创建按钮宽度的位图
            let button_width = TITLE_BAR_BUTTON_WIDTH;
            let button_height = TOPEXTENDWIDTH;

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

            // 填充悬停背景色（使用传入的颜色）
            for y in 0..button_height {
                for x in 0..button_width {
                    let dst_idx = (y * button_width + x) as usize * 4;
                    if dst_idx + 3 < bits_slice.len() {
                        bits_slice[dst_idx] = hover_color.2; // B
                        bits_slice[dst_idx + 1] = hover_color.1; // G
                        bits_slice[dst_idx + 2] = hover_color.0; // R
                        bits_slice[dst_idx + 3] = 255; // A
                    }
                }
            }

            // 在中心绘制图标（保持原始颜色）
            let icon_x_offset = (button_width - ICON_SIZE) / 2;
            let icon_y_offset = (button_height - ICON_SIZE) / 2;

            for y in 0..ICON_SIZE {
                for x in 0..ICON_SIZE {
                    let src_idx = (y * ICON_SIZE + x) as usize * 4;
                    let dst_x = x + icon_x_offset;
                    let dst_y = y + icon_y_offset;
                    let dst_idx = (dst_y * button_width + dst_x) as usize * 4;

                    if src_idx + 3 < pixel_data.len() && dst_idx + 3 < bits_slice.len() {
                        let alpha = pixel_data[src_idx + 3];
                        if alpha > 128 {
                            // 只处理足够不透明的像素，保持原始颜色（黑色）
                            bits_slice[dst_idx] = 0; // B - 黑色
                            bits_slice[dst_idx + 1] = 0; // G - 黑色
                            bits_slice[dst_idx + 2] = 0; // R - 黑色
                            bits_slice[dst_idx + 3] = alpha; // 保持原始透明度
                        }
                        // alpha <= 128 的像素保持背景色，不绘制图标内容
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
            let svg_path = format!("icons/{}", filename);
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
                            // 圆角外的区域保持透明
                            bits_slice[dst_idx] = 0;
                            bits_slice[dst_idx + 1] = 0;
                            bits_slice[dst_idx + 2] = 0;
                            bits_slice[dst_idx + 3] = 0; // 透明
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
                            // 有内容的像素，设置为黑色
                            bits_slice[dst_idx] = 0; // B - 黑色
                            bits_slice[dst_idx + 1] = 0; // G - 黑色
                            bits_slice[dst_idx + 2] = 0; // R - 黑色
                            bits_slice[dst_idx + 3] = alpha; // 保持原始透明度
                        }
                    }
                }
            }

            Some(bitmap)
        }
    }

    /// 创建带完全透明背景的图标位图
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

            // 处理像素数据，图标部分为黑色，背景完全透明
            let pixel_data = pixmap.data();
            let bits_slice =
                std::slice::from_raw_parts_mut(bits_ptr as *mut u8, (size * size * 4) as usize);

            for i in 0..(size * size) as usize {
                let src_idx = i * 4;
                let dst_idx = i * 4;

                if src_idx + 3 < pixel_data.len() && dst_idx + 3 < bits_slice.len() {
                    let alpha = pixel_data[src_idx + 3];

                    if alpha == 0 {
                        // 完全透明的像素，保持完全透明（不设置任何背景色）
                        bits_slice[dst_idx] = 0; // B
                        bits_slice[dst_idx + 1] = 0; // G  
                        bits_slice[dst_idx + 2] = 0; // R
                        bits_slice[dst_idx + 3] = 0; // A - 完全透明
                    } else {
                        // 有内容的像素，设置为黑色
                        bits_slice[dst_idx] = 0; // B - 黑色
                        bits_slice[dst_idx + 1] = 0; // G - 黑色
                        bits_slice[dst_idx + 2] = 0; // R - 黑色
                        bits_slice[dst_idx + 3] = alpha; // 保持原始透明度
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

            // 使用与托盘相同的图标
            let _icon = crate::system_tray::create_default_icon().unwrap_or_else(|_| {
                // 如果加载失败，使用默认应用程序图标
                LoadIconW(None, IDI_APPLICATION).unwrap_or_default()
            });

            let window_class = WNDCLASSW {
                lpfnWndProc: Some(Self::window_proc),
                hInstance: instance.into(),
                lpszClassName: class_name,
                hCursor: LoadCursorW(None, IDC_ARROW)?,
                hbrBackground: HBRUSH::default(), // 不使用背景画刷，让我们自己控制绘制
                style: CS_HREDRAW | CS_VREDRAW,
                hIcon: HICON::default(), // 不使用图标
                ..Default::default()
            };

            RegisterClassW(&window_class);

            // 从 BMP 数据获取实际图片尺寸
            let (bitmap, actual_width, actual_height) = Self::create_bitmap_from_data(&image_data)?;

            // 获取屏幕尺寸
            let screen_width = GetSystemMetrics(SM_CXSCREEN);
            let screen_height = GetSystemMetrics(SM_CYSCREEN);

            // 右边文字区域宽度（固定350像素）
            let text_area_width = 350;

            // 图像保持原始尺寸，不进行缩放
            let display_image_width = actual_width;
            let display_image_height = actual_height;

            // 左边图像区域宽度（实际显示宽度 + 边距，比图片大一圈）
            let image_area_width = display_image_width + 40; // 左右各20像素边距
            // 总窗口宽度
            let window_width = image_area_width + text_area_width + 20; // 中间分隔20像素

            // 使用Windows API获取准确的窗口装饰尺寸
            let caption_height = GetSystemMetrics(SM_CYCAPTION); // 标题栏高度
            let border_height = GetSystemMetrics(SM_CYBORDER); // 边框高度
            let frame_height = GetSystemMetrics(SM_CYFRAME); // 窗口框架高度

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

            // 创建自定义标题栏窗口样式 - 完全禁用原生的最小化、最大化、关闭按钮
            let hwnd = CreateWindowExW(
                WS_EX_APPWINDOW, // 显示在任务栏
                class_name,
                windows::core::w!("识别结果"),              // 窗口标题
                WS_OVERLAPPED | WS_THICKFRAME | WS_VISIBLE, // 完全移除 WS_SYSMENU 来禁用所有原生标题栏按钮
                window_x,
                window_y,
                window_width,
                window_height,
                None,
                None,
                Some(instance.into()),
                None,
            )?;

            // 立即扩展DWM框架到客户端区域
            let margins = MARGINS {
                cxLeftWidth: LEFTEXTENDWIDTH,
                cxRightWidth: RIGHTEXTENDWIDTH,
                cyTopHeight: TOPEXTENDWIDTH,
                cyBottomHeight: BOTTOMEXTENDWIDTH,
            };
            let _ = DwmExtendFrameIntoClientArea(hwnd, &margins as *const MARGINS as *const _);

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
            let title_bar_height = TOPEXTENDWIDTH; // 使用实际的标题栏高度
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

            // 创建文本编辑控件（可选择的文本）
            let text_edit = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                windows::core::w!("EDIT"),
                windows::core::w!(""),
                WS_CHILD | WS_VISIBLE | WS_VSCROLL | WINDOW_STYLE(0x0004 | 0x0010), // ES_MULTILINE | ES_READONLY
                text_area_rect.left,
                text_area_rect.top,
                text_area_rect.right - text_area_rect.left,
                text_area_rect.bottom - text_area_rect.top,
                Some(hwnd),
                None,
                Some(instance.into()),
                None,
            )?;

            // 设置文本编辑控件的字体
            SendMessageW(
                text_edit,
                WM_SETFONT,
                Some(WPARAM(font.0 as usize)),
                Some(LPARAM(1)),
            );

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

            // 设置文本内容
            let text_wide: Vec<u16> = all_text.encode_utf16().chain(std::iter::once(0)).collect();
            SetWindowTextW(text_edit, windows::core::PCWSTR(text_wide.as_ptr()))?;

            // 加载所有SVG图标（普通图标 + 标题栏按钮）
            let mut svg_icons = Self::load_all_svg_icons();
            let mut title_bar_buttons = Self::create_title_bar_buttons(window_width);
            svg_icons.append(&mut title_bar_buttons);

            // 创建窗口实例
            let window = Self {
                hwnd,
                text_edit,
                ocr_results,
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

                // 初始化缓冲相关字段
                content_buffer: None,
                buffer_width: 0,
                buffer_height: 0,
                buffer_valid: false,

                // 初始化图标缓存
                left_icons_cache: None,
            };

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
            let row_size = ((width as usize * bytes_per_pixel + 3) / 4) * 4; // 4字节对齐

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

            // 计算内容区域尺寸（去除标题栏）
            let content_width = rect.right - rect.left;
            let content_height = rect.bottom - TOPEXTENDWIDTH;

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

                // 只绘制内容区域，避开标题栏
                let _ = BitBlt(
                    hdc,
                    0,
                    TOPEXTENDWIDTH, // 从标题栏下方开始
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
                    // 扩展框架到客户端区域
                    let margins = MARGINS {
                        cxLeftWidth: LEFTEXTENDWIDTH,
                        cxRightWidth: RIGHTEXTENDWIDTH,
                        cyTopHeight: TOPEXTENDWIDTH,
                        cyBottomHeight: BOTTOMEXTENDWIDTH,
                    };

                    let _ =
                        DwmExtendFrameIntoClientArea(hwnd, &margins as *const MARGINS as *const _);
                    LRESULT(0)
                }
                WM_PAINT => {
                    let mut ps = PAINTSTRUCT::default();
                    let hdc = BeginPaint(hwnd, &mut ps);

                    // 绘制自定义标题栏
                    Self::paint_custom_caption(hwnd, hdc);

                    // 同时绘制窗口内容，避免重复绘制
                    let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Self;
                    if !window_ptr.is_null() {
                        let window = &mut *window_ptr;
                        let _ = window.paint_content_only(hdc);
                    }

                    let _ = EndPaint(hwnd, &ps);
                    LRESULT(0)
                }
                WM_NCCALCSIZE => {
                    if wparam.0 != 0 {
                        let pncsp = lparam.0 as *mut NCCALCSIZE_PARAMS;
                        if !pncsp.is_null() {
                            let ncsp = &mut *pncsp;
                            // 完全移除所有边框，让客户端区域占据整个窗口
                            // 这样就彻底消除了左右padding
                            ncsp.rgrc[0].left = ncsp.rgrc[0].left;
                            ncsp.rgrc[0].top = ncsp.rgrc[0].top;
                            ncsp.rgrc[0].right = ncsp.rgrc[0].right;
                            ncsp.rgrc[0].bottom = ncsp.rgrc[0].bottom;
                        }
                        LRESULT(0)
                    } else {
                        DefWindowProcW(hwnd, msg, wparam, lparam)
                    }
                }
                WM_NCHITTEST => {
                    // 根据官方文档，这里实现自定义点击测试
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

            // 创建双缓冲：内存DC和兼容位图
            let mem_dc = CreateCompatibleDC(Some(hdc));
            let buffer_bitmap = CreateCompatibleBitmap(hdc, rect.right, TOPEXTENDWIDTH);
            let old_bitmap = SelectObject(mem_dc, buffer_bitmap.into());

            // 获取窗口实例来访问标题栏按钮和图标
            let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Self;
            if !window_ptr.is_null() {
                let window = &*window_ptr;

                // 在内存DC中绘制标题栏内容
                // 设置背景为浅灰色标题栏 (#EDEDED)
                let bg_brush = CreateSolidBrush(COLORREF(0x00EDEDED));
                let title_bar_rect = RECT {
                    left: 0,
                    top: 0,
                    right: rect.right,
                    bottom: TOPEXTENDWIDTH,
                };
                FillRect(mem_dc, &title_bar_rect, bg_brush);

                // 绘制标题文字
                SetTextColor(mem_dc, COLORREF(0x00000000)); // 黑色文字适配浅色背景
                SetBkMode(mem_dc, TRANSPARENT);

                let title_font = CreateFontW(
                    16,
                    0,
                    0,
                    0,
                    FW_NORMAL.0 as i32,
                    0,
                    0,
                    0,
                    DEFAULT_CHARSET,
                    OUT_DEFAULT_PRECIS,
                    CLIP_DEFAULT_PRECIS,
                    CLEARTYPE_QUALITY,
                    (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
                    windows::core::PCWSTR(
                        "微软雅黑\0".encode_utf16().collect::<Vec<u16>>().as_ptr(),
                    ),
                );
                let old_font = SelectObject(mem_dc, title_font.into());

                // 计算图标占用的总宽度
                let icon_count = window.svg_icons.len() as i32;
                let total_icon_width = icon_count * (ICON_SIZE + ICON_SPACING) - ICON_SPACING;
                let title_start_x = ICON_START_X + total_icon_width + 20; // 图标后面留20像素间距

                // 为标题栏按钮留出空间
                let button_area_width = TITLE_BAR_BUTTON_WIDTH * 3; // 三个按钮的宽度，无间距

                let mut title_rect = RECT {
                    left: title_start_x,
                    top: 8,
                    right: rect.right - button_area_width - 10, // 为按钮留出空间
                    bottom: TOPEXTENDWIDTH - 8,
                };

                let mut title_text = "OCR识别结果\0".encode_utf16().collect::<Vec<u16>>();
                DrawTextW(
                    mem_dc,
                    &mut title_text,
                    &mut title_rect,
                    DT_LEFT | DT_VCENTER | DT_SINGLELINE,
                );

                SelectObject(mem_dc, old_font);
                let _ = DeleteObject(title_font.into());

                // 绘制所有SVG图标到内存DC（包括标题栏按钮）
                for icon in &window.svg_icons {
                    let icon_size = icon.rect.right - icon.rect.left;

                    // 根据悬停状态选择正确的位图
                    let bitmap_to_use = if icon.hovered {
                        icon.hover_bitmap
                    } else {
                        icon.bitmap
                    };

                    // 绘制图标到内存DC
                    let icon_mem_dc = CreateCompatibleDC(Some(mem_dc));
                    let old_icon_bitmap = SelectObject(icon_mem_dc, bitmap_to_use.into());

                    if icon.hovered && icon.is_title_bar_button {
                        // 标题栏按钮悬停状态：绘制满高背景
                        let button_width = TITLE_BAR_BUTTON_WIDTH;
                        let button_x = icon.rect.left - (button_width - icon_size) / 2;
                        let _ = BitBlt(
                            mem_dc,
                            button_x,
                            0, // 从顶部开始
                            button_width,
                            TOPEXTENDWIDTH, // 满高
                            Some(icon_mem_dc),
                            0,
                            0,
                            SRCCOPY,
                        );
                    } else if icon.hovered && !icon.is_title_bar_button {
                        // 普通图标悬停状态：绘制圆角背景
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
                        // 普通状态：使用 BitBlt 绘制以保持清晰度
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

                let _ = DeleteObject(bg_brush.into());
            }

            // 一次性将整个缓冲区复制到屏幕DC，减少闪烁
            let _ = BitBlt(
                hdc,
                0,
                0,
                rect.right,
                TOPEXTENDWIDTH,
                Some(mem_dc),
                0,
                0,
                SRCCOPY,
            );

            // 清理双缓冲资源
            SelectObject(mem_dc, old_bitmap);
            let _ = DeleteObject(buffer_bitmap.into());
            let _ = DeleteDC(mem_dc);
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

            // 转换为客户端坐标
            let client_x = pt_mouse_x - rc_window.left;
            let client_y = pt_mouse_y - rc_window.top;

            // 获取窗口实例来检查按钮和图标区域
            let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Self;
            if !window_ptr.is_null() {
                let window = &*window_ptr;

                // 检查是否在标题栏区域
                if client_y >= 0 && client_y <= TOPEXTENDWIDTH {
                    for icon in &window.svg_icons {
                        let in_click_area = if icon.is_title_bar_button {
                            // 标题栏按钮：使用按钮的完整宽度区域进行点击检测
                            let button_width = TITLE_BAR_BUTTON_WIDTH;
                            let button_left = icon.rect.left - (button_width - ICON_SIZE) / 2;
                            let button_right = button_left + button_width;
                            client_x >= button_left
                                && client_x <= button_right
                                && client_y >= 0
                                && client_y <= TOPEXTENDWIDTH
                        } else {
                            // 普通图标：使用图标矩形区域加padding
                            let click_padding = ICON_CLICK_PADDING;
                            client_x >= (icon.rect.left - click_padding)
                                && client_x <= (icon.rect.right + click_padding)
                                && client_y >= (icon.rect.top - click_padding)
                                && client_y <= (icon.rect.bottom + click_padding)
                        };

                        if in_click_area {
                            // println!("点击测试: 鼠标在SVG图标 {} 区域", icon.name);
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
            if pt_mouse_y >= rc_window.top && pt_mouse_y < rc_window.top + TOPEXTENDWIDTH {
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
                [HTLEFT as isize, HTNOWHERE as isize, HTRIGHT as isize],
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
                WM_GETMINMAXINFO => {
                    // 设置窗口最小尺寸为800*500
                    let minmax_info = lparam.0 as *mut MINMAXINFO;
                    if !minmax_info.is_null() {
                        let info = &mut *minmax_info;
                        info.ptMinTrackSize.x = 800;
                        info.ptMinTrackSize.y = 500;
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

                        println!("WM_LBUTTONDOWN: 坐标 ({}, {})", x, y);

                        // 处理SVG图标点击（包括标题栏按钮）
                        for icon in &mut window.svg_icons {
                            let in_click_area = if icon.is_title_bar_button {
                                // 标题栏按钮：使用按钮的完整宽度区域进行点击检测
                                let button_width = TITLE_BAR_BUTTON_WIDTH;
                                let button_left = icon.rect.left - (button_width - ICON_SIZE) / 2;
                                let button_right = button_left + button_width;
                                x >= button_left
                                    && x <= button_right
                                    && y >= 0
                                    && y <= TOPEXTENDWIDTH
                            } else {
                                // 普通图标：使用图标矩形区域加padding
                                let click_padding = ICON_CLICK_PADDING;
                                x >= (icon.rect.left - click_padding)
                                    && x <= (icon.rect.right + click_padding)
                                    && y >= (icon.rect.top - click_padding)
                                    && y <= (icon.rect.bottom + click_padding)
                            };

                            if in_click_area {
                                println!("图标 {} 被点击", icon.name);

                                // 处理标题栏按钮点击
                                match icon.name.as_str() {
                                    "minus" => {
                                        println!("最小化按钮被点击");
                                        // 执行最小化
                                        let _ = ShowWindow(hwnd, SW_MINIMIZE);
                                        return LRESULT(0);
                                    }
                                    "square" => {
                                        println!("最大化按钮被点击");
                                        // 执行最大化
                                        let _ = ShowWindow(hwnd, SW_MAXIMIZE);
                                        // 立即更新状态
                                        window.is_maximized = true;
                                        window.update_title_bar_buttons();
                                        let _ = InvalidateRect(Some(hwnd), None, false);
                                        return LRESULT(0);
                                    }
                                    "reduction" => {
                                        println!("还原按钮被点击");
                                        // 执行还原
                                        let _ = ShowWindow(hwnd, SW_RESTORE);
                                        // 立即更新状态
                                        window.is_maximized = false;
                                        window.update_title_bar_buttons();
                                        let _ = InvalidateRect(Some(hwnd), None, false);
                                        return LRESULT(0);
                                    }
                                    "x" => {
                                        println!("关闭按钮被点击");
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
                                        println!("SVG图标 {} 被点击", icon.name);
                                        return LRESULT(0);
                                    }
                                }
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
                                        println!("图标 {} 被释放", icon.name);
                                    }
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

                        // 启用鼠标跟踪以确保接收到 WM_MOUSELEAVE 消息
                        let mut tme = TRACKMOUSEEVENT {
                            cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                            dwFlags: TME_LEAVE,
                            hwndTrack: hwnd,
                            dwHoverTime: 0,
                        };
                        let _ = TrackMouseEvent(&mut tme);

                        let mut needs_repaint = false;

                        // 处理SVG图标悬停 - 只在标题栏区域内检测
                        if y >= 0 && y <= TOPEXTENDWIDTH {
                            for icon in &mut window.svg_icons {
                                let hovered = if icon.is_title_bar_button {
                                    // 标题栏按钮：使用按钮的完整宽度区域进行悬停检测
                                    let button_width = TITLE_BAR_BUTTON_WIDTH;
                                    let button_left =
                                        icon.rect.left - (button_width - ICON_SIZE) / 2;
                                    let button_right = button_left + button_width;
                                    x >= button_left
                                        && x <= button_right
                                        && y >= 0
                                        && y <= TOPEXTENDWIDTH
                                } else {
                                    // 普通图标：使用padding进行悬停检测
                                    let hover_padding = ICON_HOVER_PADDING;
                                    x >= (icon.rect.left - hover_padding)
                                        && x <= (icon.rect.right + hover_padding)
                                        && y >= (icon.rect.top - hover_padding)
                                        && y <= (icon.rect.bottom + hover_padding)
                                };

                                if icon.hovered != hovered {
                                    icon.hovered = hovered;
                                    needs_repaint = true;
                                    // 减少调试输出以避免性能影响
                                    if hovered {
                                        println!("图标 {} 悬停", icon.name);
                                    }
                                }
                            }
                        } else {
                            // 如果鼠标不在标题栏区域，清除所有图标的悬停状态
                            for icon in &mut window.svg_icons {
                                if icon.hovered {
                                    icon.hovered = false;
                                    needs_repaint = true;
                                    println!("清除图标 {} 的悬停状态", icon.name);
                                }
                            }
                        }

                        if needs_repaint {
                            // 只标记需要重绘，不立即更新，让系统批量处理
                            let title_bar_rect = RECT {
                                left: 0,
                                top: 0,
                                right: window.window_width,
                                bottom: TOPEXTENDWIDTH,
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
                                println!("鼠标离开 - 清除图标 {} 的悬停状态", icon.name);
                            }
                        }

                        if needs_repaint {
                            let title_bar_rect = RECT {
                                left: 0,
                                top: 0,
                                right: window.window_width,
                                bottom: TOPEXTENDWIDTH,
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
                    }
                    LRESULT(0)
                }
                WM_CTLCOLOREDIT => {
                    // 处理Edit控件的颜色
                    let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Self;
                    if !window_ptr.is_null() {
                        let window = &*window_ptr;
                        let hdc = HDC(wparam.0 as *mut _);
                        let edit_hwnd = HWND(lparam.0 as *mut _);

                        // 检查是否是我们的文本编辑控件
                        if edit_hwnd == window.text_edit {
                            if window.is_no_text_detected {
                                // 设置灰色文本
                                SetTextColor(hdc, COLORREF(0x00808080)); // 灰色
                            } else {
                                // 设置正常黑色文本
                                SetTextColor(hdc, COLORREF(0x00000000)); // 黑色
                            }

                            // 设置白色背景
                            SetBkColor(hdc, COLORREF(0x00FFFFFF)); // 白色背景
                            SetBkMode(hdc, OPAQUE);

                            // 创建白色画刷作为背景
                            let white_brush = CreateSolidBrush(COLORREF(0x00FFFFFF));
                            return LRESULT(white_brush.0 as isize);
                        }
                    }
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }
                WM_MOUSEWHEEL => {
                    // 将滚轮事件转发给文本编辑控件
                    let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Self;
                    if !window_ptr.is_null() {
                        let window = &*window_ptr;
                        SendMessageW(window.text_edit, msg, Some(wparam), Some(lparam));
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
                                println!(
                                    "标题栏按钮已更新: 宽度从 {} 变为 {}",
                                    old_width, new_width
                                );
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
