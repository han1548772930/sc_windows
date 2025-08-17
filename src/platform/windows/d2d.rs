// Direct2D渲染器实现
//
// 基于Direct2D的Windows平台渲染器

use crate::platform::traits::*;
use std::collections::HashMap;

// 添加Windows API导入（从原始代码迁移）
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::System::Com::*;
use windows::core::*;

/// Direct2D渲染器（从原始WindowState迁移）
pub struct Direct2DRenderer {
    // Direct2D 资源（从原始代码迁移）
    pub d2d_factory: Option<ID2D1Factory>,
    pub render_target: Option<ID2D1HwndRenderTarget>,

    // DirectWrite 资源
    pub dwrite_factory: Option<IDWriteFactory>,
    pub text_format: Option<IDWriteTextFormat>,

    // 画刷缓存（真正的Direct2D画刷）
    brushes: HashMap<BrushId, ID2D1SolidColorBrush>,
    /// 字体缓存
    fonts: HashMap<FontId, String>, // 临时保留
    /// 下一个画刷ID
    next_brush_id: BrushId,
    /// 下一个字体ID
    next_font_id: FontId,

    // 屏幕尺寸
    screen_width: i32,
    screen_height: i32,
}

impl Direct2DRenderer {
    /// 创建新的Direct2D渲染器
    pub fn new() -> std::result::Result<Self, PlatformError> {
        Ok(Self {
            d2d_factory: None,
            render_target: None,
            dwrite_factory: None,
            text_format: None,
            brushes: HashMap::new(),
            fonts: HashMap::new(),
            next_brush_id: 1,
            next_font_id: 1,
            screen_width: 0,
            screen_height: 0,
        })
    }

