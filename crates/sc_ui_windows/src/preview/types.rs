use sc_app::selection::RectI32;
use std::collections::HashMap;
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap;

/// D2D 图标位图集合
pub struct D2DIconBitmaps {
    pub normal: ID2D1Bitmap,
    pub hover: ID2D1Bitmap,
    // active 状态可以共用 hover 或 normal，或者单独添加
    pub active_normal: Option<ID2D1Bitmap>,
    pub active_hover: Option<ID2D1Bitmap>,
}

/// SVG 图标结构 - 简化版，只保存位置和状态信息
#[derive(Clone)]
pub struct SvgIcon {
    pub name: String,
    pub rect: RectI32,
    pub hovered: bool,
    pub selected: bool,            // 是否选中（用于绘图工具）
    pub is_title_bar_button: bool, // 是否是标题栏按钮
}

/// DWM边距结构
#[repr(C)]
#[derive(Clone, Copy)]
#[allow(non_snake_case)]
pub struct MARGINS {
    pub cxLeftWidth: i32,
    pub cxRightWidth: i32,
    pub cyTopHeight: i32,
    pub cyBottomHeight: i32,
}

/// 图标缓存类型别名
pub type IconCache = HashMap<String, D2DIconBitmaps>;
