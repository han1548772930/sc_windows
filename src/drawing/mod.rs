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

use std::sync::{Arc, RwLock};

use windows::Win32::Foundation::{POINT, RECT};

use crate::message::{Command, DrawingMessage};
use crate::rendering::{LayerCache, LayerType};
use crate::settings::Settings;

pub mod cache;
pub mod elements;
pub mod history;
pub mod interaction;
pub mod rendering;
pub mod text_editing;
pub mod tools;
pub mod types;

// Re-export types for convenience
pub use types::{DragMode, DrawingElement, DrawingTool, ElementInteractionMode};

use cache::GeometryCache;
use elements::ElementManager;
use history::HistoryManager;
use tools::ToolManager;

pub struct DrawingManager {
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
    pub(super) interaction_start_pos: POINT,
    pub(super) interaction_start_rect: RECT,
    pub(super) interaction_start_font_size: f32,
    /// 交互开始时元素的点集合（用于命令模式记录）
    pub(super) interaction_start_points: Vec<POINT>,
    pub(super) text_editing: bool,
    pub(super) editing_element_index: Option<usize>,
    pub(super) text_cursor_pos: usize,
    pub(super) text_cursor_visible: bool,
    pub(super) cursor_timer_id: usize,
    pub(super) just_saved_text: bool,
    /// 图层缓存管理器
    pub(super) layer_cache: LayerCache,
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
            interaction_start_pos: POINT { x: 0, y: 0 },
            interaction_start_rect: RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            interaction_start_font_size: 0.0,
            interaction_start_points: Vec::new(),