    /// 初始化Direct2D资源（从原始代码迁移）
    pub fn initialize(
        &mut self,
        hwnd: HWND,
        width: i32,
        height: i32,
    ) -> std::result::Result<(), PlatformError> {
        self.screen_width = width;
        self.screen_height = height;

        // 初始化COM（从原始代码迁移）
        unsafe {
            let result = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            if result.is_err() {
                return Err(PlatformError::InitializationError(format!(
                    "COM init failed: {:?}",
                    result
                )));
            }
        }

        // 创建D2D工厂（从原始代码迁移）
        let d2d_factory: ID2D1Factory =
            unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None) }.map_err(|e| {
                PlatformError::InitializationError(format!("D2D factory creation failed: {:?}", e))
            })?;

        // 创建渲染目标（从原始代码迁移）
        let render_target_properties = D2D1_RENDER_TARGET_PROPERTIES {
            r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: 0.0,
            dpiY: 0.0,
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

        let render_target: ID2D1HwndRenderTarget = unsafe {
            d2d_factory
                .CreateHwndRenderTarget(&render_target_properties, &hwnd_render_target_properties)
                .map_err(|e| {
                    PlatformError::InitializationError(format!(
                        "Render target creation failed: {:?}",
                        e
                    ))
                })?
        };

        // 创建DirectWrite工厂（从原始代码迁移）
        let dwrite_factory: IDWriteFactory = unsafe {
            DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED).map_err(|e| {
                PlatformError::InitializationError(format!(
                    "DirectWrite factory creation failed: {:?}",
                    e
                ))
            })?
        };

        // 创建文本格式（从原始代码迁移）
        let text_format: IDWriteTextFormat = unsafe {
            dwrite_factory
                .CreateTextFormat(
                    w!("Segoe UI"),
                    None,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    14.0,
                    w!("en-us"),
                )
                .map_err(|e| {
                    PlatformError::InitializationError(format!(
                        "Text format creation failed: {:?}",
                        e
                    ))
                })?
        };

        // 创建所有必要的画刷（从原始代码迁移）
        let selection_border_brush = unsafe {
            render_target
                .CreateSolidColorBrush(&crate::constants::COLOR_SELECTION_BORDER, None)
                .map_err(|e| {
                    PlatformError::InitializationError(format!(
                        "Selection border brush creation failed: {:?}",
                        e
                    ))
                })?
        };

        let handle_fill_brush = unsafe {
            render_target
                .CreateSolidColorBrush(&crate::constants::COLOR_HANDLE_FILL, None)
                .map_err(|e| {
                    PlatformError::InitializationError(format!(
                        "Handle fill brush creation failed: {:?}",
                        e
                    ))
                })?
        };

        let handle_border_brush = unsafe {
            render_target
                .CreateSolidColorBrush(&crate::constants::COLOR_HANDLE_BORDER, None)
                .map_err(|e| {
                    PlatformError::InitializationError(format!(
                        "Handle border brush creation failed: {:?}",
                        e
                    ))
                })?
        };

        let toolbar_bg_brush = unsafe {
            render_target
                .CreateSolidColorBrush(&crate::constants::COLOR_TOOLBAR_BG, None)
                .map_err(|e| {
                    PlatformError::InitializationError(format!(
                        "Toolbar bg brush creation failed: {:?}",
                        e
                    ))
                })?
        };

        let button_hover_brush = unsafe {
            render_target
                .CreateSolidColorBrush(&crate::constants::COLOR_BUTTON_HOVER, None)
                .map_err(|e| {
                    PlatformError::InitializationError(format!(
                        "Button hover brush creation failed: {:?}",
                        e
                    ))
                })?
        };

        let button_active_brush = unsafe {
            render_target
                .CreateSolidColorBrush(&crate::constants::COLOR_BUTTON_ACTIVE, None)
                .map_err(|e| {
                    PlatformError::InitializationError(format!(
                        "Button active brush creation failed: {:?}",
                        e
                    ))
                })?
        };

        let text_brush = unsafe {
            render_target
                .CreateSolidColorBrush(&crate::constants::COLOR_TEXT_NORMAL, None)
                .map_err(|e| {
                    PlatformError::InitializationError(format!(
                        "Text brush creation failed: {:?}",
                        e
                    ))
                })?
        };

        let mask_brush = unsafe {
            render_target
                .CreateSolidColorBrush(&crate::constants::COLOR_MASK, None)
                .map_err(|e| {
                    PlatformError::InitializationError(format!(
                        "Mask brush creation failed: {:?}",
                        e
                    ))
                })?
        };

        // 存储画刷到缓存中（使用预定义的ID）
        self.brushes.insert(1, selection_border_brush);
        self.brushes.insert(2, handle_fill_brush);
        self.brushes.insert(3, handle_border_brush);
        self.brushes.insert(4, toolbar_bg_brush);
        self.brushes.insert(5, button_hover_brush);
        self.brushes.insert(6, button_active_brush);
        self.brushes.insert(7, text_brush);
        self.brushes.insert(8, mask_brush);

        // 存储资源
        self.d2d_factory = Some(d2d_factory);
        self.render_target = Some(render_target);
        self.dwrite_factory = Some(dwrite_factory);
        self.text_format = Some(text_format);

        println!("D2D bitmap created successfully");
        Ok(())
    }

    /// 从GDI位图创建D2D位图（从原始代码迁移）
    pub fn create_d2d_bitmap_from_gdi(
        &self,
        gdi_dc: windows::Win32::Graphics::Gdi::HDC,
        width: i32,
        height: i32,
    ) -> std::result::Result<ID2D1Bitmap, PlatformError> {
        use std::ffi::c_void;
        use windows::Win32::Graphics::Dxgi::Common::*;
        use windows::Win32::Graphics::Gdi::*;

        if let Some(ref render_target) = self.render_target {
            unsafe {
                // 创建DIB来传输像素数据（从原始代码迁移）
                let bmi = BITMAPINFO {
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
                    Some(windows::Win32::Foundation::HANDLE(std::ptr::null_mut())),
                    0,
                )
                .map_err(|e| {
                    PlatformError::ResourceError(format!("CreateDIBSection failed: {:?}", e))
                })?;

                let temp_dc = CreateCompatibleDC(Some(gdi_dc));
                let old_bitmap = SelectObject(temp_dc, dib.into());

                BitBlt(temp_dc, 0, 0, width, height, Some(gdi_dc), 0, 0, SRCCOPY)
                    .map_err(|e| PlatformError::ResourceError(format!("BitBlt failed: {:?}", e)))?;

                SelectObject(temp_dc, old_bitmap);
                let _ = DeleteDC(temp_dc);

                // 创建D2D位图（从原始代码迁移）
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
                let bitmap = render_target
                    .CreateBitmap(
                        size,
                        Some(pixels as *const c_void),
                        stride,
                        &bitmap_properties,
                    )
                    .map_err(|e| {
                        PlatformError::ResourceError(format!("CreateBitmap failed: {:?}", e))
                    })?;

                let _ = DeleteObject(dib.into());
                Ok(bitmap)
            }
        } else {
            Err(PlatformError::ResourceError(
                "No render target available".to_string(),
            ))
        }
    }

    /// 获取或创建画刷（从原始代码迁移）
    fn get_or_create_brush(
        &mut self,
        color: Color,
    ) -> std::result::Result<ID2D1SolidColorBrush, PlatformError> {
        // 简化实现：每次都创建新画刷
        if let Some(ref render_target) = self.render_target {
            let d2d_color = D2D1_COLOR_F {
                r: color.r,
                g: color.g,
                b: color.b,
                a: color.a,
            };

            let brush =
                unsafe { render_target.CreateSolidColorBrush(&d2d_color, None) }.map_err(|e| {
                    PlatformError::ResourceError(format!("Failed to create brush: {:?}", e))
                })?;

            Ok(brush)
        } else {
            Err(PlatformError::ResourceError(
                "No render target available".to_string(),
            ))
        }
    }

    /// 绘制GDI位图作为背景（从原始代码迁移）
    pub fn draw_gdi_bitmap_background(
        &mut self,
        gdi_dc: windows::Win32::Graphics::Gdi::HDC,
        width: i32,
        height: i32,
    ) -> std::result::Result<(), PlatformError> {
        if let Some(ref render_target) = self.render_target {
            // 从GDI位图创建D2D位图
            let d2d_bitmap = self.create_d2d_bitmap_from_gdi(gdi_dc, width, height)?;

            // 绘制D2D位图作为背景
            let dest_rect = D2D_RECT_F {
                left: 0.0,
                top: 0.0,
                right: width as f32,
                bottom: height as f32,
            };

            unsafe {
                render_target.DrawBitmap(
                    &d2d_bitmap,
                    Some(&dest_rect),
                    1.0, // 不透明度
                    D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                    None, // 源矩形（使用整个位图）
                );
            }

            Ok(())
        } else {
            Err(PlatformError::ResourceError(
                "No render target available".to_string(),
            ))
        }
    }

    /// 获取屏幕宽度
    pub fn get_screen_width(&self) -> i32 {
        self.screen_width
    }

    /// 获取屏幕高度
    pub fn get_screen_height(&self) -> i32 {
        self.screen_height
    }

    /// 获取画刷（从原始代码迁移）
    pub fn get_brush(&self, id: &u32) -> Option<&ID2D1SolidColorBrush> {
        self.brushes.get(id)
    }
}

