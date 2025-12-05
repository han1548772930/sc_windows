//! 用户界面模块
//!
//! 提供工具栏、图标和光标等 UI 组件。
//!
//! # 主要组件
//! - [`UIManager`]: UI 管理器，统一管理用户界面
//! - [`ToolbarManager`](toolbar::ToolbarManager): 工具栏管理
//! - [`SvgIconManager`](svg_icons::SvgIconManager): SVG 图标管理
//! - [`cursor`]: 光标管理功能

use std::collections::HashSet;

use windows::Win32::Foundation::RECT;

use crate::constants::{COLOR_SELECTION_BORDER, HANDLE_SIZE};
use crate::drawing::DrawingTool;
use crate::message::{Command, UIMessage};
use crate::platform::traits::{Color, Rectangle};
use crate::platform::windows::d2d::Direct2DRenderer;
use crate::rendering::{RenderItem, RenderList, z_order};

pub mod cursor;
pub mod preview;
pub mod svg_icons;
pub mod toolbar;
pub mod types;
pub mod file_dialog;

// Re-export types for convenience
pub use preview::PreviewWindow;
pub use types::ToolbarButton;

use svg_icons::SvgIconManager;
use toolbar::ToolbarManager;

/// UI管理器
pub struct UIManager {
    /// 工具栏管理器
    toolbar: ToolbarManager,
    /// SVG图标管理器
    svg_icons: SvgIconManager,
}

