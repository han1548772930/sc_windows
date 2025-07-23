use crate::ocr::OcrResult;
use anyhow::Result;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

/// OCR 结果显示窗口
pub struct OcrResultWindow {
    hwnd: HWND,
    image_data: Vec<u8>,
    ocr_results: Vec<OcrResult>,
    image_bitmap: Option<HBITMAP>,
    image_width: i32,
    image_height: i32,
    font: HFONT,
    text_scroll_offset: i32, // 文字区域的滚动偏移
    text_area_rect: RECT,    // 文字显示区域
    window_width: i32,       // 窗口宽度
    window_height: i32,      // 窗口高度
}

impl OcrResultWindow {
    /// 创建并显示 OCR 结果窗口
    pub fn show(
        image_data: Vec<u8>,
        ocr_results: Vec<OcrResult>,
        selection_rect: RECT,
    ) -> Result<()> {
        unsafe {
            // 注册窗口类
            let class_name = windows::core::w!("OcrResultWindow");
            let instance = windows::Win32::System::LibraryLoader::GetModuleHandleW(None)?;

            // 使用与托盘相同的图标
            let icon = crate::system_tray::create_default_icon().unwrap_or_else(|_| {
                // 如果加载失败，使用默认应用程序图标
                LoadIconW(None, IDI_APPLICATION).unwrap_or_default()
            });

            let window_class = WNDCLASSW {
                lpfnWndProc: Some(Self::window_proc),
                hInstance: instance.into(),
                lpszClassName: class_name,
                hCursor: LoadCursorW(None, IDC_ARROW)?,
                hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as *mut _),
                style: CS_HREDRAW | CS_VREDRAW,
                hIcon: icon, // 设置窗口图标
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

            // 创建标准窗口，带标题栏
            let hwnd = CreateWindowExW(
                WS_EX_OVERLAPPEDWINDOW,
                class_name,
                windows::core::w!("识别结果"),    // 窗口标题
                WS_OVERLAPPEDWINDOW | WS_VISIBLE, // 标准窗口样式
                window_x,
                window_y,
                window_width,
                window_height,
                None,
                None,
                Some(instance.into()),
                None,
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

            // 计算文字显示区域，增加更多padding
            let text_padding_left = 20; // 左侧padding
            let text_padding_right = 20; // 右侧padding
            let text_padding_top = 15; // 顶部padding
            let text_padding_bottom = 15; // 底部padding

            let text_area_rect = RECT {
                left: image_area_width + text_padding_left,
                top: text_padding_top,
                right: window_width - text_padding_right,
                bottom: window_height - text_padding_bottom,
            };

            // 创建窗口实例
            let window = Self {
                hwnd,
                image_data,
                ocr_results,
                image_bitmap: Some(bitmap),
                image_width: width,
                image_height: height,
                font,
                text_scroll_offset: 0,
                text_area_rect,
                window_width,
                window_height,
            };

            // 将窗口实例指针存储到窗口数据中
            let window_ptr = Box::into_raw(Box::new(window));
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, window_ptr as isize);

            // 显示窗口
            ShowWindow(hwnd, SW_SHOW);
            UpdateWindow(hwnd);

            Ok(())
        }
    }

