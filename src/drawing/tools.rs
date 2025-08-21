// 绘图工具管理
//
// 负责管理各种绘图工具的状态和配置

use crate::types::DrawingTool;

/// 工具管理器
pub struct ToolManager {
    /// 当前工具
    current_tool: DrawingTool,
}

// 注意：ToolConfigs 结构体已被移除
// 现在配置直接从 SimpleSettings 读取，避免重复存储和同步问题

impl ToolManager {
    /// 创建新的工具管理器
    pub fn new() -> Self {
        Self {
            current_tool: DrawingTool::None,
        }
    }

    /// 设置当前工具
    pub fn set_current_tool(&mut self, tool: DrawingTool) {
        self.current_tool = tool;
    }

    /// 获取当前工具
    pub fn get_current_tool(&self) -> DrawingTool {
        self.current_tool
    }

    /// 获取画笔颜色（直接从设置读取）
    pub fn get_brush_color(&self) -> windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F {
        let (drawing_color, _text_color, _selection_border_color, _toolbar_bg_color) =
            crate::constants::get_colors_from_settings();
        drawing_color
    }

    /// 获取画笔粗细（直接从设置读取）
    pub fn get_line_thickness(&self) -> f32 {
        let settings = crate::settings::Settings::load();
        settings.line_thickness
    }

    /// 获取文字大小（直接从设置读取）
    pub fn get_text_size(&self) -> f32 {
        let settings = crate::settings::Settings::load();
        settings.font_size
    }

    // 注意：设置器方法已被移除
    // 配置现在直接从 SimpleSettings 读取，避免重复存储和同步问题
    // 如需修改配置，请直接修改 SimpleSettings 并保存
}
