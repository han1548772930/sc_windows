// UI管理器模块
//
// 负责用户界面相关功能：工具栏、覆盖层、对话框等

use crate::message::{Command, UIMessage};
use crate::platform::{PlatformError, PlatformRenderer};

pub mod cursor;
pub mod dialogs;
pub mod overlay;
pub mod svg_icons;
pub mod toolbar;

use dialogs::DialogManager;
use overlay::OverlayManager;
use svg_icons::SvgIconManager;
use toolbar::ToolbarManager;

/// UI管理器
pub struct UIManager {
    /// 工具栏管理器
    toolbar: ToolbarManager,
    /// 覆盖层管理器
    overlay: OverlayManager,
    /// 对话框管理器
    dialogs: DialogManager,
    /// SVG图标管理器
    svg_icons: SvgIconManager,
}

impl UIManager {
    /// 创建新的UI管理器
    pub fn new() -> Result<Self, UIError> {
        let mut svg_icons = SvgIconManager::new();
        // 加载SVG图标
        if let Err(e) = svg_icons.load_icons() {
            eprintln!("Failed to load SVG icons: {}", e);
        }

        Ok(Self {
            toolbar: ToolbarManager::new()?,
            overlay: OverlayManager::new()?,
            dialogs: DialogManager::new()?,
            svg_icons,
        })
    }

    /// 重置状态（从原始reset_to_initial_state迁移）
    pub fn reset_state(&mut self) {
        // 隐藏工具栏
        self.toolbar.hide();

        // 重置工具栏按钮状态
        self.toolbar.clicked_button = crate::types::ToolbarButton::None;

        // 关闭所有对话框
        self.dialogs.close_current_dialog();
    }

    /// 处理UI消息
    pub fn handle_message(
        &mut self,
        message: UIMessage,
        screen_width: i32,
        screen_height: i32,
    ) -> Vec<Command> {
        match message {
            UIMessage::ShowToolbar(rect) => {
                // 使用正确的屏幕尺寸（从原始代码迁移）
                self.toolbar
                    .update_position(&rect, screen_width, screen_height);
                self.toolbar.show(); // 显示工具栏
                vec![Command::RequestRedraw]
            }
            UIMessage::HideToolbar => {
                self.toolbar.hide();
                vec![Command::RequestRedraw]
            }
            UIMessage::UpdateToolbarPosition(rect) => {
                // 更新工具栏位置但不改变可见性（使用正确的屏幕尺寸）
                self.toolbar
                    .update_position(&rect, screen_width, screen_height);
                vec![Command::RequestRedraw]
            }
            UIMessage::ToolbarButtonClicked(button) => self.toolbar.handle_button_click(button),
            UIMessage::ShowDialog(dialog_type) => {
                self.dialogs.show_dialog(dialog_type);
                vec![Command::RequestRedraw]
            }
            UIMessage::CloseDialog => {
                self.dialogs.close_current_dialog();
                vec![Command::RequestRedraw]
            }
        }
    }

    /// 渲染UI元素
    pub fn render(
        &self,
        renderer: &mut dyn PlatformRenderer<Error = PlatformError>,
    ) -> Result<(), UIError> {
        // 渲染覆盖层
        self.overlay.render(renderer)?;

        // 渲染工具栏
        self.toolbar.render(renderer, &self.svg_icons)?;

        // 渲染对话框
        self.dialogs.render(renderer)?;

        Ok(())
    }

    /// 处理鼠标移动
    pub fn handle_mouse_move(&mut self, x: i32, y: i32) -> Vec<Command> {
        let mut commands = Vec::new();

        // 工具栏处理鼠标移动
        commands.extend(self.toolbar.handle_mouse_move(x, y));

        // 对话框处理鼠标移动
        commands.extend(self.dialogs.handle_mouse_move(x, y));

        commands
    }

    /// 处理鼠标按下
    pub fn handle_mouse_down(&mut self, x: i32, y: i32) -> Vec<Command> {
        let mut commands = Vec::new();

        // 工具栏处理鼠标按下
        commands.extend(self.toolbar.handle_mouse_down(x, y));

        // 如果工具栏没有处理，则传递给对话框
        if commands.is_empty() {
            commands.extend(self.dialogs.handle_mouse_down(x, y));
        }

        commands
    }

    /// 处理鼠标释放
    pub fn handle_mouse_up(&mut self, x: i32, y: i32) -> Vec<Command> {
        let mut commands = Vec::new();

        commands.extend(self.toolbar.handle_mouse_up(x, y));
        commands.extend(self.dialogs.handle_mouse_up(x, y));

        commands
    }

    /// 处理双击事件
    pub fn handle_double_click(&mut self, x: i32, y: i32) -> Vec<Command> {
        let mut commands = Vec::new();
        // 工具栏可能需要处理双击（例如快速操作）
        commands.extend(self.toolbar.handle_double_click(x, y));

        commands
    }

    /// 处理键盘输入
    pub fn handle_key_input(&mut self, key: u32) -> Vec<Command> {
        // 对话框优先处理键盘输入
        self.dialogs.handle_key_input(key)
    }

    /// 工具栏是否可见
    pub fn is_toolbar_visible(&self) -> bool {
        self.toolbar.is_visible()
    }

    /// 获取当前悬停的工具栏按钮
    pub fn get_hovered_button(&self) -> crate::types::ToolbarButton {
        self.toolbar.hovered_button
    }

    /// 查询某个按钮是否被禁用
    pub fn is_button_disabled(&self, button: crate::types::ToolbarButton) -> bool {
        self.toolbar.disabled_buttons.contains(&button)
    }

    /// 更新工具栏选中的绘图工具（从原始代码迁移）
    pub fn update_toolbar_selected_tool(&mut self, tool: crate::types::DrawingTool) {
        // 将绘图工具转换为工具栏按钮
        let button = match tool {
            crate::types::DrawingTool::Rectangle => crate::types::ToolbarButton::Rectangle,
            crate::types::DrawingTool::Circle => crate::types::ToolbarButton::Circle,
            crate::types::DrawingTool::Arrow => crate::types::ToolbarButton::Arrow,
            crate::types::DrawingTool::Pen => crate::types::ToolbarButton::Pen,
            crate::types::DrawingTool::Text => crate::types::ToolbarButton::Text,
            crate::types::DrawingTool::None => crate::types::ToolbarButton::None,
        };

        // 更新工具栏的选中按钮状态
        self.toolbar.clicked_button = button;
    }
}

impl UIManager {
    /// 设置工具栏禁用按钮状态
    pub fn set_toolbar_disabled(
        &mut self,
        buttons: std::collections::HashSet<crate::types::ToolbarButton>,
    ) {
        self.toolbar.set_disabled(buttons);
    }
}

/// UI错误类型
#[derive(Debug)]
pub enum UIError {
    /// 渲染错误
    RenderError(String),
    /// 初始化错误
    InitError(String),
}

impl std::fmt::Display for UIError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UIError::RenderError(msg) => write!(f, "UI render error: {}", msg),
            UIError::InitError(msg) => write!(f, "UI init error: {}", msg),
        }
    }
}

impl std::error::Error for UIError {}