impl PlatformRenderer for Direct2DRenderer {
    type Error = PlatformError;

    fn begin_frame(&mut self) -> std::result::Result<(), Self::Error> {
        // 实现Direct2D的BeginDraw（从原始代码迁移）
        if let Some(ref render_target) = self.render_target {
            unsafe {
                render_target.BeginDraw();
            }
        }
        Ok(())
    }

    fn end_frame(&mut self) -> std::result::Result<(), Self::Error> {
        // 实现Direct2D的EndDraw（从原始代码迁移）
        if let Some(ref render_target) = self.render_target {
            unsafe {
                let result = render_target.EndDraw(None, None);
                if result.is_err() {
                    return Err(PlatformError::RenderError("EndDraw failed".to_string()));
                }
            }
        }
        Ok(())
    }

    fn clear(&mut self, color: Color) -> std::result::Result<(), Self::Error> {
        // 实现Direct2D的Clear（从原始代码迁移）
        if let Some(ref render_target) = self.render_target {
            let d2d_color = D2D1_COLOR_F {
                r: color.r,
                g: color.g,
                b: color.b,
                a: color.a,
            };
            unsafe {
                render_target.Clear(Some(&d2d_color));
            }
        }
        Ok(())
    }

    fn draw_image(
        &mut self,
        _image: &Image,
        _rect: Rectangle,
    ) -> std::result::Result<(), Self::Error> {
        // TODO: 实现Direct2D的DrawBitmap
        Ok(())
    }

