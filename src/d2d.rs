use crate::WindowState;
use crate::utils::*;
use crate::*;

use std::ffi::c_void;

use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Dxgi::Common::*;

use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::Com::*;

use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::*;

impl WindowState {
    pub fn new(hwnd: HWND) -> Result<Self> {
        unsafe {
            // 初始化COM
            CoInitialize(None);

            let screen_width = GetSystemMetrics(SM_CXSCREEN);
            let screen_height = GetSystemMetrics(SM_CYSCREEN);

            // 创建传统GDI资源用于屏幕捕获
            let screen_dc = GetDC(HWND(std::ptr::null_mut()));
            let screenshot_dc = CreateCompatibleDC(screen_dc);
            let gdi_screenshot_bitmap =
                CreateCompatibleBitmap(screen_dc, screen_width, screen_height);
            SelectObject(screenshot_dc, gdi_screenshot_bitmap);

            // 捕获屏幕
            BitBlt(
                screenshot_dc,
                0,
                0,
                screen_width,
                screen_height,
                screen_dc,
                0,
                0,
                SRCCOPY,
            )?;
            ReleaseDC(HWND(std::ptr::null_mut()), screen_dc);

            // 创建Direct2D Factory
            let d2d_factory =
                D2D1CreateFactory::<ID2D1Factory>(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;

            // 创建DirectWrite Factory
            let dwrite_factory: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;

            // 创建基本文本格式
            let text_format = dwrite_factory.CreateTextFormat(
                w!("Microsoft YaHei"),
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                20.0,
                w!(""),
            )?;

            // 创建居中的文本格式（用于工具栏）
            let centered_text_format = dwrite_factory.CreateTextFormat(
                w!("Segoe UI Emoji"),
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                18.0,
                w!(""),
            )?;
            centered_text_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
            centered_text_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;

            // 创建渲染目标
            let render_target_properties = D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 96.0,
                dpiY: 96.0,
                usage: D2D1_RENDER_TARGET_USAGE_NONE,
                minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
            };

            let hwnd_render_target_properties = D2D1_HWND_RENDER_TARGET_PROPERTIES {
                hwnd,
                pixelSize: D2D_SIZE_U {
                    width: screen_width as u32,
                    height: screen_height as u32,
                },
                presentOptions: D2D1_PRESENT_OPTIONS_NONE,
            };

            let render_target = d2d_factory.CreateHwndRenderTarget(
                &render_target_properties,
                &hwnd_render_target_properties,
            )?;

            // 从GDI位图创建D2D位图
            let screenshot_bitmap = Self::create_d2d_bitmap_from_gdi(
                &render_target,
                screenshot_dc,
                screen_width,
                screen_height,
            )?;

            // 创建画刷
            let selection_border_brush =
                render_target.CreateSolidColorBrush(&COLOR_SELECTION_BORDER, None)?;
            let handle_fill_brush =
                render_target.CreateSolidColorBrush(&COLOR_HANDLE_FILL, None)?;
            let handle_border_brush =
                render_target.CreateSolidColorBrush(&COLOR_HANDLE_BORDER, None)?;
            let toolbar_bg_brush = render_target.CreateSolidColorBrush(&COLOR_TOOLBAR_BG, None)?;
            let button_hover_brush =
                render_target.CreateSolidColorBrush(&COLOR_BUTTON_HOVER, None)?;
            let button_active_brush =
                render_target.CreateSolidColorBrush(&COLOR_BUTTON_ACTIVE, None)?;
            let text_brush = render_target.CreateSolidColorBrush(&COLOR_TEXT_NORMAL, None)?;
            let mask_brush = render_target.CreateSolidColorBrush(&COLOR_MASK, None)?;

            // 创建圆角矩形几何
            let rounded_rect = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: 0.0,
                    top: 0.0,
                    right: 100.0,
                    bottom: 100.0,
                },
                radiusX: 6.0,
                radiusY: 6.0,
            };
            let rounded_rect_geometry =
                d2d_factory.CreateRoundedRectangleGeometry(&rounded_rect)?;

