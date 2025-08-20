// UI管理器模块
//
// 负责用户界面相关功能：工具栏、覆盖层、对话框等

use crate::message::{Command, UIMessage};
use crate::platform::{PlatformError, PlatformRenderer};

pub mod cursor;
pub mod dialogs;
pub mod svg_icons;
pub mod toolbar;

use dialogs::DialogManager;
use svg_icons::SvgIconManager;
use toolbar::ToolbarManager;

/// UI管理器
pub struct UIManager {
    /// 工具栏管理器
    toolbar: ToolbarManager,
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
                // 显示时同步一次禁用/选中状态，确保初次展示（热键启动后）Undo 为禁用
                vec![Command::UpdateToolbar, Command::RequestRedraw]
            }
            UIMessage::HideToolbar => {
                self.toolbar.hide();
                vec![Command::RequestRedraw]
            }
            UIMessage::UpdateToolbarPosition(rect) => {
                // 修复：只有在工具栏已经可见时才更新位置，避免在创建选择框时意外显示
                if self.toolbar.is_visible() {
                    self.toolbar
                        .update_position(&rect, screen_width, screen_height);
                    vec![Command::RequestRedraw]
                } else {
                    vec![]
                }
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
        // 渲染工具栏
        self.toolbar.render(renderer, &self.svg_icons)?;

        // 渲染对话框
        self.dialogs.render(renderer)?;

        Ok(())
    }

    /// 渲染选区相关的UI元素（遮罩、边框、手柄）
    ///
    /// # 参数
    /// * `renderer` - 平台渲染器
    /// * `screen_size` - 屏幕尺寸 (width, height)
    /// * `selection_rect` - 选择区域矩形（如果有）
    /// * `show_handles` - 是否显示调整手柄
    /// * `hide_ui_for_capture` - 是否因截图而隐藏UI
    /// * `has_auto_highlight` - 是否有自动高亮
    pub fn render_selection_ui(
        &self,
        renderer: &mut dyn PlatformRenderer<Error = PlatformError>,
        screen_size: (i32, i32),
        selection_rect: Option<&windows::Win32::Foundation::RECT>,
        show_handles: bool,
        hide_ui_for_capture: bool,
        has_auto_highlight: bool,
    ) -> Result<(), UIError> {
        // 如果截图时隐藏UI，则不绘制任何选区UI
        if hide_ui_for_capture {
            return Ok(());
        }

        // 如果有选择区域，绘制遮罩、边框和手柄
        if let Some(selection_rect) = selection_rect {
            use crate::platform::traits::{Color, Rectangle};

            // 绘制选择区域遮罩
            let screen_rect = Rectangle {
                x: 0.0,
                y: 0.0,
                width: screen_size.0 as f32,
                height: screen_size.1 as f32,
            };
            let selection_rect_platform = Rectangle {
                x: selection_rect.left as f32,
                y: selection_rect.top as f32,
                width: (selection_rect.right - selection_rect.left) as f32,
                height: (selection_rect.bottom - selection_rect.top) as f32,
            };
            let mask_color = Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.6,
            };

            renderer
                .draw_selection_mask(screen_rect, selection_rect_platform, mask_color)
                .map_err(|e| UIError::RenderError(format!("draw selection mask failed: {}", e)))?;

            // 绘制选择框边框
            if has_auto_highlight {
                let color = Color {
                    r: 1.0,
                    g: 0.2,
                    b: 0.2,
                    a: 1.0,
                };
                renderer
                    .draw_selection_border(selection_rect_platform, color, 3.0, None)
                    .map_err(|e| {
                        UIError::RenderError(format!("draw auto-highlight border failed: {}", e))
                    })?;
            } else {
                let c = &crate::constants::COLOR_SELECTION_BORDER;
                let color = Color {
                    r: c.r,
                    g: c.g,
                    b: c.b,
                    a: c.a,
                };
                renderer
                    .draw_selection_border(selection_rect_platform, color, 2.0, None)
                    .map_err(|e| {
                        UIError::RenderError(format!("draw selection border failed: {}", e))
                    })?;
            }

            // 绘制选择框手柄（如果需要显示）
            if show_handles {
                let fill_color = Color {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                };
                let border_color = Color {
                    r: 0.0,
                    g: 0.47,
                    b: 0.84,
                    a: 1.0,
                };

                renderer
                    .draw_selection_handles(
                        selection_rect_platform,
                        crate::constants::HANDLE_SIZE,
                        fill_color,
                        border_color,
                        1.0,
                    )
                    .map_err(|e| {
                        UIError::RenderError(format!("draw selection handles failed: {}", e))
                    })?;
            }
        }

        Ok(())
    }

    /// 处理鼠标移动
    /// 返回 (命令列表, 是否消费了事件)
    pub fn handle_mouse_move(&mut self, x: i32, y: i32) -> (Vec<Command>, bool) {
        let mut commands = Vec::new();

        // 工具栏处理鼠标移动
        let toolbar_commands = self.toolbar.handle_mouse_move(x, y);
        let toolbar_consumed = !toolbar_commands.is_empty();
        commands.extend(toolbar_commands);

        // 如果工具栏没有消费，传递给对话框
        if !toolbar_consumed {
            let dialog_commands = self.dialogs.handle_mouse_move(x, y);
            let dialog_consumed = !dialog_commands.is_empty();
            commands.extend(dialog_commands);
            (commands, dialog_consumed)
        } else {
            (commands, true)
        }
    }

    /// 处理鼠标按下
    /// 返回 (命令列表, 是否消费了事件)
    pub fn handle_mouse_down(&mut self, x: i32, y: i32) -> (Vec<Command>, bool) {
        let mut commands = Vec::new();

        // 工具栏处理鼠标按下
        let toolbar_commands = self.toolbar.handle_mouse_down(x, y);
        let toolbar_consumed = !toolbar_commands.is_empty();
        commands.extend(toolbar_commands);

        // 如果工具栏没有处理，则传递给对话框
        if !toolbar_consumed {
            let dialog_commands = self.dialogs.handle_mouse_down(x, y);
            let dialog_consumed = !dialog_commands.is_empty();
            commands.extend(dialog_commands);
            (commands, dialog_consumed)
        } else {
            (commands, true)
        }
    }

    /// 处理鼠标释放
    /// 返回 (命令列表, 是否消费了事件)
    pub fn handle_mouse_up(&mut self, x: i32, y: i32) -> (Vec<Command>, bool) {
        let mut commands = Vec::new();

        let toolbar_commands = self.toolbar.handle_mouse_up(x, y);
        let toolbar_consumed = !toolbar_commands.is_empty();
        commands.extend(toolbar_commands);

        let dialog_commands = self.dialogs.handle_mouse_up(x, y);
        let dialog_consumed = !dialog_commands.is_empty();
        commands.extend(dialog_commands);

        (commands, toolbar_consumed || dialog_consumed)
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