impl UIManager {
    /// 创建新的UI管理器
    pub fn new() -> Result<Self, UIError> {
        let mut svg_icons = SvgIconManager::new();
        // 加载SVG图标
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
        // 隐藏工具栏
        self.toolbar.hide();

        // 重置工具栏按钮状态
        self.toolbar.clicked_button = ToolbarButton::None;
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
                // 使用正确的屏幕尺寸
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
            UIMessage::ShowDialog(_dialog_type) => {
                // No-op
                vec![]
            }
            UIMessage::CloseDialog => {
                // No-op
                vec![]
            }
        }
    }

    /// 渲染UI元素
    pub fn render(
        &self,
        d2d_renderer: &mut Direct2DRenderer,
    ) -> Result<(), UIError> {
        // 渲染工具栏
        self.toolbar.render(d2d_renderer, &self.svg_icons)?;

        Ok(())
    }

    /// 渲染选区相关的UI元素（遮罩、边框、手柄）
    ///
    /// 使用 RenderList 收集渲染图元，按 z_order 排序后统一绘制。
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
        d2d_renderer: &mut Direct2DRenderer,
        screen_size: (i32, i32),
        selection_rect: Option<&RECT>,
        show_handles: bool,
        hide_ui_for_capture: bool,
        has_auto_highlight: bool,
    ) -> Result<(), UIError> {
        // 如果截图时隐藏UI，则不绘制任何选区UI
        if hide_ui_for_capture {
            return Ok(());
        }

        // 如果没有选择区域，直接返回
        let Some(selection_rect) = selection_rect else {
            return Ok(());
        };

        // 创建渲染列表
        let mut render_list = RenderList::with_capacity(4);

        // 准备矩形数据
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

        // 1. 提交选择区域遮罩
        let mask_color = Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.6,
        };
        render_list.submit(RenderItem::SelectionMask {
            screen_rect,
            selection_rect: selection_rect_platform,
            mask_color,
            z_order: z_order::MASK,
        });

        // 2. 提交选择框边框
        let (border_color, border_width) = if has_auto_highlight {
            (
                Color {
                    r: 1.0,
                    g: 0.2,
                    b: 0.2,
                    a: 1.0,
                },
                3.0,
            )
        } else {
            (
                Color {
                    r: COLOR_SELECTION_BORDER.r,
                    g: COLOR_SELECTION_BORDER.g,
                    b: COLOR_SELECTION_BORDER.b,
                    a: COLOR_SELECTION_BORDER.a,
                },
                2.0,
            )
        };
        render_list.submit(RenderItem::SelectionBorder {
            rect: selection_rect_platform,
            color: border_color,
            width: border_width,
            dash_pattern: None,
            z_order: z_order::SELECTION_BORDER,
        });

        // 3. 提交选择框手柄（如果需要显示）
        if show_handles {
            let fill_color = Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            };
            let handle_border_color = Color {
                r: 0.0,
                g: 0.47,
                b: 0.84,
                a: 1.0,
            };
            render_list.submit(RenderItem::SelectionHandles {
                rect: selection_rect_platform,
                handle_size: HANDLE_SIZE,
                fill_color,
                border_color: handle_border_color,
                border_width: 1.0,
                z_order: z_order::SELECTION_HANDLES,
            });
        }

        // 执行渲染列表（按 z_order 排序后统一绘制）
        render_list
            .execute(d2d_renderer)
            .map_err(|e| UIError::RenderError(format!("render list execute failed: {e}")))?;

        Ok(())
    }

    /// 处理鼠标移动
    /// 返回 (命令列表, 是否消费了事件)
    pub fn handle_mouse_move(&mut self, x: i32, y: i32) -> (Vec<Command>, bool) {
        // 工具栏处理鼠标移动
        let toolbar_commands = self.toolbar.handle_mouse_move(x, y);
        let toolbar_consumed = !toolbar_commands.is_empty();

        (toolbar_commands, toolbar_consumed)
    }

    /// 处理鼠标按下
    /// 返回 (命令列表, 是否消费了事件)
    pub fn handle_mouse_down(&mut self, x: i32, y: i32) -> (Vec<Command>, bool) {
        // 工具栏处理鼠标按下
        let toolbar_commands = self.toolbar.handle_mouse_down(x, y);
        let toolbar_consumed = !toolbar_commands.is_empty();

        (toolbar_commands, toolbar_consumed)
    }

    /// 处理鼠标释放
    /// 返回 (命令列表, 是否消费了事件)
    pub fn handle_mouse_up(&mut self, x: i32, y: i32) -> (Vec<Command>, bool) {
        let toolbar_commands = self.toolbar.handle_mouse_up(x, y);
        let toolbar_consumed = !toolbar_commands.is_empty();

        (toolbar_commands, toolbar_consumed)
    }

    /// 处理双击事件
    pub fn handle_double_click(&mut self, x: i32, y: i32) -> Vec<Command> {
        let mut commands = Vec::new();
        // 工具栏可能需要处理双击（例如快速操作）
        commands.extend(self.toolbar.handle_double_click(x, y));

        commands
    }

    /// 处理键盘输入
    pub fn handle_key_input(&mut self, _key: u32) -> Vec<Command> {
        // No-op
        vec![]
    }

    /// 工具栏是否可见
    pub fn is_toolbar_visible(&self) -> bool {
        self.toolbar.is_visible()
    }

    /// 获取当前悬停的工具栏按钮
    pub fn get_hovered_button(&self) -> ToolbarButton {
        self.toolbar.hovered_button
    }

    /// 查询某个按钮是否被禁用
    pub fn is_button_disabled(&self, button: ToolbarButton) -> bool {
        self.toolbar.disabled_buttons.contains(&button)
    }

    /// 更新工具栏选中的绘图工具
    pub fn update_toolbar_selected_tool(&mut self, tool: DrawingTool) {
        // 将绘图工具转换为工具栏按钮
        let button = match tool {
            DrawingTool::Rectangle => ToolbarButton::Rectangle,
            DrawingTool::Circle => ToolbarButton::Circle,
            DrawingTool::Arrow => ToolbarButton::Arrow,
            DrawingTool::Pen => ToolbarButton::Pen,
            DrawingTool::Text => ToolbarButton::Text,
            DrawingTool::None => ToolbarButton::None,
        };

        // 更新工具栏的选中按钮状态
        self.toolbar.clicked_button = button;
    }
}

impl UIManager {
    /// 设置工具栏禁用按钮状态
    pub fn set_toolbar_disabled(
        &mut self,
        buttons: HashSet<ToolbarButton>,
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
            UIError::RenderError(msg) => write!(f, "UI render error: {msg}"),
            UIError::InitError(msg) => write!(f, "UI init error: {msg}"),
        }
    }
}

impl std::error::Error for UIError {}
