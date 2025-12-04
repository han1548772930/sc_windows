//! 绘图模块
//!
//! 提供截图编辑功能，包括各种绘图工具和元素管理。
//!
//! # 主要组件
//! - [`DrawingManager`]: 绘图管理器，统一管理绘图状态和操作
//! - [`ElementManager`](elements::ElementManager): 元素管理器，管理所有绘图元素
//! - [`HistoryManager`](history::HistoryManager): 历史记录管理器，支持撤销/重做
//!
//! # 支持的绘图工具
//! - 铅笔（自由绘制）
//! - 矩形
//! - 圆形/椭圆
//! - 箭头
//! - 文本标注

use crate::message::{Command, DrawingMessage};
use crate::settings::Settings;
use crate::types::{DrawingElement, DrawingTool};
use std::sync::{Arc, RwLock};

pub mod cache;
pub mod elements;
pub mod history;
pub mod rendering;
pub mod text_editing;
pub mod tools;

use cache::GeometryCache;
use elements::ElementManager;
use history::HistoryManager;
use tools::ToolManager;

#[derive(Debug, Clone, PartialEq)]
pub enum ElementInteractionMode {
    None,
    Drawing,
    MovingElement,
    ResizingElement(crate::types::DragMode),
}

impl ElementInteractionMode {
    fn from_drag_mode(drag_mode: crate::types::DragMode) -> Self {
        match drag_mode {
            crate::types::DragMode::None => ElementInteractionMode::None,
            crate::types::DragMode::DrawingShape => ElementInteractionMode::Drawing,
            crate::types::DragMode::MovingElement => ElementInteractionMode::MovingElement,
            crate::types::DragMode::ResizingTopLeft
            | crate::types::DragMode::ResizingTopCenter
            | crate::types::DragMode::ResizingTopRight
            | crate::types::DragMode::ResizingMiddleRight
            | crate::types::DragMode::ResizingBottomRight
            | crate::types::DragMode::ResizingBottomCenter
            | crate::types::DragMode::ResizingBottomLeft
            | crate::types::DragMode::ResizingMiddleLeft => {
                ElementInteractionMode::ResizingElement(drag_mode)
            }
            _ => ElementInteractionMode::None,
        }
    }
}

pub struct DrawingManager {
    /// 共享的配置引用
    pub(super) settings: Arc<RwLock<Settings>>,
    pub(super) tools: ToolManager,
    pub(super) elements: ElementManager,
    pub(super) history: HistoryManager,
    /// 几何体缓存管理器
    pub(super) geometry_cache: GeometryCache,
    pub(super) current_tool: DrawingTool,
    pub(super) current_element: Option<DrawingElement>,
    pub(super) selected_element: Option<usize>,
    pub(super) interaction_mode: ElementInteractionMode,
    pub(super) mouse_pressed: bool,
    pub(super) interaction_start_pos: windows::Win32::Foundation::POINT,
    pub(super) interaction_start_rect: windows::Win32::Foundation::RECT,
    pub(super) interaction_start_font_size: f32,
    pub(super) text_editing: bool,
    pub(super) editing_element_index: Option<usize>,
    pub(super) text_cursor_pos: usize,
    pub(super) text_cursor_visible: bool,
    pub(super) cursor_timer_id: usize,
    pub(super) just_saved_text: bool,
    pub(super) cache_dirty: std::cell::RefCell<bool>,
}

impl DrawingManager {
    /// 设置当前绘图工具（同步 ToolManager 与本地状态）
    pub fn set_current_tool(&mut self, tool: DrawingTool) {
        self.current_tool = tool;
        self.tools.set_current_tool(tool);
    }

    /// 创建新的绘图管理器
    ///
    /// # 参数
    /// - `settings`: 共享的配置引用
    pub fn new(settings: Arc<RwLock<Settings>>) -> Result<Self, DrawingError> {
        Ok(Self {
            tools: ToolManager::new(Arc::clone(&settings)),
            settings,
            elements: ElementManager::new(),
            history: HistoryManager::new(),
            geometry_cache: GeometryCache::new(),
            current_tool: DrawingTool::None,
            current_element: None,
            selected_element: None,

            interaction_mode: ElementInteractionMode::None,
            mouse_pressed: false,
            interaction_start_pos: windows::Win32::Foundation::POINT { x: 0, y: 0 },
            interaction_start_rect: windows::Win32::Foundation::RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            interaction_start_font_size: 0.0,

            text_editing: false,
            editing_element_index: None,
            text_cursor_pos: 0,
            text_cursor_visible: false,
            cursor_timer_id: 1001,
            just_saved_text: false,
            cache_dirty: std::cell::RefCell::new(true),
        })
    }