    /// 从 BMP 数据创建位图
    fn create_bitmap_from_data(bmp_data: &[u8]) -> Result<(HBITMAP, i32, i32)> {
        unsafe {
            if bmp_data.len() < 54 {
                return Err(anyhow::anyhow!("BMP 数据太小"));
            }

            // 解析 BMP 头部获取尺寸信息
            let width =
                i32::from_le_bytes([bmp_data[18], bmp_data[19], bmp_data[20], bmp_data[21]]);
            let height =
                i32::from_le_bytes([bmp_data[22], bmp_data[23], bmp_data[24], bmp_data[25]]).abs(); // 取绝对值，因为可能是负数

            // 获取屏幕 DC
            let screen_dc = GetDC(None);

            // 创建兼容的内存 DC
            let mem_dc = CreateCompatibleDC(Some(screen_dc));

            // 创建兼容的位图
            let bitmap = CreateCompatibleBitmap(screen_dc, width, height);

            // 选择位图到内存 DC
            let old_bitmap = SelectObject(mem_dc, bitmap.into());

            // 创建 BITMAPINFO 结构
            let bitmap_info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    biHeight: -height, // 负值表示自顶向下
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

            // 获取像素数据（跳过 BMP 文件头）
            let pixel_data = &bmp_data[54..];

            // 将像素数据设置到位图
            SetDIBits(
                Some(mem_dc),
                bitmap,
                0,
                height as u32,
                pixel_data.as_ptr() as *const _,
                &bitmap_info,
                DIB_RGB_COLORS,
            );

            // 清理
            SelectObject(mem_dc, old_bitmap);
            DeleteDC(mem_dc);
            ReleaseDC(None, screen_dc);

            Ok((bitmap, width, height))
        }
    }

