use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;

use super::DrawingTool;
use crate::DrawingConfig;

/// 工具管理器
///
/// 管理当前选中的绘图工具，并通过 host 注入的 [`DrawingConfig`] 获取工具配置。
pub struct ToolManager {
    current_tool: DrawingTool,
    config: DrawingConfig,
}

impl ToolManager {
    pub fn new(config: DrawingConfig) -> Self {
        Self {
            current_tool: DrawingTool::None,
            config,
        }
    }

    pub fn update_config(&mut self, config: DrawingConfig) {
        self.config = config;
    }

    pub fn set_current_tool(&mut self, tool: DrawingTool) {
        self.current_tool = tool;
    }

    pub fn get_current_tool(&self) -> DrawingTool {
        self.current_tool
    }

    pub fn get_brush_color(&self) -> D2D1_COLOR_F {
        let (r, g, b) = self.config.drawing_color;
        D2D1_COLOR_F {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: 1.0,
        }
    }

    pub fn get_line_thickness(&self) -> f32 {
        self.config.line_thickness
    }

    pub fn get_text_size(&self) -> f32 {
        self.config.font_size
    }
}
