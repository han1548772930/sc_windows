pub mod constants;
pub mod interaction;
pub mod rendering;
pub mod text_editing;
pub mod tools;

use sc_drawing::{Point, Rect};

use sc_host_protocol::{Command, DrawingMessage};

// Re-export core types for convenience.
pub use sc_drawing::history::{
    ActionHistory as HistoryManager, DrawingAction, HistoryEntry, HistoryState,
};
pub use sc_drawing::{
    DragMode, DrawingElement, DrawingTool, ElementInteractionMode, ElementManager,
};

use sc_drawing::history;

use tools::ToolManager;

/// Host-provided drawing configuration.
#[derive(Debug, Clone)]
pub struct DrawingConfig {
    pub line_thickness: f32,
    pub drawing_color: (u8, u8, u8),

    // Text config
    pub font_size: f32,
    pub font_name: String,
    pub font_weight: i32,
    pub font_italic: bool,
    pub font_underline: bool,
    pub font_strikeout: bool,
    pub font_color: (u8, u8, u8),
}

impl Default for DrawingConfig {
    fn default() -> Self {
        Self {
            line_thickness: 3.0,
            drawing_color: (255, 0, 0),

            font_size: 20.0,
            font_name: "Microsoft YaHei".to_string(),
            font_weight: 400,
            font_italic: false,
            font_underline: false,
            font_strikeout: false,
            font_color: (0, 0, 0),
        }
    }
}

pub struct DrawingManager {
    config: DrawingConfig,
    tools: ToolManager,
    elements: ElementManager,
    history: HistoryManager,
    /// Windows renderer (static layer cache / incremental pen strokes, etc.).
    win_renderer: sc_drawing::windows::DrawingRenderer,
    current_tool: DrawingTool,
    current_element: Option<DrawingElement>,
    selected_element: Option<usize>,
    interaction_mode: ElementInteractionMode,
    mouse_pressed: bool,
    interaction_start_pos: Point,
    interaction_start_rect: Rect,
    interaction_start_font_size: f32,
    /// Start points snapshot for command history.
    interaction_start_points: Vec<Point>,
    text_editing: bool,
    editing_element_index: Option<usize>,
    text_cursor_pos: usize,
    text_cursor_visible: bool,
    cursor_timer_id: usize,
    just_saved_text: bool,
    /// Whether the static layer needs rebuild (actual cache lives in `win_renderer`).
    static_layer_dirty: bool,
}

impl DrawingManager {
    pub fn update_config(&mut self, config: DrawingConfig) {
        self.config = config.clone();
        self.tools.update_config(config);
    }

    /// Set current drawing tool (sync ToolManager and local state).
    pub fn set_current_tool(&mut self, tool: DrawingTool) {
        self.current_tool = tool;
        self.tools.set_current_tool(tool);
    }

    /// Create a new drawing manager.
    pub fn new(config: DrawingConfig) -> Result<Self, DrawingError> {
        Ok(Self {
            tools: ToolManager::new(config.clone()),
            config,
            elements: ElementManager::new(),
            history: HistoryManager::new(),
            win_renderer: sc_drawing::windows::DrawingRenderer::new(),
            current_tool: DrawingTool::None,
            current_element: None,
            selected_element: None,

            interaction_mode: ElementInteractionMode::None,
            mouse_pressed: false,
            interaction_start_pos: Point::new(0, 0),
            interaction_start_rect: Rect::new(0, 0, 0, 0),
            interaction_start_font_size: 0.0,
            interaction_start_points: Vec::new(),

            text_editing: false,
            editing_element_index: None,
            text_cursor_pos: 0,
            text_cursor_visible: false,
            cursor_timer_id: 1001,
            just_saved_text: false,
            static_layer_dirty: true,
        })
    }

    /// Reset state.
    pub fn reset_state(&mut self) {
        self.current_element = None;
        self.selected_element = None;
        self.current_tool = DrawingTool::None;
        self.history.clear();
        self.elements.clear();
        self.win_renderer.invalidate_all();
        self.interaction_mode = ElementInteractionMode::None;
        self.mouse_pressed = false;
        self.text_editing = false;
        self.editing_element_index = None;
        self.text_cursor_pos = 0;
        self.text_cursor_visible = false;
        self.just_saved_text = false;
        self.static_layer_dirty = true;
    }

