// Windows平台实现
//
// 提供Windows平台特定的渲染实现

pub mod d2d;
pub mod gdi;

pub mod system;

pub use d2d::Direct2DRenderer;
