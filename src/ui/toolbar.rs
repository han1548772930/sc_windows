// 工具栏管理
//
// 负责工具栏的显示、交互和状态管理

use super::UIError;
use crate::message::Command;
use crate::platform::{PlatformError, PlatformRenderer};
use crate::types::ToolbarButton;
use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;

/// 工具栏管理器（从原始Toolbar迁移）
pub struct ToolbarManager {
    /// 工具栏矩形区域
    pub rect: D2D_RECT_F,
    /// 是否可见
    pub visible: bool,
    /// 按钮列表（位置和类型）
    pub buttons: Vec<(D2D_RECT_F, ToolbarButton)>,
    /// 当前悬停的按钮
    pub hovered_button: ToolbarButton,
    /// 当前点击的按钮（选中状态）
    pub clicked_button: ToolbarButton,
    /// 当前按下的按钮（用于跟踪按下和释放）
    pub pressed_button: ToolbarButton,
    /// 按钮禁用状态
    pub disabled_buttons: std::collections::HashSet<ToolbarButton>,
}

impl ToolbarManager {
    /// 创建新的工具栏管理器（从原始Toolbar::new迁移）
    pub fn new() -> Result<Self, UIError> {
        Ok(Self {
            rect: D2D_RECT_F {
                left: 0.0,
                top: 0.0,
                right: 0.0,
                bottom: 0.0,
            },
            visible: false,
            buttons: Vec::new(),
            hovered_button: ToolbarButton::None,
            clicked_button: ToolbarButton::None,
            pressed_button: ToolbarButton::None,
            disabled_buttons: std::collections::HashSet::new(),
        })
    }

    /// 更新工具栏位置（从原始update_position迁移）
    pub fn update_position(
        &mut self,
        selection_rect: &windows::Win32::Foundation::RECT,
        screen_width: i32,
        screen_height: i32,
    ) {
        // 工具栏尺寸常量（从原始constants.rs迁移）
        const TOOLBAR_HEIGHT: f32 = 40.0;
        const BUTTON_WIDTH: f32 = 30.0;
        const BUTTON_HEIGHT: f32 = 30.0;
        const BUTTON_SPACING: f32 = 4.0;
        const TOOLBAR_PADDING: f32 = 8.0;
        const TOOLBAR_MARGIN: f32 = 3.0;
        const BUTTON_COUNT: i32 = 12;

        let toolbar_width = BUTTON_WIDTH * BUTTON_COUNT as f32
            + BUTTON_SPACING * (BUTTON_COUNT - 1) as f32
            + TOOLBAR_PADDING * 2.0;

        let mut toolbar_x = selection_rect.left as f32
            + (selection_rect.right - selection_rect.left) as f32 / 2.0
            - toolbar_width / 2.0;
        let mut toolbar_y = selection_rect.bottom as f32 + TOOLBAR_MARGIN;

        if toolbar_y + TOOLBAR_HEIGHT > screen_height as f32 {
            toolbar_y = selection_rect.top as f32 - TOOLBAR_HEIGHT - TOOLBAR_MARGIN;
        }

        toolbar_x = toolbar_x.max(0.0).min(screen_width as f32 - toolbar_width);
        toolbar_y = toolbar_y
            .max(0.0)
            .min(screen_height as f32 - TOOLBAR_HEIGHT);

        self.rect = D2D_RECT_F {
            left: toolbar_x,
            top: toolbar_y,
            right: toolbar_x + toolbar_width,
            bottom: toolbar_y + TOOLBAR_HEIGHT,
        };

        self.buttons.clear();

        // 修改：让按钮垂直居中，形成正方形（从原始代码迁移）
        let button_y = toolbar_y + (TOOLBAR_HEIGHT - BUTTON_HEIGHT) / 2.0; // 垂直居中
        let mut button_x = toolbar_x + TOOLBAR_PADDING;

        let buttons = [
            ToolbarButton::Rectangle,
            ToolbarButton::Circle,
            ToolbarButton::Arrow,
            ToolbarButton::Pen,
            ToolbarButton::Text,
            ToolbarButton::Undo,
            ToolbarButton::ExtractText,
            ToolbarButton::Languages,
            ToolbarButton::Save,
            ToolbarButton::Pin,
            ToolbarButton::Confirm,
            ToolbarButton::Cancel,
        ];

        for button_type in buttons.iter() {
            let button_rect = D2D_RECT_F {
                left: button_x,
                top: button_y,
                right: button_x + BUTTON_WIDTH,
                bottom: button_y + BUTTON_HEIGHT, // 使用BUTTON_HEIGHT而不是toolbar高度
            };
            self.buttons.push((button_rect, *button_type));
            button_x += BUTTON_WIDTH + BUTTON_SPACING;
        }

        self.visible = true;
    }

