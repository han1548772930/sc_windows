// 平台渲染器trait定义
//
// 定义了平台无关的渲染接口

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

/// 平台渲染器trait
pub trait PlatformRenderer {
    type Error: std::error::Error;

    /// 开始渲染帧
    fn begin_frame(&mut self) -> Result<(), Self::Error>;

    /// 结束渲染帧
    fn end_frame(&mut self) -> Result<(), Self::Error>;

    /// 清除画布
    fn clear(&mut self, color: Color) -> Result<(), Self::Error>;

    /// 绘制图像
    fn draw_image(&mut self, image: &Image, rect: Rectangle) -> Result<(), Self::Error>;

    /// 绘制矩形
    fn draw_rectangle(&mut self, rect: Rectangle, style: &DrawStyle) -> Result<(), Self::Error>;

    /// 绘制圆形
    fn draw_circle(
        &mut self,
        center: Point,
        radius: f32,
        style: &DrawStyle,
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

    /// 创建画刷
    fn create_brush(&mut self, color: Color) -> Result<BrushId, Self::Error>;

    /// 创建字体
    fn create_font(&mut self, family: &str, size: f32) -> Result<FontId, Self::Error>;

    /// 获取文本尺寸
    fn measure_text(&self, text: &str, style: &TextStyle) -> Result<(f32, f32), Self::Error>;

    /// 设置裁剪区域（从原始代码迁移）
    fn push_clip_rect(&mut self, rect: Rectangle) -> Result<(), Self::Error>;

    /// 恢复裁剪区域（从原始代码迁移）
    fn pop_clip_rect(&mut self) -> Result<(), Self::Error>;

    /// 获取Any引用（用于向下转型）
    fn as_any(&self) -> &dyn std::any::Any;

    /// 获取可变Any引用（用于向下转型）
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

/// 平台错误类型
#[derive(Debug)]
pub enum PlatformError {
    /// 渲染错误
    RenderError(String),
    /// 资源创建错误
    ResourceError(String),
    /// 初始化错误
    InitializationError(String),
    /// 初始化错误
    InitError(String),
}

impl std::fmt::Display for PlatformError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlatformError::RenderError(msg) => write!(f, "Platform render error: {}", msg),
            PlatformError::ResourceError(msg) => write!(f, "Platform resource error: {}", msg),
            PlatformError::InitializationError(msg) => {
                write!(f, "Platform initialization error: {}", msg)
            }
            PlatformError::InitError(msg) => write!(f, "Platform init error: {}", msg),
        }
    }
}

impl std::error::Error for PlatformError {}
