use crate::WindowState;
use crate::svg_icons::SvgIconManager;
use crate::utils::*;
use crate::*;

use std::ffi::c_void;

use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows_numerics::*;

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
            let screen_dc = GetDC(Some(HWND(std::ptr::null_mut())));
            let screenshot_dc = CreateCompatibleDC(Some(screen_dc));
            let gdi_screenshot_bitmap =
                CreateCompatibleBitmap(screen_dc, screen_width, screen_height);
            SelectObject(screenshot_dc, gdi_screenshot_bitmap.into());

            // 捕获屏幕
            BitBlt(
                screenshot_dc,
                0,
                0,
                screen_width,
                screen_height,
                Some(screen_dc),
                0,
                0,
                SRCCOPY,
            )?;
            ReleaseDC(Some(HWND(std::ptr::null_mut())), screen_dc);

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
                drag_start_font_size: 20.0,
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
                svg_icon_manager: {
                    let mut manager = SvgIconManager::new();
                    let _ = manager.load_icons(); // 忽略加载错误
                    manager
                },

                // 文字输入相关字段初始化
                text_editing: false,
                editing_element_index: None,
                text_cursor_pos: 0,
                text_cursor_visible: true,
                cursor_timer_id: 1,     // 定时器ID
                just_saved_text: false, // 初始化为false
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
            let screen_dc = GetDC(Some(HWND(std::ptr::null_mut())));
            let mem_dc = CreateCompatibleDC(Some(screen_dc));
            let bitmap = CreateCompatibleBitmap(screen_dc, width, height);
            let old_bitmap = SelectObject(mem_dc, bitmap.into());

            // 直接从屏幕复制选择区域（包含窗口内容和绘图）
            BitBlt(
                mem_dc,
                0,
                0,
                width,
                height,
                Some(screen_dc),
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
            DeleteObject(bitmap.into());
            DeleteDC(mem_dc);
            ReleaseDC(Some(HWND(std::ptr::null_mut())), screen_dc);

            // 调整窗口大小和位置到选择区域
            SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
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
            Some(gdi_dc),
            &bmi,
            DIB_RGB_COLORS,
            &mut pixels,
            Some(HANDLE(std::ptr::null_mut())),
            0,
        )?;

        let temp_dc = CreateCompatibleDC(Some(gdi_dc));
        let old_bitmap = SelectObject(temp_dc, dib.into());

        BitBlt(temp_dc, 0, 0, width, height, Some(gdi_dc), 0, 0, SRCCOPY)?;

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

        DeleteObject(dib.into());
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
                        if !element.points.is_empty() {
                            self.draw_text_element(element);
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

                            let ellipse: D2D1_ELLIPSE = D2D1_ELLIPSE {
                                point: windows_numerics::Vector2 {
                                    X: center_x,
                                    Y: center_y,
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
                    // 对于文本元素，只有在编辑模式下才显示选择边框（拖动时不显示输入框）
                    if element.tool == DrawingTool::Text {
                        let is_editing_this_element = self.text_editing
                            && self.editing_element_index.map_or(false, |idx| {
                                idx < self.drawing_elements.len() && idx == element_index
                            });
                        if !is_editing_this_element {
                            return; // 文本元素未在编辑状态时不显示选择边框
                        }
                    }
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
                                        point: Vector2 {
                                            X: point.x as f32,
                                            Y: point.y as f32,
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

                                // 根据元素类型绘制不同数量的手柄
                                let half_handle = HANDLE_SIZE / 2.0;

                                let handles = if element.tool == DrawingTool::Text {
                                    // 文本元素只绘制4个对角手柄
                                    vec![
                                        (element.rect.left, element.rect.top),
                                        (element.rect.right, element.rect.top),
                                        (element.rect.right, element.rect.bottom),
                                        (element.rect.left, element.rect.bottom),
                                    ]
                                } else {
                                    // 其他元素绘制8个手柄
                                    let center_x = (element.rect.left + element.rect.right) / 2;
                                    let center_y = (element.rect.top + element.rect.bottom) / 2;
                                    vec![
                                        (element.rect.left, element.rect.top),
                                        (center_x, element.rect.top),
                                        (element.rect.right, element.rect.top),
                                        (element.rect.right, center_y),
                                        (element.rect.right, element.rect.bottom),
                                        (center_x, element.rect.bottom),
                                        (element.rect.left, element.rect.bottom),
                                        (element.rect.left, center_y),
                                    ]
                                };

                                for (hx, hy) in handles.iter() {
                                    // 只有当手柄在选择框内时才显示
                                    if *hx >= self.selection_rect.left
                                        && *hx <= self.selection_rect.right
                                        && *hy >= self.selection_rect.top
                                        && *hy <= self.selection_rect.bottom
                                    {
                                        let handle_ellipse = D2D1_ELLIPSE {
                                            point: Vector2 {
                                                X: *hx as f32,
                                                Y: *hy as f32,
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
            for (rect, button_type) in &self.toolbar.buttons {
                // 检查按钮是否应该被禁用
                let is_disabled = match button_type {
                    ToolbarButton::Undo => !self.can_undo(), // 撤销按钮根据历史记录状态
                    // 可以添加其他按钮的禁用逻辑
                    _ => false,
                };

                // 绘制按钮背景状态 - 只有 hover 时才显示背景
                if !is_disabled && *button_type == self.toolbar.hovered_button {
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
                }

                // 文字颜色不再需要，因为我们只使用 SVG 图标

                // 确定图标颜色
                let icon_color = if *button_type == self.toolbar.clicked_button {
                    // 选中状态 - 绿色
                    Some((33, 196, 94)) // #21c45e 绿色
                } else {
                    // 普通状态 - 默认颜色（黑色）
                    Some((16, 16, 16)) // #101010 深灰色
                };

                // 渲染 SVG 图标
                if let Ok(Some(icon_bitmap)) = self.svg_icon_manager.render_icon_to_bitmap(
                    *button_type,
                    &self.render_target,
                    24, // 图标大小
                    icon_color,
                ) {
                    // 计算图标居中位置
                    let icon_size = 20.0; // 显示大小
                    let icon_x = rect.left + (rect.right - rect.left - icon_size) / 2.0;
                    let icon_y = rect.top + (rect.bottom - rect.top - icon_size) / 2.0;

                    let icon_rect = D2D_RECT_F {
                        left: icon_x,
                        top: icon_y,
                        right: icon_x + icon_size,
                        bottom: icon_y + icon_size,
                    };

                    // 绘制图标
                    self.render_target.DrawBitmap(
                        &icon_bitmap,
                        Some(&icon_rect),
                        if is_disabled { 0.4 } else { 1.0 }, // 禁用时半透明
                        D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                        None,
                    );
                }
            }
        }
    }

    // 绘制文字元素
    pub fn draw_text_element(&self, element: &DrawingElement) {
        unsafe {
            // 计算文字区域
            let text_rect = if element.points.len() >= 2 {
                // 如果有两个点，使用它们定义矩形
                d2d_rect(
                    element.points[0].x,
                    element.points[0].y,
                    element.points[1].x,
                    element.points[1].y,
                )
            } else if !element.points.is_empty() {
                // 如果只有一个点，使用默认大小
                d2d_rect(
                    element.points[0].x,
                    element.points[0].y,
                    element.points[0].x + DEFAULT_TEXT_WIDTH,
                    element.points[0].y + DEFAULT_TEXT_HEIGHT,
                )
            } else {
                return;
            };

            // 只有在文本编辑模式下且正在编辑此元素时才绘制边框（拖动时不显示输入框）
            if self.text_editing
                && self.editing_element_index.map_or(false, |idx| {
                    idx < self.drawing_elements.len()
                        && std::ptr::eq(element, &self.drawing_elements[idx])
                })
            {
                // 创建灰色画刷
                let border_brush = self
                    .render_target
                    .CreateSolidColorBrush(&COLOR_TEXT_BORDER, None);

                if let Ok(brush) = border_brush {
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
                        // 绘制虚线边框
                        self.render_target.DrawRectangle(
                            &text_rect,
                            &brush,
                            1.0,
                            Some(&dashed_stroke),
                        );
                    }
                }

                // 绘制四个手柄
                let half_handle = HANDLE_SIZE / 2.0;
                let handles = [
                    (text_rect.left, text_rect.top),     // 左上
                    (text_rect.right, text_rect.top),    // 右上
                    (text_rect.right, text_rect.bottom), // 右下
                    (text_rect.left, text_rect.bottom),  // 左下
                ];

                for (hx, hy) in handles.iter() {
                    let handle_rect = D2D_RECT_F {
                        left: hx - half_handle,
                        top: hy - half_handle,
                        right: hx + half_handle,
                        bottom: hy + half_handle,
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

            // 绘制文字内容（透明背景）
            if !element.text.is_empty() {
                // 创建文字画刷
                let text_brush = self
                    .render_target
                    .CreateSolidColorBrush(&element.color, None);

                if let Ok(brush) = text_brush {
                    // 添加内边距
                    let text_content_rect = D2D_RECT_F {
                        left: text_rect.left + TEXT_PADDING,
                        top: text_rect.top + TEXT_PADDING,
                        right: text_rect.right - TEXT_PADDING,
                        bottom: text_rect.bottom - TEXT_PADDING,
                    };

                    // 支持多行文字显示
                    let lines: Vec<&str> = if element.text.is_empty() {
                        vec![""] // 空文本时显示一个空行（用于显示光标）
                    } else {
                        element.text.lines().collect()
                    };
                    // 使用动态行高，基于字体大小
                    let font_size = element.thickness.max(8.0);
                    let line_height = font_size * 1.2;

                    for (i, line) in lines.iter().enumerate() {
                        let line_rect = D2D_RECT_F {
                            left: text_content_rect.left,
                            top: text_content_rect.top + (i as f32 * line_height),
                            right: text_content_rect.right,
                            bottom: text_content_rect.top + ((i + 1) as f32 * line_height),
                        };

                        // 即使是空行也要绘制（为了光标定位）
                        if !line.is_empty() {
                            let line_wide = to_wide_chars(line);

                            // 为每个文本元素创建动态字体大小的文本格式
                            let font_size = element.thickness.max(8.0);
                            if let Ok(dynamic_text_format) = self.dwrite_factory.CreateTextFormat(
                                w!("Microsoft YaHei"),
                                None,
                                DWRITE_FONT_WEIGHT_NORMAL,
                                DWRITE_FONT_STYLE_NORMAL,
                                DWRITE_FONT_STRETCH_NORMAL,
                                font_size,
                                w!(""),
                            ) {
                                self.render_target.DrawText(
                                    &line_wide[..line_wide.len() - 1],
                                    &dynamic_text_format,
                                    &line_rect,
                                    &brush,
                                    D2D1_DRAW_TEXT_OPTIONS_NONE,
                                    DWRITE_MEASURING_MODE_NATURAL,
                                );
                            }
                        }
                    }
                }
            }

            // 如果正在编辑此文字元素，绘制光标
            if self.text_editing {
                if let Some(editing_index) = self.editing_element_index {
                    if editing_index < self.drawing_elements.len()
                        && std::ptr::eq(element, &self.drawing_elements[editing_index])
                        && self.text_cursor_visible
                    {
                        self.draw_text_cursor(element, &text_rect);
                    }
                }
            }
        }
    }

    // 精确测量文本尺寸的方法
    pub fn measure_text_precise(&self, text: &str, max_width: f32) -> Result<(f32, f32)> {
        unsafe {
            if text.is_empty() {
                return Ok((0.0, LINE_HEIGHT as f32));
            }

            let text_wide = to_wide_chars(text);
            let text_layout = self.dwrite_factory.CreateTextLayout(
                &text_wide[..text_wide.len() - 1],
                &self.text_format,
                max_width,
                f32::MAX,
            )?;

            let mut metrics = std::mem::zeroed::<DWRITE_TEXT_METRICS>();
            text_layout.GetMetrics(&mut metrics)?;
            Ok((metrics.width, metrics.height))
        }
    }

    // 使用指定字体大小精确测量文本尺寸的方法
    pub fn measure_text_precise_with_font_size(
        &self,
        text: &str,
        max_width: f32,
        font_size: f32,
    ) -> Result<(f32, f32)> {
        unsafe {
            if text.is_empty() {
                return Ok((0.0, font_size * 1.2)); // 使用字体大小的1.2倍作为行高
            }

            // 创建动态字体大小的文本格式
            let dynamic_text_format = self.dwrite_factory.CreateTextFormat(
                w!("Microsoft YaHei"),
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                font_size,
                w!(""),
            )?;

            let text_wide = to_wide_chars(text);
            let text_layout = self.dwrite_factory.CreateTextLayout(
                &text_wide[..text_wide.len() - 1],
                &dynamic_text_format,
                max_width,
                f32::MAX,
            )?;

            let mut metrics = std::mem::zeroed::<DWRITE_TEXT_METRICS>();
            text_layout.GetMetrics(&mut metrics)?;
            Ok((metrics.width, metrics.height))
        }
    }

    // 精确测量光标前文本的宽度
    pub fn measure_text_width_before_cursor(&self, text: &str, cursor_pos: usize) -> Result<f32> {
        unsafe {
            if text.is_empty() || cursor_pos == 0 {
                return Ok(0.0);
            }

            // 获取光标前的文本（使用字符索引而不是字节索引）
            let text_before_cursor = text.chars().take(cursor_pos).collect::<String>();

            // 找到光标所在的行
            let lines: Vec<&str> = text_before_cursor.lines().collect();
            let current_line_text = if text_before_cursor.ends_with('\n') {
                "" // 如果以换行符结尾，光标在新行开始
            } else {
                lines.last().map_or("", |&line| line)
            };

            if current_line_text.is_empty() {
                return Ok(0.0);
            }

            let line_wide = to_wide_chars(current_line_text);
            let text_layout = self.dwrite_factory.CreateTextLayout(
                &line_wide[..line_wide.len() - 1],
                &self.text_format,
                f32::MAX,
                f32::MAX,
            )?;

            let mut metrics = std::mem::zeroed::<DWRITE_TEXT_METRICS>();
            text_layout.GetMetrics(&mut metrics)?;
            Ok(metrics.width)
        }
    }

    // 计算光标所在的行号
    pub fn get_cursor_line_number(&self, text: &str, cursor_pos: usize) -> usize {
        if text.is_empty() || cursor_pos == 0 {
            return 0;
        }

        let text_before_cursor = text.chars().take(cursor_pos).collect::<String>();
        let lines_before_cursor: Vec<&str> = text_before_cursor.lines().collect();

        if text_before_cursor.ends_with('\n') {
            lines_before_cursor.len()
        } else {
            lines_before_cursor.len().saturating_sub(1)
        }
    }

    // 使用指定字体大小精确测量光标前文本的宽度
    pub fn measure_text_width_before_cursor_with_font_size(
        &self,
        text: &str,
        cursor_pos: usize,
        font_size: f32,
    ) -> Result<f32> {
        unsafe {
            if text.is_empty() || cursor_pos == 0 {
                return Ok(0.0);
            }

            // 获取光标前的文本（使用字符索引而不是字节索引）
            let text_before_cursor = text.chars().take(cursor_pos).collect::<String>();

            // 找到光标所在的行
            let lines: Vec<&str> = text_before_cursor.lines().collect();
            let current_line_text = if text_before_cursor.ends_with('\n') {
                "" // 如果以换行符结尾，光标在新行开始
            } else {
                lines.last().map_or("", |&line| line)
            };

            if current_line_text.is_empty() {
                return Ok(0.0);
            }

            // 创建动态字体大小的文本格式
            let dynamic_text_format = self.dwrite_factory.CreateTextFormat(
                w!("Microsoft YaHei"),
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                font_size,
                w!(""),
            )?;

            let line_wide = to_wide_chars(current_line_text);
            let text_layout = self.dwrite_factory.CreateTextLayout(
                &line_wide[..line_wide.len() - 1],
                &dynamic_text_format,
                f32::MAX,
                f32::MAX,
            )?;

            let mut metrics = std::mem::zeroed::<DWRITE_TEXT_METRICS>();
            text_layout.GetMetrics(&mut metrics)?;
            Ok(metrics.width)
        }
    }

    // 绘制文字光标
    fn draw_text_cursor(&self, element: &DrawingElement, text_rect: &D2D_RECT_F) {
        unsafe {
            // 创建光标画刷
            let cursor_brush = self
                .render_target
                .CreateSolidColorBrush(&COLOR_TEXT_CURSOR, None);

            if let Ok(brush) = cursor_brush {
                // 使用精确测量计算光标位置
                let cursor_line = self.get_cursor_line_number(&element.text, self.text_cursor_pos);

                // 使用动态字体大小精确测量光标前文本的宽度
                let font_size = element.thickness.max(8.0); // 移除最大字体限制
                let cursor_x_offset = self
                    .measure_text_width_before_cursor_with_font_size(
                        &element.text,
                        self.text_cursor_pos,
                        font_size,
                    )
                    .unwrap_or(0.0);

                let cursor_x = text_rect.left + TEXT_PADDING + cursor_x_offset;

                // 计算光标的垂直位置，使用动态行高
                let line_height = font_size * 1.2;
                let cursor_y_top =
                    text_rect.top + TEXT_PADDING + (cursor_line as f32 * line_height);
                let cursor_y_bottom = cursor_y_top + line_height - 2.0;

                // 绘制光标线，线条粗细也根据字体大小调整
                let cursor_thickness = (font_size / 20.0).max(1.0).min(3.0);
                let cursor_start = d2d_point(cursor_x as i32, cursor_y_top as i32);
                let cursor_end = d2d_point(cursor_x as i32, cursor_y_bottom as i32);

                self.render_target.DrawLine(
                    cursor_start,
                    cursor_end,
                    &brush,
                    cursor_thickness,
                    None,
                );
            }
        }
    }
}
impl Drop for WindowState {
    fn drop(&mut self) {
        unsafe {
            DeleteDC(self.screenshot_dc);
            DeleteObject(self.gdi_screenshot_bitmap.into());
            CoUninitialize();
        }
    }
}
