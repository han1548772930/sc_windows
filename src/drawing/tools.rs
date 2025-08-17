// 绘图工具管理
//
// 负责管理各种绘图工具的状态和配置

use crate::types::DrawingTool;

/// 工具管理器
pub struct ToolManager {
    /// 当前工具
    current_tool: DrawingTool,
    /// 工具配置
    tool_configs: ToolConfigs,
}

/// 工具配置
pub struct ToolConfigs {
    /// 画笔粗细
    pub brush_thickness: f32,
    /// 画笔颜色
    pub brush_color: (f32, f32, f32, f32), // RGBA
    /// 文字大小
    pub text_size: f32,
}

impl ToolManager {
    /// 创建新的工具管理器
    pub fn new() -> Self {
        Self {
            current_tool: DrawingTool::None,
            tool_configs: ToolConfigs {
                brush_thickness: 3.0,
                brush_color: (1.0, 0.0, 0.0, 1.0), // 红色
                text_size: 16.0,
            },
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

    /// 获取工具配置
    pub fn get_configs(&self) -> &ToolConfigs {
        &self.tool_configs
    }

    /// 设置画笔粗细
    pub fn set_brush_thickness(&mut self, thickness: f32) {
        self.tool_configs.brush_thickness = thickness;
    }

    /// 设置画笔颜色
    pub fn set_brush_color(&mut self, r: f32, g: f32, b: f32, a: f32) {
        self.tool_configs.brush_color = (r, g, b, a);
    }

    /// 设置文字大小
    pub fn set_text_size(&mut self, size: f32) {
        self.tool_configs.text_size = size;
    }
}
