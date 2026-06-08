pub use sc_rendering::{BitmapId, Color, DrawStyle, Point, Rectangle, TextStyle};
#[derive(Debug)]
pub enum PlatformError {
    RenderError(String),
    ResourceError(String),
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
