use std::collections::HashSet;

use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;

use super::svg_icons::SvgIconManager;
use super::{ToolbarButton, UIError};
use sc_drawing::DrawingTool;
use sc_host_protocol::Command;
use sc_platform_windows::windows::d2d::Direct2DRenderer;

/// 工具栏管理器
pub struct ToolbarManager {
    /// 是否可见
    pub visible: bool,

    /// 当前 layout（平台无关），由 `sc_ui` 计算。
    layout: Option<sc_ui::toolbar::ToolbarLayout>,

    /// 当前悬停的按钮
    pub hovered_button: ToolbarButton,
    /// 当前点击的按钮（选中状态）
    pub clicked_button: ToolbarButton,
    /// 当前按下的按钮（用于跟踪按下和释放）
    pub pressed_button: ToolbarButton,
    /// 按钮禁用状态
    pub disabled_buttons: HashSet<ToolbarButton>,
}

impl ToolbarManager {
    /// 创建新的工具栏管理器
    pub fn new() -> Result<Self, UIError> {
        Ok(Self {
            visible: false,
            layout: None,
            hovered_button: ToolbarButton::None,
            clicked_button: ToolbarButton::None,
            pressed_button: ToolbarButton::None,
            disabled_buttons: HashSet::new(),
        })
    }

    /// 更新工具栏位置
    pub fn update_position(
        &mut self,
        selection_rect: sc_app::selection::RectI32,
        screen_width: i32,
        screen_height: i32,
    ) {
        let style = sc_ui::toolbar::ToolbarStyle::default();

        let Some(layout) = sc_ui::toolbar::layout_toolbar(
            (screen_width, screen_height),
            Some(selection_rect),
            &style,
        ) else {
            return;
        };

        self.layout = Some(layout);
        self.visible = true;
    }

    /// 获取指定位置的按钮
    pub fn get_button_at_position(&self, x: i32, y: i32) -> ToolbarButton {
        self.layout
            .as_ref()
            .map(|layout| layout.hit_test(x, y))
            .unwrap_or(ToolbarButton::None)
    }

    /// 设置悬停按钮
    pub fn set_hovered_button(&mut self, button: ToolbarButton) {
        self.hovered_button = button;
    }

    /// 设置点击按钮
    pub fn set_clicked_button(&mut self, button: ToolbarButton) {
        self.clicked_button = button;
    }
    /// 更新禁用按钮集合
    pub fn set_disabled(&mut self, buttons: HashSet<ToolbarButton>) {
        self.disabled_buttons = buttons;
    }

    /// 清除点击按钮
    pub fn clear_clicked_button(&mut self) {
        self.clicked_button = ToolbarButton::None;
    }

    /// 显示工具栏
    pub fn show(&mut self) {
        self.visible = true;
    }

    /// 隐藏工具栏
    pub fn hide(&mut self) {
        self.visible = false;
        self.layout = None;
        self.hovered_button = ToolbarButton::None;
        self.pressed_button = ToolbarButton::None;
    }

    /// 处理按钮点击
    pub fn handle_button_click(&mut self, button: ToolbarButton) -> Vec<Command> {
        // 只有绘图工具按钮才设置为选中状态
        match button {
            ToolbarButton::Rectangle
            | ToolbarButton::Circle
            | ToolbarButton::Arrow
            | ToolbarButton::Pen
            | ToolbarButton::Text => {
                self.clicked_button = button;
            }
            _ => {
                // 其他按钮（如 Undo、Save、Pin 等）不保持选中状态
            }
        }

        match button {
            ToolbarButton::Save => {
                // 走 core: SaveSelectionToFile
                vec![Command::Core(sc_app::Action::SaveSelectionToFile)]
            }
            ToolbarButton::Rectangle => vec![Command::Core(sc_app::Action::SelectDrawingTool(
                DrawingTool::Rectangle,
            ))],
            ToolbarButton::Circle => vec![Command::Core(sc_app::Action::SelectDrawingTool(
                DrawingTool::Circle,
            ))],
            ToolbarButton::Arrow => vec![Command::Core(sc_app::Action::SelectDrawingTool(
                DrawingTool::Arrow,
            ))],
            ToolbarButton::Pen => vec![Command::Core(sc_app::Action::SelectDrawingTool(
                DrawingTool::Pen,
            ))],
            ToolbarButton::Text => vec![Command::Core(sc_app::Action::SelectDrawingTool(
                DrawingTool::Text,
            ))],
            ToolbarButton::Undo => vec![Command::Core(sc_app::Action::Undo)],
            ToolbarButton::ExtractText => vec![Command::Core(sc_app::Action::ExtractText)],
            ToolbarButton::Languages => {
                // Reserved for future language selection UI.
                vec![]
            }
            ToolbarButton::Pin => {
                // 走 core: PinSelection
                vec![Command::Core(sc_app::Action::PinSelection)]
            }
            ToolbarButton::Confirm => {
                // 走 core: SaveSelectionToClipboard（由宿主命令执行隐藏窗口/重置）
                vec![Command::Core(sc_app::Action::SaveSelectionToClipboard)]
            }
            ToolbarButton::Cancel => {
                // 走 core: Cancel -> [ResetToInitialState, HideWindow]
                vec![Command::Core(sc_app::Action::Cancel)]
            }
            _ => vec![Command::None],
        }
    }