    /// 绘制窗口内容
    fn paint(&self) -> Result<()> {
        unsafe {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(self.hwnd, &mut ps);

            let mut rect = RECT::default();
            GetClientRect(self.hwnd, &mut rect)?;

            // 设置背景色为白色
            let white_brush = CreateSolidBrush(COLORREF(0x00FFFFFF));
            FillRect(hdc, &rect, white_brush);

            // 绘制窗口边框
            let border_pen = CreatePen(PS_SOLID, 2, COLORREF(0x00CCCCCC));
            let old_pen = SelectObject(hdc, border_pen.into());
            let old_brush = SelectObject(hdc, GetStockObject(NULL_BRUSH));

            Rectangle(hdc, rect.left, rect.top, rect.right, rect.bottom);

            SelectObject(hdc, old_pen);
            SelectObject(hdc, old_brush);
            DeleteObject(border_pen.into());

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
                bottom: self.window_height - 10,
            };

            let border_pen = CreatePen(PS_SOLID, 1, COLORREF(0x00CCCCCC));
            let old_pen = SelectObject(hdc, border_pen.into());
            let old_brush = SelectObject(hdc, GetStockObject(NULL_BRUSH));

            Rectangle(
                hdc,
                image_rect.left,
                image_rect.top,
                image_rect.right,
                image_rect.bottom,
            );

            SelectObject(hdc, old_pen);
            SelectObject(hdc, old_brush);
            DeleteObject(border_pen.into());

            // 绘制实际的截图图像
            if let Some(bitmap) = self.image_bitmap {
                // 创建内存 DC 来绘制位图
                let mem_dc = CreateCompatibleDC(Some(hdc));
                let old_bitmap = SelectObject(mem_dc, bitmap.into());

                // 计算图像显示区域（保持宽高比，居中显示）
                let available_width = image_area_width - 40;
                let available_height = self.window_height - 40;

                let scale_x = available_width as f32 / self.image_width as f32;
                let scale_y = available_height as f32 / self.image_height as f32;
                let scale = scale_x.min(scale_y).min(1.0); // 不放大

                let scaled_width = (self.image_width as f32 * scale) as i32;
                let scaled_height = (self.image_height as f32 * scale) as i32;

                let x_offset = 20 + (available_width - scaled_width) / 2;
                let y_offset = 20 + (available_height - scaled_height) / 2;

                // 使用 StretchBlt 绘制缩放的图像
                StretchBlt(
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
                DeleteDC(mem_dc);
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

            // 右半部分显示识别的文本内容（带滚动）
            self.draw_text_area(hdc);

            // 恢复原来的字体
            SelectObject(hdc, old_font);

            DeleteObject(white_brush.into());
            EndPaint(self.hwnd, &ps);

            Ok(())
        }
    }

    /// 绘制文字区域（带滚动）
    fn draw_text_area(&self, hdc: HDC) {
        unsafe {
            // 绘制文字区域边框
            let border_pen = CreatePen(PS_SOLID, 1, COLORREF(0x00DDDDDD));
            let old_pen = SelectObject(hdc, border_pen.into());
            let old_brush = SelectObject(hdc, GetStockObject(NULL_BRUSH));

            Rectangle(
                hdc,
                self.text_area_rect.left,
                self.text_area_rect.top,
                self.text_area_rect.right,
                self.text_area_rect.bottom,
            );

            SelectObject(hdc, old_pen);
            SelectObject(hdc, old_brush);
            DeleteObject(border_pen.into());

            // 设置裁剪区域，只在文字区域内绘制
            let clip_region = CreateRectRgn(
                self.text_area_rect.left + 1,
                self.text_area_rect.top + 1,
                self.text_area_rect.right - 1,
                self.text_area_rect.bottom - 1,
            );
            SelectClipRgn(hdc, Some(clip_region));

            // 将所有识别的文本按行合并，每行一个换行
            let mut all_text = String::new();
            for (i, result) in self.ocr_results.iter().enumerate() {
                if i > 0 {
                    all_text.push('\n'); // 每行之间用单换行分隔
                }
                all_text.push_str(&result.text);
            }

            // 如果没有识别到文本，显示提示
            if all_text.trim().is_empty() {
                all_text = "未识别到文本内容".to_string();
            }

            // 计算文字绘制区域（考虑滚动偏移）
            let mut text_rect = RECT {
                left: self.text_area_rect.left + 10,
                top: self.text_area_rect.top + 10 - self.text_scroll_offset,
                right: self.text_area_rect.right - 10,
                bottom: self.text_area_rect.bottom + 1000, // 给足够的高度
            };

            let mut text_wide: Vec<u16> =
                all_text.encode_utf16().chain(std::iter::once(0)).collect();
            DrawTextW(
                hdc,
                &mut text_wide,
                &mut text_rect,
                DT_LEFT | DT_TOP | DT_WORDBREAK,
            );

            // 恢复裁剪区域
            SelectClipRgn(hdc, None);
            DeleteObject(clip_region.into());
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
            match msg {
                WM_PAINT => {
                    let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Self;
                    if !window_ptr.is_null() {
                        let window = &*window_ptr;
                        let _ = window.paint();
                    } else {
                        let mut ps = PAINTSTRUCT::default();
                        let _hdc = BeginPaint(hwnd, &mut ps);
                        EndPaint(hwnd, &ps);
                    }
                    LRESULT(0)
                }
                // 移除自定义左键处理，使用标准窗口行为
                WM_RBUTTONUP => {
                    // 右键点击关闭窗口
                    PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
                    LRESULT(0)
                }
                WM_KEYDOWN => {
                    // 处理键盘按键
                    if wparam.0 == VK_ESCAPE.0 as usize {
                        // ESC 键关闭窗口
                        PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
                    }
                    LRESULT(0)
                }
                WM_MOUSEWHEEL => {
                    // 处理鼠标滚轮事件
                    let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Self;
                    if !window_ptr.is_null() {
                        let window = &mut *window_ptr;
                        let delta = ((wparam.0 >> 16) & 0xFFFF) as i16 as i32;
                        let scroll_amount = 30; // 每次滚动的像素数

                        if delta > 0 {
                            // 向上滚动
                            window.text_scroll_offset =
                                (window.text_scroll_offset - scroll_amount).max(0);
                        } else {
                            // 向下滚动
                            window.text_scroll_offset += scroll_amount;
                        }

                        // 只重绘文字区域，避免图片闪烁
                        InvalidateRect(Some(hwnd), Some(&window.text_area_rect), FALSE.into());
                    }
                    LRESULT(0)
                }
                WM_CLOSE => {
                    let window_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Self;
                    if !window_ptr.is_null() {
                        let window = Box::from_raw(window_ptr);
                        // 清理字体资源
                        DeleteObject(window.font.into());
                        // 清理位图资源
                        if let Some(bitmap) = window.image_bitmap {
                            DeleteObject(bitmap.into());
                        }
                        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                    }
                    DestroyWindow(hwnd);
                    LRESULT(0)
                }
                _ => DefWindowProcW(hwnd, msg, wparam, lparam),
            }
        }
    }
}
