use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;

pub const WINDOW_CLASS_NAME: &str = "sc_windows_main";
pub const MIN_BOX_SIZE: i32 = 50;
pub const TEXT_BOX_WIDTH: i32 = 100;
pub const TEXT_BOX_HEIGHT: i32 = 30;
// 颜色常量 - 使用D2D1_COLOR_F格式
pub const COLOR_SELECTION_BORDER: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.0,
    g: 0.47,
    b: 0.84,
    a: 1.0,
};
pub const COLOR_SELECTION_DASHED: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.31,
    g: 0.31,
    b: 0.31,
    a: 1.0,
};
pub const COLOR_HANDLE_FILL: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 1.0,
};
pub const COLOR_HANDLE_BORDER: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.0,
    g: 0.47,
    b: 0.84,
    a: 1.0,
};
pub const COLOR_MASK: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.6,
};
pub const COLOR_TOOLBAR_BG: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 0.95,
};
pub const COLOR_BUTTON_HOVER: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.85, // 更深的hover颜色
    g: 0.85,
    b: 0.85,
    a: 1.0,
};
pub const COLOR_BUTTON_ACTIVE: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.78,
    g: 0.9,
    b: 1.0,
    a: 1.0,
};
pub const COLOR_TEXT_NORMAL: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.25,
    g: 0.25,
    b: 0.25,
    a: 1.0,
};

// 文字输入相关颜色
pub const COLOR_TEXT_BORDER: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.5, // 灰色
    g: 0.5,
    b: 0.5,
    a: 1.0,
};

pub const COLOR_TEXT_CURSOR: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.0, // 绿色光标
    g: 1.0,
    b: 0.0,
    a: 1.0,
};

// 工具栏尺寸常量
pub const TOOLBAR_HEIGHT: f32 = 40.0;
pub const BUTTON_WIDTH: f32 = 30.0;
pub const BUTTON_HEIGHT: f32 = 30.0;
pub const BUTTON_SPACING: f32 = 4.0;
pub const TOOLBAR_PADDING: f32 = 8.0;
pub const TOOLBAR_MARGIN: f32 = 3.0;
pub const BUTTON_COUNT: i32 = 12;

// 尺寸常量
pub const HANDLE_SIZE: f32 = 8.0;
pub const HANDLE_DETECTION_RADIUS: f32 = 10.0;

// 文字输入相关常量
pub const DEFAULT_TEXT_WIDTH: i32 = 120; // 调整为更合理的初始宽度
pub const DEFAULT_TEXT_HEIGHT: i32 = 32; // 调整为更合理的初始高度
pub const MIN_TEXT_WIDTH: i32 = 40;
pub const MIN_TEXT_HEIGHT: i32 = 20;
pub const MAX_TEXT_WIDTH: i32 = 400; // 保留用于向后兼容，但实际不再使用
pub const LINE_HEIGHT: i32 = 24; // 每行高度
pub const CHAR_WIDTH: f32 = 15.0; // 平均字符宽度（进一步增大以确保准确性）
pub const TEXT_PADDING: f32 = 8.0; // 增加内边距以确保文字不被挤压

/// 从设置文件加载颜色，如果加载失败则使用默认值
pub fn get_colors_from_settings() -> (D2D1_COLOR_F, D2D1_COLOR_F, D2D1_COLOR_F, D2D1_COLOR_F) {
    let settings = crate::simple_settings::SimpleSettings::load();

    // 绘图颜色（用于画笔、矩形、圆形、箭头等）
    let drawing_color = D2D1_COLOR_F {
        r: settings.drawing_color_red as f32 / 255.0,
        g: settings.drawing_color_green as f32 / 255.0,
        b: settings.drawing_color_blue as f32 / 255.0,
        a: 1.0,
    };

    // 文字颜色
    let text_color = D2D1_COLOR_F {
        r: settings.text_color_red as f32 / 255.0,
        g: settings.text_color_green as f32 / 255.0,
        b: settings.text_color_blue as f32 / 255.0,
        a: 1.0,
    };

    // 选择框边框颜色（使用绘图颜色但稍微调亮）
    let selection_border_color = D2D1_COLOR_F {
        r: (settings.drawing_color_red as f32 / 255.0 * 0.8 + 0.2).min(1.0),
        g: (settings.drawing_color_green as f32 / 255.0 * 0.8 + 0.2).min(1.0),
        b: (settings.drawing_color_blue as f32 / 255.0 * 0.8 + 0.2).min(1.0),
        a: 1.0,
    };

    // 工具栏背景颜色（使用深色）
    let toolbar_bg_color = D2D1_COLOR_F {
        r: 0.2,
        g: 0.2,
        b: 0.2,
        a: 0.95, // 保持一定透明度
    };

    (
        drawing_color,
        text_color,
        selection_border_color,
        toolbar_bg_color,
    )
}

// OCR 结果窗口相关常量（从 ocr_result_window.rs 集中至此）
pub const LEFTEXTENDWIDTH: i32 = 0;
pub const RIGHTEXTENDWIDTH: i32 = 0;
pub const BOTTOMEXTENDWIDTH: i32 = 0;
pub const TOPEXTENDWIDTH: i32 = 60; // 标准标题栏高度
pub const TOPEXTENDWIDTHMAX: i32 = 70; // 最大化时标题栏高度
pub const MAXIMIZE_OFFSET: i32 = 15; // 最大化时的偏移量

// SVG 图标常量
pub const ICON_SIZE: i32 = 24; // 图标大小
pub const ICON_SPACING: i32 = 20; // 图标间距
pub const ICON_START_X: i32 = 12; // 图标起始位置 - 左对齐
pub const ICON_HOVER_PADDING: i32 = 8; // 图标悬停背景 padding
pub const ICON_CLICK_PADDING: i32 = 16; // 图标点击检测区域 padding
pub const ICON_HOVER_BG_COLOR: (u8, u8, u8) = (0xE1, 0xF3, 0xFF); // 悬停背景颜色（浅蓝色）
pub const ICON_HOVER_RADIUS: f32 = 6.0; // 悬停背景圆角半径

// 标题栏按钮常量
pub const TITLE_BAR_BUTTON_WIDTH: i32 = 70; // 标题栏按钮宽度
pub const TITLE_BAR_BUTTON_HOVER_PADDING: i32 = 20; // 标题栏按钮悬停背景 padding
pub const TITLE_BAR_BUTTON_HOVER_BG_COLOR: (u8, u8, u8) = (0xE0, 0xE0, 0xE0); // 悬停背景颜色（灰色）
pub const CLOSE_BUTTON_HOVER_BG_COLOR: (u8, u8, u8) = (0xE8, 0x11, 0x23); // 关闭按钮悬停背景颜色（红色）