    /// 重置状态
    pub fn reset_state(&mut self) {
        self.current_element = None;
        self.selected_element = None;
        self.current_tool = DrawingTool::None;
        self.history.clear();
        self.elements.clear();
        self.geometry_cache.invalidate_all();
        self.interaction_mode = ElementInteractionMode::None;
        self.mouse_pressed = false;
        self.text_editing = false;
        self.editing_element_index = None;
        self.text_cursor_pos = 0;
        self.text_cursor_visible = false;
        self.just_saved_text = false;
        self.cache_dirty.replace(true);
    }

    /// 处理绘图消息
    pub fn handle_message(&mut self, message: DrawingMessage) -> Vec<Command> {
        match message {
            DrawingMessage::SelectTool(tool) => {
                let mut commands = Vec::new();

                if self.text_editing {
                    commands.extend(self.stop_text_editing());
                }

                self.current_tool = tool;
                self.tools.set_current_tool(tool);
                self.selected_element = None;
                self.elements.set_selected(None);

                commands.extend(vec![Command::UpdateToolbar, Command::RequestRedraw]);
                commands
            }
            DrawingMessage::StartDrawing(x, y) => {
                if self.current_tool != DrawingTool::None {
                    let mut element = DrawingElement::new(self.current_tool);
                    element
                        .points
                        .push(windows::Win32::Foundation::POINT { x, y });
                    self.current_element = Some(element);
                    vec![Command::RequestRedraw]
                } else {
                    vec![]
                }
            }
            DrawingMessage::UpdateDrawing(x, y) => {
                if let Some(ref mut element) = self.current_element {
                    match self.current_tool {
                        DrawingTool::Pen => {
                            element
                                .points
                                .push(windows::Win32::Foundation::POINT { x, y });
                            // Invalidate geometry cache
                            element.path_geometry.replace(None);
                        }
                        DrawingTool::Rectangle | DrawingTool::Circle | DrawingTool::Arrow => {
                            if element.points.len() >= 2 {
                                element.points[1] = windows::Win32::Foundation::POINT { x, y };
                            } else {
                                element
                                    .points
                                    .push(windows::Win32::Foundation::POINT { x, y });
                            }
                        }
                        _ => {}
                    }
                    element.update_bounding_rect();
                    vec![Command::RequestRedraw]
                } else {
                    vec![]
                }
            }
            DrawingMessage::FinishDrawing => {
                if let Some(element) = self.current_element.take() {
                    self.elements.add_element(element);
                    self.cache_dirty.replace(true);
                    vec![Command::UpdateToolbar, Command::RequestRedraw]
                } else {
                    vec![]
                }
            }
            DrawingMessage::Undo => {
                if let Some((elements, sel)) = self.history.undo() {
                    // 获取变更的元素索引（在恢复状态前）
                    let changed = self.history.get_last_changed_indices().to_vec();
                    
                    self.elements.restore_state(elements);
                    self.selected_element = sel;
                    self.elements.set_selected(self.selected_element);
                    self.cache_dirty.replace(true);
                    
                    // 使用增量式缓存失效：只标记变更的元素为 dirty
                    if changed.is_empty() {
                        self.geometry_cache.invalidate_all();
                    } else {
                        self.geometry_cache.mark_dirty_batch(&changed);
                    }
                    vec![Command::UpdateToolbar, Command::RequestRedraw]
                } else {
                    vec![Command::UpdateToolbar]
                }
            }
            DrawingMessage::Redo => {
                if let Some((elements, sel)) = self.history.redo() {
                    // 获取变更的元素索引（在恢复状态前）
                    let changed = self.history.get_last_changed_indices().to_vec();
                    
                    self.elements.restore_state(elements);
                    self.selected_element = sel;
                    self.elements.set_selected(self.selected_element);
                    self.cache_dirty.replace(true);
                    
                    // 使用增量式缓存失效
                    if changed.is_empty() {
                        self.geometry_cache.invalidate_all();
                    } else {
                        self.geometry_cache.mark_dirty_batch(&changed);
                    }
                    vec![Command::RequestRedraw]
                } else {
                    vec![]
                }
            }
            DrawingMessage::DeleteElement(index) => {
                self.history
                    .save_state(&self.elements, self.selected_element);
                if self.elements.remove_element(index) {
                    self.selected_element = None;
                    self.cache_dirty.replace(true);
                    self.geometry_cache.remove(index); // 删除对应元素的缓存
                    vec![Command::RequestRedraw]
                } else {
                    vec![]
                }
            }
            DrawingMessage::SelectElement(index) => {
                let old_selection = self.selected_element;
                self.selected_element = index;
                self.elements.set_selected(index);

                if let Some(idx) = index
                    && let Some(element) = self.elements.get_elements().get(idx) {
                        self.current_tool = element.tool;
                        self.tools.set_current_tool(element.tool);
                    }

                // Only invalidate cache if selection changed (an element moves from static layer to dynamic, or vice-versa)
                if old_selection != index {
                    self.cache_dirty.replace(true);
                }

                vec![Command::UpdateToolbar, Command::RequestRedraw]
            }
            DrawingMessage::AddElement(element) => {
                self.history
                    .save_state(&self.elements, self.selected_element);
                self.elements.add_element(*element);
                vec![Command::RequestRedraw]
            }
            DrawingMessage::CheckElementClick(x, y) => {
                if let Some(element_index) = self.elements.get_element_at_position(x, y) {
                    let old_selection = self.selected_element;
                    self.selected_element = Some(element_index);
                    self.elements.set_selected(self.selected_element);

                    if old_selection != self.selected_element {
                        self.cache_dirty.replace(true);
                    }
                    vec![Command::UpdateToolbar, Command::RequestRedraw]
                } else {
                    let old_selection = self.selected_element;
                    self.selected_element = None;
                    self.elements.set_selected(None);

                    if old_selection.is_some() {
                        self.cache_dirty.replace(true);
                    }
                    vec![Command::UpdateToolbar, Command::RequestRedraw]
                }
            }
        }
    }

    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }
    pub fn handle_mouse_move(
        &mut self,
        x: i32,
        y: i32,
        selection_rect: Option<windows::Win32::Foundation::RECT>,
    ) -> (Vec<Command>, bool) {
        if self.mouse_pressed {
            // 添加拖拽距离阈值检查（与旧代码保持一致）
            // 只有当移动距离超过阈值时才开始真正的拖拽
            if crate::utils::is_drag_threshold_exceeded(
                self.interaction_start_pos.x,
                self.interaction_start_pos.y,
                x,
                y,
            ) {
                self.update_drag(x, y, selection_rect);
                (vec![Command::RequestRedraw], true)
            } else {
                // 移动距离不够，不进行拖拽，但仍然消费事件（因为鼠标已按下）
                (vec![], true)
            }
        } else {
            // 检查是否悬停在元素上（用于改变光标/预览）
            if let Some(_index) = self.elements.get_element_at_position(x, y) {
                // 可在后续添加悬停反馈，但不消费事件
                (vec![], false)
            } else {
                (vec![], false)
            }
        }
    }

    fn update_drag(
        &mut self,
        x: i32,
        y: i32,
        selection_rect: Option<windows::Win32::Foundation::RECT>,
    ) {
        match &self.interaction_mode {
            ElementInteractionMode::Drawing => {
                if let Some(ref mut element) = self.current_element {
                    // 如果有选择框，限制绘制在选择框内（从原始代码迁移）
                    let (clamped_x, clamped_y) = if let Some(rect) = selection_rect {
                        crate::utils::clamp_to_rect(x, y, &rect)
                    } else {
                        (x, y)
                    };

                    match element.tool {
                        DrawingTool::Pen => {
                            element.points.push(windows::Win32::Foundation::POINT {
                                x: clamped_x,
                                y: clamped_y,
                            });
                            // Invalidate geometry cache
                            element.path_geometry.replace(None);
                        }
                        DrawingTool::Rectangle | DrawingTool::Circle | DrawingTool::Arrow => {
                            if element.points.is_empty() {
                                element.points.push(self.interaction_start_pos);
                            }
                            if element.points.len() == 1 {
                                element.points.push(windows::Win32::Foundation::POINT {
                                    x: clamped_x,
                                    y: clamped_y,
                                });
                            } else {
                                element.points[1] = windows::Win32::Foundation::POINT {
                                    x: clamped_x,
                                    y: clamped_y,
                                };
                            }
                            // 更新rect信息（用于边界检查）
                            let start = &element.points[0];
                            let end = &element.points[1];
                            element.rect = windows::Win32::Foundation::RECT {
                                left: start.x.min(end.x),
                                top: start.y.min(end.y),
                                right: start.x.max(end.x),
                                bottom: start.y.max(end.y),
                            };
                        }
                        _ => {}
                    }
                }
            }
            ElementInteractionMode::MovingElement => {
                if let Some(index) = self.selected_element
                    && let Some(element) = self.elements.get_elements().get(index)
                        && element.tool != DrawingTool::Pen {
                            let dx = x - self.interaction_start_pos.x;
                            let dy = y - self.interaction_start_pos.y;
                            if let Some(el) = self.elements.get_element_mut(index) {
                                let current_dx = el.rect.left - self.interaction_start_rect.left;
                                let current_dy = el.rect.top - self.interaction_start_rect.top;
                                el.move_by(-current_dx, -current_dy);
                                el.move_by(dx, dy);
                            }
                        }
            }
            ElementInteractionMode::ResizingElement(resize_mode) => {
                if let Some(index) = self.selected_element
                    && let Some(el) = self.elements.get_element_mut(index) {
                        if el.tool == DrawingTool::Pen {
                            return;
                        }
                        let mut new_rect = self.interaction_start_rect;
                        let dx = x - self.interaction_start_pos.x;
                        let dy = y - self.interaction_start_pos.y;
                        match resize_mode {
                            crate::types::DragMode::ResizingTopLeft => {
                                new_rect.left += dx;
                                new_rect.top += dy;
                            }
                            crate::types::DragMode::ResizingTopCenter => {
                                new_rect.top += dy;
                            }
                            crate::types::DragMode::ResizingTopRight => {
                                new_rect.right += dx;
                                new_rect.top += dy;
                            }
                            crate::types::DragMode::ResizingMiddleRight => {
                                new_rect.right += dx;
                            }
                            crate::types::DragMode::ResizingBottomRight => {
                                new_rect.right += dx;
                                new_rect.bottom += dy;
                            }
                            crate::types::DragMode::ResizingBottomCenter => {
                                new_rect.bottom += dy;
                            }
                            crate::types::DragMode::ResizingBottomLeft => {
                                new_rect.left += dx;
                                new_rect.bottom += dy;
                            }
                            crate::types::DragMode::ResizingMiddleLeft => {
                                new_rect.left += dx;
                            }
                            _ => {}
                        }

                        match el.tool {
                            DrawingTool::Arrow => {
                                // 仅支持通过左上角/右下角手柄调整起点/终点（与旧逻辑一致）
                                if el.points.len() >= 2 {
                                    match resize_mode {
                                        crate::types::DragMode::ResizingTopLeft => {
                                            el.points[0] =
                                                windows::Win32::Foundation::POINT { x, y };
                                            el.update_bounding_rect();
                                        }
                                        crate::types::DragMode::ResizingBottomRight => {
                                            el.points[1] =
                                                windows::Win32::Foundation::POINT { x, y };
                                            el.update_bounding_rect();
                                        }
                                        _ => {
                                            // 其他手柄对箭头不生效
                                        }
                                    }
                                }
                            }
                            DrawingTool::Text => {
                                // 文本元素：只允许通过四个角等比例缩放
                                // 边缘中点不允许调整文本框大小
                                let is_corner_resize = matches!(
                                    resize_mode,
                                    crate::types::DragMode::ResizingTopLeft
                                        | crate::types::DragMode::ResizingTopRight
                                        | crate::types::DragMode::ResizingBottomLeft
                                        | crate::types::DragMode::ResizingBottomRight
                                );

                                if is_corner_resize {
                                    let original_width = (self.interaction_start_rect.right
                                        - self.interaction_start_rect.left)
                                        .max(1);
                                    let original_height = (self.interaction_start_rect.bottom
                                        - self.interaction_start_rect.top)
                                        .max(1);

                                    // 计算缩放比例（取X和Y方向的平均值，确保等比例缩放）
                                    let (scale_x, scale_y) = match resize_mode {
                                        crate::types::DragMode::ResizingTopLeft => (
                                            (original_width - dx) as f32 / original_width as f32,
                                            (original_height - dy) as f32 / original_height as f32,
                                        ),
                                        crate::types::DragMode::ResizingTopRight => (
                                            (original_width + dx) as f32 / original_width as f32,
                                            (original_height - dy) as f32 / original_height as f32,
                                        ),
                                        crate::types::DragMode::ResizingBottomRight => (
                                            (original_width + dx) as f32 / original_width as f32,
                                            (original_height + dy) as f32 / original_height as f32,
                                        ),
                                        crate::types::DragMode::ResizingBottomLeft => (
                                            (original_width - dx) as f32 / original_width as f32,
                                            (original_height + dy) as f32 / original_height as f32,
                                        ),
                                        _ => (1.0, 1.0),
                                    };

                                    // 等比例缩放：取两个方向的平均值作为统一缩放比例
                                    // 最小缩放到原始大小的 0.6
                                    let scale = ((scale_x + scale_y) / 2.0).max(0.7);

                                    // 计算新字体大小
                                    let new_font_size =
                                        (self.interaction_start_font_size * scale).max(8.0);
                                    el.set_font_size(new_font_size);

                                    // 计算等比例缩放后的新尺寸
                                    let new_width = (original_width as f32 * scale) as i32;
                                    let new_height = (original_height as f32 * scale) as i32;

                                    // 根据拖拽的角确定新矩形的位置
                                    let proportional_rect = match resize_mode {
                                        crate::types::DragMode::ResizingTopLeft => {
                                            // 右下角固定
                                            windows::Win32::Foundation::RECT {
                                                left: self.interaction_start_rect.right - new_width,
                                                top: self.interaction_start_rect.bottom
                                                    - new_height,
                                                right: self.interaction_start_rect.right,
                                                bottom: self.interaction_start_rect.bottom,
                                            }
                                        }
                                        crate::types::DragMode::ResizingTopRight => {
                                            // 左下角固定
                                            windows::Win32::Foundation::RECT {
                                                left: self.interaction_start_rect.left,
                                                top: self.interaction_start_rect.bottom
                                                    - new_height,
                                                right: self.interaction_start_rect.left + new_width,
                                                bottom: self.interaction_start_rect.bottom,
                                            }
                                        }
                                        crate::types::DragMode::ResizingBottomRight => {
                                            // 左上角固定
                                            windows::Win32::Foundation::RECT {
                                                left: self.interaction_start_rect.left,
                                                top: self.interaction_start_rect.top,
                                                right: self.interaction_start_rect.left + new_width,
                                                bottom: self.interaction_start_rect.top
                                                    + new_height,
                                            }
                                        }
                                        crate::types::DragMode::ResizingBottomLeft => {
                                            // 右上角固定
                                            windows::Win32::Foundation::RECT {
                                                left: self.interaction_start_rect.right - new_width,
                                                top: self.interaction_start_rect.top,
                                                right: self.interaction_start_rect.right,
                                                bottom: self.interaction_start_rect.top
                                                    + new_height,
                                            }
                                        }
                                        _ => new_rect,
                                    };

                                    el.resize(proportional_rect);
                                }
                                // 边缘中点拖拽不做任何处理，保持文本框不变
                            }
                            _ => {
                                // 其他元素（Rectangle, Circle, Pen等）：按矩形调整
                                el.resize(new_rect);
                            }
                        }
                    }
            }
            _ => {}
        }
    }

    /// 检测指定元素矩形上的手柄命中（参考旧代码逻辑）
    pub fn get_element_handle_at_position(
        &self,
        x: i32,
        y: i32,
        rect: &windows::Win32::Foundation::RECT,
        tool: DrawingTool,
        element_index: usize,
    ) -> crate::types::DragMode {
        let _detection_radius = crate::constants::HANDLE_DETECTION_RADIUS as i32;

        // 获取元素的点集合（用于箭头等特殊元素）
        let element_points = self
            .elements
            .get_elements()
            .get(element_index)
            .map(|element| element.points.as_slice());

        // 使用统一的绘图元素手柄检测函数
        // 根据工具类型选择合适的配置
        let config = match tool {
            crate::types::DrawingTool::Arrow => {
                // 箭头元素的特殊处理
                let detection_radius = crate::constants::HANDLE_DETECTION_RADIUS as i32;
                if let Some(points) = element_points
                    && points.len() >= 2 {
                        let start = points[0];
                        let end = points[1];
                        let dx = x - start.x;
                        let dy = y - start.y;
                        if dx * dx + dy * dy <= detection_radius * detection_radius {
                            return crate::types::DragMode::ResizingTopLeft;
                        }
                        let dx2 = x - end.x;
                        let dy2 = y - end.y;
                        if dx2 * dx2 + dy2 * dy2 <= detection_radius * detection_radius {
                            return crate::types::DragMode::ResizingBottomRight;
                        }
                    }
                return crate::types::DragMode::None;
            }
            crate::types::DrawingTool::Text => crate::utils::HandleConfig::Corners,
            _ => crate::utils::HandleConfig::Full,
        };

        // 委托给统一的检测函数
        crate::utils::detect_handle_at_position_unified(x, y, rect, config, false)
    }

    pub fn handle_mouse_down(
        &mut self,
        x: i32,
        y: i32,
        selection_rect: Option<windows::Win32::Foundation::RECT>,
    ) -> (Vec<Command>, bool) {
        // 重置标志，下次点击可以创建新文本（与原代码保持一致）
        // 注意：这必须在函数开始时重置，确保每次新的点击事件都会重置状态
        self.just_saved_text = false;

        // 约束：除UI外，绘图交互仅在选择框内生效（保持与原始逻辑一致）
        let inside_selection = match selection_rect {
            Some(r) => x >= r.left && x <= r.right && y >= r.top && y <= r.bottom,
            None => true,
        };

        // 文本编辑状态下的特殊处理（从原始代码迁移）
        if self.text_editing
            && let Some(editing_index) = self.editing_element_index {
                // 检查是否点击了正在编辑的文本元素
                if let Some(element) = self.elements.get_elements().get(editing_index)
                    && element.contains_point(x, y) {
                        // 点击正在编辑的文本元素，检查是否点击了手柄
                        let handle_mode = self.get_element_handle_at_position(
                            x,
                            y,
                            &element.rect,
                            element.tool,
                            editing_index,
                        );
                        if handle_mode != crate::types::DragMode::None {
                            // 点击了手柄，开始拖拽
                            self.interaction_mode =
                                ElementInteractionMode::from_drag_mode(handle_mode);
                            self.mouse_pressed = true;
                            self.interaction_start_pos = windows::Win32::Foundation::POINT { x, y };
                            self.interaction_start_rect = element.rect;
                            self.interaction_start_font_size = element.font_size;
                            return (vec![Command::RequestRedraw], true);
                        }
                        // 点击文本内容区域，继续编辑；返回重绘以消费事件，避免上层退出
                        return (vec![Command::RequestRedraw], true);
                    }
                // 点击了其他地方，停止编辑并立即返回（修复空文本框删除问题）
                // 避免后续逻辑干扰 stop_text_editing 的清理操作
                let stop_commands = self.stop_text_editing();
                return (stop_commands, true);
            }

        // 文本工具特殊处理（从原始代码迁移）
        if inside_selection
            && self.current_tool == DrawingTool::Text
            && !self.text_editing
            && !self.just_saved_text
        {
            // 检查是否点击了任何现有元素（与原始代码保持一致）
            if let Some(idx) = self.elements.get_element_at_position(x, y) {
                // 如果点击的是文本元素，选择它
                if let Some(element) = self.elements.get_elements().get(idx)
                    && element.tool == DrawingTool::Text {
                        // 先获取元素信息，避免借用冲突
                        let element_rect = element.rect;
                        let element_font_size = element.font_size;

                        // 单击只选择文本元素，不进入编辑模式（双击才进入编辑模式）
                        self.handle_message(DrawingMessage::SelectElement(Some(idx)));

                        // 立即设置拖动状态，就像原代码那样（修复文本无法拖动的问题）
                        self.interaction_mode = ElementInteractionMode::MovingElement;
                        self.mouse_pressed = true;
                        self.interaction_start_pos = windows::Win32::Foundation::POINT { x, y };
                        self.interaction_start_rect = element_rect;
                        self.interaction_start_font_size = element_font_size;

                        return (vec![Command::UpdateToolbar, Command::RequestRedraw], true);
                    }
                // 点击了其他类型元素：不在此处处理，继续后续通用元素命中逻辑（允许选择并拖动该元素）
            } else {
                // 未命中任何元素：在选框内空白处创建新文本并直接进入编辑
                return self.create_and_edit_text_element(x, y);
            }
        }

        // 选择框外：不消费，让Screenshot处理（例如拖拽选择框）
        if !inside_selection {
            // 但如果当前没有选择框（None），仍应允许绘图交互
            if selection_rect.is_some() {
                return (vec![], false);
            }
        }

        // 先尝试与现有元素交互（无论当前工具是什么）
        // 1) 已选元素的手柄优先 - 但必须在选择框内
        if inside_selection
            && let Some(sel_idx) = self.selected_element
                && let Some(element) = self.elements.get_elements().get(sel_idx)
                    && element.tool != DrawingTool::Pen {
                        let handle_mode = self.get_element_handle_at_position(
                            x,
                            y,
                            &element.rect,
                            element.tool,
                            sel_idx,
                        );
                        if handle_mode != crate::types::DragMode::None {
                            self.interaction_mode =
                                ElementInteractionMode::from_drag_mode(handle_mode);
                            self.mouse_pressed = true;
                            self.interaction_start_pos = windows::Win32::Foundation::POINT { x, y };
                            self.interaction_start_rect = element.rect;
                            self.interaction_start_font_size = element.font_size;
                            return (vec![Command::RequestRedraw], true);
                        }
                        // 2) 已选元素内部（移动）- 也必须在选择框内且元素可见
                        if element.contains_point(x, y) {
                            // 检查元素是否在选择框内可见（与原代码逻辑一致）
                            let element_visible = if let Some(sel_rect) = selection_rect {
                                self.elements
                                    .is_element_visible_in_selection(element, &sel_rect)
                            } else {
                                true
                            };

                            if element_visible {
                                self.interaction_mode = ElementInteractionMode::MovingElement;
                                self.mouse_pressed = true;
                                self.interaction_start_pos =
                                    windows::Win32::Foundation::POINT { x, y };
                                self.interaction_start_rect = element.rect;
                                return (vec![Command::RequestRedraw], true);
                            }
                        }
                    }

        // 3) 检查是否点击其他元素 - 但必须在选择框内
        if inside_selection
            && let Some(idx) =
                self.elements
                    .get_element_at_position_with_selection(x, y, selection_rect)
            {
                // 先获取元素信息，避免借用冲突
                let (element_tool, element_rect, element_font_size) = {
                    if let Some(element) = self.elements.get_elements().get(idx) {
                        if element.tool == DrawingTool::Pen {
                            return (vec![], false); // 返回空命令，不消费此事件
                        }

                        (element.tool, element.rect, element.font_size)
                    } else {
                        return (vec![], false);
                    }
                };

                // 如果是画笔元素，不允许选择（与旧代码保持一致）
                if element_tool == DrawingTool::Pen {
                    // 笔画不能被选择，直接返回空命令
                    return (vec![], false);
                }

                // 选择该元素（仅非笔画元素）
                self.handle_message(DrawingMessage::SelectElement(Some(idx)));

                // 设置交互起始状态
                self.interaction_start_rect = element_rect;
                self.interaction_start_pos = windows::Win32::Foundation::POINT { x, y };
                self.interaction_start_font_size = element_font_size;

                // 检查是否点击了手柄（与原代码逻辑一致）
                let handle_mode =
                    self.get_element_handle_at_position(x, y, &element_rect, element_tool, idx);

                if handle_mode != crate::types::DragMode::None {
                    // 点击了手柄，开始手柄拖拽
                    self.interaction_mode = ElementInteractionMode::from_drag_mode(handle_mode);
                    self.mouse_pressed = true;
                    return (vec![Command::UpdateToolbar, Command::RequestRedraw], true);
                } else {
                    // 没有点击手柄，立即开始移动元素（与原代码逻辑一致）
                    self.interaction_mode = ElementInteractionMode::MovingElement;
                    self.mouse_pressed = true;
                    return (vec![Command::UpdateToolbar, Command::RequestRedraw], true);
                }
            }

        // 4) 若没有元素命中，且选择了绘图工具，则尝试开始绘制
        if self.current_tool != DrawingTool::None {
            if inside_selection {
                self.interaction_start_pos = windows::Win32::Foundation::POINT { x, y };
                self.start_drawing_shape(x, y);
                self.mouse_pressed = true;
                return (vec![Command::RequestRedraw], true);
            }
            // 不在选择框内，不消费事件
            return (vec![], false);
        }

        // 5) 工具为None且未命中元素：清除选中，但不消费事件（让 ScreenshotManager 有机会处理）
        if self.selected_element.is_some() {
            // 只有当确实有选中元素需要清除时才处理
            self.selected_element = None;
            self.elements.set_selected(None);
            (vec![Command::UpdateToolbar, Command::RequestRedraw], true)
        } else {
            // 没有选中元素，不消费事件
            (vec![], false)
        }
    }

    fn start_drawing_shape(&mut self, x: i32, y: i32) {
        // 在开始新的绘制前清除元素选择（保持原始行为）
        self.selected_element = None;
        self.elements.set_selected(None);

        // 保存历史状态（在操作开始前保存，以便精确撤销）
        self.history
            .save_state(&self.elements, self.selected_element);

        // 设置交互模式为绘制图形
        self.interaction_mode = ElementInteractionMode::Drawing;

        // 创建新元素
        let mut new_element = DrawingElement::new(self.current_tool);
        if self.current_tool == DrawingTool::Text {
            // 文本元素使用字体颜色与字体设置（从共享配置获取）
            if let Ok(settings) = self.settings.read() {
                new_element.color = windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F {
                    r: settings.font_color.0 as f32 / 255.0,
                    g: settings.font_color.1 as f32 / 255.0,
                    b: settings.font_color.2 as f32 / 255.0,
                    a: 1.0,
                };
                new_element.font_size = settings.font_size;
                new_element.font_name = settings.font_name.clone();
                new_element.font_weight = settings.font_weight;
                new_element.font_italic = settings.font_italic;
                new_element.font_underline = settings.font_underline;
                new_element.font_strikeout = settings.font_strikeout;
            }
        } else {
            // 其他元素使用绘图颜色与线宽（从 ToolManager 获取）
            new_element.color = self.tools.get_brush_color();
            new_element.thickness = self.tools.get_line_thickness();
        }

        match self.current_tool {
            DrawingTool::Pen => {
                new_element
                    .points
                    .push(windows::Win32::Foundation::POINT { x, y });
            }
            DrawingTool::Rectangle | DrawingTool::Circle | DrawingTool::Arrow => {
                new_element
                    .points
                    .push(windows::Win32::Foundation::POINT { x, y });
            }
            DrawingTool::Text => {
                new_element
                    .points
                    .push(windows::Win32::Foundation::POINT { x, y });
            }
            _ => {}
        }

        self.current_element = Some(new_element);
    }

    pub fn handle_mouse_up(&mut self, _x: i32, _y: i32) -> (Vec<Command>, bool) {
        if self.mouse_pressed {
            self.end_drag();
            self.mouse_pressed = false;
            self.interaction_mode = ElementInteractionMode::None;
            // 添加 UpdateToolbar 以便画完元素后立即更新撤回按钮状态
            (vec![Command::UpdateToolbar, Command::RequestRedraw], true)
        } else {
            (vec![], false)
        }
    }

    fn end_drag(&mut self) {
        if self.interaction_mode == ElementInteractionMode::Drawing
            && let Some(mut element) = self.current_element.take() {
                // 根据不同工具类型判断是否保存
                let should_save = match element.tool {
                    DrawingTool::Pen => {
                        // 手绘工具：至少要有2个点
                        element.points.len() > 1
                    }
                    DrawingTool::Rectangle | DrawingTool::Circle | DrawingTool::Arrow => {
                        // 形状工具：检查尺寸
                        if element.points.len() >= 2 {
                            let dx = (element.points[1].x - element.points[0].x).abs();
                            let dy = (element.points[1].y - element.points[0].y).abs();
                            dx > 5 || dy > 5 // 至少有一个方向大于5像素
                        } else {
                            false
                        }
                    }
                    DrawingTool::Text => {
                        // 文本工具：必须有位置点且文本内容不为空
                        !element.points.is_empty() && !element.text.trim().is_empty()
                    }
                    _ => false,
                };

                if should_save {
                    // 关键：保存前更新边界矩形
                    element.update_bounding_rect();
                    // 通过 ElementManager 添加元素
                    self.elements.add_element(element);
                }
            }
    }

    pub fn handle_key_input(&mut self, key: u32) -> Vec<Command> {
        if self.text_editing {
            match key {
                0x1B => {
                    // VK_ESCAPE - 退出文字编辑模式
                    return self.stop_text_editing();
                }
                0x0D => {
                    // VK_RETURN - 插入换行符
                    return self.handle_text_input('\n');
                }
                0x08 => {
                    // VK_BACK - 退格键删除字符
                    return self.handle_backspace();
                }
                0x25 => {
                    // VK_LEFT - 光标向左移动
                    return self.move_cursor_left();
                }
                0x27 => {
                    // VK_RIGHT - 光标向右移动
                    return self.move_cursor_right();
                }
                0x24 => {
                    // VK_HOME - 光标移动到行首
                    return self.move_cursor_to_line_start();
                }
                0x23 => {
                    // VK_END - 光标移动到行尾
                    return self.move_cursor_to_line_end();
                }
                0x26 => {
                    // VK_UP - 光标向上移动一行
                    return self.move_cursor_up();
                }
                0x28 => {
                    // VK_DOWN - 光标向下移动一行
                    return self.move_cursor_down();
                }
                _ => {}
            }
        }

        // 常规键盘快捷键
        match key {
            // Ctrl+Z - 撤销
            26 => self.handle_message(DrawingMessage::Undo),
            // Ctrl+Y - 重做
            25 => self.handle_message(DrawingMessage::Redo),
            // Delete - 删除选中元素
            46 => {
                if let Some(index) = self.selected_element {
                    self.handle_message(DrawingMessage::DeleteElement(index))
                } else {
                    vec![]
                }
            }
            _ => vec![],
        }
    }

    /// 获取当前拖拽模式（用于光标显示）：
    /// - 元素移动 => Moving
    /// - 元素调整大小 => 返回对应的 Resizing* 模式
    /// - 非拖拽 => None
    pub fn get_current_drag_mode(&self) -> Option<crate::types::DragMode> {
        match &self.interaction_mode {
            ElementInteractionMode::MovingElement => Some(crate::types::DragMode::Moving),
            ElementInteractionMode::ResizingElement(mode) => Some(*mode),
            _ => None,
        }
    }

    /// 是否正在进行任何绘图交互（绘制/移动/调整）
    pub fn is_dragging(&self) -> bool {
        self.mouse_pressed && self.interaction_mode != ElementInteractionMode::None
    }

    /// 获取当前工具
    pub fn get_current_tool(&self) -> DrawingTool {
        self.current_tool
    }

    /// 是否正在文本编辑
    pub fn is_text_editing(&self) -> bool {
        self.text_editing
    }

    /// 当前编辑元素索引
    pub fn get_editing_element_index(&self) -> Option<usize> {
        self.editing_element_index
    }

    /// 获取已选中元素索引
    pub fn get_selected_element_index(&self) -> Option<usize> {
        self.selected_element
    }

    /// 只读获取元素引用
    pub fn get_element_ref(&self, index: usize) -> Option<&DrawingElement> {
        self.elements.get_elements().get(index)
    }

    /// 获取选中元素的工具类型（用于同步工具栏状态）
    pub fn get_selected_element_tool(&self) -> Option<DrawingTool> {
        if let Some(index) = self.selected_element
            && let Some(element) = self.elements.get_elements().get(index) {
                return Some(element.tool);
            }
        None
    }

    pub fn reload_drawing_properties(&mut self) {
        // ToolManager 直接从设置读取配置
    }

    /// 处理双击事件（优先用于文本编辑）
    pub fn handle_double_click(
        &mut self,
        x: i32,
        y: i32,
        _selection_rect: Option<&windows::Win32::Foundation::RECT>,
    ) -> Vec<Command> {
        // 如果双击的是文本元素，则进入编辑模式
        if let Some(index) = self.get_text_element_at_position(x, y) {
            return self.start_text_editing(index);
        }
        vec![]
    }
}

/// 绘图错误类型
#[derive(Debug)]
pub enum DrawingError {
    /// 渲染错误
    RenderError(String),
    /// 初始化错误
    InitError(String),
}

impl std::fmt::Display for DrawingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DrawingError::RenderError(msg) => write!(f, "Drawing render error: {msg}"),
            DrawingError::InitError(msg) => write!(f, "Drawing init error: {msg}"),
        }
    }
}

impl std::error::Error for DrawingError {}