    /// 获取指定位置的按钮（从原始代码迁移）
    pub fn get_button_at_position(&self, x: i32, y: i32) -> ToolbarButton {
        for (rect, button_type) in &self.buttons {
            if x as f32 >= rect.left
                && x as f32 <= rect.right
                && y as f32 >= rect.top
                && y as f32 <= rect.bottom
            {
                return *button_type;
            }
        }
        ToolbarButton::None
    }

    /// 设置悬停按钮（从原始代码迁移）
    pub fn set_hovered_button(&mut self, button: ToolbarButton) {
        self.hovered_button = button;
    }

    /// 设置点击按钮（从原始代码迁移）
    pub fn set_clicked_button(&mut self, button: ToolbarButton) {
        self.clicked_button = button;
    }
    /// 更新禁用按钮集合
    pub fn set_disabled(&mut self, buttons: std::collections::HashSet<ToolbarButton>) {
        self.disabled_buttons = buttons;
    }

    /// 清除点击按钮（从原始代码迁移）
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
        self.hovered_button = ToolbarButton::None;
        self.pressed_button = ToolbarButton::None;
    }

    /// 处理按钮点击（从原始代码迁移）
    pub fn handle_button_click(&mut self, button: ToolbarButton) -> Vec<Command> {
        // 只有绘图工具按钮才设置为选中状态（从原始代码迁移）
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
                // 动作按钮：不改变当前 clicked_button；触发保存动作
                vec![Command::SaveSelectionToFile]
            }
            ToolbarButton::Rectangle => vec![Command::SelectDrawingTool(
                crate::types::DrawingTool::Rectangle,
            )],
            ToolbarButton::Circle => vec![Command::SelectDrawingTool(
                crate::types::DrawingTool::Circle,
            )],
            ToolbarButton::Arrow => {
                vec![Command::SelectDrawingTool(crate::types::DrawingTool::Arrow)]
            }
            ToolbarButton::Pen => vec![Command::SelectDrawingTool(crate::types::DrawingTool::Pen)],
            ToolbarButton::Text => {
                vec![Command::SelectDrawingTool(crate::types::DrawingTool::Text)]
            }
            ToolbarButton::Undo => {
                vec![Command::Drawing(crate::message::DrawingMessage::Undo)]
            }
            ToolbarButton::ExtractText => {
                // 实现文本提取功能（从原始代码迁移）
                vec![Command::ExtractText]
            }
            ToolbarButton::Languages => {
                // 显示设置窗口以选择OCR语言（从原始代码迁移）
                vec![Command::ShowSettings]
            }
            ToolbarButton::Pin => {
                // 实现固定功能（从原始代码迁移）
                vec![Command::PinSelection]
            }
            ToolbarButton::Confirm => {
                // 保存到剪贴板并隐藏窗口（从原始代码迁移）
                vec![Command::SaveSelectionToClipboard, Command::HideWindow]
            }
            ToolbarButton::Cancel => {
                // 重置状态并隐藏窗口（从原始代码迁移）
                vec![Command::ResetToInitialState, Command::HideWindow]
            }
            _ => vec![Command::None],
        }
    }

    /// 渲染工具栏（从原始代码迁移）
    pub fn render(
        &self,
        renderer: &mut dyn PlatformRenderer<Error = PlatformError>,
        svg_icons: &super::svg_icons::SvgIconManager,
    ) -> Result<(), UIError> {
        if !self.visible {
            return Ok(());
        }

        // 尝试使用Direct2D直接渲染
        if let Some(d2d_renderer) = renderer
            .as_any()
            .downcast_ref::<crate::platform::windows::d2d::Direct2DRenderer>()
        {
            unsafe {
                if let Some(render_target) = &d2d_renderer.render_target {
                    // 绘制工具栏背景
                    let toolbar_rect = self.rect;

                    // 创建工具栏背景画刷（从原始代码迁移）
                    let bg_color = windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F {
                        r: 1.0,
                        g: 1.0,
                        b: 1.0,
                        a: 0.95,
                    };

                    if let Ok(bg_brush) = render_target.CreateSolidColorBrush(&bg_color, None) {
                        // 绘制圆角矩形背景（从原始代码迁移）
                        let rounded_rect = windows::Win32::Graphics::Direct2D::D2D1_ROUNDED_RECT {
                            rect: toolbar_rect,
                            radiusX: 10.0,
                            radiusY: 10.0,
                        };
                        render_target.FillRoundedRectangle(&rounded_rect, &bg_brush);
                    }

                    // 绘制按钮（简化版本）
                    for (button_rect, button_type) in &self.buttons {
                        // 创建按钮画刷
                        // 绘制按钮背景状态 - 只有 hover 时才显示背景（从原始代码迁移）
                        if self.hovered_button == *button_type {
                            // 悬停状态
                            let hover_color =
                                windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F {
                                    r: 0.75,
                                    g: 0.75,
                                    b: 0.75,
                                    a: 1.0,
                                };

                            if let Ok(hover_brush) =
                                render_target.CreateSolidColorBrush(&hover_color, None)
                            {
                                let button_rounded_rect =
                                    windows::Win32::Graphics::Direct2D::D2D1_ROUNDED_RECT {
                                        rect: *button_rect,
                                        radiusX: 6.0,
                                        radiusY: 6.0,
                                    };
                                render_target
                                    .FillRoundedRectangle(&button_rounded_rect, &hover_brush);
                            }
                        }

                        // 确定图标颜色（从原始代码迁移）
                        let is_disabled = self.disabled_buttons.contains(button_type);
                        let icon_color = if is_disabled {
                            // 禁用状态 - 灰色
                            Some((170, 170, 170))
                        } else if self.clicked_button == *button_type {
                            // 选中状态 - 绿色
                            Some((33, 196, 94))
                        } else {
                            // 普通状态 - 深灰色
                            Some((16, 16, 16))
                        };

                        // 渲染 SVG 图标（从原始代码迁移）
                        if let Ok(Some(icon_bitmap)) = svg_icons.render_icon_to_bitmap(
                            *button_type,
                            render_target,
                            24, // 图标大小
                            icon_color,
                        ) {
                            // 计算图标居中位置
                            let icon_size = 20.0; // 显示大小
                            let icon_x = button_rect.left
                                + (button_rect.right - button_rect.left - icon_size) / 2.0;
                            let icon_y = button_rect.top
                                + (button_rect.bottom - button_rect.top - icon_size) / 2.0;

                            let icon_rect =
                                windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F {
                                    left: icon_x,
                                    top: icon_y,
                                    right: icon_x + icon_size,
                                    bottom: icon_y + icon_size,
                                };

                            // 绘制图标
                            render_target.DrawBitmap(
                                &icon_bitmap,
                                Some(&icon_rect),
                                1.0, // 不透明度
                                windows::Win32::Graphics::Direct2D::D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                                None,
                            );
                        }
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
        let mut hovered_button = ToolbarButton::None;
        for (button_rect, button_type) in &self.buttons {
            if x as f32 >= button_rect.left
                && x as f32 <= button_rect.right
                && y as f32 >= button_rect.top
                && y as f32 <= button_rect.bottom
            {
                hovered_button = *button_type;
                break;
            }
        }

        if self.hovered_button != hovered_button {
            self.hovered_button = hovered_button;
            vec![Command::RequestRedraw]
        } else {
            vec![]
        }
    }

    /// 处理鼠标按下（按照原始代码逻辑）
    pub fn handle_mouse_down(&mut self, x: i32, y: i32) -> Vec<Command> {
        if !self.visible {
            return vec![];
        }

        // 检查是否点击了工具栏按钮（按照原始代码逻辑）
        for (button_rect, button_type) in &self.buttons {
            if x as f32 >= button_rect.left
                && x as f32 <= button_rect.right
                && y as f32 >= button_rect.top
                && y as f32 <= button_rect.bottom
            {
                // 禁用按钮不可点击
                if self.disabled_buttons.contains(button_type) {
                    return vec![];
                }

                // 记录按下的按钮（从原始代码迁移）
                self.pressed_button = *button_type;
                // 不在这里设置 clicked_button，交由 handle_button_click 根据按钮类型（工具/动作）决定
                // 调试输出
                eprintln!("Toolbar button clicked: {:?}", button_type);
                // 立即处理按钮点击（其中仅绘图工具会设置 clicked_button）
                return self.handle_button_click(*button_type);
            }
        }

        vec![]
    }

    /// 处理鼠标释放（按照原始代码逻辑）
    pub fn handle_mouse_up(&mut self, x: i32, y: i32) -> Vec<Command> {
        if !self.visible {
            return vec![];
        }

        // 检查是否在同一个按钮上释放（按照原始代码逻辑）
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
