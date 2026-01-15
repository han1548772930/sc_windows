use sc_platform::WindowId;

use crate::system::SystemError;
use sc_app::selection as core_selection;
use sc_drawing::{DragMode, Rect};
use sc_highlight::{AutoHighlightMoveAction, AutoHighlightMoveArgs, AutoHighlighter};
use sc_host_protocol::Command;
use sc_platform_windows::windows::Direct2DRenderer;

pub mod selection;

use selection::SelectionState;

/// 截图管理器
pub struct ScreenshotManager {
    /// 选择状态
    selection: SelectionState,
    /// 当前截图数据
    current_screenshot: Option<ScreenshotData>,


    /// 屏幕尺寸
    screen_width: i32,
    screen_height: i32,

    /// UI隐藏状态（截图时隐藏UI元素）
    hide_ui_for_capture: bool,

    /// 是否显示选择框手柄（用于与绘图工具联动）
    show_selection_handles: bool,

    /// 自动高亮（hover 高亮 + 状态机）
    auto_highlight: AutoHighlighter,

    /// 当前窗口（用于排除自己的窗口）
    current_window: WindowId,
}

/// 截图数据
pub struct ScreenshotData {
    /// 图像数据
    pub data: Vec<u8>,
    /// 宽度
    pub width: u32,
    /// 高度
    pub height: u32,
}

impl ScreenshotManager {
    pub fn has_screenshot(&self) -> bool {
        self.current_screenshot.is_some()
    }

    /// 创建新的截图管理器
    pub fn new(screen_size: (i32, i32)) -> Result<Self, ScreenshotError> {
        let (screen_width, screen_height) = screen_size;

        Ok(Self {
            selection: SelectionState::new(),
            current_screenshot: None,

            // 初始化从 WindowState 迁移的字段
            screen_width,
            screen_height,
            hide_ui_for_capture: false,
            show_selection_handles: true,
            auto_highlight: {
                let mut highlighter = AutoHighlighter::new();
                highlighter.start_detection()?; // 启用窗口检测
                highlighter
            },
            current_window: WindowId::INVALID,
        })
    }

    /// 与绘图工具联动：控制是否显示选择框手柄
    pub fn set_show_selection_handles(&mut self, show: bool) {
        self.show_selection_handles = show;
    }

    pub fn get_screen_width(&self) -> i32 {
        self.screen_width
    }

    pub fn get_screen_height(&self) -> i32 {
        self.screen_height
    }

    /// 是否应该显示选择框手柄
    pub fn should_show_selection_handles(&self) -> bool {
        self.show_selection_handles
    }

    /// 是否因截图而隐藏UI
    pub fn is_hiding_ui_for_capture(&self) -> bool {
        self.hide_ui_for_capture
    }

    /// 渲染截图内容
    pub fn render(&mut self, d2d_renderer: &mut Direct2DRenderer) -> Result<(), ScreenshotError> {
        d2d_renderer.draw_background_bitmap_fullscreen();
        Ok(())
    }

    /// 重置状态
    pub fn reset_state(&mut self, screen_size: (i32, i32)) {
        // 清除当前截图
        self.current_screenshot = None;

        // 重置选择状态
        self.selection.reset();

        // 工具状态由Drawing模块管理

        // 重置屏幕尺寸（如果之前被pin功能修改过）
        let (w, h) = screen_size;
        self.screen_width = w;
        self.screen_height = h;

        // 重置UI隐藏状态
        self.hide_ui_for_capture = false;

        // 默认显示选择框手柄（新一轮截图开始时应可见）
        self.show_selection_handles = true;

        // 重新启用自动窗口高亮功能
        self.auto_highlight.reset();
    }

    /// 设置当前窗口句柄（用于排除自己的窗口）
    pub fn set_current_window(&mut self, window: WindowId) {
        self.current_window = window;
    }

    // 注意: 绘图工具管理已移至Drawing模块

