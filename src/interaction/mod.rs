// 统一交互抽象（阶段1 PoC）
//
// 目标：以最小侵入的方式引入 InteractionTarget + InteractionController，
// 仅用于 Selection 路径，复用现有 SelectionState 的交互实现，
// 不搬迁内部状态，后续阶段可逐步内聚到控制器。

use crate::types::DragMode;
use windows::Win32::Foundation::RECT;

/// 交互目标接口（阶段1最小集合）
/// 由具体对象（如选择框、绘图元素）实现，将已有的交互方法适配进来。
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

    /// 可选：暴露矩形，便于通用行为（本阶段不强制使用）
    fn rect(&self) -> Option<RECT> { None }
}

/// 交互控制器（阶段1：无内部状态，薄封装，仅负责编排调用）
#[derive(Default)]
pub struct InteractionController;

impl InteractionController {
    pub fn new() -> Self { Self }

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