    fn draw_rectangle(
        &mut self,
        rect: Rectangle,
        style: &DrawStyle,
    ) -> std::result::Result<(), Self::Error> {
        // 实现Direct2D的矩形绘制（从原始代码迁移）
        // 先创建所有需要的画刷，避免借用冲突
        let fill_brush = if let Some(fill_color) = style.fill_color {
            Some(self.get_or_create_brush(fill_color)?)
        } else {
            None
        };

        let stroke_brush = if style.stroke_width > 0.0 {
            Some(self.get_or_create_brush(style.stroke_color)?)
        } else {
            None
        };

        // 现在可以安全地使用render_target
        if let Some(ref render_target) = self.render_target {
            // 创建矩形
            let d2d_rect = D2D_RECT_F {
                left: rect.x,
                top: rect.y,
                right: rect.x + rect.width,
                bottom: rect.y + rect.height,
            };

            // 如果有填充颜色，绘制填充
            if let Some(ref brush) = fill_brush {
                unsafe {
                    render_target.FillRectangle(&d2d_rect, brush);
                }
            }

            // 如果有描边，绘制边框
            if let Some(ref brush) = stroke_brush {
                unsafe {
                    render_target.DrawRectangle(&d2d_rect, brush, style.stroke_width, None);
                }
            }
        }
        Ok(())
    }

    fn draw_circle(
        &mut self,
        center: Point,
        radius: f32,
        style: &DrawStyle,
    ) -> std::result::Result<(), Self::Error> {
        // 实现Direct2D的DrawEllipse（从原始代码迁移）
        if let Some(ref render_target) = self.render_target {
            // 创建填充画刷
            let fill_brush = if let Some(fill_color) = style.fill_color {
                let d2d_color = D2D1_COLOR_F {
                    r: fill_color.r,
                    g: fill_color.g,
                    b: fill_color.b,
                    a: fill_color.a,
                };
                unsafe { render_target.CreateSolidColorBrush(&d2d_color, None).ok() }
            } else {
                None
            };

            // 创建描边画刷
            let stroke_brush = if style.stroke_width > 0.0 {
                let d2d_color = D2D1_COLOR_F {
                    r: style.stroke_color.r,
                    g: style.stroke_color.g,
                    b: style.stroke_color.b,
                    a: style.stroke_color.a,
                };
                unsafe { render_target.CreateSolidColorBrush(&d2d_color, None).ok() }
            } else {
                None
            };

            // 创建椭圆
            let ellipse = D2D1_ELLIPSE {
                point: windows_numerics::Vector2 {
                    X: center.x,
                    Y: center.y,
                },
                radiusX: radius,
                radiusY: radius,
            };

            // 如果有填充颜色，绘制填充
            if let Some(ref brush) = fill_brush {
                unsafe {
                    render_target.FillEllipse(&ellipse, brush);
                }
            }

            // 如果有描边，绘制边框
            if let Some(ref brush) = stroke_brush {
                unsafe {
                    render_target.DrawEllipse(&ellipse, brush, style.stroke_width, None);
                }
            }
        }
        Ok(())
    }

    fn draw_line(
        &mut self,
        start: Point,
        end: Point,
        style: &DrawStyle,
    ) -> std::result::Result<(), Self::Error> {
        // 实现Direct2D的DrawLine（从原始代码迁移）
        if let Some(ref render_target) = self.render_target {
            // 创建描边画刷
            let d2d_color = D2D1_COLOR_F {
                r: style.stroke_color.r,
                g: style.stroke_color.g,
                b: style.stroke_color.b,
                a: style.stroke_color.a,
            };

            if let Ok(brush) = unsafe { render_target.CreateSolidColorBrush(&d2d_color, None) } {
                let start_point = windows_numerics::Vector2 {
                    X: start.x,
                    Y: start.y,
                };
                let end_point = windows_numerics::Vector2 { X: end.x, Y: end.y };

                unsafe {
                    render_target.DrawLine(
                        start_point,
                        end_point,
                        &brush,
                        style.stroke_width,
                        None,
                    );
                }
            }
        }
        Ok(())
    }