    /// 重新截取当前屏幕
    pub fn capture_screen(
        &mut self,
        screen_size: (i32, i32),
    ) -> std::result::Result<(), ScreenshotError> {
        self.capture_screen_with_exclude_window(screen_size, self.current_window)
    }

    /// 重新截取当前屏幕（用于排除某个窗口）
    pub fn capture_screen_with_exclude_window(
        &mut self,
        screen_size: (i32, i32),
        _exclude_window: WindowId,
    ) -> std::result::Result<(), ScreenshotError> {
        // 获取当前屏幕尺寸（可能在pin后发生了变化）
        let (current_screen_width, current_screen_height) = screen_size;

        // 更新屏幕尺寸
        self.screen_width = current_screen_width;
        self.screen_height = current_screen_height;

        // 标记截图数据已更新（实际捕获将按需进行）
        self.current_screenshot = Some(ScreenshotData {
            width: self.screen_width as u32,
            height: self.screen_height as u32,
            data: vec![], // 暂时为空，实际数据将按需捕获
        });

        // 刷新窗口列表
        if let Err(e) = self.auto_highlight.refresh_windows() {
            eprintln!("Warning: Failed to refresh windows: {e:?}");
            // 继续运行，不退出程序
        }

        Ok(())
    }

    /// 捕获屏幕并创建 D2D 位图
    pub fn capture_screen_to_d2d_bitmap(
        &mut self,
        d2d_renderer: &mut Direct2DRenderer,
    ) -> std::result::Result<(), ScreenshotError> {
        let screen_rect = Rect {
            left: 0,
            top: 0,
            right: self.screen_width,
            bottom: self.screen_height,
        };

        let (d2d_bitmap, bmp_data) = d2d_renderer
            .capture_screen_region_to_d2d_bitmap_and_bmp_data(screen_rect)
            .map_err(|e| {
                ScreenshotError::RenderError(format!("Failed to create D2D bitmap: {e:?}"))
            })?;

        if let Some(bmp_data) = bmp_data
            && let Some(ref mut screenshot) = self.current_screenshot
        {
            screenshot.data = bmp_data;
        }

        d2d_renderer.set_background_bitmap(d2d_bitmap);
        Ok(())
    }

    /// 获取当前截图的原始图像数据（如果可用）
    pub fn get_current_image_data(&self) -> Option<&[u8]> {
        self.current_screenshot.as_ref().map(|s| s.data.as_slice())
    }

    /// 处理鼠标移动（包含拖拽检测）
    /// 返回 (命令列表, 是否消费了事件)
    pub fn handle_mouse_move(
        &mut self,
        x: i32,
        y: i32,
        selection_rect: Option<core_selection::RectI32>,
        hover_selection: Option<core_selection::RectI32>,
    ) -> (Vec<Command>, bool) {
        // 绘图工具处理已移至Drawing模块

        // 拖拽已有选择框：几何更新由 core 执行。
        if self.selection.is_interacting() {
            return (
                vec![
                    Command::Core(sc_app::Action::Selection(
                        core_selection::Action::EditDragMove { x, y },
                    )),
                    Command::RequestRedraw,
                ],
                true,
            );
        }

        // 鼠标按下（尚未有 confirmed selection）期间：始终把 MouseMove 交给 core。
        // core 负责：
        // - auto-highlight 模式下的 drag-threshold gating（避免小抖动误显示 drag box）
        // - drag-create 的 selection rect 几何演算
        if self.selection.is_mouse_pressed() && selection_rect.is_none() {
            return (
                vec![
                    Command::Core(sc_app::Action::Selection(
                        core_selection::Action::MouseMove { x, y },
                    )),
                    Command::RequestRedraw,
                ],
                true,
            );
        }

        // Hover highlight（auto-highlight）：只在非按下/非交互时更新。
        let action = self
            .auto_highlight
            .handle_mouse_move(AutoHighlightMoveArgs {
                x,
                y,
                screen_width: self.screen_width,
                screen_height: self.screen_height,
                current_highlight: hover_selection,
                selecting: false,
                interacting: false,
            });

        match action {
            AutoHighlightMoveAction::SetHighlight(target) => (
                vec![
                    Command::Core(sc_app::Action::Selection(
                        core_selection::Action::SetHoverSelection {
                            selection: Some(target.rect),
                        },
                    )),
                    Command::Core(sc_app::Action::Selection(
                        core_selection::Action::SetAutoHighlightActive { active: true },
                    )),
                    Command::RequestRedraw,
                ],
                true,
            ),
            AutoHighlightMoveAction::ClearHighlight => (
                vec![
                    Command::Core(sc_app::Action::Selection(
                        core_selection::Action::SetHoverSelection { selection: None },
                    )),
                    Command::Core(sc_app::Action::Selection(
                        core_selection::Action::SetAutoHighlightActive { active: false },
                    )),
                    Command::RequestRedraw,
                ],
                true,
            ),

            AutoHighlightMoveAction::None => (vec![], false),
        }
    }

