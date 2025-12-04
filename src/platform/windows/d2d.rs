use crate::platform::traits;
use crate::platform::traits::*;
use std::collections::HashMap;

use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::System::Com::*;
use windows::core::*;

pub struct Direct2DRenderer {
    // Direct2D 资源（从原始代码迁移）
    pub d2d_factory: Option<ID2D1Factory>,
    pub render_target: Option<ID2D1HwndRenderTarget>,
    // Cache for layered rendering
    pub layer_target: Option<ID2D1BitmapRenderTarget>,

    // DirectWrite 资源
    pub dwrite_factory: Option<IDWriteFactory>,
    pub text_format: Option<IDWriteTextFormat>,

    // 画刷缓存（真正的Direct2D画刷）
    brushes: HashMap<BrushId, ID2D1SolidColorBrush>,
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
            dwrite_factory: None,
            text_format: None,
            brushes: HashMap::new(),
            brush_cache: HashMap::new(),
            text_format_cache: std::sync::Mutex::new(HashMap::new()),
            frame_count: 0,
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
        // 如果已经初始化且尺寸未变，直接返回
        if self.render_target.is_some()
            && self.screen_width == width
            && self.screen_height == height
        {
            return Ok(());
        }

        // Resize logic: if size changed, we need to recreate layer_target too
        self.layer_target = None;

        // 如果 RenderTarget 已存在，尝试 Resize
        if let Some(ref render_target) = self.render_target {
            let size = D2D_SIZE_U {
                width: width as u32,
                height: height as u32,
            };
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

        // 初始化COM（从原始代码迁移）
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

        // 创建D2D工厂（从原始代码迁移）
        let d2d_factory: ID2D1Factory = unsafe {
            D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)
        }
        .map_err(|e| PlatformError::InitError(format!("D2D factory creation failed: {e:?}")))?;

        // 创建渲染目标（从原始代码迁移）
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

