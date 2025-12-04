/// 颜色定义
#[derive(Debug, Clone, Copy)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

/// 点定义
#[derive(Debug, Clone, Copy)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

/// 矩形定义
#[derive(Debug, Clone, Copy)]
pub struct Rectangle {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rectangle {
    /// 创建新的矩形
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// 从左上角和右下角坐标创建矩形
    pub fn from_bounds(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            x: left,
            y: top,
            width: right - left,
            height: bottom - top,
        }
    }
}

impl From<windows::Win32::Foundation::RECT> for Rectangle {
    fn from(rect: windows::Win32::Foundation::RECT) -> Self {
        Rectangle {
            x: rect.left as f32,
            y: rect.top as f32,
            width: (rect.right - rect.left) as f32,
            height: (rect.bottom - rect.top) as f32,
        }
    }
}

impl From<&windows::Win32::Foundation::RECT> for Rectangle {
    fn from(rect: &windows::Win32::Foundation::RECT) -> Self {
        Rectangle {
            x: rect.left as f32,
            y: rect.top as f32,
            width: (rect.right - rect.left) as f32,
            height: (rect.bottom - rect.top) as f32,
        }
    }
}

/// 文本样式
#[derive(Debug, Clone)]
pub struct TextStyle {
    pub font_size: f32,
    pub color: Color,
    pub font_family: String,
}

/// 绘制样式
#[derive(Debug, Clone)]
pub struct DrawStyle {
    pub stroke_color: Color,
    pub fill_color: Option<Color>,
    pub stroke_width: f32,
}

/// 图像数据
pub struct Image {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// 画刷ID
pub type BrushId = u32;

/// 字体ID
pub type FontId = u32;

/// 平台位图ID（由渲染后端管理的位图句柄）
pub type BitmapId = u64;

/// 平台渲染器trait
pub trait PlatformRenderer: Send + Sync {
    type Error: std::error::Error + Send + Sync;

    /// 开始渲染帧
    fn begin_frame(&mut self) -> Result<(), Self::Error>;

    /// 结束渲染帧
    fn end_frame(&mut self) -> Result<(), Self::Error>;

    /// 清除画布
    fn clear(&mut self, color: Color) -> Result<(), Self::Error>;

    /// 绘制矩形
    fn draw_rectangle(&mut self, rect: Rectangle, style: &DrawStyle) -> Result<(), Self::Error>;

    /// 绘制圆角矩形
    fn draw_rounded_rectangle(
        &mut self,
        rect: Rectangle,
        _radius: f32,
        style: &DrawStyle,
    ) -> Result<(), Self::Error> {
        // 默认实现回退到绘制普通矩形
        self.draw_rectangle(rect, style)
    }

    /// 绘制圆形（支持填充color）
    fn draw_circle(
        &mut self,
        center: Point,
        radius: f32,
        style: &DrawStyle,
    ) -> Result<(), Self::Error>;

    /// 绘制虚线矩形（用于选择高亮）
    fn draw_dashed_rectangle(
        &mut self,
        rect: Rectangle,
        style: &DrawStyle,
        dash_pattern: &[f32],
    ) -> Result<(), Self::Error>;

    /// 绘制线条
    fn draw_line(&mut self, start: Point, end: Point, style: &DrawStyle)
    -> Result<(), Self::Error>;

    /// 绘制文本
    fn draw_text(
        &mut self,
        text: &str,
        position: Point,
        style: &TextStyle,
    ) -> Result<(), Self::Error>;

    /// 获取文本尺寸
    fn measure_text(&self, text: &str, style: &TextStyle) -> Result<(f32, f32), Self::Error>;

    /// 设置裁剪区域
    fn push_clip_rect(&mut self, rect: Rectangle) -> Result<(), Self::Error>;

    /// 恢复裁剪区域
    fn pop_clip_rect(&mut self) -> Result<(), Self::Error>;

    /// 获取Any引用（用于向下转型）
    fn as_any(&self) -> &dyn std::any::Any;

    /// 获取可变Any引用（用于向下转型）
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;

    /// 从GDI位图创建并缓存平台位图（平台无关接口）
    fn create_bitmap_from_gdi(
        &mut self,
        gdi_dc: windows::Win32::Graphics::Gdi::HDC,
        width: i32,
        height: i32,
    ) -> Result<(), Self::Error>;