    /// 处理鼠标按下
    /// 返回 (命令列表, 是否消费了事件)
    pub fn handle_mouse_down(
        &mut self,
        x: i32,
        y: i32,
        selection_rect: Option<core_selection::RectI32>,
        has_auto_highlight: bool,
    ) -> (Vec<Command>, bool) {
        if self.current_screenshot.is_none() {
            return (vec![], false);
        }

        // 设置鼠标按下状态
        self.selection.set_mouse_pressed(true);

        // 绘图工具和元素点击检查已移至Drawing模块

        // 当自动高亮处于 hover 状态时：
        // - 单击：确认当前高亮（在 mouse_up 中完成）
        // - 拖拽：超过阈值后转为手动框选（在 mouse_move 中完成）
        // 这里不要直接进入“移动/缩放高亮框”的交互，否则会导致无法自定义框选。
        if has_auto_highlight {
            return (vec![], true);
        }

        // 第三优先级：检查工具栏点击
        if selection_rect.is_some() {
            // 工具栏点击检测需要通过UI管理器处理
            // 这里暂时跳过，让UI管理器在App层处理工具栏点击
        }

        // 检查手柄/内部点击：通过统一交互控制器
        if selection_rect.is_some() {
            let drag_mode = self.selection.get_handle_at_position(selection_rect, x, y);
            if drag_mode != DragMode::None {
                self.selection.start_interaction(x, y, drag_mode);
                return (
                    vec![
                        Command::Core(sc_app::Action::Selection(
                            core_selection::Action::BeginEditDrag { drag_mode, x, y },
                        )),
                        Command::RequestRedraw,
                    ],
                    true,
                );
            }

            // 有选区但未命中手柄/内部：保持原行为，忽略此次点击
            self.selection.set_mouse_pressed(false);
            return (vec![], false);
        }

        // 第四优先级：开始新的选择框创建
        // 修复：当启用自动高亮时，不要立即开始选择框创建，
        // 等待在 handle_mouse_move 中超过拖拽阈值后再开始，
        // 以避免小幅移动被误判为“单击确认”。
        if !self.auto_highlight.enabled() {
            // 开始新的手动框选：drag-create 的几何由 core 计算。
            (
                vec![
                    Command::Core(sc_app::Action::Selection(
                        core_selection::Action::SetHoverSelection { selection: None },
                    )),
                    Command::Core(sc_app::Action::Selection(
                        core_selection::Action::SetAutoHighlightActive { active: false },
                    )),
                    // Seed initial selection so UI can immediately derive from core state.
                    Command::Core(sc_app::Action::Selection(
                        core_selection::Action::MouseMove { x, y },
                    )),
                    Command::RequestRedraw,
                ],
                true,
            )
        } else {
            (vec![], false)
        }
    }

