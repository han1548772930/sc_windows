//! 处理中状态处理器
//!
//! 处理应用程序在执行OCR或保存等操作时的事件

use super::AppStateHandler;
use crate::state::ProcessingOperation;

/// 处理中状态
pub struct ProcessingState {
    /// 处理操作类型
    pub operation: ProcessingOperation,
}

impl ProcessingState {
    pub fn new(operation: ProcessingOperation) -> Self {
        Self { operation }
    }
}

/// ProcessingState 使用 AppStateHandler 的默认实现
/// 
/// 处理中状态不响应任何鼠标/键盘事件（ESC 由 App 层统一处理）。
impl AppStateHandler for ProcessingState {
    fn name(&self) -> &'static str {
        "Processing"
    }
    // 其他方法使用 trait 默认实现（返回空命令、不消费事件、无状态转换）
}