            Ok(WindowState {
                d2d_factory,
                render_target,
                screenshot_bitmap,
                dwrite_factory,
                text_format,
                centered_text_format,
                selection_border_brush,
                handle_fill_brush,
                handle_border_brush,
                toolbar_bg_brush,
                button_hover_brush,
                button_active_brush,
                text_brush,
                mask_brush,
                rounded_rect_geometry,
                screenshot_dc,
                gdi_screenshot_bitmap,
                screen_width,
                screen_height,
                selection_rect: RECT {
                    left: 0,
                    top: 0,
                    right: 0,
                    bottom: 0,
                },
                has_selection: false,
                drag_mode: DragMode::None,
                mouse_pressed: false,
                drag_start_pos: POINT { x: 0, y: 0 },
                drag_start_rect: RECT {
                    left: 0,
                    top: 0,
                    right: 0,
                    bottom: 0,
                },
                toolbar: Toolbar::new(),
                current_tool: DrawingTool::None,
                drawing_elements: Vec::new(),
                current_element: None,
                selected_element: None,
                drawing_color: D2D1_COLOR_F {
                    r: 1.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                },
                drawing_thickness: 3.0,
                history: Vec::new(),
                is_pinned: false,                     // 新增字段初始化
                original_window_pos: RECT::default(), // 新增字段初始化
            })
        }
    }
    pub fn pin_selection(&mut self, hwnd: HWND) -> Result<()> {
        unsafe {
            let width = self.selection_rect.right - self.selection_rect.left;
            let height = self.selection_rect.bottom - self.selection_rect.top;

            if width <= 0 || height <= 0 {
                return Ok(());
            }

            // 保存当前窗口位置（如果还没保存的话）
            if !self.is_pinned {
                let mut current_rect = RECT::default();
                GetWindowRect(hwnd, &mut current_rect);
                self.original_window_pos = current_rect;
            }

            // 获取选择区域的屏幕截图（包含绘图内容）
            let screen_dc = GetDC(HWND(std::ptr::null_mut()));
            let mem_dc = CreateCompatibleDC(screen_dc);
            let bitmap = CreateCompatibleBitmap(screen_dc, width, height);
            let old_bitmap = SelectObject(mem_dc, bitmap);

            // 直接从屏幕复制选择区域（包含窗口内容和绘图）
            BitBlt(
                mem_dc,
                0,
                0,
                width,
                height,
                screen_dc,
                self.selection_rect.left,
                self.selection_rect.top,
                SRCCOPY,
            );

            // 从GDI位图创建新的D2D位图
            if let Ok(new_d2d_bitmap) =
                Self::create_d2d_bitmap_from_gdi(&self.render_target, mem_dc, width, height)
            {
                // 替换当前的截图位图
                self.screenshot_bitmap = new_d2d_bitmap;
            }

            // 清理GDI资源
            SelectObject(mem_dc, old_bitmap);
            DeleteObject(bitmap);
            DeleteDC(mem_dc);
            ReleaseDC(HWND(std::ptr::null_mut()), screen_dc);

            // 调整窗口大小和位置到选择区域
            SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                self.selection_rect.left,
                self.selection_rect.top,
                width,
                height,
                SWP_SHOWWINDOW,
            );

            // 更新内部状态
            self.screen_width = width;
            self.screen_height = height;

            // 重置选择区域为整个新窗口
            self.selection_rect = RECT {
                left: 0,
                top: 0,
                right: width,
                bottom: height,
            };

            // 清除所有绘图元素和选择状态
            self.drawing_elements.clear();
            self.current_element = None;
            self.selected_element = None;
            self.current_tool = DrawingTool::None;
            self.has_selection = false;

            // 隐藏工具栏
            self.toolbar.hide();

            // 标记为已pin
            self.is_pinned = true;

            // 重新创建渲染目标以适应新尺寸
            if let Ok(new_render_target) = self.create_render_target_for_size(hwnd, width, height) {
                self.render_target = new_render_target;

                // 重新创建画刷（因为render_target改变了）
                if let Ok(brushes) = self.recreate_brushes() {
                    self.selection_border_brush = brushes.0;
                    self.handle_fill_brush = brushes.1;
                    self.handle_border_brush = brushes.2;
                    self.toolbar_bg_brush = brushes.3;
                    self.button_hover_brush = brushes.4;
                    self.button_active_brush = brushes.5;
                    self.text_brush = brushes.6;
                    self.mask_brush = brushes.7;
                }
            }

            Ok(())
        }
    }

    // 创建指定尺寸的渲染目标
    unsafe fn create_render_target_for_size(
        &self,
        hwnd: HWND,
        width: i32,
        height: i32,
    ) -> Result<ID2D1HwndRenderTarget> {
        let render_target_properties = D2D1_RENDER_TARGET_PROPERTIES {
            r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: 96.0,
            dpiY: 96.0,
            usage: D2D1_RENDER_TARGET_USAGE_NONE,
            minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
        };

        let hwnd_render_target_properties = D2D1_HWND_RENDER_TARGET_PROPERTIES {
            hwnd,
            pixelSize: D2D_SIZE_U {
                width: width as u32,
                height: height as u32,
            },
            presentOptions: D2D1_PRESENT_OPTIONS_NONE,
        };

        self.d2d_factory
            .CreateHwndRenderTarget(&render_target_properties, &hwnd_render_target_properties)
    }

    // 重新创建画刷
    unsafe fn recreate_brushes(
        &self,
    ) -> Result<(
        ID2D1SolidColorBrush,
        ID2D1SolidColorBrush,
        ID2D1SolidColorBrush,
        ID2D1SolidColorBrush,
        ID2D1SolidColorBrush,
        ID2D1SolidColorBrush,
        ID2D1SolidColorBrush,
        ID2D1SolidColorBrush,
    )> {
        Ok((
            self.render_target
                .CreateSolidColorBrush(&COLOR_SELECTION_BORDER, None)?,
            self.render_target
                .CreateSolidColorBrush(&COLOR_HANDLE_FILL, None)?,
            self.render_target
                .CreateSolidColorBrush(&COLOR_HANDLE_BORDER, None)?,
            self.render_target
                .CreateSolidColorBrush(&COLOR_TOOLBAR_BG, None)?,
            self.render_target
                .CreateSolidColorBrush(&COLOR_BUTTON_HOVER, None)?,
            self.render_target
                .CreateSolidColorBrush(&COLOR_BUTTON_ACTIVE, None)?,
            self.render_target
                .CreateSolidColorBrush(&COLOR_TEXT_NORMAL, None)?,
            self.render_target
                .CreateSolidColorBrush(&COLOR_MASK, None)?,
        ))
    }
    unsafe fn create_d2d_bitmap_from_gdi(
        render_target: &ID2D1HwndRenderTarget,
        gdi_dc: HDC,
        width: i32,
        height: i32,
    ) -> Result<ID2D1Bitmap> {
        // 创建DIB来传输像素数据
        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height, // 负值表示自上而下
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

        let mut pixels: *mut std::ffi::c_void = std::ptr::null_mut();
        let dib = CreateDIBSection(
            gdi_dc,
            &bmi,
            DIB_RGB_COLORS,
            &mut pixels,
            HANDLE(std::ptr::null_mut()),
            0,
        )?;

        let temp_dc = CreateCompatibleDC(gdi_dc);
        let old_bitmap = SelectObject(temp_dc, dib);

        BitBlt(temp_dc, 0, 0, width, height, gdi_dc, 0, 0, SRCCOPY)?;

        SelectObject(temp_dc, old_bitmap);
        DeleteDC(temp_dc);

        // 创建D2D位图
        let bitmap_properties = D2D1_BITMAP_PROPERTIES {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: 96.0,
            dpiY: 96.0,
        };

        let size = D2D_SIZE_U {
            width: width as u32,
            height: height as u32,
        };

        let stride = width as u32 * 4;
        let bitmap = render_target.CreateBitmap(
            size,
            Some(pixels as *const c_void),
            stride,
            &bitmap_properties,
        )?;

        DeleteObject(dib);
        Ok(bitmap)
    }

    pub fn paint(&self, hwnd: HWND) {
        unsafe {
            let mut ps = PAINTSTRUCT::default();
            BeginPaint(hwnd, &mut ps);
            self.render();
            EndPaint(hwnd, &ps);
        }
    }

    pub fn render(&self) {
        unsafe {
            self.render_target.BeginDraw();

            // 清除背景（透明）
            self.render_target.Clear(Some(&D2D1_COLOR_F {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            }));

            // 绘制截图背景
            let dest_rect = d2d_rect(0, 0, self.screen_width, self.screen_height);
            self.render_target.DrawBitmap(
                &self.screenshot_bitmap,
                Some(&dest_rect),
                1.0,
                D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                None,
            );

            // 如果是pinned状态，只显示图片，不显示选择框等UI元素
            if !self.is_pinned {
                if self.has_selection {
                    // 绘制遮罩
                    self.draw_dimmed_overlay();

                    // 绘制选择框边框
                    self.draw_selection_border();

                    // 设置裁剪区域到选择框
                    self.push_selection_clip();

                    // 绘制绘图元素（会被裁剪显示）
                    for element in &self.drawing_elements {
                        self.draw_element(element);
                    }

                    if let Some(ref element) = self.current_element {
                        self.draw_element(element);
                    }

                    // 恢复裁剪区域
                    self.pop_clip();

                    // 绘制选择框手柄（不被裁剪）
                    if self.current_tool == DrawingTool::None {
                        self.draw_handles();
                    }

                    // 绘制元素选择（不被裁剪）
                    self.draw_element_selection();

                    // 绘制工具栏（不被裁剪）
                    if self.toolbar.visible {
                        self.draw_toolbar();
                    }
                } else {
                    // 全屏遮罩
                    let screen_rect = d2d_rect(0, 0, self.screen_width, self.screen_height);
                    self.render_target
                        .FillRectangle(&screen_rect, &self.mask_brush);
                }
            }
            // 如果是pinned状态，什么都不绘制，只显示背景截图

            let _ = self.render_target.EndDraw(None, None);
        }
    }
    pub fn push_selection_clip(&self) {
        unsafe {
            let clip_rect = d2d_rect(
                self.selection_rect.left,
                self.selection_rect.top,
                self.selection_rect.right,
                self.selection_rect.bottom,
            );

            self.render_target
                .PushAxisAlignedClip(&clip_rect, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
        }
    }

    // 恢复裁剪区域
    pub fn pop_clip(&self) {
        unsafe {
            self.render_target.PopAxisAlignedClip();
        }
    }

    pub fn draw_dimmed_overlay(&self) {
        unsafe {
            // 绘制整个屏幕的遮罩
            let screen_rect = d2d_rect(0, 0, self.screen_width, self.screen_height);
            self.render_target
                .FillRectangle(&screen_rect, &self.mask_brush);

            // 在选择区域绘制原图
            let selection_rect = d2d_rect(
                self.selection_rect.left,
                self.selection_rect.top,
                self.selection_rect.right,
                self.selection_rect.bottom,
            );

            let source_rect = D2D_RECT_F {
                left: self.selection_rect.left as f32,
                top: self.selection_rect.top as f32,
                right: self.selection_rect.right as f32,
                bottom: self.selection_rect.bottom as f32,
            };

            self.render_target.DrawBitmap(
                &self.screenshot_bitmap,
                Some(&selection_rect),
                1.0,
                D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                Some(&source_rect),
            );
        }
    }

    pub fn draw_selection_border(&self) {
        unsafe {
            let rect = d2d_rect(
                self.selection_rect.left,
                self.selection_rect.top,
                self.selection_rect.right,
                self.selection_rect.bottom,
            );

            self.render_target
                .DrawRectangle(&rect, &self.selection_border_brush, 2.0, None);
        }
    }

    pub fn draw_handles(&self) {
        unsafe {
            let center_x = (self.selection_rect.left + self.selection_rect.right) / 2;
            let center_y = (self.selection_rect.top + self.selection_rect.bottom) / 2;
            let half_handle = HANDLE_SIZE / 2.0;

            let handles = [
                (self.selection_rect.left, self.selection_rect.top),
                (center_x, self.selection_rect.top),
                (self.selection_rect.right, self.selection_rect.top),
                (self.selection_rect.right, center_y),
                (self.selection_rect.right, self.selection_rect.bottom),
                (center_x, self.selection_rect.bottom),
                (self.selection_rect.left, self.selection_rect.bottom),
                (self.selection_rect.left, center_y),
            ];

            for (hx, hy) in handles.iter() {
                let handle_rect = D2D_RECT_F {
                    left: *hx as f32 - half_handle,
                    top: *hy as f32 - half_handle,
                    right: *hx as f32 + half_handle,
                    bottom: *hy as f32 + half_handle,
                };

                self.render_target
                    .FillRectangle(&handle_rect, &self.handle_fill_brush);
                self.render_target.DrawRectangle(
                    &handle_rect,
                    &self.handle_border_brush,
                    1.0,
                    None,
                );
            }
        }
    }

    pub fn draw_element(&self, element: &DrawingElement) {
        // 基本的边界检查
        let element_rect = element.get_bounding_rect();
        if element_rect.right < self.selection_rect.left
            || element_rect.left > self.selection_rect.right
            || element_rect.bottom < self.selection_rect.top
            || element_rect.top > self.selection_rect.bottom
        {
            return;
        }

        unsafe {
            let element_brush = self
                .render_target
                .CreateSolidColorBrush(&element.color, None);
            if let Ok(brush) = element_brush {
                match element.tool {
                    DrawingTool::Text => {
                        // 暂时用虚线框代替复杂的文本输入
                        if !element.points.is_empty() {
                            // 创建虚线样式
                            let stroke_style_properties = D2D1_STROKE_STYLE_PROPERTIES {
                                startCap: D2D1_CAP_STYLE_FLAT,
                                endCap: D2D1_CAP_STYLE_FLAT,
                                dashCap: D2D1_CAP_STYLE_FLAT,
                                lineJoin: D2D1_LINE_JOIN_MITER,
                                miterLimit: 10.0,
                                dashStyle: D2D1_DASH_STYLE_DASH,
                                dashOffset: 0.0,
                            };

                            if let Ok(dashed_stroke) = self
                                .d2d_factory
                                .CreateStrokeStyle(&stroke_style_properties, None)
                            {
                                // 绘制虚线文本框

                                let text_rect = d2d_rect(
                                    element.points[0].x,
                                    element.points[0].y,
                                    element.points[0].x + TEXT_BOX_WIDTH, // 使用常量
                                    element.points[0].y + TEXT_BOX_HEIGHT, // 使用常量
                                );

                                self.render_target.DrawRectangle(
                                    &text_rect,
                                    &brush,
                                    2.0,
                                    Some(&dashed_stroke),
                                );

                                // 在框内绘制占位文字
                                if !element.text.is_empty() {
                                    let text_wide = to_wide_chars(&element.text);
                                    self.render_target.DrawText(
                                        &text_wide[..text_wide.len() - 1],
                                        &self.text_format,
                                        &text_rect,
                                        &brush,
                                        D2D1_DRAW_TEXT_OPTIONS_NONE,
                                        DWRITE_MEASURING_MODE_NATURAL,
                                    );
                                } else {
                                    // 显示占位符
                                    let placeholder = "Text";
                                    let placeholder_wide = to_wide_chars(placeholder);
                                    self.render_target.DrawText(
                                        &placeholder_wide[..placeholder_wide.len() - 1],
                                        &self.text_format,
                                        &text_rect,
                                        &brush,
                                        D2D1_DRAW_TEXT_OPTIONS_NONE,
                                        DWRITE_MEASURING_MODE_NATURAL,
                                    );
                                }
                            }
                        }
                    }
                    DrawingTool::Rectangle => {
                        if element.points.len() >= 2 {
                            let rect = d2d_rect(
                                element.points[0].x,
                                element.points[0].y,
                                element.points[1].x,
                                element.points[1].y,
                            );
                            self.render_target.DrawRectangle(
                                &rect,
                                &brush,
                                element.thickness,
                                None,
                            );
                        }
                    }
                    DrawingTool::Circle => {
                        if element.points.len() >= 2 {
                            let center_x = (element.points[0].x + element.points[1].x) as f32 / 2.0;
                            let center_y = (element.points[0].y + element.points[1].y) as f32 / 2.0;
                            let radius_x =
                                (element.points[1].x - element.points[0].x).abs() as f32 / 2.0;
                            let radius_y =
                                (element.points[1].y - element.points[0].y).abs() as f32 / 2.0;

                            let ellipse = D2D1_ELLIPSE {
                                point: D2D_POINT_2F {
                                    x: center_x,
                                    y: center_y,
                                },
                                radiusX: radius_x,
                                radiusY: radius_y,
                            };

                            self.render_target.DrawEllipse(
                                &ellipse,
                                &brush,
                                element.thickness,
                                None,
                            );
                        }
                    }
                    DrawingTool::Arrow => {
                        if element.points.len() >= 2 {
                            let start = d2d_point(element.points[0].x, element.points[0].y);
                            let end = d2d_point(element.points[1].x, element.points[1].y);

                            self.render_target.DrawLine(
                                start,
                                end,
                                &brush,
                                element.thickness,
                                None,
                            );

                            // 绘制箭头头部
                            let dx = element.points[1].x - element.points[0].x;
                            let dy = element.points[1].y - element.points[0].y;
                            let length = ((dx * dx + dy * dy) as f64).sqrt();

                            if length > 20.0 {
                                let arrow_length = 15.0f64;
                                let arrow_angle = 0.5f64;
                                let unit_x = dx as f64 / length;
                                let unit_y = dy as f64 / length;

                                let wing1 = d2d_point(
                                    element.points[1].x
                                        - (arrow_length
                                            * (unit_x * arrow_angle.cos()
                                                + unit_y * arrow_angle.sin()))
                                            as i32,
                                    element.points[1].y
                                        - (arrow_length
                                            * (unit_y * arrow_angle.cos()
                                                - unit_x * arrow_angle.sin()))
                                            as i32,
                                );

                                let wing2 = d2d_point(
                                    element.points[1].x
                                        - (arrow_length
                                            * (unit_x * arrow_angle.cos()
                                                - unit_y * arrow_angle.sin()))
                                            as i32,
                                    element.points[1].y
                                        - (arrow_length
                                            * (unit_y * arrow_angle.cos()
                                                + unit_x * arrow_angle.sin()))
                                            as i32,
                                );

                                self.render_target.DrawLine(
                                    end,
                                    wing1,
                                    &brush,
                                    element.thickness,
                                    None,
                                );
                                self.render_target.DrawLine(
                                    end,
                                    wing2,
                                    &brush,
                                    element.thickness,
                                    None,
                                );
                            }
                        }
                    }
                    DrawingTool::Pen => {
                        if element.points.len() > 1 {
                            for i in 0..element.points.len() - 1 {
                                let start = d2d_point(element.points[i].x, element.points[i].y);
                                let end =
                                    d2d_point(element.points[i + 1].x, element.points[i + 1].y);
                                self.render_target.DrawLine(
                                    start,
                                    end,
                                    &brush,
                                    element.thickness,
                                    None,
                                );
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    pub fn draw_element_selection(&self) {
        if let Some(element_index) = self.selected_element {
            if element_index < self.drawing_elements.len() {
                let element = &self.drawing_elements[element_index];

                // 检查元素是否在选择框内可见
                if !self.is_element_visible_in_selection(element) {
                    return; // 如果元素不在选择框内，不显示选择手柄
                }

                if element.selected && element.tool != DrawingTool::Pen {
                    unsafe {
                        if element.tool == DrawingTool::Arrow && element.points.len() >= 2 {
                            // 箭头只显示起点和终点手柄（如果在选择框内）
                            let half_handle = HANDLE_SIZE / 2.0;

                            for point in &element.points[..2] {
                                // 只有当手柄在选择框内时才显示
                                if point.x >= self.selection_rect.left
                                    && point.x <= self.selection_rect.right
                                    && point.y >= self.selection_rect.top
                                    && point.y <= self.selection_rect.bottom
                                {
                                    let handle_ellipse = D2D1_ELLIPSE {
                                        point: D2D_POINT_2F {
                                            x: point.x as f32,
                                            y: point.y as f32,
                                        },
                                        radiusX: half_handle,
                                        radiusY: half_handle,
                                    };

                                    self.render_target
                                        .FillEllipse(&handle_ellipse, &self.handle_fill_brush);
                                    self.render_target.DrawEllipse(
                                        &handle_ellipse,
                                        &self.handle_border_brush,
                                        1.0,
                                        None,
                                    );
                                }
                            }
                        } else {
                            // 其他元素显示虚线框和8个手柄（只显示在选择框内的部分）
                            let element_rect = d2d_rect(
                                element.rect.left,
                                element.rect.top,
                                element.rect.right,
                                element.rect.bottom,
                            );

                            // 计算元素矩形与选择框的交集
                            let intersect_left = element.rect.left.max(self.selection_rect.left);
                            let intersect_top = element.rect.top.max(self.selection_rect.top);
                            let intersect_right = element.rect.right.min(self.selection_rect.right);
                            let intersect_bottom =
                                element.rect.bottom.min(self.selection_rect.bottom);

                            // 只有当有交集时才绘制
                            if intersect_left < intersect_right && intersect_top < intersect_bottom
                            {
                                // 设置裁剪区域到选择框
                                self.push_selection_clip();

                                // 创建虚线样式
                                let stroke_style_properties = D2D1_STROKE_STYLE_PROPERTIES {
                                    startCap: D2D1_CAP_STYLE_FLAT,
                                    endCap: D2D1_CAP_STYLE_FLAT,
                                    dashCap: D2D1_CAP_STYLE_FLAT,
                                    lineJoin: D2D1_LINE_JOIN_MITER,
                                    miterLimit: 10.0,
                                    dashStyle: D2D1_DASH_STYLE_DASH,
                                    dashOffset: 0.0,
                                };

                                if let Ok(stroke_style) = self
                                    .d2d_factory
                                    .CreateStrokeStyle(&stroke_style_properties, None)
                                {
                                    let dashed_brush = self
                                        .render_target
                                        .CreateSolidColorBrush(&COLOR_SELECTION_DASHED, None);
                                    if let Ok(brush) = dashed_brush {
                                        self.render_target.DrawRectangle(
                                            &element_rect,
                                            &brush,
                                            1.0,
                                            Some(&stroke_style),
                                        );
                                    }
                                }

                                // 恢复裁剪区域
                                self.pop_clip();

                                // 绘制8个手柄（只显示在选择框内的）
                                let center_x = (element.rect.left + element.rect.right) / 2;
                                let center_y = (element.rect.top + element.rect.bottom) / 2;
                                let half_handle = HANDLE_SIZE / 2.0;

                                let handles = [
                                    (element.rect.left, element.rect.top),
                                    (center_x, element.rect.top),
                                    (element.rect.right, element.rect.top),
                                    (element.rect.right, center_y),
                                    (element.rect.right, element.rect.bottom),
                                    (center_x, element.rect.bottom),
                                    (element.rect.left, element.rect.bottom),
                                    (element.rect.left, center_y),
                                ];

                                for (hx, hy) in handles.iter() {
                                    // 只有当手柄在选择框内时才显示
                                    if *hx >= self.selection_rect.left
                                        && *hx <= self.selection_rect.right
                                        && *hy >= self.selection_rect.top
                                        && *hy <= self.selection_rect.bottom
                                    {
                                        let handle_ellipse = D2D1_ELLIPSE {
                                            point: D2D_POINT_2F {
                                                x: *hx as f32,
                                                y: *hy as f32,
                                            },
                                            radiusX: half_handle,
                                            radiusY: half_handle,
                                        };

                                        self.render_target
                                            .FillEllipse(&handle_ellipse, &self.handle_fill_brush);
                                        self.render_target.DrawEllipse(
                                            &handle_ellipse,
                                            &self.handle_border_brush,
                                            1.0,
                                            None,
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn draw_toolbar(&self) {
        unsafe {
            // 绘制工具栏背景
            let toolbar_rounded_rect = D2D1_ROUNDED_RECT {
                rect: self.toolbar.rect,
                radiusX: 10.0,
                radiusY: 10.0,
            };

            self.render_target
                .FillRoundedRectangle(&toolbar_rounded_rect, &self.toolbar_bg_brush);

            // 绘制按钮
            for (rect, button_type, icon_data) in &self.toolbar.buttons {
                // 检查按钮是否应该被禁用
                let is_disabled = match button_type {
                    ToolbarButton::Undo => !self.can_undo(), // 撤销按钮根据历史记录状态
                    // 可以添加其他按钮的禁用逻辑
                    _ => false,
                };

                // 绘制按钮背景状态
                if is_disabled {
                    // 禁用状态 - 不绘制任何背景，保持默认状态
                } else if *button_type == self.toolbar.hovered_button {
                    // 悬停状态 - 只有未禁用的按钮才能悬停
                    let hover_color = D2D1_COLOR_F {
                        r: 0.75,
                        g: 0.75,
                        b: 0.75,
                        a: 1.0,
                    };

                    if let Ok(hover_brush) =
                        self.render_target.CreateSolidColorBrush(&hover_color, None)
                    {
                        let button_rounded_rect = D2D1_ROUNDED_RECT {
                            rect: *rect,
                            radiusX: 6.0,
                            radiusY: 6.0,
                        };
                        self.render_target
                            .FillRoundedRectangle(&button_rounded_rect, &hover_brush);
                    }
                } else {
                    // 检查是否是当前选中的工具 - 蓝色背景
                    let is_current_tool = match button_type {
                        ToolbarButton::Rectangle => self.current_tool == DrawingTool::Rectangle,
                        ToolbarButton::Circle => self.current_tool == DrawingTool::Circle,
                        ToolbarButton::Arrow => self.current_tool == DrawingTool::Arrow,
                        ToolbarButton::Pen => self.current_tool == DrawingTool::Pen,
                        ToolbarButton::Text => self.current_tool == DrawingTool::Text,
                        _ => false,
                    };

                    if is_current_tool {
                        let button_rounded_rect = D2D1_ROUNDED_RECT {
                            rect: *rect,
                            radiusX: 6.0,
                            radiusY: 6.0,
                        };
                        self.render_target
                            .FillRoundedRectangle(&button_rounded_rect, &self.button_active_brush);
                    }
                }

                // 确定文字颜色
                let text_color = if is_disabled {
                    // 禁用状态 - 浅灰色文字
                    D2D1_COLOR_F {
                        r: 0.6,
                        g: 0.6,
                        b: 0.6,
                        a: 1.0,
                    }
                } else if *button_type == self.toolbar.clicked_button {
                    // 点击状态 - 绿色文字
                    D2D1_COLOR_F {
                        r: 0.13,
                        g: 0.77,
                        b: 0.37,
                        a: 1.0,
                    }
                } else {
                    // 普通状态 - 深色文字
                    D2D1_COLOR_F {
                        r: 0.1,
                        g: 0.1,
                        b: 0.1,
                        a: 1.0,
                    }
                };

                // 创建对应颜色的画刷并绘制居中文字
                if let Ok(text_brush) = self.render_target.CreateSolidColorBrush(&text_color, None)
                {
                    let text_wide = to_wide_chars(&icon_data.text);

                    self.render_target.DrawText(
                        &text_wide[..text_wide.len() - 1],
                        &self.centered_text_format,
                        rect,
                        &text_brush,
                        D2D1_DRAW_TEXT_OPTIONS_NONE,
                        DWRITE_MEASURING_MODE_NATURAL,
                    );
                }
            }
        }
    }
}
impl Drop for WindowState {
    fn drop(&mut self) {
        unsafe {
            DeleteDC(self.screenshot_dc);
            DeleteObject(self.gdi_screenshot_bitmap);
            CoUninitialize();
        }
    }
}
