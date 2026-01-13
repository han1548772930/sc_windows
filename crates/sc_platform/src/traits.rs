pub use sc_rendering::{BitmapId, Color, DrawStyle, Point, Rectangle, TextStyle};
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
