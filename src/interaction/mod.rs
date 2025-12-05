//! 交互控制模块
//!
//! 提供统一的鼠标交互处理逻辑。
//!
//! # 主要组件
//! - [`InteractionTarget`]: 交互目标 trait，由可交互对象实现
//! - [`InteractionController`]: 交互控制器，负责编排交互流程
//!
//! # 设计理念
//! 采用 trait object 设计，将交互逻辑与具体对象解耦，
//! 便于统一处理选区边框、绘图元素等的拖拽和调整操作。

use crate::drawing::DragMode;
use windows::Win32::Foundation::RECT;

/// 交互目标接口
/// 由具体对象（如选择框、绘图元素）实现。
pub trait InteractionTarget {
    /// 命中测试：返回拖拽模式（None 表示未命中）
    fn hit_test(&self, x: i32, y: i32) -> DragMode;
    /// 开始交互（记录起点与模式）
    fn begin_interaction(&mut self, x: i32, y: i32, mode: DragMode);
    /// 交互过程更新，返回是否更新了几何（用于是否触发重绘）
    fn update_interaction(&mut self, x: i32, y: i32) -> bool;
    /// 结束交互
    fn end_interaction(&mut self);
    /// 当前是否处于交互中
    fn is_interacting(&self) -> bool;

    /// 可选：暴露矩形，便于通用行为
    fn rect(&self) -> Option<RECT> {
        None
    }
}

/// 交互控制器，负责编排交互流程
#[derive(Default)]
pub struct InteractionController;

impl InteractionController {
    pub fn new() -> Self {
        Self
    }

    /// 处理按下：
    /// - 若命中目标手柄/内部，调用 begin_interaction 并返回已消费
    /// - 否则返回未消费
    pub fn mouse_down<T: InteractionTarget>(&mut self, target: &mut T, x: i32, y: i32) -> bool {
        let mode = target.hit_test(x, y);
        if mode != DragMode::None {
            target.begin_interaction(x, y, mode);
            true
        } else {
            false
        }
    }

    /// 处理移动：若目标正在交互，调用 update_interaction
    pub fn mouse_move<T: InteractionTarget>(&mut self, target: &mut T, x: i32, y: i32) -> bool {
        if target.is_interacting() {
            target.update_interaction(x, y)
        } else {
            false
        }
    }

    /// 处理释放：若目标正在交互，调用 end_interaction 并视为已消费
    pub fn mouse_up<T: InteractionTarget>(&mut self, target: &mut T, _x: i32, _y: i32) -> bool {
        if target.is_interacting() {
            target.end_interaction();
            true
        } else {
            false
        }
    }
}