    /// Handle drawing messages.
    pub fn handle_message(&mut self, message: DrawingMessage) -> Vec<Command> {
        match message {
            DrawingMessage::SelectTool(tool) => {
                let mut commands = Vec::new();

                if self.text_editing {
                    commands.extend(self.stop_text_editing());
                }
                if self.selected_element.is_some() {
                    self.static_layer_dirty = true;
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
                    element.add_point(x, y);
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
                            element.add_point(x, y);
                            // Pen current stroke caching is handled by `win_renderer`.
                        }
                        DrawingTool::Rectangle | DrawingTool::Circle | DrawingTool::Arrow => {
                            element.set_end_point(x, y);
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
                    self.static_layer_dirty = true;
                    vec![Command::UpdateToolbar, Command::RequestRedraw]
                } else {
                    vec![]
                }
            }
            DrawingMessage::Undo => {
                if let Some((action, sel)) = self.history.undo_action() {
                    self.elements.apply_undo(&action);
                    self.selected_element = sel;
                    self.elements.set_selected(self.selected_element);

                    self.static_layer_dirty = true;
                    self.win_renderer.invalidate_all();

                    vec![Command::UpdateToolbar, Command::RequestRedraw]
                } else {
                    vec![Command::UpdateToolbar]
                }
            }
            DrawingMessage::Redo => {
                if let Some((action, sel)) = self.history.redo_action() {
                    self.elements.apply_redo(&action);
                    self.selected_element = sel;
                    self.elements.set_selected(self.selected_element);

                    self.static_layer_dirty = true;
                    self.win_renderer.invalidate_all();

                    vec![Command::RequestRedraw]
                } else {
                    vec![]
                }
            }
            DrawingMessage::DeleteElement(index) => {
                let element_id = self.elements.get_elements().get(index).map(|e| e.id);

                if let Some(element) = self.elements.get_elements().get(index).cloned() {
                    let action = history::DrawingAction::RemoveElement { element, index };
                    self.history
                        .record_action(action, self.selected_element, None);
                }

                if self.elements.remove_element(index) {
                    self.selected_element = None;
                    self.static_layer_dirty = true;
                    if let Some(id) = element_id {
                        self.win_renderer.remove_element_cache(id);
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

                if old_selection != index {
                    self.static_layer_dirty = true;
                }

                vec![Command::UpdateToolbar, Command::RequestRedraw]
            }
            DrawingMessage::AddElement(element) => {
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
                        self.static_layer_dirty = true;
                    }
                    vec![Command::UpdateToolbar, Command::RequestRedraw]
                } else {
                    let old_selection = self.selected_element;
                    self.selected_element = None;
                    self.elements.set_selected(None);

                    if old_selection.is_some() {
                        self.static_layer_dirty = true;
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
    // Interaction methods are implemented in `interaction.rs`.
    // ============================================================

    /// Get current tool.
    pub fn get_current_tool(&self) -> DrawingTool {
        self.current_tool
    }

    /// Is text editing active.
    pub fn is_text_editing(&self) -> bool {
        self.text_editing
    }

    /// Current editing element index.
    pub fn get_editing_element_index(&self) -> Option<usize> {
        self.editing_element_index
    }

    /// Selected element index.
    pub fn get_selected_element_index(&self) -> Option<usize> {
        self.selected_element
    }

    /// Read-only element reference.
    pub fn get_element_ref(&self, index: usize) -> Option<&DrawingElement> {
        self.elements.get_elements().get(index)
    }

    /// Selected element tool type (for syncing toolbar).
    pub fn get_selected_element_tool(&self) -> Option<DrawingTool> {
        if let Some(index) = self.selected_element
            && let Some(element) = self.elements.get_elements().get(index)
        {
            return Some(element.tool);
        }
        None
    }

    /// Legacy no-op (kept temporarily to reduce churn during migration).
    pub fn reload_drawing_properties(&mut self) {}

    /// Set screen size (called on window resize).
    pub fn set_screen_size(&mut self, _width: u32, _height: u32) {
        self.static_layer_dirty = true;
    }

    pub fn needs_layer_rebuild(&self) -> bool {
        self.static_layer_dirty
    }

    pub fn mark_layer_rebuilt(&mut self) {
        self.static_layer_dirty = false;
    }
}

/// Drawing errors.
#[derive(Debug, thiserror::Error)]
pub enum DrawingError {
    #[error("Drawing render error: {0}")]
    RenderError(String),
    #[error("Drawing init error: {0}")]
    InitError(String),
}
