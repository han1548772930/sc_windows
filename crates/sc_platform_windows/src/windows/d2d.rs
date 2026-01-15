use sc_drawing::Rect;
use sc_platform::WindowId;
use sc_platform::traits::*;
use std::collections::HashMap;
use std::ffi::c_void;

use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::Com::*;
use windows::core::*;

pub struct Direct2DRenderer {
    // Direct2D 资源
    pub d2d_factory: Option<ID2D1Factory>,
    pub render_target: Option<ID2D1HwndRenderTarget>,
    // Cache for layered rendering
    pub layer_target: Option<ID2D1BitmapRenderTarget>,
    pub background_bitmap: Option<ID2D1Bitmap>,

    // DirectWrite 资源
    pub dwrite_factory: Option<IDWriteFactory>,
    pub text_format: Option<IDWriteTextFormat>,

    // 颜色到画刷的缓存（避免每帧创建）: (Brush, LastUsedFrame)
    brush_cache: HashMap<u32, (ID2D1SolidColorBrush, u64)>,
    // 文本格式缓存（避免每次测量/绘制时创建）
    text_format_cache: std::sync::Mutex<HashMap<(String, u32), IDWriteTextFormat>>,

    // Frame counter for LRU
    frame_count: u64,

    // 屏幕尺寸
    pub screen_width: i32,
    pub screen_height: i32,
}

impl Direct2DRenderer {
    /// 创建新的Direct2D渲染器
    pub fn new() -> std::result::Result<Self, PlatformError> {
        Ok(Self {
            d2d_factory: None,
            render_target: None,
            layer_target: None,
            background_bitmap: None,
            dwrite_factory: None,
            text_format: None,
            brush_cache: HashMap::new(),
            text_format_cache: std::sync::Mutex::new(HashMap::new()),
            frame_count: 0,
            screen_width: 0,
            screen_height: 0,
        })
    }

    /// 创建使用共享 Factory 的 Direct2D 渲染器
    ///
    /// 优先使用全局共享的 Factory 实例，避免重复创建重量级 COM 对象。
    pub fn new_with_shared_factories() -> std::result::Result<Self, PlatformError> {
        let factories = super::factory::SharedFactories::try_get().ok_or_else(|| {
            PlatformError::InitError("Failed to get shared factories".to_string())
        })?;

        Ok(Self {
            d2d_factory: Some(factories.d2d_factory_clone()),
            render_target: None,
            layer_target: None,
            background_bitmap: None,
            dwrite_factory: Some(factories.dwrite_factory_clone()),
            text_format: None,
            brush_cache: HashMap::new(),
            text_format_cache: std::sync::Mutex::new(HashMap::new()),
            frame_count: 0,
            screen_width: 0,
            screen_height: 0,
        })
    }