            text_editing: false,
            editing_element_index: None,
            text_cursor_pos: 0,
            text_cursor_visible: false,
            cursor_timer_id: 1001,
            just_saved_text: false,
            // 默认初始化，屏幕尺寸将在渲染器初始化时更新
            layer_cache: LayerCache::new(1920, 1080),
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
        self.layer_cache.invalidate_all();
    }

    /// 处理绘图消息
    pub fn handle_message(&mut self, message: DrawingMessage) -> Vec<Command> {
        match message {
            DrawingMessage::SelectTool(tool) => {
                let mut commands = Vec::new();

                if self.text_editing {
                    commands.extend(self.stop_text_editing());
                }
                if self.selected_element.is_some() {
                    self.layer_cache
                        .invalidate(LayerType::StaticElements);
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
                        .push(POINT { x, y });
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
                                .push(POINT { x, y });
                            // 正在绘制的元素还没有索引，缓存失效在 FinishDrawing 时处理
                        }
                        DrawingTool::Rectangle | DrawingTool::Circle | DrawingTool::Arrow => {
                            if element.points.len() >= 2 {
                                element.points[1] = POINT { x, y };
                            } else {
                                element
                                    .points
                                    .push(POINT { x, y });
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
                    // 新元素加入静态层，需要重建缓存
                    self.layer_cache.invalidate(LayerType::StaticElements);
                    vec![Command::UpdateToolbar, Command::RequestRedraw]
                } else {
                    vec![]
                }
            }
            DrawingMessage::Undo => {
                if let Some((action, sel)) = self.history.undo_action() {
                    let changed = action.affected_indices();

                    // 应用撤销操作
                    self.elements.apply_undo(&action);
                    self.selected_element = sel;
                    self.elements.set_selected(self.selected_element);
                    // 撤销操作可能改变元素集合，需要重建缓存
                    self.layer_cache.invalidate(LayerType::StaticElements);

                    // 使用增量式缓存失效
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
                if let Some((action, sel)) = self.history.redo_action() {
                    let changed = action.affected_indices();

                    // 应用重做操作
                    self.elements.apply_redo(&action);
                    self.selected_element = sel;
                    self.elements.set_selected(self.selected_element);
                    // 重做操作可能改变元素集合，需要重建缓存
                    self.layer_cache.invalidate(LayerType::StaticElements);

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
                // 命令模式：记录删除操作
                // 先获取元素的 id 用于清理缓存（在删除前获取）
                let element_id = self.elements.get_elements().get(index).map(|e| e.id);
                
                if let Some(element) = self.elements.get_elements().get(index).cloned() {
                    let action = history::DrawingAction::RemoveElement { element, index };
                    self.history.record_action(
                        action,
                        self.selected_element,
                        None, // 删除后无选中
                    );
                }

                if self.elements.remove_element(index) {
                    self.selected_element = None;
                    // 删除元素后需要重建静态层缓存
                    self.layer_cache.invalidate(LayerType::StaticElements);
                    // 使用 element.id 作为缓存 key（而非 index）
                    if let Some(id) = element_id {
                        self.geometry_cache.remove(id as usize);
                    }
                    vec![Command::UpdateToolbar, Command::RequestRedraw]
                } else {
                    vec![]
                }
            }
            DrawingMessage::SelectElement(index) => {
                let old_selection = self.selected_element;
                self.selected_element = index;
                self.elements.set_selected(index);

                if let Some(idx) = index
                    && let Some(element) = self.elements.get_elements().get(idx)
                {
                    self.current_tool = element.tool;
                    self.tools.set_current_tool(element.tool);
                }

                // 当选中状态改变时，元素从静态层移到动态层（或反之），需要重建缓存
                if old_selection != index {
                    self.layer_cache.invalidate(LayerType::StaticElements);
                }

                vec![Command::UpdateToolbar, Command::RequestRedraw]
            }
            DrawingMessage::AddElement(element) => {
                // 命令模式：记录添加操作
                let index = self.elements.count();
                let action = history::DrawingAction::AddElement {
                    element: (*element).clone(),
                    index,
                };
                self.history
                    .record_action(action, self.selected_element, None);
                self.elements.add_element(*element);
                vec![Command::RequestRedraw]
            }
            DrawingMessage::CheckElementClick(x, y) => {
                if let Some(element_index) = self.elements.get_element_at_position(x, y) {
                    let old_selection = self.selected_element;
                    self.selected_element = Some(element_index);
                    self.elements.set_selected(self.selected_element);

                    if old_selection != self.selected_element {
                        self.layer_cache.invalidate(LayerType::StaticElements);
                    }
                    vec![Command::UpdateToolbar, Command::RequestRedraw]
                } else {
                    let old_selection = self.selected_element;
                    self.selected_element = None;
                    self.elements.set_selected(None);

                    if old_selection.is_some() {
                        self.layer_cache.invalidate(LayerType::StaticElements);
                    }
                    vec![Command::UpdateToolbar, Command::RequestRedraw]
                }
            }
        }
    }

    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    // ============================================================
    // 交互方法已移至 interaction.rs
    // - handle_mouse_move
    // - handle_mouse_down
    // - handle_mouse_up
    // - handle_key_input
    // - handle_double_click
    // - get_current_drag_mode
    // - is_dragging
    // - get_element_handle_at_position
    // ============================================================

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
            && let Some(element) = self.elements.get_elements().get(index)
        {
            return Some(element.tool);
        }
        None
    }

    pub fn reload_drawing_properties(&mut self) {
        // ToolManager 直接从设置读取配置
    }

    /// 设置屏幕尺寸（窗口缩放时调用）
    ///
    /// 会自动使所有图层缓存失效
    pub fn set_screen_size(&mut self, width: u32, height: u32) {
        self.layer_cache.set_screen_size(width, height);
    }

    /// 检查静态元素层是否需要重建
    pub fn needs_layer_rebuild(&self) -> bool {
        self.layer_cache.needs_rebuild(LayerType::StaticElements)
    }

    /// 标记静态元素层已重建
    pub fn mark_layer_rebuilt(&mut self) {
        // 使用 0 作为占位符 bitmap_id，因为我们使用 layer_target 而不是 ID
        self.layer_cache.mark_rebuilt(LayerType::StaticElements, 0);
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
