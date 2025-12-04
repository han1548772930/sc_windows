//! 应用状态相关类型定义
//!
//! 包含应用程序状态机和处理操作类型。

use windows::Win32::Foundation::RECT;
use crate::drawing::types::DrawingTool;

/// 应用程序状态机
///
/// 统一管理应用的主要状态，简化状态转换和事件处理逻辑。
#[derive(Debug, Clone, PartialEq)]
pub enum AppState {
    /// 空闲状态 - 窗口隐藏，等待热键触发
    Idle,
    /// 框选状态 - 正在框选屏幕区域
    Selecting {
        /// 框选起点
        start_x: i32,
        start_y: i32,
        /// 当前位置
        current_x: i32,
        current_y: i32,
    },
    /// 编辑状态 - 已选择区域，可以进行绘图编辑
    Editing {
        /// 选择区域
        selection: RECT,
        /// 当前绘图工具
        tool: DrawingTool,
    },
    /// 处理中状态 - 正在执行OCR或保存等操作
    Processing {
        /// 处理类型描述
        operation: ProcessingOperation,
    },
}

/// 处理操作类型
#[derive(Debug, Clone, PartialEq)]
pub enum ProcessingOperation {
    /// OCR文字识别
    Ocr,
    /// 保存文件
    Saving,
    /// 复制到剪贴板
    CopyingToClipboard,
}

impl Default for AppState {
    fn default() -> Self {
        Self::Idle
    }
}

impl AppState {
    /// 检查是否处于空闲状态
    pub fn is_idle(&self) -> bool {
        matches!(self, AppState::Idle)
    }

    /// 检查是否处于框选状态
    pub fn is_selecting(&self) -> bool {
        matches!(self, AppState::Selecting { .. })
    }

    /// 检查是否处于编辑状态
    pub fn is_editing(&self) -> bool {
        matches!(self, AppState::Editing { .. })
    }

    /// 检查是否处于处理中状态
    pub fn is_processing(&self) -> bool {
        matches!(self, AppState::Processing { .. })
    }

    /// 获取当前选择区域（如果有）
    pub fn get_selection(&self) -> Option<RECT> {
        match self {
            AppState::Editing { selection, .. } => Some(*selection),
            _ => None,
        }
    }

    /// 获取当前绘图工具（如果在编辑状态）
    pub fn get_tool(&self) -> Option<DrawingTool> {
        match self {
            AppState::Editing { tool, .. } => Some(*tool),
            _ => None,
        }
    }
}
