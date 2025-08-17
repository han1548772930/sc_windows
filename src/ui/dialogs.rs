// 对话框管理
//
// 负责各种对话框的显示和交互

use super::UIError;
use crate::message::{Command, DialogType};
use crate::platform::{PlatformError, PlatformRenderer};

/// 对话框管理器
pub struct DialogManager {
    /// 当前显示的对话框
    current_dialog: Option<DialogType>,
    /// 对话框位置和大小
    dialog_rect: DialogRect,
    /// 按钮状态
    button_states: DialogButtonStates,
}

/// 对话框矩形
#[derive(Debug, Clone)]
struct DialogRect {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

/// 对话框按钮状态
#[derive(Debug, Clone)]
struct DialogButtonStates {
    ok_hovered: bool,
    cancel_hovered: bool,
    settings_hovered: bool,
}

impl DialogManager {
    /// 创建新的对话框管理器
    pub fn new() -> Result<Self, UIError> {
        Ok(Self {
            current_dialog: None,
            dialog_rect: DialogRect {
                x: 300.0,
                y: 200.0,
                width: 400.0,
                height: 300.0,
            },
            button_states: DialogButtonStates {
                ok_hovered: false,
                cancel_hovered: false,
                settings_hovered: false,
            },
        })
    }

    /// 显示对话框
    pub fn show_dialog(&mut self, dialog_type: DialogType) {
        self.current_dialog = Some(dialog_type);
    }

    /// 关闭当前对话框
    pub fn close_current_dialog(&mut self) {
        self.current_dialog = None;
    }

    /// 渲染对话框
    pub fn render(
        &self,
        renderer: &mut dyn PlatformRenderer<Error = PlatformError>,
    ) -> Result<(), UIError> {
        if let Some(ref dialog_type) = self.current_dialog {
            self.render_dialog_background(renderer)?;

            match dialog_type {
                DialogType::Save => {
                    self.render_save_dialog(renderer)?;
                }
                DialogType::Settings => {
                    self.render_settings_dialog(renderer)?;
                }
                DialogType::About => {
                    self.render_about_dialog(renderer)?;
                }
            }

            self.render_dialog_buttons(renderer, dialog_type)?;
        }
        Ok(())
    }

    /// 渲染对话框背景
    fn render_dialog_background(
        &self,
        renderer: &mut dyn PlatformRenderer<Error = PlatformError>,
    ) -> Result<(), UIError> {
        use crate::platform::traits::{Color, DrawStyle, Rectangle};

        // 半透明背景覆盖整个屏幕
        let overlay_rect = Rectangle {
            x: 0.0,
            y: 0.0,
            width: 1920.0, // 假设最大屏幕尺寸
            height: 1080.0,
        };

        let overlay_style = DrawStyle {
            stroke_color: Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            },
            fill_color: Some(Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.5,
            }),
            stroke_width: 0.0,
        };

        renderer
            .draw_rectangle(overlay_rect, &overlay_style)
            .map_err(|e| UIError::RenderError(e.to_string()))?;

        // 对话框背景
        let dialog_rect = Rectangle {
            x: self.dialog_rect.x,
            y: self.dialog_rect.y,
            width: self.dialog_rect.width,
            height: self.dialog_rect.height,
        };

        let dialog_style = DrawStyle {
            stroke_color: Color {
                r: 0.5,
                g: 0.5,
                b: 0.5,
                a: 1.0,
            },
            fill_color: Some(Color {
                r: 0.95,
                g: 0.95,
                b: 0.95,
                a: 1.0,
            }),
            stroke_width: 2.0,
        };

        renderer
            .draw_rectangle(dialog_rect, &dialog_style)
            .map_err(|e| UIError::RenderError(e.to_string()))?;