    /// 渲染工具栏
    pub fn render(
        &self,
        d2d_renderer: &mut Direct2DRenderer,
        svg_icons: &SvgIconManager,
    ) -> Result<(), UIError> {
        if !self.visible {
            return Ok(());
        }

        // Background + hover highlight are expressed as a platform-neutral RenderList.
        let style = sc_ui::toolbar::ToolbarStyle::default();

        let Some(layout) = self.layout.as_ref() else {
            return Ok(());
        };

        let mut list = sc_ui::toolbar::build_toolbar_background_render_list(
            layout,
            self.hovered_button,
            &style,
        );

        list.execute(d2d_renderer)
            .map_err(|e| UIError::RenderError(format!("render list execute failed: {e:?}")))?;

        // Icons remain host-specific.
        unsafe {
            if let Some(render_target) = &d2d_renderer.render_target {
                for b in &layout.buttons {
                    let button_rect = b.rect;
                    let button_type = b.button;

                    // Determine icon color.
                    let is_disabled = self.disabled_buttons.contains(&button_type);
                    let icon_color = if is_disabled {
                        Some((170, 170, 170))
                    } else if self.clicked_button == button_type {
                        Some((33, 196, 94))
                    } else {
                        Some((16, 16, 16))
                    };

                    if let Ok(Some(icon_bitmap)) =
                        svg_icons.render_icon_to_bitmap(button_type, render_target, 24, icon_color)
                    {
                        let icon_size = 20.0;
                        let icon_x = button_rect.x + (button_rect.width - icon_size) / 2.0;
                        let icon_y = button_rect.y + (button_rect.height - icon_size) / 2.0;

                        let icon_rect = D2D_RECT_F {
                            left: icon_x,
                            top: icon_y,
                            right: icon_x + icon_size,
                            bottom: icon_y + icon_size,
                        };

                        render_target.DrawBitmap(
                            &icon_bitmap,
                            Some(&icon_rect),
                            1.0,
                            windows::Win32::Graphics::Direct2D::D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                            None,
                        );
                    }
                }
            }
        }

        Ok(())
    }

    /// 处理鼠标移动
    pub fn handle_mouse_move(&mut self, x: i32, y: i32) -> Vec<Command> {
        if !self.visible {
            return vec![];
        }

        // 检查鼠标是否悬停在按钮上
        let hovered_button = self.get_button_at_position(x, y);

        if self.hovered_button != hovered_button {
            self.hovered_button = hovered_button;
            vec![Command::RequestRedraw]
        } else {
            vec![]
        }
    }

    /// 处理鼠标按下
    pub fn handle_mouse_down(&mut self, x: i32, y: i32) -> Vec<Command> {
        if !self.visible {
            return vec![];
        }

        // 检查是否点击了工具栏按钮
        let button_type = self.get_button_at_position(x, y);
        if button_type != ToolbarButton::None {
            // 禁用按钮不可点击
            if self.disabled_buttons.contains(&button_type) {
                return vec![];
            }

            // 记录按下的按钮
            self.pressed_button = button_type;
            // 不在这里设置 clicked_button，交由 handle_button_click 根据按钮类型（工具/动作）决定
            // 立即处理按钮点击（其中仅绘图工具会设置 clicked_button）
            return self.handle_button_click(button_type);
        }

        vec![]
    }

    /// 处理鼠标释放
    pub fn handle_mouse_up(&mut self, x: i32, y: i32) -> Vec<Command> {
        if !self.visible {
            return vec![];
        }

        // 检查是否在同一个按钮上释放
        let toolbar_button = self.get_button_at_position(x, y);
        if toolbar_button != ToolbarButton::None && toolbar_button == self.pressed_button {
            // 按钮点击已经在 handle_mouse_down 中处理，这里只需要清除按下状态
            self.pressed_button = ToolbarButton::None;
            vec![]
        } else {
            // 如果不是在同一个按钮上释放，清除按下状态
            self.pressed_button = ToolbarButton::None;
            vec![]
        }
    }

    /// 是否可见
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// 处理双击事件
    pub fn handle_double_click(&mut self, x: i32, y: i32) -> Vec<Command> {
        // 工具栏双击暂时不处理特殊逻辑，使用单击逻辑
        self.handle_mouse_down(x, y)
    }
}