    /// 处理鼠标释放
    /// 返回 (命令列表, 是否消费了事件)
    pub fn handle_mouse_up(
        &mut self,
        _x: i32,
        _y: i32,
        selection_rect: Option<core_selection::RectI32>,
    ) -> (Vec<Command>, bool) {
        let mut commands = Vec::new();

        // 绘图工具完成处理已移至Drawing模块

        // 处理选择框创建和拖拽结束
        let is_manual_selecting =
            !self.auto_highlight.enabled() && self.selection.is_mouse_pressed() && selection_rect.is_none();

        if is_manual_selecting {
            // 结束手动框选：确认规则完全由 core 决策；host 仅请求重绘。
            commands.push(Command::RequestRedraw);
        } else if self.selection.is_interacting() {
            self.selection.end_interaction();
            commands.push(Command::RequestRedraw);

            commands.push(Command::Core(sc_app::Action::Selection(
                core_selection::Action::EndEditDrag,
            )));
        }

        // 重置鼠标按下状态
        self.selection.set_mouse_pressed(false);

        let consumed = !commands.is_empty();
        (commands, consumed)
    }

    pub fn handle_auto_highlight_mouse_up(
        &mut self,
        is_click: bool,
        selection_has_selection: bool,
        had_active_highlight: bool,
    ) -> bool {
        self.auto_highlight
            .handle_mouse_up(is_click, selection_has_selection, had_active_highlight)
    }

    /// 处理双击事件
    pub fn handle_double_click(
        &mut self,
        _x: i32,
        _y: i32,
        selection_rect: Option<core_selection::RectI32>,
    ) -> Vec<Command> {
        // 双击可能用于确认选择或快速操作
        // 如果有选择区域，双击可能表示确认选择
        if selection_rect.is_some() {
            // 走 core: SaveSelectionToClipboard（由宿主命令执行隐藏窗口/重置）
            vec![Command::Core(sc_app::Action::SaveSelectionToClipboard)]
        } else {
            vec![]
        }
    }
}

/// 截图错误类型
#[derive(Debug)]
pub enum ScreenshotError {
    /// 捕获失败
    CaptureError(String),
    /// 保存失败
    SaveError(String),
    /// 初始化失败
    InitError(String),
    /// 渲染失败
    RenderError(String),
    /// 系统错误
    SystemError(SystemError),
}

impl From<SystemError> for ScreenshotError {
    fn from(error: SystemError) -> Self {
        ScreenshotError::SystemError(error)
    }
}

impl From<sc_highlight::WindowDetectionError> for ScreenshotError {
    fn from(error: sc_highlight::WindowDetectionError) -> Self {
        ScreenshotError::SystemError(SystemError::WindowDetectionError(error.to_string()))
    }
}

impl std::fmt::Display for ScreenshotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScreenshotError::CaptureError(msg) => write!(f, "Capture error: {msg}"),
            ScreenshotError::SaveError(msg) => write!(f, "Save error: {msg}"),
            ScreenshotError::InitError(msg) => write!(f, "Init error: {msg}"),
            ScreenshotError::RenderError(msg) => write!(f, "Render error: {msg}"),
            ScreenshotError::SystemError(err) => write!(f, "System error: {err}"),
        }
    }
}

impl std::error::Error for ScreenshotError {}

impl ScreenshotManager {
    /// 代理选择状态的手柄命中检测（方便App层统一处理光标）
    pub fn get_handle_at_position(
        &self,
        selection_rect: Option<core_selection::RectI32>,
        x: i32,
        y: i32,
    ) -> DragMode {
        self.selection.get_handle_at_position(selection_rect, x, y)
    }
}

impl ScreenshotManager {
    /// 获取选区图像数据（用于前置条件检查）
    pub fn get_selection_image(
        &self,
        selection_rect: Option<core_selection::RectI32>,
    ) -> Option<Vec<u8>> {
        if selection_rect.is_some() && self.has_screenshot() {
            self.current_screenshot.as_ref().map(|s| s.data.clone())
        } else {
            None
        }
    }
}