    /// 初始化Direct2D资源
    pub fn initialize(
        &mut self,
        window: WindowId,
        width: i32,
        height: i32,
    ) -> std::result::Result<(), PlatformError> {
        let hwnd = super::hwnd(window);
        // 如果已经初始化且尺寸未变，直接返回
        if self.render_target.is_some()
            && self.screen_width == width
            && self.screen_height == height
        {
            return Ok(());
        }

        // Resize logic: if size changed, we need to recreate layer_target too
        self.layer_target = None;
        self.background_bitmap = None;

        // 如果 RenderTarget 已存在，尝试 Resize
        if let Some(ref render_target) = self.render_target {
            let size = D2D_SIZE_U {
                width: width as u32,
                height: height as u32,
            };
            // SAFETY: Resize 是安全的 COM 方法调用
            unsafe {
                if render_target.Resize(&size).is_ok() {
                    self.screen_width = width;
                    self.screen_height = height;
                    return Ok(());
                }
            }
            // 如果 Resize 失败，则继续往下走，重新创建资源
        }

        self.screen_width = width;
        self.screen_height = height;

        // 初始化 COM
        // SAFETY: COM 初始化是幂等操作，重复调用是安全的
        unsafe {
            let result = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            if result.is_err() {
                // RPC_E_CHANGED_MODE is ok (already initialized)
                if result != RPC_E_CHANGED_MODE {
                    return Err(PlatformError::InitError(format!(
                        "COM init failed: {result:?}"
                    )));
                }
            }
        }

        // 优先使用已有的 Factory（可能来自 new_with_shared_factories）
        // 如果没有，尝试从全局共享单例获取，最后才创建新的
        let d2d_factory: ID2D1Factory = if let Some(ref factory) = self.d2d_factory {
            factory.clone()
        } else if let Some(shared) = super::factory::SharedFactories::try_get() {
            let factory = shared.d2d_factory_clone();
            self.d2d_factory = Some(factory.clone());
            factory
        } else {
            // SAFETY: D2D1CreateFactory 是 Windows API 的安全封装
            let factory: ID2D1Factory =
                unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None) }.map_err(
                    |e| PlatformError::InitError(format!("D2D factory creation failed: {e:?}")),
                )?;
            self.d2d_factory = Some(factory.clone());
            factory
        };

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
                width: width as u32,
                height: height as u32,
            },
            presentOptions: D2D1_PRESENT_OPTIONS_NONE,
        };

        let render_target: ID2D1HwndRenderTarget = unsafe {
            d2d_factory
                .CreateHwndRenderTarget(&render_target_properties, &hwnd_render_target_properties)
                .map_err(|e| {
                    PlatformError::InitError(format!("Render target creation failed: {e:?}"))
                })?
        };

        // 优先使用已有的 DWrite Factory，否则从共享单例获取或创建新的
        let dwrite_factory: IDWriteFactory = if let Some(ref factory) = self.dwrite_factory {
            factory.clone()
        } else if let Some(shared) = super::factory::SharedFactories::try_get() {
            let factory = shared.dwrite_factory_clone();
            self.dwrite_factory = Some(factory.clone());
            factory
        } else {
            // SAFETY: DWriteCreateFactory 是 Windows API 的安全封装
            let factory: IDWriteFactory = unsafe {
                DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED).map_err(|e| {
                    PlatformError::InitError(format!("DirectWrite factory creation failed: {e:?}"))
                })?
            };
            self.dwrite_factory = Some(factory.clone());
            factory
        };

        // 创建文本格式
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
                    PlatformError::InitError(format!("Text format creation failed: {e:?}"))
                })?
        };

        // 存储资源
        self.d2d_factory = Some(d2d_factory);
        self.render_target = Some(render_target);
        self.dwrite_factory = Some(dwrite_factory);
        self.text_format = Some(text_format);

        // 渲染目标重建时清空颜色画刷缓存
        self.brush_cache.clear();
        if let Ok(mut cache) = self.text_format_cache.lock() {
            cache.clear();
        }

        Ok(())
    }

    /// 从 GDI 位图创建 D2D 位图
    pub fn create_d2d_bitmap_from_gdi(
        &self,
        gdi_dc: HDC,
        width: i32,
        height: i32,
    ) -> std::result::Result<ID2D1Bitmap, PlatformError> {
        if let Some(ref render_target) = self.render_target {
            unsafe {
                // 创建 DIB 来传输像素数据
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

                let mut pixels: *mut c_void = std::ptr::null_mut();
                let dib = CreateDIBSection(
                    Some(gdi_dc),
                    &bmi,
                    DIB_RGB_COLORS,
                    &mut pixels,
                    Some(HANDLE(std::ptr::null_mut())),
                    0,
                )
                .map_err(|e| {
                    PlatformError::ResourceError(format!("CreateDIBSection failed: {e:?}"))
                })?;

                let temp_dc = super::resources::ManagedDC::new(CreateCompatibleDC(Some(gdi_dc)));
                let old_bitmap = SelectObject(temp_dc.handle(), dib.into());

                BitBlt(
                    temp_dc.handle(),
                    0,
                    0,
                    width,
                    height,
                    Some(gdi_dc),
                    0,
                    0,
                    SRCCOPY,
                )
                .map_err(|e| PlatformError::ResourceError(format!("BitBlt failed: {e:?}")))?;

                SelectObject(temp_dc.handle(), old_bitmap);
                // temp_dc 会在函数结束时自动释放

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
                let bitmap = render_target
                    .CreateBitmap(
                        size,
                        Some(pixels as *const c_void),
                        stride,
                        &bitmap_properties,
                    )
                    .map_err(|e| {
                        PlatformError::ResourceError(format!("CreateBitmap failed: {e:?}"))
                    })?;

                // dib 在函数结束时会被 DeleteObject 释放
                let _managed_dib = super::resources::ManagedBitmap::new(dib);
                Ok(bitmap)
            }
        } else {
            Err(PlatformError::ResourceError(
                "No render target available".to_string(),
            ))
        }
    }

    /// 从 HBITMAP 创建 D2D 位图，并尽量提取 BMP 数据（用于 OCR）。
    pub fn create_d2d_bitmap_and_bmp_data_from_hbitmap(
        &self,
        bitmap: HBITMAP,
        width: i32,
        height: i32,
    ) -> std::result::Result<(ID2D1Bitmap, Option<Vec<u8>>), PlatformError> {
        let desktop = HWND(std::ptr::null_mut());
        let screen_dc = unsafe { GetDC(Some(desktop)) };

        let result = (|| {
            let mem_dc =
                super::resources::ManagedDC::new(unsafe { CreateCompatibleDC(Some(screen_dc)) });
            unsafe {
                SelectObject(mem_dc.handle(), bitmap.into());
            }

            let d2d_bitmap = self.create_d2d_bitmap_from_gdi(mem_dc.handle(), width, height)?;
            let bmp_data =
                super::bmp::bitmap_to_bmp_data(mem_dc.handle(), bitmap, width, height).ok();

            Ok((d2d_bitmap, bmp_data))
        })();

        unsafe {
            let _ = ReleaseDC(Some(desktop), screen_dc);
        }

        result
    }

    /// 捕获屏幕区域并转换为 D2D 位图，同时尽量提取 BMP 数据（用于 OCR）。
    pub fn capture_screen_region_to_d2d_bitmap_and_bmp_data(
        &self,
        selection_rect: Rect,
    ) -> std::result::Result<(ID2D1Bitmap, Option<Vec<u8>>), PlatformError> {
        let width = selection_rect.right - selection_rect.left;
        let height = selection_rect.bottom - selection_rect.top;

        let bitmap = unsafe { super::gdi::capture_screen_region_to_hbitmap(selection_rect) }
            .map_err(|e| PlatformError::ResourceError(format!("GDI capture failed: {e:?}")))?;
        let managed_bitmap = super::resources::ManagedBitmap::new(bitmap);

        self.create_d2d_bitmap_and_bmp_data_from_hbitmap(managed_bitmap.handle(), width, height)
    }

    pub fn set_background_bitmap(&mut self, bitmap: ID2D1Bitmap) {
        self.background_bitmap = Some(bitmap);
    }

    pub fn clear_background_bitmap(&mut self) {
        self.background_bitmap = None;
    }

    pub fn draw_background_bitmap_fullscreen(&self) {
        if let Some(bitmap) = &self.background_bitmap {
            self.draw_bitmap_fullscreen(bitmap);
        }
    }

    pub fn draw_bitmap_fullscreen(&self, bitmap: &ID2D1Bitmap) {
        unsafe {
            if let Some(render_target) = &self.render_target {
                let dest_rect = D2D_RECT_F {
                    left: 0.0,
                    top: 0.0,
                    right: self.screen_width as f32,
                    bottom: self.screen_height as f32,
                };
                render_target.DrawBitmap(
                    bitmap,
                    Some(&dest_rect),
                    1.0,
                    D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                    None,
                );
            }
        }
    }

    /// 从像素数据创建D2D位图
    pub fn create_bitmap_from_pixels(
        &self,
        pixels: &[u8],
        width: u32,
        height: u32,
    ) -> std::result::Result<ID2D1Bitmap, PlatformError> {
        if let Some(ref render_target) = self.render_target {
            let bitmap_properties = D2D1_BITMAP_PROPERTIES {
                pixelFormat: D2D1_PIXEL_FORMAT {
                    // tiny-skia 输出 RGBA
                    format: DXGI_FORMAT_R8G8B8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 96.0,
                dpiY: 96.0,
            };

            let size = D2D_SIZE_U { width, height };
            let stride = width * 4;

            unsafe {
                render_target
                    .CreateBitmap(
                        size,
                        Some(pixels.as_ptr() as *const std::ffi::c_void),
                        stride,
                        &bitmap_properties,
                    )
                    .map_err(|e| {
                        PlatformError::ResourceError(format!(
                            "CreateBitmap from pixels failed: {e:?}"
                        ))
                    })
            }
        } else {
            Err(PlatformError::ResourceError(
                "No render target available".to_string(),
            ))
        }
    }

    /// 将颜色量化为缓存键（ARGB 8bit）
    fn color_key(color: Color) -> u32 {
        let r = (color.r.clamp(0.0, 1.0) * 255.0 + 0.5) as u32;
        let g = (color.g.clamp(0.0, 1.0) * 255.0 + 0.5) as u32;
        let b = (color.b.clamp(0.0, 1.0) * 255.0 + 0.5) as u32;
        let a = (color.a.clamp(0.0, 1.0) * 255.0 + 0.5) as u32;
        (a << 24) | (r << 16) | (g << 8) | b
    }

    /// 获取或创建画刷（带缓存）
    pub fn get_or_create_brush(
        &mut self,
        color: Color,
    ) -> std::result::Result<ID2D1SolidColorBrush, PlatformError> {
        if self.render_target.is_none() {
            return Err(PlatformError::ResourceError(
                "No render target available".to_string(),
            ));
        }
        let key = Self::color_key(color);
        if let Some((brush, last_used)) = self.brush_cache.get_mut(&key) {
            *last_used = self.frame_count;
            return Ok(brush.clone());
        }
        // Safety: 前面已经检查过 render_target.is_none() 会提前返回错误
        let render_target = match self.render_target.as_ref() {
            Some(rt) => rt,
            None => {
                return Err(PlatformError::ResourceError(
                    "No render target available".to_string(),
                ));
            }
        };
        let d2d_color = D2D1_COLOR_F {
            r: color.r,
            g: color.g,
            b: color.b,
            a: color.a,
        };
        let brush = unsafe { render_target.CreateSolidColorBrush(&d2d_color, None) }
            .map_err(|e| PlatformError::ResourceError(format!("Failed to create brush: {e:?}")))?;

        // LRU 清理策略：如果缓存太大，移除最近最少使用的
        if self.brush_cache.len() > 100 {
            // 找出最久未使用的 20 个
            let mut entries: Vec<(u32, u64)> =
                self.brush_cache.iter().map(|(k, v)| (*k, v.1)).collect();
            entries.sort_by_key(|&(_, last_used)| last_used);

            for (k, _) in entries.iter().take(20) {
                self.brush_cache.remove(k);
            }
        }

        self.brush_cache
            .insert(key, (brush.clone(), self.frame_count));
        Ok(brush)
    }

    /// 获取或创建文本格式（带缓存）
    fn get_or_create_text_format(
        &self,
        font_family: &str,
        font_size: f32,
    ) -> std::result::Result<IDWriteTextFormat, PlatformError> {
        // 使用 u32 表示 float bits 作为 key
        let key = (font_family.to_string(), font_size.to_bits());

        if let Ok(mut cache) = self.text_format_cache.lock() {
            if let Some(fmt) = cache.get(&key) {
                return Ok(fmt.clone());
            }

            // 创建新的格式
            if let Some(ref dwrite_factory) = self.dwrite_factory {
                unsafe {
                    let format = dwrite_factory
                        .CreateTextFormat(
                            &HSTRING::from(font_family),
                            None,
                            DWRITE_FONT_WEIGHT_NORMAL,
                            DWRITE_FONT_STYLE_NORMAL,
                            DWRITE_FONT_STRETCH_NORMAL,
                            font_size,
                            w!(""),
                        )
                        .map_err(|e| {
                            PlatformError::ResourceError(format!(
                                "Failed to create text format: {e:?}"
                            ))
                        })?;

                    cache.insert(key, format.clone());
                    return Ok(format);
                }
            }
        }

        Err(PlatformError::ResourceError(
            "DWrite factory not available or lock failed".to_string(),
        ))
    }

    /// 获取屏幕宽度
    pub fn get_screen_width(&self) -> i32 {
        self.screen_width
    }

    /// 获取屏幕高度
    pub fn get_screen_height(&self) -> i32 {
        self.screen_height
    }

    pub fn render_background_selection_to_bmp(
        &mut self,
        selection_rect: &Rect,
        render_elements_fn: impl FnOnce(
            &ID2D1RenderTarget,
            &mut Self,
        ) -> std::result::Result<(), PlatformError>,
    ) -> std::result::Result<Vec<u8>, PlatformError> {
        let Some(source_bitmap) = self.background_bitmap.as_ref() else {
            return Err(PlatformError::ResourceError(
                "No background bitmap available".to_string(),
            ));
        };
        let source_bitmap = source_bitmap.clone();
        self.render_selection_to_bmp(&source_bitmap, selection_rect, render_elements_fn)
    }

    /// 渲染选择区域到BMP数据（包含绘图元素）
    ///
    /// 创建一个GDI兼容的离屏渲染目标，将源位图和绘图元素合成，返回BMP格式数据
    pub fn render_selection_to_bmp(
        &mut self,
        source_bitmap: &ID2D1Bitmap,
        selection_rect: &Rect,
        render_elements_fn: impl FnOnce(
            &ID2D1RenderTarget,
            &mut Self,
        ) -> std::result::Result<(), PlatformError>,
    ) -> std::result::Result<Vec<u8>, PlatformError> {
        let render_target = self.render_target.as_ref().ok_or_else(|| {
            PlatformError::ResourceError("No render target available".to_string())
        })?;

        let width = (selection_rect.right - selection_rect.left) as u32;
        let height = (selection_rect.bottom - selection_rect.top) as u32;

        if width == 0 || height == 0 {
            return Err(PlatformError::ResourceError(
                "Invalid selection dimensions".to_string(),
            ));
        }

        let (offscreen_target, gdi_target) =
            create_gdi_offscreen_target(render_target, width, height)?;

        begin_offscreen_draw(&offscreen_target, source_bitmap, selection_rect, width, height);

        // 渲染绘图元素（通过回调函数）
        let offscreen_render_target: &ID2D1RenderTarget = &offscreen_target;
        render_elements_fn(offscreen_render_target, self)?;

        // 在EndDraw之前获取DC（这是关键！GetDC必须在BeginDraw和EndDraw之间调用）
        let pixel_data =
            read_pixels_from_gdi_target(&offscreen_target, &gdi_target, width, height)?;

        // 结束渲染
        unsafe {
            if let Err(e) = offscreen_target.EndDraw(None, None) {
                return Err(PlatformError::RenderError(format!(
                    "Offscreen EndDraw failed: {e:?}"
                )));
            }
        }

        Ok(build_bmp_bytes(width, height, &pixel_data))
    }

    /// Get or create the intermediate layer render target
    pub fn get_or_create_layer_target(&mut self) -> Option<&ID2D1BitmapRenderTarget> {
        if self.layer_target.is_some() {
            return self.layer_target.as_ref();
        }

        if let Some(ref render_target) = self.render_target {
            let _size = D2D_SIZE_F {
                width: self.screen_width as f32,
                height: self.screen_height as f32,
            };
            let _pixel_size = D2D_SIZE_U {
                width: self.screen_width as u32,
                height: self.screen_height as u32,
            };
            let _pixel_format = D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            };

            unsafe {
                // Create compatible render target
                // Use default options (None) to inherit properties from parent which is safer
                // We pass None for size/format to inherit from the parent render target
                // This ensures compatibility with the window's pixel format and DPI
                if let Ok(target) = render_target.CreateCompatibleRenderTarget(
                    None, // Inherit size
                    None, // Inherit pixel size
                    None, // Inherit pixel format
                    D2D1_COMPATIBLE_RENDER_TARGET_OPTIONS_NONE,
                ) {
                    self.layer_target = Some(target);
                } else {
                    eprintln!("Failed to create compatible render target");
                }
            }
        }
        self.layer_target.as_ref()
    }
}