    fn draw_text(
        &mut self,
        text: &str,
        position: Point,
        style: &TextStyle,
    ) -> std::result::Result<(), Self::Error> {
        // 实现Direct2D的DrawText（从原始代码迁移）
        if let Some(ref render_target) = self.render_target {
            if let Some(ref dwrite_factory) = self.dwrite_factory {
                // 创建文本格式
                let text_format = unsafe {
                    dwrite_factory
                        .CreateTextFormat(
                            w!("Microsoft YaHei"),
                            None,
                            windows::Win32::Graphics::DirectWrite::DWRITE_FONT_WEIGHT_NORMAL,
                            windows::Win32::Graphics::DirectWrite::DWRITE_FONT_STYLE_NORMAL,
                            windows::Win32::Graphics::DirectWrite::DWRITE_FONT_STRETCH_NORMAL,
                            style.font_size,
                            w!(""),
                        )
                        .map_err(|e| {
                            PlatformError::RenderError(format!(
                                "Failed to create text format: {:?}",
                                e
                            ))
                        })?
                };

                // 创建文本画刷
                let d2d_color = D2D1_COLOR_F {
                    r: style.color.r,
                    g: style.color.g,
                    b: style.color.b,
                    a: style.color.a,
                };

                if let Ok(brush) = unsafe { render_target.CreateSolidColorBrush(&d2d_color, None) }
                {
                    // 创建文本布局矩形
                    let text_rect = D2D_RECT_F {
                        left: position.x,
                        top: position.y,
                        right: position.x + 1000.0, // 足够大的宽度
                        bottom: position.y + style.font_size * 2.0, // 足够的高度
                    };

                    // 转换文本为UTF-16
                    let text_utf16: Vec<u16> = text.encode_utf16().collect();

                    unsafe {
                        render_target.DrawText(
                            &text_utf16,
                            &text_format,
                            &text_rect,
                            &brush,
                            windows::Win32::Graphics::Direct2D::D2D1_DRAW_TEXT_OPTIONS_NONE,
                            windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL,
                        );
                    }
                }
            }
        }
        Ok(())
    }

    fn create_brush(&mut self, _color: Color) -> std::result::Result<BrushId, Self::Error> {
        let id = self.next_brush_id;
        self.next_brush_id += 1;

        // TODO: 创建实际的Direct2D画刷
        // 暂时不存储到HashMap，因为类型不匹配
        // self.brushes.insert(id, format!("Brush_{}", id));

        Ok(id)
    }

    fn create_font(&mut self, family: &str, size: f32) -> std::result::Result<FontId, Self::Error> {
        let id = self.next_font_id;
        self.next_font_id += 1;

        // TODO: 创建实际的DirectWrite字体
        self.fonts.insert(id, format!("Font_{}_{}", family, size));

        Ok(id)
    }

    fn measure_text(
        &self,
        text: &str,
        style: &TextStyle,
    ) -> std::result::Result<(f32, f32), Self::Error> {
        // TODO: 实现文本测量
        Ok((text.len() as f32 * style.font_size * 0.6, style.font_size))
    }

    /// 设置裁剪区域（从原始代码迁移）
    fn push_clip_rect(&mut self, rect: Rectangle) -> std::result::Result<(), Self::Error> {
        if let Some(ref render_target) = self.render_target {
            unsafe {
                let clip_rect = D2D_RECT_F {
                    left: rect.x,
                    top: rect.y,
                    right: rect.x + rect.width,
                    bottom: rect.y + rect.height,
                };
                render_target.PushAxisAlignedClip(
                    &clip_rect,
                    windows::Win32::Graphics::Direct2D::D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
                );
            }
        }
        Ok(())
    }

    /// 恢复裁剪区域（从原始代码迁移）
    fn pop_clip_rect(&mut self) -> std::result::Result<(), Self::Error> {
        if let Some(ref render_target) = self.render_target {
            unsafe {
                render_target.PopAxisAlignedClip();
            }
        }
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Direct2DRenderer {
    /// 绘制D2D位图（从原始代码迁移）
    pub fn draw_d2d_bitmap(
        &self,
        bitmap: &ID2D1Bitmap,
        dest_rect: Option<D2D_RECT_F>,
        opacity: f32,
        source_rect: Option<D2D_RECT_F>,
    ) -> std::result::Result<(), PlatformError> {
        if let Some(ref render_target) = self.render_target {
            unsafe {
                render_target.DrawBitmap(
                    bitmap,
                    dest_rect.as_ref().map(|r| r as *const _),
                    opacity,
                    D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                    source_rect.as_ref().map(|r| r as *const _),
                );
            }
            Ok(())
        } else {
            Err(PlatformError::RenderError(
                "No render target available".to_string(),
            ))
        }
    }

    /// 清除背景（从原始代码迁移）
    pub fn clear_background(
        &self,
        color: Option<D2D1_COLOR_F>,
    ) -> std::result::Result<(), PlatformError> {
        if let Some(ref render_target) = self.render_target {
            unsafe {
                let clear_color = color.unwrap_or(D2D1_COLOR_F {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.0,
                });
                render_target.Clear(Some(&clear_color));
            }
            Ok(())
        } else {
            Err(PlatformError::RenderError(
                "No render target available".to_string(),
            ))
        }
    }
}
