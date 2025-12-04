use crate::settings::Settings;
use crate::types::DrawingTool;
use std::sync::{Arc, RwLock};

/// 工具管理器
///
/// 管理当前选中的绘图工具，并通过共享的 Settings 引用获取工具配置。
pub struct ToolManager {
    /// 当前工具
    current_tool: DrawingTool,
    /// 共享的配置引用
    settings: Arc<RwLock<Settings>>,
}

impl ToolManager {
    /// 创建新的工具管理器
    pub fn new(settings: Arc<RwLock<Settings>>) -> Self {
        Self {
            current_tool: DrawingTool::None,
            settings,
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

    /// 获取画笔颜色
    pub fn get_brush_color(&self) -> windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F {
        let settings = self.settings.read().unwrap_or_else(|e| e.into_inner());
        windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F {
            r: settings.drawing_color_red as f32 / 255.0,
            g: settings.drawing_color_green as f32 / 255.0,
            b: settings.drawing_color_blue as f32 / 255.0,
            a: 1.0,
        }
    }

    /// 获取画笔粗细
    pub fn get_line_thickness(&self) -> f32 {
        self.settings
            .read()
            .map(|s| s.line_thickness)
            .unwrap_or(3.0)
    }

    /// 获取文字大小
    pub fn get_text_size(&self) -> f32 {
        self.settings
            .read()
            .map(|s| s.font_size)
            .unwrap_or(20.0)
    }
}