        // 创建DirectWrite工厂（从原始代码迁移）
        let dwrite_factory: IDWriteFactory = unsafe {
            DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED).map_err(|e| {
                PlatformError::InitError(format!("DirectWrite factory creation failed: {e:?}"))
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
                    PlatformError::InitError(format!("Text format creation failed: {e:?}"))
                })?
        };

        // 创建所有必要的画刷（从原始代码迁移）
        let selection_border_brush = unsafe {
            render_target
                .CreateSolidColorBrush(&crate::constants::COLOR_SELECTION_BORDER, None)
                .map_err(|e| {
                    PlatformError::InitError(format!(
                        "Selection border brush creation failed: {e:?}"
                    ))
                })?
        };

        let handle_fill_brush = unsafe {
            render_target
                .CreateSolidColorBrush(&crate::constants::COLOR_HANDLE_FILL, None)
                .map_err(|e| {
                    PlatformError::InitError(format!("Handle fill brush creation failed: {e:?}"))
                })?
        };

        let handle_border_brush = unsafe {
            render_target
                .CreateSolidColorBrush(&crate::constants::COLOR_HANDLE_BORDER, None)
                .map_err(|e| {
                    PlatformError::InitError(format!("Handle border brush creation failed: {e:?}"))
                })?
        };

        let toolbar_bg_brush = unsafe {
            render_target
                .CreateSolidColorBrush(&crate::constants::COLOR_TOOLBAR_BG, None)
                .map_err(|e| {
                    PlatformError::InitError(format!("Toolbar bg brush creation failed: {e:?}"))
                })?
        };

        let button_hover_brush = unsafe {
            render_target
                .CreateSolidColorBrush(&crate::constants::COLOR_BUTTON_HOVER, None)
                .map_err(|e| {
                    PlatformError::InitError(format!("Button hover brush creation failed: {e:?}"))
                })?
        };

        let button_active_brush = unsafe {
            render_target
                .CreateSolidColorBrush(&crate::constants::COLOR_BUTTON_ACTIVE, None)
                .map_err(|e| {
                    PlatformError::InitError(format!("Button active brush creation failed: {e:?}"))
                })?
        };

        let text_brush = unsafe {
            render_target
                .CreateSolidColorBrush(&crate::constants::COLOR_TEXT_NORMAL, None)
                .map_err(|e| {
                    PlatformError::InitError(format!("Text brush creation failed: {e:?}"))
                })?
        };

        let mask_brush = unsafe {
            render_target
                .CreateSolidColorBrush(&crate::constants::COLOR_MASK, None)
                .map_err(|e| {
                    PlatformError::InitError(format!("Mask brush creation failed: {e:?}"))
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

        // 渲染目标重建时清空颜色画刷缓存
        self.brush_cache.clear();
        if let Ok(mut cache) = self.text_format_cache.lock() {
            cache.clear();
        }

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
                    PlatformError::ResourceError(format!("CreateDIBSection failed: {e:?}"))
                })?;

                let temp_dc = super::resources::ManagedDC::new(CreateCompatibleDC(Some(gdi_dc)));
                let old_bitmap = SelectObject(temp_dc.handle(), dib.into());

                BitBlt(temp_dc.handle(), 0, 0, width, height, Some(gdi_dc), 0, 0, SRCCOPY)
                    .map_err(|e| PlatformError::ResourceError(format!("BitBlt failed: {e:?}")))?;

                SelectObject(temp_dc.handle(), old_bitmap);
                // temp_dc 会在函数结束时自动释放

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
    pub(crate) fn get_or_create_brush(
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
            None => return Err(PlatformError::ResourceError("No render target available".to_string())),
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
            let mut entries: Vec<(u32, u64)> = self.brush_cache.iter().map(|(k, v)| (*k, v.1)).collect();
            entries.sort_by_key(|&(_, last_used)| last_used);
            
            for (k, _) in entries.iter().take(20) {
                self.brush_cache.remove(k);
            }
        }

        self.brush_cache.insert(key, (brush.clone(), self.frame_count));
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

    /// 渲染选择区域到BMP数据（包含绘图元素）
    /// 
    /// 创建一个GDI兼容的离屏渲染目标，将源位图和绘图元素合成，返回BMP格式数据
    pub fn render_selection_to_bmp(
        &mut self,
        source_bitmap: &ID2D1Bitmap,
        selection_rect: &windows::Win32::Foundation::RECT,
        render_elements_fn: impl FnOnce(&ID2D1RenderTarget, &mut Self) -> std::result::Result<(), PlatformError>,
    ) -> std::result::Result<Vec<u8>, PlatformError> {
        use windows::Win32::Graphics::Gdi::*;

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

        // 创建GDI兼容的离屏渲染目标
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

        // 获取GDI互操作接口
        let gdi_target: ID2D1GdiInteropRenderTarget = offscreen_target.cast().map_err(|e| {
            PlatformError::ResourceError(format!("Failed to cast to GDI interop: {e:?}"))
        })?;

        // 开始离屏渲染
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

        // 渲染绘图元素（通过回调函数）
        let offscreen_render_target: &ID2D1RenderTarget = &offscreen_target;
        render_elements_fn(offscreen_render_target, self)?;

        // 在EndDraw之前获取DC（这是关键！GetDC必须在BeginDraw和EndDraw之间调用）
        let pixel_data = unsafe {
            let hdc = gdi_target.GetDC(D2D1_DC_INITIALIZE_MODE_COPY).map_err(|e| {
                PlatformError::ResourceError(format!("Failed to get DC: {e:?}"))
            })?;

            // 创建DIB来提取像素
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

            pixel_data
        };

        // 结束渲染
        unsafe {
            if let Err(e) = offscreen_target.EndDraw(None, None) {
                return Err(PlatformError::RenderError(format!("Offscreen EndDraw failed: {e:?}")));
            }
        }

        // 构建BMP文件格式
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
        bmp_data.extend_from_slice(&pixel_data);

        Ok(bmp_data)
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

impl PlatformRenderer for Direct2DRenderer {
    type Error = PlatformError;

    fn begin_frame(&mut self) -> std::result::Result<(), Self::Error> {
        self.frame_count += 1;
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

    fn draw_rounded_rectangle(
        &mut self,
        rect: Rectangle,
        radius: f32,
        style: &DrawStyle,
    ) -> std::result::Result<(), Self::Error> {
        // 实现Direct2D的圆角矩形绘制
        // 先创建需要的画刷，避免借用冲突
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

        if let Some(ref render_target) = self.render_target {
            let rounded_rect = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: rect.x,
                    top: rect.y,
                    right: rect.x + rect.width,
                    bottom: rect.y + rect.height,
                },
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

    fn draw_circle(
        &mut self,
        center: Point,
        radius: f32,
        style: &DrawStyle,
    ) -> std::result::Result<(), Self::Error> {
        // 实现Direct2D的DrawEllipse（从原始代码迁移）
        // 先创建需要的画刷，避免借用冲突
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

    fn draw_line(
        &mut self,
        start: Point,
        end: Point,
        style: &DrawStyle,
    ) -> std::result::Result<(), Self::Error> {
        // 实现Direct2D的DrawLine（从原始代码迁移）
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

    fn draw_dashed_rectangle(
        &mut self,
        rect: Rectangle,
        style: &DrawStyle,
        dash_pattern: &[f32],
    ) -> std::result::Result<(), Self::Error> {
        if let (Some(render_target), Some(d2d_factory)) = (&self.render_target, &self.d2d_factory) {
            unsafe {
                // 笔刷
                let stroke_brush = {
                    let d2d_color = D2D1_COLOR_F {
                        r: style.stroke_color.r,
                        g: style.stroke_color.g,
                        b: style.stroke_color.b,
                        a: style.stroke_color.a,
                    };
                    render_target
                        .CreateSolidColorBrush(&d2d_color, None)
                        .map_err(|e| {
                            PlatformError::RenderError(format!(
                                "CreateSolidColorBrush failed: {e:?}"
                            ))
                        })?
                };

                // 虚线样式
                let mut dashes: Vec<f32> = dash_pattern.to_vec();
                if dashes.is_empty() {
                    dashes = vec![4.0, 2.0];
                }
                let stroke_props = D2D1_STROKE_STYLE_PROPERTIES {
                    startCap: D2D1_CAP_STYLE_FLAT,
                    endCap: D2D1_CAP_STYLE_FLAT,
                    dashCap: D2D1_CAP_STYLE_FLAT,
                    lineJoin: D2D1_LINE_JOIN_MITER,
                    miterLimit: 10.0,
                    dashStyle: D2D1_DASH_STYLE_CUSTOM,
                    dashOffset: 0.0,
                };
                let stroke_style = d2d_factory
                    .CreateStrokeStyle(&stroke_props, Some(&dashes))
                    .map_err(|e| {
                        PlatformError::RenderError(format!("CreateStrokeStyle failed: {e:?}"))
                    })?;

                let d2d_rect = D2D_RECT_F {
                    left: rect.x,
                    top: rect.y,
                    right: rect.x + rect.width,
                    bottom: rect.y + rect.height,
                };
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

    fn draw_text(
        &mut self,
        text: &str,
        position: Point,
        style: &TextStyle,
    ) -> std::result::Result<(), Self::Error> {
        // 实现Direct2D的DrawText（从原始代码迁移）
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

    fn measure_text(
        &self,
        text: &str,
        style: &TextStyle,
    ) -> std::result::Result<(f32, f32), Self::Error> {
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

        unsafe {
            // 文本 UTF-16
            let utf16: Vec<u16> = text.encode_utf16().collect();
            let layout = dwrite_factory
                .CreateTextLayout(&utf16, &text_format, f32::MAX, f32::MAX)
                .map_err(|e| {
                    PlatformError::RenderError(format!("Failed to create text layout: {e:?}"))
                })?;

            let mut metrics = DWRITE_TEXT_METRICS::default();
            let _ = layout.GetMetrics(&mut metrics);
            Ok((metrics.width, metrics.height))
        }
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

    /// 从GDI位图创建平台位图（平台无关接口实现）
    fn create_bitmap_from_gdi(
        &mut self,
        gdi_dc: windows::Win32::Graphics::Gdi::HDC,
        width: i32,
        height: i32,
    ) -> std::result::Result<(), traits::PlatformError> {
        // 调用现有的实现方法
        let _bitmap = self.create_d2d_bitmap_from_gdi(gdi_dc, width, height)?;
        // 位图已经创建并可以在后续渲染中使用
        Ok(())
    }

    // ---------- 高层绘图接口实现 ----------

    fn draw_selection_mask(
        &mut self,
        screen_rect: Rectangle,
        selection_rect: Rectangle,
        mask_color: Color,
    ) -> std::result::Result<(), Self::Error> {
        if let Some(ref render_target) = self.render_target {
            unsafe {
                // 创建遮罩画刷
                let d2d_color = D2D1_COLOR_F {
                    r: mask_color.r,
                    g: mask_color.g,
                    b: mask_color.b,
                    a: mask_color.a,
                };

                if let Ok(mask_brush) = render_target.CreateSolidColorBrush(&d2d_color, None) {
                    // 绘制四个矩形覆盖选区外区域
                    let left = selection_rect.x;
                    let top = selection_rect.y;
                    let right = selection_rect.x + selection_rect.width;
                    let bottom = selection_rect.y + selection_rect.height;

                    // 上方区域
                    if top > screen_rect.y {
                        let rect = D2D_RECT_F {
                            left: screen_rect.x,
                            top: screen_rect.y,
                            right: screen_rect.x + screen_rect.width,
                            bottom: top,
                        };
                        render_target.FillRectangle(&rect, &mask_brush);
                    }

                    // 下方区域
                    if bottom < screen_rect.y + screen_rect.height {
                        let rect = D2D_RECT_F {
                            left: screen_rect.x,
                            top: bottom,
                            right: screen_rect.x + screen_rect.width,
                            bottom: screen_rect.y + screen_rect.height,
                        };
                        render_target.FillRectangle(&rect, &mask_brush);
                    }

                    // 左侧区域
                    if left > screen_rect.x {
                        let rect = D2D_RECT_F {
                            left: screen_rect.x,
                            top,
                            right: left,
                            bottom,
                        };
                        render_target.FillRectangle(&rect, &mask_brush);
                    }

                    // 右侧区域
                    if right < screen_rect.x + screen_rect.width {
                        let rect = D2D_RECT_F {
                            left: right,
                            top,
                            right: screen_rect.x + screen_rect.width,
                            bottom,
                        };
                        render_target.FillRectangle(&rect, &mask_brush);
                    }
                }
            }
        }
        Ok(())
    }

    fn draw_selection_border(
        &mut self,
        rect: Rectangle,
        color: Color,
        width: f32,
        dash_pattern: Option<&[f32]>,
    ) -> std::result::Result<(), Self::Error> {
        if let Some(ref render_target) = self.render_target {
            unsafe {
                // 创建边框画刷
                let d2d_color = D2D1_COLOR_F {
                    r: color.r,
                    g: color.g,
                    b: color.b,
                    a: color.a,
                };

                if let Ok(border_brush) = render_target.CreateSolidColorBrush(&d2d_color, None) {
                    let d2d_rect = D2D_RECT_F {
                        left: rect.x,
                        top: rect.y,
                        right: rect.x + rect.width,
                        bottom: rect.y + rect.height,
                    };

                    if let Some(dash) = dash_pattern {
                        // 创建虚线样式
                        if let Some(ref d2d_factory) = self.d2d_factory {
                            let stroke_style_props = D2D1_STROKE_STYLE_PROPERTIES {
                                startCap: D2D1_CAP_STYLE_FLAT,
                                endCap: D2D1_CAP_STYLE_FLAT,
                                dashCap: D2D1_CAP_STYLE_FLAT,
                                lineJoin: D2D1_LINE_JOIN_MITER,
                                miterLimit: 10.0,
                                dashStyle: D2D1_DASH_STYLE_CUSTOM,
                                dashOffset: 0.0,
                            };

                            if let Ok(stroke_style) =
                                d2d_factory.CreateStrokeStyle(&stroke_style_props, Some(dash))
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
        }
        Ok(())
    }

    fn draw_selection_handles(
        &mut self,
        rect: Rectangle,
        handle_size: f32,
        fill_color: Color,
        border_color: Color,
        border_width: f32,
    ) -> std::result::Result<(), Self::Error> {
        if let Some(ref render_target) = self.render_target {
            unsafe {
                // 创建填充和边框画刷
                let fill_d2d_color = D2D1_COLOR_F {
                    r: fill_color.r,
                    g: fill_color.g,
                    b: fill_color.b,
                    a: fill_color.a,
                };
                let border_d2d_color = D2D1_COLOR_F {
                    r: border_color.r,
                    g: border_color.g,
                    b: border_color.b,
                    a: border_color.a,
                };

                if let (Ok(fill_brush), Ok(border_brush)) = (
                    render_target.CreateSolidColorBrush(&fill_d2d_color, None),
                    render_target.CreateSolidColorBrush(&border_d2d_color, None),
                ) {
                    let half = handle_size / 2.0;
                    let cx = rect.x + rect.width / 2.0;
                    let cy = rect.y + rect.height / 2.0;

                    // 8个手柄位置：4个角 + 4个边中点
                    let handle_positions = [
                        (rect.x, rect.y),                            // 左上
                        (cx, rect.y),                                // 上中
                        (rect.x + rect.width, rect.y),               // 右上
                        (rect.x + rect.width, cy),                   // 右中
                        (rect.x + rect.width, rect.y + rect.height), // 右下
                        (cx, rect.y + rect.height),                  // 下中
                        (rect.x, rect.y + rect.height),              // 左下
                        (rect.x, cy),                                // 左中
                    ];

                    for (px, py) in handle_positions.iter() {
                        let handle_rect = D2D_RECT_F {
                            left: *px - half,
                            top: *py - half,
                            right: *px + half,
                            bottom: *py + half,
                        };

                        // 绘制填充
                        render_target.FillRectangle(&handle_rect, &fill_brush);

                        // 绘制边框
                        if border_width > 0.0 {
                            render_target.DrawRectangle(
                                &handle_rect,
                                &border_brush,
                                border_width,
                                None,
                            );
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn draw_element_handles(
        &mut self,
        rect: Rectangle,
        handle_radius: f32,
        fill_color: Color,
        border_color: Color,
        border_width: f32,
    ) -> std::result::Result<(), Self::Error> {
        if let Some(ref render_target) = self.render_target {
            unsafe {
                // 创建填充和边框画刷
                let fill_d2d_color = D2D1_COLOR_F {
                    r: fill_color.r,
                    g: fill_color.g,
                    b: fill_color.b,
                    a: fill_color.a,
                };
                let border_d2d_color = D2D1_COLOR_F {
                    r: border_color.r,
                    g: border_color.g,
                    b: border_color.b,
                    a: border_color.a,
                };

                if let (Ok(fill_brush), Ok(border_brush)) = (
                    render_target.CreateSolidColorBrush(&fill_d2d_color, None),
                    render_target.CreateSolidColorBrush(&border_d2d_color, None),
                ) {
                    let cx = rect.x + rect.width / 2.0;
                    let cy = rect.y + rect.height / 2.0;

                    // 8个圆形手柄位置：4个角 + 4个边中点
                    let handle_positions = [
                        (rect.x, rect.y),                            // 左上
                        (cx, rect.y),                                // 上中
                        (rect.x + rect.width, rect.y),               // 右上
                        (rect.x + rect.width, cy),                   // 右中
                        (rect.x + rect.width, rect.y + rect.height), // 右下
                        (cx, rect.y + rect.height),                  // 下中
                        (rect.x, rect.y + rect.height),              // 左下
                        (rect.x, cy),                                // 左中
                    ];

                    for (px, py) in handle_positions.iter() {
                        let handle_ellipse = D2D1_ELLIPSE {
                            point: windows_numerics::Vector2 { X: *px, Y: *py },
                            radiusX: handle_radius,
                            radiusY: handle_radius,
                        };

                        // 绘制填充
                        render_target.FillEllipse(&handle_ellipse, &fill_brush);

                        // 绘制边框
                        if border_width > 0.0 {
                            render_target.DrawEllipse(
                                &handle_ellipse,
                                &border_brush,
                                border_width,
                                None,
                            );
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

impl Direct2DRenderer {
    /// 绘制多行文本（支持滚动和裁剪）
    pub fn draw_multiline_text(
        &mut self,
        text: &str,
        rect: Rectangle,
        style: &TextStyle,
        scroll_offset: f32,
    ) -> std::result::Result<(), PlatformError> {
        if self.render_target.is_none() || self.dwrite_factory.is_none() {
            return Ok(());
        }

        // 使用缓存获取文本格式
        let text_format = self.get_or_create_text_format(&style.font_family, style.font_size)?;

        // 获取画刷（避免借用冲突，提前获取）
        let brush = self.get_or_create_brush(style.color)?;

        // 必须使用 dwrite_factory 的引用
        // Safety: 前面已经检查过 is_none() 会提前返回
        let (dwrite_factory, render_target) = match (&self.dwrite_factory, &self.render_target) {
            (Some(dw), Some(rt)) => (dw, rt),
            _ => return Ok(()), // 前面已经检查过，这里不应该到达
        };

        unsafe {
            // 创建文本布局
            let text_utf16: Vec<u16> = text.encode_utf16().collect();
            let text_layout = dwrite_factory
                .CreateTextLayout(
                    &text_utf16,
                    &text_format,
                    rect.width,
                    f32::MAX, // 高度不限制，允许完全布局
                )
                .map_err(|e| {
                    PlatformError::RenderError(format!("Failed to create text layout: {:?}", e))
                })?;

            // 设置裁剪区域（限制在rect内显示）
            let clip_rect = D2D_RECT_F {
                left: rect.x,
                top: rect.y,
                right: rect.x + rect.width,
                bottom: rect.y + rect.height,
            };

            render_target.PushAxisAlignedClip(&clip_rect, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);

            // 绘制文本布局
            // 文本起点为 rect.x, rect.y，由于滚动，y 需要减去 offset
            let origin = windows_numerics::Vector2 {
                X: rect.x,
                Y: rect.y - scroll_offset,
            };

            render_target.DrawTextLayout(origin, &text_layout, &brush, D2D1_DRAW_TEXT_OPTIONS_NONE);

            render_target.PopAxisAlignedClip();
        }

        Ok(())
    }

    /// 测量多行文本尺寸（用于计算滚动高度）
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

        unsafe {
            let text_utf16: Vec<u16> = text.encode_utf16().collect();
            let text_layout = dwrite_factory
                .CreateTextLayout(&text_utf16, &text_format, width, f32::MAX)
                .map_err(|e| {
                    PlatformError::RenderError(format!(
                        "Failed to create text layout for measure: {:?}",
                        e
                    ))
                })?;

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

        unsafe {
            let text_utf16: Vec<u16> = text.encode_utf16().collect();
            let text_layout = dwrite_factory
                .CreateTextLayout(&text_utf16, &text_format, width, f32::MAX)
                .map_err(|e| {
                    PlatformError::RenderError(format!(
                        "Failed to create text layout for split: {:?}",
                        e
                    ))
                })?;

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
                // metric.length includes trailing whitespace/newline
                // but we might want to trim newline if we are storing lines for rendering individually
                // However, OcrResultWindow logic seems to handle lines individually.
                // The original GDI logic split by \n first, then wrapped words.
                // DWrite CreateTextLayout handles \n too.

                if current_pos + len <= text_utf16.len() {
                    let line_utf16 = &text_utf16[current_pos..current_pos + len];
                    // Convert back to String
                    let line_str = String::from_utf16_lossy(line_utf16);
                    // Trim trailing newline if desired, but maybe keep it to match original behavior?
                    // The original wrap_text_lines output lines without \n usually, but let's check.
                    // Original split by \n, then wrapped.
                    // DWrite will generate a line for \n.
                    // Let's just return what DWrite gives.
                    // Remove \r\n or \n at end if present?
                    // render loop draws text. DrawText accepts \n but if we draw line by line...
                    // The previous logic in OcrResultWindow used TextOutW which doesn't handle \n (displays block).
                    // So lines should probably be clean.

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

        unsafe {
            let text_utf16: Vec<u16> = text.encode_utf16().collect();
            // Layout width doesn't matter for single line hit test, make it large
            let text_layout = dwrite_factory
                .CreateTextLayout(&text_utf16, &text_format, 10000.0, 1000.0)
                .map_err(|e| {
                    PlatformError::RenderError(format!(
                        "Failed to create text layout for hit test: {:?}",
                        e
                    ))
                })?;

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
