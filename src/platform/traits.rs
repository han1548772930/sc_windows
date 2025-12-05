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
