use sc_app::selection::RectI32;
use std::collections::HashMap;
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap;

pub struct D2DIconBitmaps {
    pub normal: ID2D1Bitmap,
    pub hover: ID2D1Bitmap,
    pub active_normal: Option<ID2D1Bitmap>,
    pub active_hover: Option<ID2D1Bitmap>,
}

#[derive(Clone)]
pub struct SvgIcon {
    pub name: String,
    pub rect: RectI32,
    pub hovered: bool,
    pub selected: bool,
    pub is_title_bar_button: bool,
}

#[repr(C)]
#[derive(Clone, Copy)]
#[allow(non_snake_case)]
pub struct Margins {
    pub cxLeftWidth: i32,
    pub cxRightWidth: i32,
    pub cyTopHeight: i32,
    pub cyBottomHeight: i32,
}

pub type IconCache = HashMap<String, D2DIconBitmaps>;