// 以下方法原属 PlatformRenderer trait，现已直接实现在 Direct2DRenderer 上
impl Direct2DRenderer {
    pub fn begin_frame(&mut self) -> std::result::Result<(), PlatformError> {
        self.frame_count += 1;
        // BeginDraw
        if let Some(ref render_target) = self.render_target {
            unsafe {
                render_target.BeginDraw();
            }
        }
        Ok(())
    }
    fn prepare_fill_stroke_brushes(
        &mut self,
        style: &DrawStyle,
    ) -> std::result::Result<
        (Option<ID2D1SolidColorBrush>, Option<ID2D1SolidColorBrush>),
        PlatformError,
    > {
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

        Ok((fill_brush, stroke_brush))
    }

    pub fn end_frame(&mut self) -> std::result::Result<(), PlatformError> {
        // EndDraw
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

    pub fn clear(&mut self, color: Color) -> std::result::Result<(), PlatformError> {
        // Clear
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

    pub fn draw_rectangle(
        &mut self,
        rect: Rectangle,
        style: &DrawStyle,
    ) -> std::result::Result<(), PlatformError> {
        // 实现Direct2D的矩形绘制
        // 先创建所有需要的画刷，避免借用冲突
        let (fill_brush, stroke_brush) = self.prepare_fill_stroke_brushes(style)?;

        // 现在可以安全地使用render_target
        if let Some(ref render_target) = self.render_target {
            // 创建矩形
            let d2d_rect = rect_to_d2d(rect);

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

    pub fn draw_rounded_rectangle(
        &mut self,
        rect: Rectangle,
        radius: f32,
        style: &DrawStyle,
    ) -> std::result::Result<(), PlatformError> {
        // 实现Direct2D的圆角矩形绘制
        // 先创建需要的画刷，避免借用冲突
        let (fill_brush, stroke_brush) = self.prepare_fill_stroke_brushes(style)?;

        if let Some(ref render_target) = self.render_target {
            let rounded_rect = D2D1_ROUNDED_RECT {
                rect: rect_to_d2d(rect),
                radiusX: radius,
                radiusY: radius,
            };

            unsafe {
                if let Some(ref brush) = fill_brush {
                    render_target.FillRoundedRectangle(&rounded_rect, brush);
                }
                if let Some(ref brush) = stroke_brush {
                    render_target.DrawRoundedRectangle(
                        &rounded_rect,
                        brush,
                        style.stroke_width,
                        None,
                    );
                }
            }
        }
        Ok(())
    }

    pub fn draw_circle(
        &mut self,
        center: Point,
        radius: f32,
        style: &DrawStyle,
    ) -> std::result::Result<(), PlatformError> {
        // DrawEllipse
        // 先创建需要的画刷，避免借用冲突
        let (fill_brush, stroke_brush) = self.prepare_fill_stroke_brushes(style)?;

        if let Some(ref render_target) = self.render_target {
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

    pub fn draw_line(
        &mut self,
        start: Point,
        end: Point,
        style: &DrawStyle,
    ) -> std::result::Result<(), PlatformError> {
        // DrawLine
        // 先取画刷，避免与render_target借用冲突
        let brush = self.get_or_create_brush(style.stroke_color)?;
        if let Some(ref render_target) = self.render_target {
            let start_point = windows_numerics::Vector2 {
                X: start.x,
                Y: start.y,
            };
            let end_point = windows_numerics::Vector2 { X: end.x, Y: end.y };
            unsafe {
                render_target.DrawLine(start_point, end_point, &brush, style.stroke_width, None);
            }
        }
        Ok(())
    }

    pub fn draw_dashed_rectangle(
        &mut self,
        rect: Rectangle,
        style: &DrawStyle,
        dash_pattern: &[f32],
    ) -> std::result::Result<(), PlatformError> {
        // 使用缓存画刷
        let stroke_brush = self.get_or_create_brush(style.stroke_color)?;

        if let (Some(render_target), Some(d2d_factory)) = (&self.render_target, &self.d2d_factory) {
            unsafe {
                let stroke_style =
                    create_dash_stroke_style(d2d_factory, dash_pattern, true)?;

                let d2d_rect = rect_to_d2d(rect);
                render_target.DrawRectangle(
                    &d2d_rect,
                    &stroke_brush,
                    style.stroke_width.max(1.0),
                    Some(&stroke_style),
                );
            }
        }
        Ok(())
    }

    pub fn draw_text(
        &mut self,
        text: &str,
        position: Point,
        style: &TextStyle,
    ) -> std::result::Result<(), PlatformError> {
        // 实现Direct2D的DrawText
        if self.render_target.is_none() || self.dwrite_factory.is_none() {
            return Ok(());
        }

        // 使用缓存获取文本格式，并修复字体硬编码问题
        let text_format = self.get_or_create_text_format(&style.font_family, style.font_size)?;

        // 创建文本布局矩形
        let text_rect = D2D_RECT_F {
            left: position.x,
            top: position.y,
            right: position.x + 1000.0,
            bottom: position.y + style.font_size * 2.0,
        };

        // 转换文本为UTF-16
        let text_utf16: Vec<u16> = text.encode_utf16().collect();

        // 获取画刷后再用，避免借用冲突
        let brush = self.get_or_create_brush(style.color)?;

        // 为避免同时借用self和render_target，改为重新获取render_target局部变量
        // Safety: 前面已经检查过 render_target.is_none() 会提前返回
        let rt = match self.render_target.as_ref() {
            Some(rt) => rt,
            None => return Ok(()), // 前面已经检查过，这里不应该到达
        };
        unsafe {
            rt.DrawText(
                &text_utf16,
                &text_format,
                &text_rect,
                &brush,
                windows::Win32::Graphics::Direct2D::D2D1_DRAW_TEXT_OPTIONS_NONE,
                windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL,
            );
        }
        Ok(())
    }

    pub fn measure_text(
        &self,
        text: &str,
        style: &TextStyle,
    ) -> std::result::Result<(f32, f32), PlatformError> {
        // 使用 DirectWrite 进行精确文本测量
        if text.is_empty() {
            return Ok((0.0, style.font_size));
        }

        let dwrite_factory = match &self.dwrite_factory {
            Some(f) => f,
            None => {
                // 回退到近似值
                return Ok((text.len() as f32 * style.font_size * 0.6, style.font_size));
            }
        };

        // 使用缓存获取文本格式
        let text_format = self.get_or_create_text_format(&style.font_family, style.font_size)?;

        // 文本 UTF-16
        let utf16: Vec<u16> = text.encode_utf16().collect();
        let layout = create_text_layout_for_utf16(
            dwrite_factory,
            &text_format,
            &utf16,
            f32::MAX,
            f32::MAX,
            "Failed to create text layout",
        )?;

        unsafe {
            let mut metrics = DWRITE_TEXT_METRICS::default();
            let _ = layout.GetMetrics(&mut metrics);
            Ok((metrics.width, metrics.height))
        }
    }

    /// 设置裁剪区域
    pub fn push_clip_rect(&mut self, rect: Rectangle) -> std::result::Result<(), PlatformError> {
        if let Some(ref render_target) = self.render_target {
            unsafe {
                let clip_rect = rect_to_d2d(rect);
                render_target.PushAxisAlignedClip(
                    &clip_rect,
                    windows::Win32::Graphics::Direct2D::D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
                );
            }
        }
        Ok(())
    }

    /// 恢复裁剪区域
    pub fn pop_clip_rect(&mut self) -> std::result::Result<(), PlatformError> {
        if let Some(ref render_target) = self.render_target {
            unsafe {
                render_target.PopAxisAlignedClip();
            }
        }
        Ok(())
    }

    // 已删除不再需要的 as_any/as_any_mut 和 create_bitmap_from_gdi 方法
    // 这些是 PlatformRenderer trait 的遗留方法，在直接使用 Direct2DRenderer 时不再需要

    // ---------- 高层绘图接口实现 ----------

    pub fn draw_selection_mask(
        &mut self,
        screen_rect: Rectangle,
        selection_rect: Rectangle,
        mask_color: Color,
    ) -> std::result::Result<(), PlatformError> {
        // 使用缓存画刷
        let mask_brush = self.get_or_create_brush(mask_color)?;

        if let Some(ref render_target) = self.render_target {
            // 绘制四个矩形覆盖选区外区域
            let left = selection_rect.x;
            let top = selection_rect.y;
            let right = selection_rect.x + selection_rect.width;
            let bottom = selection_rect.y + selection_rect.height;

            // 上方区域
            if top > screen_rect.y {
                fill_rect(
                    render_target,
                    &mask_brush,
                    screen_rect.x,
                    screen_rect.y,
                    screen_rect.x + screen_rect.width,
                    top,
                );
            }

            // 下方区域
            if bottom < screen_rect.y + screen_rect.height {
                fill_rect(
                    render_target,
                    &mask_brush,
                    screen_rect.x,
                    bottom,
                    screen_rect.x + screen_rect.width,
                    screen_rect.y + screen_rect.height,
                );
            }

            // 左侧区域
            if left > screen_rect.x {
                fill_rect(
                    render_target,
                    &mask_brush,
                    screen_rect.x,
                    top,
                    left,
                    bottom,
                );
            }

            // 右侧区域
            if right < screen_rect.x + screen_rect.width {
                fill_rect(
                    render_target,
                    &mask_brush,
                    right,
                    top,
                    screen_rect.x + screen_rect.width,
                    bottom,
                );
            }
        }
        Ok(())
    }

    pub fn draw_selection_border(
        &mut self,
        rect: Rectangle,
        color: Color,
        width: f32,
        dash_pattern: Option<&[f32]>,
    ) -> std::result::Result<(), PlatformError> {
        // 使用缓存画刷
        let border_brush = self.get_or_create_brush(color)?;

        if let Some(ref render_target) = self.render_target {
            unsafe {
                let d2d_rect = rect_to_d2d(rect);

                if let Some(dash) = dash_pattern {
                    // 创建虚线样式
                    if let Some(ref d2d_factory) = self.d2d_factory {
                        if let Ok(stroke_style) =
                            create_dash_stroke_style(d2d_factory, dash, false)
                        {
                            render_target.DrawRectangle(
                                &d2d_rect,
                                &border_brush,
                                width,
                                Some(&stroke_style),
                            );
                        } else {
                            // 如果创建虚线样式失败，使用实线
                            render_target.DrawRectangle(&d2d_rect, &border_brush, width, None);
                        }
                    } else {
                        render_target.DrawRectangle(&d2d_rect, &border_brush, width, None);
                    }
                } else {
                    // 实线边框
                    render_target.DrawRectangle(&d2d_rect, &border_brush, width, None);
                }
            }
        }
        Ok(())
    }

    pub fn draw_selection_handles(
        &mut self,
        rect: Rectangle,
        handle_size: f32,
        fill_color: Color,
        border_color: Color,
        border_width: f32,
    ) -> std::result::Result<(), PlatformError> {
        // 使用缓存画刷
        let fill_brush = self.get_or_create_brush(fill_color)?;
        let border_brush = self.get_or_create_brush(border_color)?;

        if let Some(ref render_target) = self.render_target {
            let half = handle_size / 2.0;
            // 8个手柄位置：4个角 + 4个边中点
            let handle_positions = handle_positions(rect);

            for (px, py) in handle_positions.iter() {
                let handle_rect = D2D_RECT_F {
                    left: *px - half,
                    top: *py - half,
                    right: *px + half,
                    bottom: *py + half,
                };
                draw_rect_handle(
                    render_target,
                    &fill_brush,
                    &border_brush,
                    border_width,
                    &handle_rect,
                );
            }
        }
        Ok(())
    }

    pub fn draw_element_handles(
        &mut self,
        rect: Rectangle,
        handle_radius: f32,
        fill_color: Color,
        border_color: Color,
        border_width: f32,
    ) -> std::result::Result<(), PlatformError> {
        // 使用缓存画刷
        let fill_brush = self.get_or_create_brush(fill_color)?;
        let border_brush = self.get_or_create_brush(border_color)?;

        if let Some(ref render_target) = self.render_target {
            // 8个圆形手柄位置：4个角 + 4个边中点
            let handle_positions = handle_positions(rect);

            for (px, py) in handle_positions.iter() {
                let handle_ellipse = D2D1_ELLIPSE {
                    point: windows_numerics::Vector2 { X: *px, Y: *py },
                    radiusX: handle_radius,
                    radiusY: handle_radius,
                };
                draw_ellipse_handle(
                    render_target,
                    &fill_brush,
                    &border_brush,
                    border_width,
                    &handle_ellipse,
                );
            }
        }
        Ok(())
    }

    /// 测量多行文本尺寸
    pub fn measure_text_layout_size(
        &self,
        text: &str,
        width: f32,
        style: &TextStyle,
    ) -> std::result::Result<(f32, f32), PlatformError> {
        if text.is_empty() {
            return Ok((0.0, 0.0));
        }

        let dwrite_factory = match &self.dwrite_factory {
            Some(dw) => dw,
            None => return Ok((0.0, 0.0)),
        };
        let text_format = self.get_or_create_text_format(&style.font_family, style.font_size)?;

        let text_utf16: Vec<u16> = text.encode_utf16().collect();
        let text_layout = create_text_layout_for_utf16(
            dwrite_factory,
            &text_format,
            &text_utf16,
            width,
            f32::MAX,
            "Failed to create text layout for measure",
        )?;

        unsafe {
            let mut metrics = DWRITE_TEXT_METRICS::default();
            let _ = text_layout.GetMetrics(&mut metrics);
            Ok((metrics.width, metrics.height))
        }
    }

    /// 将文本按指定宽度分行
    pub fn split_text_into_lines(
        &self,
        text: &str,
        width: f32,
        font_family: &str,
        font_size: f32,
    ) -> std::result::Result<Vec<String>, PlatformError> {
        let dwrite_factory = match &self.dwrite_factory {
            Some(dw) => dw,
            None => return Ok(vec![text.to_string()]),
        };
        // Use internal helper or just create format here
        let text_format = self.get_or_create_text_format(font_family, font_size)?;

        let text_utf16: Vec<u16> = text.encode_utf16().collect();
        let text_layout = create_text_layout_for_utf16(
            dwrite_factory,
            &text_format,
            &text_utf16,
            width,
            f32::MAX,
            "Failed to create text layout for split",
        )?;

        unsafe {
            let mut line_count = 0;
            let _ = text_layout.GetLineMetrics(None, &mut line_count);

            if line_count == 0 {
                return Ok(Vec::new());
            }

            let mut metrics = vec![DWRITE_LINE_METRICS::default(); line_count as usize];
            let _ = text_layout.GetLineMetrics(Some(&mut metrics), &mut line_count);

            let mut lines = Vec::with_capacity(line_count as usize);
            let mut current_pos = 0;

            for metric in metrics {
                let len = metric.length as usize;
                if current_pos + len <= text_utf16.len() {
                    let line_utf16 = &text_utf16[current_pos..current_pos + len];
                    let line_str = String::from_utf16_lossy(line_utf16);
                    // 移除行末换行符，便于逐行渲染
                    lines.push(line_str.trim_end_matches(&['\r', '\n'][..]).to_string());
                    current_pos += len;
                }
            }

            Ok(lines)
        }
    }

    /// 根据坐标获取文本位置 (用于点击测试)
    pub fn get_text_position_from_point(
        &self,
        text: &str,
        point_x: f32,
        font_family: &str,
        font_size: f32,
    ) -> std::result::Result<usize, PlatformError> {
        if text.is_empty() {
            return Ok(0);
        }
        let dwrite_factory = match &self.dwrite_factory {
            Some(dw) => dw,
            None => return Ok(0),
        };
        let text_format = self.get_or_create_text_format(font_family, font_size)?;

        let text_utf16: Vec<u16> = text.encode_utf16().collect();
        // Layout width doesn't matter for single line hit test, make it large
        let text_layout = create_text_layout_for_utf16(
            dwrite_factory,
            &text_format,
            &text_utf16,
            10000.0,
            1000.0,
            "Failed to create text layout for hit test",
        )?;

        unsafe {
            let mut is_trailing_hit = BOOL(0);
            let mut is_inside = BOOL(0);
            let mut metrics = DWRITE_HIT_TEST_METRICS::default();

            let _ = text_layout.HitTestPoint(
                point_x,
                0.0, // line_y relative to layout top
                &mut is_trailing_hit,
                &mut is_inside,
                &mut metrics,
            );

            // metrics.textPosition is the index of the character
            // If trailing hit, we want the position AFTER this character
            let mut pos = metrics.textPosition as usize;
            if is_trailing_hit.as_bool() {
                pos += 1;
            }

            // Ensure we don't go out of bounds (UTF-16 length)
            // But we want to return character index (Rust char index)?
            // metrics.textPosition is UTF-16 index.
            // We need to convert UTF-16 index to char index.

            // Simple approach: count chars up to UTF-16 pos
            let utf16_pos = pos.min(text_utf16.len());
            // Convert utf16_pos to char count
            let char_count = String::from_utf16_lossy(&text_utf16[..utf16_pos])
                .chars()
                .count();

            Ok(char_count)
        }
    }

    /// 绘制 D2D 位图
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

    /// 清除背景
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

    // =====================================================================
    // 静态层缓存相关方法
    // =====================================================================

    /// 检查 layer_target 是否有效
    pub fn is_layer_target_valid(&self) -> bool {
        self.layer_target.is_some()
    }

    /// 更新静态缓存层
    ///
    /// 调用者提供的闭包将被用于绘制静态元素到 layer_target。
    /// 该方法会：
    /// 1. 获取/创建 layer_target
    /// 2. BeginDraw
    /// 3. 清空背景（透明）
    /// 4. 执行闭包绘制静态元素
    /// 5. EndDraw
    ///
    /// 支持传递额外的上下文数据给闭包（如 GeometryCache 引用），避免不必要的 clone。
    pub fn update_static_layer_with_context<F, C>(
        &mut self,
        context: C,
        draw_fn: F,
    ) -> std::result::Result<(), PlatformError>
    where
        F: FnOnce(&ID2D1RenderTarget, &mut Self, C) -> std::result::Result<(), PlatformError>,
    {
        // 确保 layer_target 存在
        if self.layer_target.is_none() {
            self.get_or_create_layer_target();
        }

        let layer_target = self.layer_target.as_ref().ok_or_else(|| {
            PlatformError::ResourceError("Failed to get or create layer target".to_string())
        })?;

        let layer_target_clone = layer_target.clone();

        unsafe {
            layer_target_clone.BeginDraw();
            let clear_color = D2D1_COLOR_F {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            };
            layer_target_clone.Clear(Some(&clear_color));
        }

        let layer_target_interface: &ID2D1RenderTarget = &layer_target_clone;
        let result = draw_fn(layer_target_interface, self, context);

        unsafe {
            let end_result = layer_target_clone.EndDraw(None, None);
            if let Err(e) = end_result {
                return Err(PlatformError::RenderError(format!(
                    "Layer target EndDraw failed: {:?}",
                    e
                )));
            }
        }

        result
    }

    /// 绘制静态缓存层到屏幕
    ///
    /// 将 layer_target 的内容作为位图绘制到主 render_target
    pub fn draw_static_layer(&self) -> std::result::Result<(), PlatformError> {
        let render_target = self.render_target.as_ref().ok_or_else(|| {
            PlatformError::ResourceError("No render target available".to_string())
        })?;

        let layer_target = self
            .layer_target
            .as_ref()
            .ok_or_else(|| PlatformError::ResourceError("No layer target available".to_string()))?;

        unsafe {
            // 从 layer_target 获取位图
            let bitmap = layer_target.GetBitmap().map_err(|e| {
                PlatformError::ResourceError(format!(
                    "Failed to get bitmap from layer target: {:?}",
                    e
                ))
            })?;

            // 绘制到主 render_target 的 (0,0) 位置
            render_target.DrawBitmap(
                &bitmap,
                None, // dest_rect: 全屏
                1.0,  // opacity
                D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                None, // source_rect: 全图
            );
        }

        Ok(())
    }

    /// 获取 layer_target 的引用（用于外部直接绘制）
    pub fn get_layer_target(&self) -> Option<&ID2D1BitmapRenderTarget> {
        self.layer_target.as_ref()
    }
}

fn handle_positions(rect: Rectangle) -> [(f32, f32); 8] {
    let cx = rect.x + rect.width / 2.0;
    let cy = rect.y + rect.height / 2.0;
    [
        (rect.x, rect.y),                            // 左上
        (cx, rect.y),                                // 上中
        (rect.x + rect.width, rect.y),               // 右上
        (rect.x + rect.width, cy),                   // 右中
        (rect.x + rect.width, rect.y + rect.height), // 右下
        (cx, rect.y + rect.height),                  // 下中
        (rect.x, rect.y + rect.height),              // 左下
        (rect.x, cy),                                // 左中
    ]
}
fn rect_to_d2d(rect: Rectangle) -> D2D_RECT_F {
    D2D_RECT_F {
        left: rect.x,
        top: rect.y,
        right: rect.x + rect.width,
        bottom: rect.y + rect.height,
    }
}
fn fill_rect(
    render_target: &ID2D1RenderTarget,
    brush: &ID2D1SolidColorBrush,
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
) {
    let rect = D2D_RECT_F {
        left,
        top,
        right,
        bottom,
    };
    unsafe {
        render_target.FillRectangle(&rect, brush);
    }
}

fn draw_rect_handle(
    render_target: &ID2D1RenderTarget,
    fill_brush: &ID2D1SolidColorBrush,
    border_brush: &ID2D1SolidColorBrush,
    border_width: f32,
    rect: &D2D_RECT_F,
) {
    unsafe {
        render_target.FillRectangle(rect, fill_brush);
        if border_width > 0.0 {
            render_target.DrawRectangle(rect, border_brush, border_width, None);
        }
    }
}

fn draw_ellipse_handle(
    render_target: &ID2D1RenderTarget,
    fill_brush: &ID2D1SolidColorBrush,
    border_brush: &ID2D1SolidColorBrush,
    border_width: f32,
    ellipse: &D2D1_ELLIPSE,
) {
    unsafe {
        render_target.FillEllipse(ellipse, fill_brush);
        if border_width > 0.0 {
            render_target.DrawEllipse(ellipse, border_brush, border_width, None);
        }
    }
}

fn normalize_dash_pattern(dash_pattern: &[f32], default_if_empty: bool) -> Vec<f32> {
    if dash_pattern.is_empty() {
        if default_if_empty {
            vec![4.0, 2.0]
        } else {
            Vec::new()
        }
    } else {
        dash_pattern.to_vec()
    }
}

fn create_dash_stroke_style(
    factory: &ID2D1Factory,
    dash_pattern: &[f32],
    default_if_empty: bool,
) -> std::result::Result<ID2D1StrokeStyle, PlatformError> {
    let dashes = normalize_dash_pattern(dash_pattern, default_if_empty);
    let stroke_props = D2D1_STROKE_STYLE_PROPERTIES {
        startCap: D2D1_CAP_STYLE_FLAT,
        endCap: D2D1_CAP_STYLE_FLAT,
        dashCap: D2D1_CAP_STYLE_FLAT,
        lineJoin: D2D1_LINE_JOIN_MITER,
        miterLimit: 10.0,
        dashStyle: D2D1_DASH_STYLE_CUSTOM,
        dashOffset: 0.0,
    };
    unsafe {
        factory
            .CreateStrokeStyle(&stroke_props, Some(&dashes))
            .map_err(|e| PlatformError::RenderError(format!("CreateStrokeStyle failed: {e:?}")))
    }
}

fn create_text_layout_for_utf16(
    dwrite_factory: &IDWriteFactory,
    text_format: &IDWriteTextFormat,
    text_utf16: &[u16],
    width: f32,
    height: f32,
    error_prefix: &str,
) -> std::result::Result<IDWriteTextLayout, PlatformError> {
    unsafe {
        dwrite_factory
            .CreateTextLayout(text_utf16, text_format, width, height)
            .map_err(|e| PlatformError::RenderError(format!("{error_prefix}: {e:?}")))
    }
}

fn create_gdi_offscreen_target(
    render_target: &ID2D1RenderTarget,
    width: u32,
    height: u32,
) -> std::result::Result<(ID2D1BitmapRenderTarget, ID2D1GdiInteropRenderTarget), PlatformError> {
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
        render_target
            .CreateCompatibleRenderTarget(
                Some(&size),
                Some(&pixel_size),
                Some(&pixel_format),
                D2D1_COMPATIBLE_RENDER_TARGET_OPTIONS_GDI_COMPATIBLE,
            )
            .map_err(|e| {
                PlatformError::ResourceError(format!(
                    "Failed to create GDI compatible offscreen target: {e:?}"
                ))
            })?
    };

    let gdi_target: ID2D1GdiInteropRenderTarget =
        offscreen_target.cast().map_err(|e| {
            PlatformError::ResourceError(format!("Failed to cast to GDI interop: {e:?}"))
        })?;

    Ok((offscreen_target, gdi_target))
}

fn begin_offscreen_draw(
    offscreen_target: &ID2D1BitmapRenderTarget,
    source_bitmap: &ID2D1Bitmap,
    selection_rect: &Rect,
    width: u32,
    height: u32,
) {
    unsafe {
        offscreen_target.BeginDraw();

        // 清除背景
        let clear_color = D2D1_COLOR_F {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };
        offscreen_target.Clear(Some(&clear_color));

        // 绘制源位图（选择区域）
        let dest_rect = D2D_RECT_F {
            left: 0.0,
            top: 0.0,
            right: width as f32,
            bottom: height as f32,
        };
        let source_rect = D2D_RECT_F {
            left: selection_rect.left as f32,
            top: selection_rect.top as f32,
            right: selection_rect.right as f32,
            bottom: selection_rect.bottom as f32,
        };

        offscreen_target.DrawBitmap(
            source_bitmap,
            Some(&dest_rect),
            1.0,
            D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
            Some(&source_rect),
        );
    }
}

fn read_pixels_from_gdi_target(
    offscreen_target: &ID2D1BitmapRenderTarget,
    gdi_target: &ID2D1GdiInteropRenderTarget,
    width: u32,
    height: u32,
) -> std::result::Result<Vec<u8>, PlatformError> {
    unsafe {
        let hdc = gdi_target
            .GetDC(D2D1_DC_INITIALIZE_MODE_COPY)
            .map_err(|e| PlatformError::ResourceError(format!("Failed to get DC: {e:?}")))?;

        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width as i32,
                biHeight: -(height as i32), // 负值表示自上而下
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

        let data_size = (width * height * 4) as usize;
        let mut pixel_data = vec![0u8; data_size];

        // 获取当前选中的位图
        let current_obj = GetCurrentObject(hdc, OBJ_BITMAP);
        let hbitmap = HBITMAP(current_obj.0);

        let result = GetDIBits(
            hdc,
            hbitmap,
            0,
            height,
            Some(pixel_data.as_mut_ptr() as *mut _),
            &bmi as *const _ as *mut _,
            DIB_RGB_COLORS,
        );

        // 释放DC（必须在EndDraw之前）
        let _ = gdi_target.ReleaseDC(None);

        if result == 0 {
            // 仍然需要EndDraw
            let _ = offscreen_target.EndDraw(None, None);
            return Err(PlatformError::ResourceError(
                "Failed to get bitmap pixels".to_string(),
            ));
        }

        Ok(pixel_data)
    }
}

fn build_bmp_bytes(width: u32, height: u32, pixel_data: &[u8]) -> Vec<u8> {
    let row_size = (width * 4).div_ceil(4) * 4; // 4字节对齐
    let image_size = row_size * height;
    let file_size = 54 + image_size;

    let mut bmp_data = Vec::with_capacity(file_size as usize);

    // BMP 文件头 (14 字节)
    bmp_data.extend_from_slice(b"BM"); // 签名
    bmp_data.extend_from_slice(&file_size.to_le_bytes()); // 文件大小
    bmp_data.extend_from_slice(&0u16.to_le_bytes()); // 保留1
    bmp_data.extend_from_slice(&0u16.to_le_bytes()); // 保留2
    bmp_data.extend_from_slice(&54u32.to_le_bytes()); // 数据偏移

    // DIB 头 (40 字节)
    bmp_data.extend_from_slice(&40u32.to_le_bytes()); // 头大小
    bmp_data.extend_from_slice(&(width as i32).to_le_bytes()); // 宽度
    bmp_data.extend_from_slice(&(-(height as i32)).to_le_bytes()); // 高度 (Top-Down)
    bmp_data.extend_from_slice(&1u16.to_le_bytes()); // 颜色平面
    bmp_data.extend_from_slice(&32u16.to_le_bytes()); // 位深度
    bmp_data.extend_from_slice(&0u32.to_le_bytes()); // 压缩方式 (BI_RGB)
    bmp_data.extend_from_slice(&image_size.to_le_bytes()); // 图像大小
    bmp_data.extend_from_slice(&0i32.to_le_bytes()); // X 分辨率
    bmp_data.extend_from_slice(&0i32.to_le_bytes()); // Y 分辨率
    bmp_data.extend_from_slice(&0u32.to_le_bytes()); // 颜色数
    bmp_data.extend_from_slice(&0u32.to_le_bytes()); // 重要颜色数

    // 像素数据
    bmp_data.extend_from_slice(pixel_data);

    bmp_data
}

impl sc_rendering::RenderBackend for Direct2DRenderer {
    type Error = PlatformError;

    fn draw_rectangle(
        &mut self,
        rect: sc_rendering::Rectangle,
        style: &sc_rendering::DrawStyle,
    ) -> std::result::Result<(), Self::Error> {
        Direct2DRenderer::draw_rectangle(self, rect, style)
    }

    fn draw_rounded_rectangle(
        &mut self,
        rect: sc_rendering::Rectangle,
        radius: f32,
        style: &sc_rendering::DrawStyle,
    ) -> std::result::Result<(), Self::Error> {
        Direct2DRenderer::draw_rounded_rectangle(self, rect, radius, style)
    }

    fn draw_circle(
        &mut self,
        center: sc_rendering::Point,
        radius: f32,
        style: &sc_rendering::DrawStyle,
    ) -> std::result::Result<(), Self::Error> {
        Direct2DRenderer::draw_circle(self, center, radius, style)
    }

    fn draw_line(
        &mut self,
        start: sc_rendering::Point,
        end: sc_rendering::Point,
        style: &sc_rendering::DrawStyle,
    ) -> std::result::Result<(), Self::Error> {
        Direct2DRenderer::draw_line(self, start, end, style)
    }

    fn draw_text(
        &mut self,
        text: &str,
        position: sc_rendering::Point,
        style: &sc_rendering::TextStyle,
    ) -> std::result::Result<(), Self::Error> {
        Direct2DRenderer::draw_text(self, text, position, style)
    }

    fn draw_dashed_rectangle(
        &mut self,
        rect: sc_rendering::Rectangle,
        style: &sc_rendering::DrawStyle,
        dash_pattern: &[f32],
    ) -> std::result::Result<(), Self::Error> {
        Direct2DRenderer::draw_dashed_rectangle(self, rect, style, dash_pattern)
    }

    fn draw_selection_mask(
        &mut self,
        screen_rect: sc_rendering::Rectangle,
        selection_rect: sc_rendering::Rectangle,
        mask_color: sc_rendering::Color,
    ) -> std::result::Result<(), Self::Error> {
        Direct2DRenderer::draw_selection_mask(self, screen_rect, selection_rect, mask_color)
    }

    fn draw_selection_border(
        &mut self,
        rect: sc_rendering::Rectangle,
        color: sc_rendering::Color,
        width: f32,
        dash_pattern: Option<&[f32]>,
    ) -> std::result::Result<(), Self::Error> {
        Direct2DRenderer::draw_selection_border(self, rect, color, width, dash_pattern)
    }

    fn draw_selection_handles(
        &mut self,
        rect: sc_rendering::Rectangle,
        handle_size: f32,
        fill_color: sc_rendering::Color,
        border_color: sc_rendering::Color,
        border_width: f32,
    ) -> std::result::Result<(), Self::Error> {
        Direct2DRenderer::draw_selection_handles(
            self,
            rect,
            handle_size,
            fill_color,
            border_color,
            border_width,
        )
    }

    fn draw_element_handles(
        &mut self,
        rect: sc_rendering::Rectangle,
        handle_radius: f32,
        fill_color: sc_rendering::Color,
        border_color: sc_rendering::Color,
        border_width: f32,
    ) -> std::result::Result<(), Self::Error> {
        Direct2DRenderer::draw_element_handles(
            self,
            rect,
            handle_radius,
            fill_color,
            border_color,
            border_width,
        )
    }

    fn push_clip_rect(
        &mut self,
        rect: sc_rendering::Rectangle,
    ) -> std::result::Result<(), Self::Error> {
        Direct2DRenderer::push_clip_rect(self, rect)
    }

    fn pop_clip_rect(&mut self) -> std::result::Result<(), Self::Error> {
        Direct2DRenderer::pop_clip_rect(self)
    }
}