        Ok(())
    }

    /// 渲染保存对话框
    fn render_save_dialog(
        &self,
        renderer: &mut dyn PlatformRenderer<Error = PlatformError>,
    ) -> Result<(), UIError> {
        use crate::platform::traits::{Color, Point, TextStyle};

        let title_pos = Point {
            x: self.dialog_rect.x + 20.0,
            y: self.dialog_rect.y + 30.0,
        };

        let title_style = TextStyle {
            font_size: 18.0,
            color: Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            font_family: "Arial".to_string(),
        };

        renderer
            .draw_text("保存截图", title_pos, &title_style)
            .map_err(|e| UIError::RenderError(e.to_string()))?;

        let message_pos = Point {
            x: self.dialog_rect.x + 20.0,
            y: self.dialog_rect.y + 80.0,
        };

        let message_style = TextStyle {
            font_size: 14.0,
            color: Color {
                r: 0.2,
                g: 0.2,
                b: 0.2,
                a: 1.0,
            },
            font_family: "Arial".to_string(),
        };

        renderer
            .draw_text("选择保存位置和格式", message_pos, &message_style)
            .map_err(|e| UIError::RenderError(e.to_string()))?;

        Ok(())
    }

    /// 渲染设置对话框
    fn render_settings_dialog(
        &self,
        renderer: &mut dyn PlatformRenderer<Error = PlatformError>,
    ) -> Result<(), UIError> {
        use crate::platform::traits::{Color, Point, TextStyle};

        let title_pos = Point {
            x: self.dialog_rect.x + 20.0,
            y: self.dialog_rect.y + 30.0,
        };

        let title_style = TextStyle {
            font_size: 18.0,
            color: Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            font_family: "Arial".to_string(),
        };

        renderer
            .draw_text("设置", title_pos, &title_style)
            .map_err(|e| UIError::RenderError(e.to_string()))?;

        // 渲染设置选项
        let options = [
            "热键设置: Ctrl+A",
            "默认颜色: 红色",
            "线条粗细: 3.0",
            "字体大小: 16.0",
        ];

        for (i, option) in options.iter().enumerate() {
            let option_pos = Point {
                x: self.dialog_rect.x + 20.0,
                y: self.dialog_rect.y + 80.0 + (i as f32 * 30.0),
            };

            let option_style = TextStyle {
                font_size: 12.0,
                color: Color {
                    r: 0.2,
                    g: 0.2,
                    b: 0.2,
                    a: 1.0,
                },
                font_family: "Arial".to_string(),
            };

            renderer
                .draw_text(option, option_pos, &option_style)
                .map_err(|e| UIError::RenderError(e.to_string()))?;
        }

        Ok(())
    }

    /// 渲染关于对话框
    fn render_about_dialog(
        &self,
        renderer: &mut dyn PlatformRenderer<Error = PlatformError>,
    ) -> Result<(), UIError> {
        use crate::platform::traits::{Color, Point, TextStyle};

        let title_pos = Point {
            x: self.dialog_rect.x + 20.0,
            y: self.dialog_rect.y + 30.0,
        };

        let title_style = TextStyle {
            font_size: 18.0,
            color: Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            font_family: "Arial".to_string(),
        };

        renderer
            .draw_text("关于 SC Windows", title_pos, &title_style)
            .map_err(|e| UIError::RenderError(e.to_string()))?;

        let info_lines = [
            "版本: 0.1.0",
            "截图工具",
            "支持绘图、OCR、Pin等功能",
            "",
            "基于 Rust 和 Direct2D 开发",
        ];

        for (i, line) in info_lines.iter().enumerate() {
            let line_pos = Point {
                x: self.dialog_rect.x + 20.0,
                y: self.dialog_rect.y + 80.0 + (i as f32 * 25.0),
            };

            let line_style = TextStyle {
                font_size: 12.0,
                color: Color {
                    r: 0.2,
                    g: 0.2,
                    b: 0.2,
                    a: 1.0,
                },
                font_family: "Arial".to_string(),
            };

            renderer
                .draw_text(line, line_pos, &line_style)
                .map_err(|e| UIError::RenderError(e.to_string()))?;
        }

        Ok(())
    }

    /// 渲染对话框按钮
    fn render_dialog_buttons(
        &self,
        renderer: &mut dyn PlatformRenderer<Error = PlatformError>,
        dialog_type: &DialogType,
    ) -> Result<(), UIError> {
        use crate::platform::traits::{Color, DrawStyle, Point, Rectangle, TextStyle};

        let button_y = self.dialog_rect.y + self.dialog_rect.height - 60.0;
        let button_width = 80.0;
        let button_height = 30.0;

        match dialog_type {
            DialogType::Save => {
                // 保存按钮
                let save_button_rect = Rectangle {
                    x: self.dialog_rect.x + self.dialog_rect.width - 200.0,
                    y: button_y,
                    width: button_width,
                    height: button_height,
                };

                let save_button_style = DrawStyle {
                    stroke_color: Color {
                        r: 0.3,
                        g: 0.3,
                        b: 0.3,
                        a: 1.0,
                    },
                    fill_color: Some(if self.button_states.ok_hovered {
                        Color {
                            r: 0.9,
                            g: 0.9,
                            b: 1.0,
                            a: 1.0,
                        }
                    } else {
                        Color {
                            r: 0.8,
                            g: 0.8,
                            b: 0.8,
                            a: 1.0,
                        }
                    }),
                    stroke_width: 1.0,
                };

                renderer
                    .draw_rectangle(save_button_rect, &save_button_style)
                    .map_err(|e| UIError::RenderError(e.to_string()))?;

                let save_text_pos = Point {
                    x: save_button_rect.x + 25.0,
                    y: save_button_rect.y + 10.0,
                };

                let button_text_style = TextStyle {
                    font_size: 12.0,
                    color: Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    },
                    font_family: "Arial".to_string(),
                };

                renderer
                    .draw_text("保存", save_text_pos, &button_text_style)
                    .map_err(|e| UIError::RenderError(e.to_string()))?;
            }
            DialogType::Settings => {
                // 确定按钮
                let ok_button_rect = Rectangle {
                    x: self.dialog_rect.x + self.dialog_rect.width - 200.0,
                    y: button_y,
                    width: button_width,
                    height: button_height,
                };

                let ok_button_style = DrawStyle {
                    stroke_color: Color {
                        r: 0.3,
                        g: 0.3,
                        b: 0.3,
                        a: 1.0,
                    },
                    fill_color: Some(if self.button_states.ok_hovered {
                        Color {
                            r: 0.9,
                            g: 0.9,
                            b: 1.0,
                            a: 1.0,
                        }
                    } else {
                        Color {
                            r: 0.8,
                            g: 0.8,
                            b: 0.8,
                            a: 1.0,
                        }
                    }),
                    stroke_width: 1.0,
                };

                renderer
                    .draw_rectangle(ok_button_rect, &ok_button_style)
                    .map_err(|e| UIError::RenderError(e.to_string()))?;

                let ok_text_pos = Point {
                    x: ok_button_rect.x + 25.0,
                    y: ok_button_rect.y + 10.0,
                };

                let button_text_style = TextStyle {
                    font_size: 12.0,
                    color: Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    },
                    font_family: "Arial".to_string(),
                };

                renderer
                    .draw_text("确定", ok_text_pos, &button_text_style)
                    .map_err(|e| UIError::RenderError(e.to_string()))?;
            }
            DialogType::About => {
                // 关闭按钮
                let close_button_rect = Rectangle {
                    x: self.dialog_rect.x + self.dialog_rect.width - 110.0,
                    y: button_y,
                    width: button_width,
                    height: button_height,
                };

                let close_button_style = DrawStyle {
                    stroke_color: Color {
                        r: 0.3,
                        g: 0.3,
                        b: 0.3,
                        a: 1.0,
                    },
                    fill_color: Some(if self.button_states.ok_hovered {
                        Color {
                            r: 0.9,
                            g: 0.9,
                            b: 1.0,
                            a: 1.0,
                        }
                    } else {
                        Color {
                            r: 0.8,
                            g: 0.8,
                            b: 0.8,
                            a: 1.0,
                        }
                    }),
                    stroke_width: 1.0,
                };

                renderer
                    .draw_rectangle(close_button_rect, &close_button_style)
                    .map_err(|e| UIError::RenderError(e.to_string()))?;

                let close_text_pos = Point {
                    x: close_button_rect.x + 25.0,
                    y: close_button_rect.y + 10.0,
                };

                let button_text_style = TextStyle {
                    font_size: 12.0,
                    color: Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    },
                    font_family: "Arial".to_string(),
                };

                renderer
                    .draw_text("关闭", close_text_pos, &button_text_style)
                    .map_err(|e| UIError::RenderError(e.to_string()))?;
            }
        }

        // 取消按钮（所有对话框都有）
        let cancel_button_rect = Rectangle {
            x: self.dialog_rect.x + self.dialog_rect.width - 110.0,
            y: button_y,
            width: button_width,
            height: button_height,
        };

        let cancel_button_style = DrawStyle {
            stroke_color: Color {
                r: 0.3,
                g: 0.3,
                b: 0.3,
                a: 1.0,
            },
            fill_color: Some(if self.button_states.cancel_hovered {
                Color {
                    r: 1.0,
                    g: 0.9,
                    b: 0.9,
                    a: 1.0,
                }
            } else {
                Color {
                    r: 0.8,
                    g: 0.8,
                    b: 0.8,
                    a: 1.0,
                }
            }),
            stroke_width: 1.0,
        };

        renderer
            .draw_rectangle(cancel_button_rect, &cancel_button_style)
            .map_err(|e| UIError::RenderError(e.to_string()))?;

        let cancel_text_pos = Point {
            x: cancel_button_rect.x + 25.0,
            y: cancel_button_rect.y + 10.0,
        };

        let button_text_style = TextStyle {
            font_size: 12.0,
            color: Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            font_family: "Arial".to_string(),
        };

        renderer
            .draw_text("取消", cancel_text_pos, &button_text_style)
            .map_err(|e| UIError::RenderError(e.to_string()))?;

        Ok(())
    }

    /// 处理鼠标移动
    pub fn handle_mouse_move(&mut self, x: i32, y: i32) -> Vec<Command> {
        if self.current_dialog.is_some() {
            // 检查鼠标是否悬停在按钮上
            let button_y = self.dialog_rect.y + self.dialog_rect.height - 60.0;
            let button_width = 80.0;
            let button_height = 30.0;

            // 重置按钮状态
            self.button_states.ok_hovered = false;
            self.button_states.cancel_hovered = false;

            // 检查确定/保存按钮
            let ok_button_rect = (
                self.dialog_rect.x + self.dialog_rect.width - 200.0,
                button_y,
                button_width,
                button_height,
            );

            if x as f32 >= ok_button_rect.0
                && x as f32 <= ok_button_rect.0 + ok_button_rect.2
                && y as f32 >= ok_button_rect.1
                && y as f32 <= ok_button_rect.1 + ok_button_rect.3
            {
                self.button_states.ok_hovered = true;
                return vec![Command::RequestRedraw];
            }

            // 检查取消按钮
            let cancel_button_rect = (
                self.dialog_rect.x + self.dialog_rect.width - 110.0,
                button_y,
                button_width,
                button_height,
            );

            if x as f32 >= cancel_button_rect.0
                && x as f32 <= cancel_button_rect.0 + cancel_button_rect.2
                && y as f32 >= cancel_button_rect.1
                && y as f32 <= cancel_button_rect.1 + cancel_button_rect.3
            {
                self.button_states.cancel_hovered = true;
                return vec![Command::RequestRedraw];
            }
        }
        vec![]
    }

    /// 处理鼠标按下
    pub fn handle_mouse_down(&mut self, x: i32, y: i32) -> Vec<Command> {
        if let Some(ref dialog_type) = self.current_dialog {
            // 检查是否点击了按钮
            let button_y = self.dialog_rect.y + self.dialog_rect.height - 60.0;
            let button_width = 80.0;
            let button_height = 30.0;

            // 检查确定/保存按钮
            let ok_button_rect = (
                self.dialog_rect.x + self.dialog_rect.width - 200.0,
                button_y,
                button_width,
                button_height,
            );

            if x as f32 >= ok_button_rect.0
                && x as f32 <= ok_button_rect.0 + ok_button_rect.2
                && y as f32 >= ok_button_rect.1
                && y as f32 <= ok_button_rect.1 + ok_button_rect.3
            {
                // 点击了确定/保存按钮
                match dialog_type {
                    DialogType::Save => {
                        self.close_current_dialog();
                        return vec![Command::SaveSelectionToFile, Command::RequestRedraw];
                    }
                    DialogType::Settings => {
                        self.close_current_dialog();
                        return vec![Command::ShowSettings, Command::RequestRedraw];
                    }
                    DialogType::About => {
                        self.close_current_dialog();
                        return vec![Command::RequestRedraw];
                    }
                }
            }

            // 检查取消按钮
            let cancel_button_rect = (
                self.dialog_rect.x + self.dialog_rect.width - 110.0,
                button_y,
                button_width,
                button_height,
            );

            if x as f32 >= cancel_button_rect.0
                && x as f32 <= cancel_button_rect.0 + cancel_button_rect.2
                && y as f32 >= cancel_button_rect.1
                && y as f32 <= cancel_button_rect.1 + cancel_button_rect.3
            {
                // 点击了取消按钮
                self.close_current_dialog();
                return vec![Command::RequestRedraw];
            }

            // 检查是否点击在对话框外部（关闭对话框）
            let x_f32 = x as f32;
            let y_f32 = y as f32;
            let outside_x =
                x_f32 < self.dialog_rect.x || x_f32 > self.dialog_rect.x + self.dialog_rect.width;
            let outside_y =
                y_f32 < self.dialog_rect.y || y_f32 > self.dialog_rect.y + self.dialog_rect.height;

            if outside_x || outside_y {
                self.close_current_dialog();
                return vec![Command::RequestRedraw];
            }
        }
        vec![]
    }

    /// 处理鼠标释放
    pub fn handle_mouse_up(&mut self, _x: i32, _y: i32) -> Vec<Command> {
        // 鼠标释放事件在对话框中通常不需要特殊处理
        vec![]
    }

    /// 处理键盘输入
    pub fn handle_key_input(&mut self, key: u32) -> Vec<Command> {
        if self.current_dialog.is_some() {
            match key {
                27 => {
                    // ESC键
                    self.close_current_dialog();
                    vec![Command::RequestRedraw]
                }
                _ => vec![],
            }
        } else {
            vec![]
        }
    }

    /// 处理双击事件
    pub fn handle_double_click(&mut self, _x: i32, _y: i32) -> Vec<Command> {
        // TODO: 实现对话框双击处理
        vec![]
    }

    /// 处理文本输入
    pub fn handle_text_input(&mut self, _character: char) -> Vec<Command> {
        // TODO: 实现对话框文本输入处理
        vec![]
    }
}