    // ---------- 高层绘图接口 ----------
    // 这些方法封装了常见的UI绘图操作，隐藏底层实现细节

    /// 绘制选择区域遮罩
    fn draw_selection_mask(
        &mut self,
        screen_rect: Rectangle,
        selection_rect: Rectangle,
        mask_color: Color,
    ) -> Result<(), Self::Error>;

    /// 绘制选择区域边框
    fn draw_selection_border(
        &mut self,
        rect: Rectangle,
        color: Color,
        width: f32,
        dash_pattern: Option<&[f32]>,
    ) -> Result<(), Self::Error>;

    /// 绘制选择区域手柄
    fn draw_selection_handles(
        &mut self,
        rect: Rectangle,
        handle_size: f32,
        fill_color: Color,
        border_color: Color,
        border_width: f32,
    ) -> Result<(), Self::Error>;

    /// 绘制元素手柄
    fn draw_element_handles(
        &mut self,
        rect: Rectangle,
        handle_radius: f32,
        fill_color: Color,
        border_color: Color,
        border_width: f32,
    ) -> Result<(), Self::Error>;
}

/// 平台错误类型
#[derive(Debug)]
pub enum PlatformError {
    /// 渲染错误
    RenderError(String),
    /// 资源创建错误
    ResourceError(String),
    /// 初始化错误
    InitError(String),
}

impl std::fmt::Display for PlatformError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlatformError::RenderError(msg) => write!(f, "Platform render error: {msg}"),
            PlatformError::ResourceError(msg) => write!(f, "Platform resource error: {msg}"),
            PlatformError::InitError(msg) => write!(f, "Platform init error: {msg}"),
        }
    }
}

impl std::error::Error for PlatformError {}

pub trait RendererExt: PlatformRenderer {
    /// Draw 8 square handles around a rectangle
    fn draw_handles_for_rect(
        &mut self,
        rect: Rectangle,
        handle_size: f32,
        fill: Color,
        border: Color,
        border_width: f32,
    ) -> Result<(), Self::Error> {
        let half = handle_size / 2.0;
        let cx = rect.x + rect.width / 2.0;
        let cy = rect.y + rect.height / 2.0;
        let points = [
            (rect.x, rect.y),                            // TL
            (cx, rect.y),                                // TC
            (rect.x + rect.width, rect.y),               // TR
            (rect.x + rect.width, cy),                   // MR
            (rect.x + rect.width, rect.y + rect.height), // BR
            (cx, rect.y + rect.height),                  // BC
            (rect.x, rect.y + rect.height),              // BL
            (rect.x, cy),                                // ML
        ];
        let style = DrawStyle {
            stroke_color: border,
            fill_color: Some(fill),
            stroke_width: border_width,
        };
        for (px, py) in points.iter() {
            let hrect = Rectangle {
                x: *px - half,
                y: *py - half,
                width: handle_size,
                height: handle_size,
            };
            self.draw_rectangle(hrect, &style)?;
        }
        Ok(())
    }
}

impl<T: PlatformRenderer + ?Sized> RendererExt for T {}

// ========== 资源管理抽象 ==========

/// 图标资源 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IconId(pub u64);

impl IconId {
    /// 空图标 ID
    pub const NONE: IconId = IconId(0);
}

/// 资源管理器 trait
/// 
/// 提供平台无关的资源管理接口，使业务层无需直接持有 HICON、HBRUSH 等平台句柄。
pub trait ResourceManager {
    /// 加载图标资源
    fn load_icon(&mut self, name: &str) -> IconId;

    /// 释放图标资源
    fn release_icon(&mut self, id: IconId);

    /// 检查图标是否有效
    fn is_icon_valid(&self, id: IconId) -> bool;
}

/// 默认的空资源管理器实现（用于测试或不需要资源管理的场景）
pub struct NullResourceManager;

impl ResourceManager for NullResourceManager {
    fn load_icon(&mut self, _name: &str) -> IconId {
        IconId::NONE
    }

    fn release_icon(&mut self, _id: IconId) {}

    fn is_icon_valid(&self, id: IconId) -> bool {
        id != IconId::NONE
    }
}
