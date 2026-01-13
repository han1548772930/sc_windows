pub mod constants;
pub mod cursor;
pub mod preview;
pub mod settings;
pub mod svg;
pub mod svg_icons;
pub mod toolbar;

use std::collections::HashSet;

use sc_app::selection::RectI32;
use sc_drawing::DrawingTool;
use sc_host_protocol::{Command, UIMessage};
use sc_platform_windows::windows::Direct2DRenderer;
use sc_ui::selection_overlay::build_selection_overlay_render_list;

// Re-export types for convenience
pub use cursor::CursorManager;
pub use preview::PreviewWindow;
pub use sc_ui::toolbar::ToolbarButton;

pub use settings::SettingsWindow;

use svg_icons::SvgIconManager;
use toolbar::ToolbarManager;

/// UI 管理器（主窗口内的工具栏/图标/遮罩等 UI 组件）
pub struct UIManager {
    /// 工具栏管理器
    toolbar: ToolbarManager,
    /// SVG 图标管理器
    svg_icons: SvgIconManager,
}

impl UIManager {
    /// 创建新的 UI 管理器
    pub fn new() -> Result<Self, UIError> {
        let mut svg_icons = SvgIconManager::new();
        if let Err(e) = svg_icons.load_icons() {
            eprintln!("Failed to load SVG icons: {e}");
        }

        Ok(Self {
            toolbar: ToolbarManager::new()?,
            svg_icons,
        })
    }

    /// 重置状态
    pub fn reset_state(&mut self) {
        self.toolbar.hide();
        self.toolbar.clicked_button = ToolbarButton::None;
    }

    /// 处理 UI 消息
    pub fn handle_message(
        &mut self,
        message: UIMessage,
        screen_width: i32,
        screen_height: i32,
    ) -> Vec<Command> {
        match message {
            UIMessage::ShowToolbar(rect) => {
                self.toolbar
                    .update_position(rect, screen_width, screen_height);
                self.toolbar.show();
                vec![Command::UpdateToolbar, Command::RequestRedraw]
            }
            UIMessage::HideToolbar => {
                self.toolbar.hide();
                vec![Command::RequestRedraw]
            }
            UIMessage::UpdateToolbarPosition(rect) => {
                if self.toolbar.is_visible() {
                    self.toolbar
                        .update_position(rect, screen_width, screen_height);
                    vec![Command::RequestRedraw]
                } else {
                    vec![]
                }
            }
        }
    }

    /// 渲染 UI（工具栏）
    pub fn render(&self, d2d_renderer: &mut Direct2DRenderer) -> Result<(), UIError> {
        self.toolbar.render(d2d_renderer, &self.svg_icons)?;
        Ok(())
    }

    /// 渲染选区相关 UI（遮罩、边框、手柄）
    pub fn render_selection_ui(
        &self,
        d2d_renderer: &mut Direct2DRenderer,
        screen_size: (i32, i32),
        selection_rect: Option<RectI32>,
        show_handles: bool,
        hide_ui_for_capture: bool,
        has_auto_highlight: bool,
    ) -> Result<(), UIError> {
        if let Some(mut render_list) = build_selection_overlay_render_list(
            screen_size,
            selection_rect,
            show_handles,
            hide_ui_for_capture,
            has_auto_highlight,
        ) {
            render_list
                .execute(d2d_renderer)
                .map_err(|e| UIError::RenderError(format!("render list execute failed: {e}")))?;
        }
        Ok(())
    }

    /// 处理鼠标移动（工具栏）
    pub fn handle_mouse_move(&mut self, x: i32, y: i32) -> (Vec<Command>, bool) {
        let commands = self.toolbar.handle_mouse_move(x, y);
        let consumed = !commands.is_empty();
        (commands, consumed)
    }

    /// 处理鼠标按下（工具栏）
    pub fn handle_mouse_down(&mut self, x: i32, y: i32) -> (Vec<Command>, bool) {
        let commands = self.toolbar.handle_mouse_down(x, y);
        let consumed = !commands.is_empty();
        (commands, consumed)
    }

    /// 处理鼠标释放（工具栏）
    pub fn handle_mouse_up(&mut self, x: i32, y: i32) -> (Vec<Command>, bool) {
        let commands = self.toolbar.handle_mouse_up(x, y);
        let consumed = !commands.is_empty();
        (commands, consumed)
    }

    /// 处理双击事件
    pub fn handle_double_click(&mut self, x: i32, y: i32) -> Vec<Command> {
        self.toolbar.handle_double_click(x, y)
    }

    /// 处理键盘输入（当前无 UI 级快捷键）
    pub fn handle_key_input(&mut self, _key: u32) -> Vec<Command> {
        vec![]
    }

    /// 工具栏是否可见
    pub fn is_toolbar_visible(&self) -> bool {
        self.toolbar.is_visible()
    }

    /// 当前悬停按钮
    pub fn get_hovered_button(&self) -> ToolbarButton {
        self.toolbar.hovered_button
    }

    /// 查询某个按钮是否被禁用
    pub fn is_button_disabled(&self, button: ToolbarButton) -> bool {
        self.toolbar.disabled_buttons.contains(&button)
    }

    /// 更新工具栏选中的绘图工具
    pub fn update_toolbar_selected_tool(&mut self, tool: DrawingTool) {
        let button = match tool {
            DrawingTool::Rectangle => ToolbarButton::Rectangle,
            DrawingTool::Circle => ToolbarButton::Circle,
            DrawingTool::Arrow => ToolbarButton::Arrow,
            DrawingTool::Pen => ToolbarButton::Pen,
            DrawingTool::Text => ToolbarButton::Text,
            DrawingTool::None => ToolbarButton::None,
        };

        self.toolbar.clicked_button = button;
    }

    /// 设置工具栏禁用按钮状态
    pub fn set_toolbar_disabled(&mut self, buttons: HashSet<ToolbarButton>) {
        self.toolbar.set_disabled(buttons);
    }
}

/// UI 错误类型
#[derive(Debug, thiserror::Error)]
pub enum UIError {
    #[error("UI render error: {0}")]
    RenderError(String),
    #[error("UI init error: {0}")]
    InitError(String),
}
