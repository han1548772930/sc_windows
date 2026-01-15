use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use anyhow::Result;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Dwm::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::*;
use windows::Win32::UI::HiDpi::{
    GetDpiForWindow, GetSystemMetricsForDpi, PROCESS_PER_MONITOR_DPI_AWARE, SetProcessDpiAwareness,
};
use windows::Win32::UI::WindowsAndMessaging::*;

use sc_app::selection::RectI32;
use sc_drawing_host::{DragMode, DrawingConfig, DrawingTool};
use sc_ocr::OcrResult;
use sc_platform::{CursorIcon, HostPlatform, WindowId};
use sc_platform_windows::windows::WindowsHostPlatform;
use sc_settings::Settings;
use sc_ui::preview_layout;

use super::drawing::PreviewDrawingState;
use super::hit_test::icon_contains_hover_point;
use super::renderer::PreviewRenderer;
use super::types::{Margins, SvgIcon};
use crate::constants::{
    ICON_SIZE, OCR_CONTENT_PADDING_BOTTOM, OCR_CONTENT_PADDING_TOP, OCR_CONTENT_PADDING_X,
    OCR_PANEL_GAP, OCR_TEXT_LINE_HEIGHT, OCR_TEXT_PANEL_WIDTH, TITLE_BAR_HEIGHT,
};

/// 预览显示窗口 (支持 OCR 结果和 Pin 模式)
pub struct PreviewWindow;

pub(super) const WM_APP_PREVIEW_OCR_DONE: u32 = WM_APP + 300;

#[derive(Debug, Clone)]
struct OcrResponse {
    text: String,
}

static PREVIEW_HWND: OnceLock<Mutex<Option<WindowId>>> = OnceLock::new();
static OCR_RESPONSES: OnceLock<Mutex<HashMap<u64, OcrResponse>>> = OnceLock::new();
static OCR_REQUEST_COUNTER: AtomicU64 = AtomicU64::new(1);

fn next_ocr_request_id() -> u64 {
    OCR_REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed)
}

fn preview_hwnd_store() -> &'static Mutex<Option<WindowId>> {
    PREVIEW_HWND.get_or_init(|| Mutex::new(None))
}

fn ocr_response_store() -> &'static Mutex<HashMap<u64, OcrResponse>> {
    OCR_RESPONSES.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(super) struct PreviewWindowState {
    pub(super) hwnd: HWND,
    // 原始图像数据，用于 D2D 位图创建
    pub(super) image_pixels: Vec<u8>,
    pub(super) image_width: i32,
    pub(super) image_height: i32,

    pub(super) text_area_rect: RectI32,
    pub(super) window_width: i32,
    pub(super) window_height: i32,
    pub(super) is_maximized: bool,
    pub(super) svg_icons: Vec<SvgIcon>,

    // 自绘文本相关
    pub(super) text_content: String,
    pub(super) scroll_offset: i32,
    pub(super) line_height: i32,
    pub(super) text_lines: Vec<String>,

    // 文本选择相关
    pub(super) is_selecting: bool,
    pub(super) selection_start: Option<(usize, usize)>,
    pub(super) selection_end: Option<(usize, usize)>,

    // 置顶/Pin 状态
    pub(super) is_pinned: bool,
    pub(super) show_text_area: bool,

    // OCR (triggered from within the preview window)
    pub(super) ocr_source_bmp_data: Vec<u8>,
    pub(super) ocr_cached_text: Option<String>,
    pub(super) ocr_in_flight: bool,
    pub(super) ocr_request_id: u64,

    // Direct2D 渲染器
    pub(super) renderer: Option<PreviewRenderer>,

    // 绘图功能
    pub(super) drawing_state: Option<PreviewDrawingState>,
}

impl PreviewWindow {
    pub fn show(
        image_data: Vec<u8>,
        ocr_results: Vec<OcrResult>,
        selection_rect: RectI32,
        is_pin_mode: bool,
        drawing_config: DrawingConfig,
        ocr_source_bmp_data: Option<Vec<u8>>,
    ) -> Result<()> {
        PreviewWindowState::show(
            image_data,
            ocr_results,
            selection_rect,
            is_pin_mode,
            drawing_config,
            ocr_source_bmp_data,
        )
    }
}

impl PreviewWindowState {
    pub(super) fn window_id(&self) -> WindowId {
        sc_platform_windows::windows::window_id(self.hwnd)
    }

    pub(super) fn min_window_width_for_title_bar() -> i32 {
        // Ensure the window is wide enough to fit all left title-bar icons and the 3 right-side
        // title-bar buttons, plus a small gap between the groups.
        let left_max_right = preview_layout::create_left_icons()
            .iter()
            .map(|i| i.rect.right)
            .max()
            .unwrap_or(0);

        let right_buttons_width = 3 * crate::constants::BUTTON_WIDTH_OCR;

        // Extra breathing room between the left icon group and the right window buttons.
        let gap = crate::constants::LEFT_ICON_SPACING;

        left_max_right + gap + right_buttons_width
    }

    fn derive_text_panel_state(
        ocr_results: &[OcrResult],
        is_pin_mode: bool,
    ) -> (String, bool, Option<String>) {
        let text_content = sc_ocr::join_result_texts_trimmed(ocr_results);
        let show_text_area = !is_pin_mode || !text_content.is_empty();
        let ocr_cached_text = if text_content.is_empty() {
            None
        } else {
            Some(text_content.clone())
        };
        (text_content, show_text_area, ocr_cached_text)
    }

    fn compute_window_size(image_width: i32, image_height: i32, show_text_area: bool) -> (i32, i32) {
        if show_text_area {
            let text_area_width = OCR_TEXT_PANEL_WIDTH;
            let image_area_width = image_width + OCR_CONTENT_PADDING_X * 2;
            let margin = OCR_PANEL_GAP;
            let content_padding_top = OCR_CONTENT_PADDING_TOP;
            let content_padding_bottom = OCR_CONTENT_PADDING_BOTTOM;
            (
                image_area_width + text_area_width + margin,
                TITLE_BAR_HEIGHT + content_padding_top + image_height + content_padding_bottom,
            )
        } else {
            (image_width, TITLE_BAR_HEIGHT + image_height)
        }
    }

    fn compute_window_position(
        selection_rect: RectI32,
        window_width: i32,
        window_height: i32,
        is_pin_mode: bool,
        screen_size: (i32, i32),
    ) -> (i32, i32) {
        let (screen_width, screen_height) = screen_size;
        let mut window_x = selection_rect.right + 20;
        let mut window_y = selection_rect.top;

        if is_pin_mode {
            window_x = selection_rect.left;
            window_y = selection_rect.top;
        } else {
            if window_x + window_width > screen_width {
                window_x = selection_rect.left - window_width - 20;
                if window_x < 0 {
                    window_x = 50;
                }
            }
            if window_y + window_height > screen_height {
                window_y = screen_height - window_height - 50;
                if window_y < 0 {
                    window_y = 50;
                }
            }
        }

        if window_x + window_width > screen_width {
            window_x = screen_width - window_width;
        }
        if window_y + window_height > screen_height {
            window_y = screen_height - window_height;
        }
        window_x = window_x.max(0);
        window_y = window_y.max(0);
        (window_x, window_y)
    }

    fn existing_hwnd() -> Option<HWND> {
        let window_id = preview_hwnd_store()
            .lock()
            .map(|g| *g)
            .unwrap_or_else(|e| *e.into_inner());

        window_id.map(sc_platform_windows::windows::hwnd)
    }

    fn set_singleton_hwnd(hwnd: HWND) {
        let mut guard = preview_hwnd_store()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        *guard = Some(sc_platform_windows::windows::window_id(hwnd));
    }

    pub(super) fn clear_singleton_hwnd(hwnd: HWND) {
        let target_id = sc_platform_windows::windows::window_id(hwnd);
        let mut guard = preview_hwnd_store()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if (*guard).map_or(false, |id| id == target_id) {
            *guard = None;
        }
    }

    fn take_ocr_response(request_id: u64) -> Option<OcrResponse> {
        let mut guard = ocr_response_store()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        guard.remove(&request_id)
    }

    pub(super) fn clear_ocr_responses() {
        let mut guard = ocr_response_store()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        guard.clear();
    }


    fn cursor_from_drag_mode(drag_mode: DragMode) -> Option<CursorIcon> {
        match drag_mode {
            DragMode::ResizingTopLeft | DragMode::ResizingBottomRight => Some(CursorIcon::SizeNWSE),
            DragMode::ResizingTopRight | DragMode::ResizingBottomLeft => Some(CursorIcon::SizeNESW),
            DragMode::ResizingTopCenter | DragMode::ResizingBottomCenter => {
                Some(CursorIcon::SizeNS)
            }
            DragMode::ResizingMiddleLeft | DragMode::ResizingMiddleRight => {
                Some(CursorIcon::SizeWE)
            }
            DragMode::Moving => Some(CursorIcon::SizeAll),
            _ => None,
        }
    }

    fn determine_cursor(&self, x: i32, y: i32) -> CursorIcon {
        // 1) Title-bar icons
        if (0..=TITLE_BAR_HEIGHT).contains(&y) {
            if self
                .svg_icons
                .iter()
                .any(|icon| icon_contains_hover_point(icon, x, y))
            {
                return CursorIcon::Hand;
            }
            return CursorIcon::Arrow;
        }

        // 2) OCR text area: use I-beam for selection/copying
        if self.show_text_area {
            let r = self.text_area_rect;
            if x >= r.left && x <= r.right && y >= r.top && y <= r.bottom {
                return CursorIcon::IBeam;
            }
        }

        // 3) Drawing area (image rect)
        if let Some(ds) = &self.drawing_state {
            let manager = &ds.manager;
            let inside_image = ds.is_in_image_area(x, y);

            // 3.1) Dragging/resizing in progress
            if let Some(drag_mode) = manager.get_current_drag_mode() {
                return Self::cursor_from_drag_mode(drag_mode).unwrap_or(CursorIcon::SizeAll);
            }

            // 3.2) Text editing mode
            if manager.is_text_editing() {
                if let Some(edit_idx) = manager.get_editing_element_index()
                    && let Some(element) = manager.get_element_ref(edit_idx)
                {
                    let handle_mode = manager.get_element_handle_at_position(
                        x,
                        y,
                        &element.rect,
                        element.tool,
                        edit_idx,
                    );
                    return Self::cursor_from_drag_mode(handle_mode).unwrap_or(CursorIcon::IBeam);
                }
                return CursorIcon::IBeam;
            }

            // 3.3) Selected element handles / move
            if let Some(sel_idx) = manager.get_selected_element_index()
                && let Some(element) = manager.get_element_ref(sel_idx)
            {
                let handle_mode = manager.get_element_handle_at_position(
                    x,
                    y,
                    &element.rect,
                    element.tool,
                    sel_idx,
                );
                if handle_mode != DragMode::None {
                    return Self::cursor_from_drag_mode(handle_mode).unwrap_or(CursorIcon::Arrow);
                }

                if element.contains_point(x, y) {
                    // Text is draggable but we keep arrow to avoid implying edit-on-click.
                    return if element.tool == DrawingTool::Text {
                        CursorIcon::Arrow
                    } else {
                        CursorIcon::SizeAll
                    };
                }
            }

            // 3.4) Tool cursor inside image area
            match manager.get_current_tool() {
                DrawingTool::Pen
                | DrawingTool::Rectangle
                | DrawingTool::Circle
                | DrawingTool::Arrow => {
                    return if inside_image {
                        CursorIcon::Crosshair
                    } else {
                        CursorIcon::Arrow
                    };
                }
                DrawingTool::Text => {
                    return if inside_image {
                        CursorIcon::IBeam
                    } else {
                        CursorIcon::Arrow
                    };
                }
                DrawingTool::None => {
                    return CursorIcon::Arrow;
                }
            }
        }

        CursorIcon::Arrow
    }

    pub(super) fn update_cursor(&self, x: i32, y: i32) {
        WindowsHostPlatform::new().set_cursor(self.determine_cursor(x, y));
    }

    pub(super) fn get_frame_thickness(hwnd: HWND) -> i32 {
        unsafe {
            let dpi = GetDpiForWindow(hwnd);
            let resize_frame = GetSystemMetricsForDpi(SM_CXSIZEFRAME, dpi);
            let padding = GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi);
            resize_frame + padding
        }
    }

    pub(super) fn cleanup_all_resources(&mut self) {
        self.svg_icons.clear();
    }

    fn create_left_icons() -> Vec<SvgIcon> {
        preview_layout::create_left_icons()
            .into_iter()
            .map(|icon| SvgIcon {
                name: icon.name.to_string(),
                rect: icon.rect,
                hovered: false,
                selected: false,
                is_title_bar_button: icon.is_title_bar_button(),
            })
            .collect()
    }

    fn update_tool_icons(&mut self) {
        let current_tool = self
            .drawing_state
            .as_ref()
            .map(|ds| ds.get_current_tool())
            .unwrap_or(DrawingTool::None);

        for icon in &mut self.svg_icons {
            let is_drawing_tool_icon = matches!(
                icon.name.as_str(),
                preview_layout::ICON_TOOL_SQUARE
                    | preview_layout::ICON_TOOL_CIRCLE
                    | preview_layout::ICON_TOOL_ARROW
                    | preview_layout::ICON_TOOL_PEN
                    | preview_layout::ICON_TOOL_TEXT
            );

            if is_drawing_tool_icon {
                icon.selected = matches!(
                    (icon.name.as_str(), current_tool),
                    (preview_layout::ICON_TOOL_SQUARE, DrawingTool::Rectangle)
                        | (preview_layout::ICON_TOOL_CIRCLE, DrawingTool::Circle)
                        | (preview_layout::ICON_TOOL_ARROW, DrawingTool::Arrow)
                        | (preview_layout::ICON_TOOL_PEN, DrawingTool::Pen)
                        | (preview_layout::ICON_TOOL_TEXT, DrawingTool::Text)
                );
                continue;
            }

            // OCR icon: selected means the right-side text panel is visible.
            if icon.name == preview_layout::ICON_OCR {
                icon.selected = self.show_text_area;
            }
        }
    }

    fn reset_text_selection(&mut self) {
        self.is_selecting = false;
        self.selection_start = None;
        self.selection_end = None;
    }

    fn refresh_text_lines(&mut self) {
        if let Some(renderer) = &self.renderer {
            let width = (self.text_area_rect.right - self.text_area_rect.left) as f32;
            self.text_lines = renderer.split_text_into_lines(&self.text_content, width);
        } else {
            self.text_lines = vec![self.text_content.clone()];
        }
    }
    pub(super) fn switch_drawing_tool(&mut self, tool: DrawingTool) {
        if let Some(ref mut ds) = self.drawing_state {
            ds.switch_tool(tool);
        }
        self.update_tool_icons();
    }

    pub(super) fn toggle_ocr_text_panel(&mut self) {
        // Reset text selection state when toggling panel visibility.
        self.reset_text_selection();

        if self.show_text_area {
            // Hide panel (keep cached OCR).
            self.show_text_area = false;
        } else {
            // Show panel.
            self.show_text_area = true;

            // Prefer cached OCR results.
            if let Some(cached) = self.ocr_cached_text.clone() {
                self.text_content = cached;
            } else if !self.ocr_in_flight {
                // No cache: start OCR.
                self.text_content = "识别中...".to_string();
                self.text_lines = vec![self.text_content.clone()];
                self.start_ocr_async();
            }
        }

        // Resize to match the two modes (same as initial window sizing in `show`).
        // If the window is maximized, keep the current size and only toggle layout.
        if !self.is_maximized {
            let (mut new_width, new_height) = Self::compute_window_size(
                self.image_width,
                self.image_height,
                self.show_text_area,
            );

            new_width = new_width.max(Self::min_window_width_for_title_bar());

            self.window_width = new_width;
            self.window_height = new_height;

            unsafe {
                let _ = SetWindowPos(
                    self.hwnd,
                    None,
                    0,
                    0,
                    new_width,
                    new_height,
                    SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
        }

        self.update_title_bar_buttons();
        self.recalculate_layout();

        // Always refresh text wrapping when showing the panel (even if the rect didn't change).
        if self.show_text_area {
            self.refresh_text_lines();
        }

        self.update_tool_icons();
    }

    fn start_ocr_async(&mut self) {
        // Mark in-flight and assign a monotonically-increasing request id so stale results can be ignored.
        self.ocr_in_flight = true;
        let request_id = next_ocr_request_id();
        self.ocr_request_id = request_id;

        let window_id = self.window_id();
        let image_data = self.ocr_source_bmp_data.clone();

        std::thread::spawn(move || {
            let text = Self::run_ocr_in_background(&image_data);

            {
                let mut guard = ocr_response_store()
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                guard.insert(request_id, OcrResponse { text });
            }

            let hwnd = sc_platform_windows::windows::hwnd(window_id);
            let posted = unsafe {
                PostMessageW(
                    Some(hwnd),
                    WM_APP_PREVIEW_OCR_DONE,
                    WPARAM(request_id as usize),
                    LPARAM(0),
                )
                .is_ok()
            };

            // If the window is gone, drop the cached response to avoid leaking.
            if !posted {
                let mut guard = ocr_response_store()
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                let _ = guard.remove(&request_id);
            }
        });
    }

    fn run_ocr_in_background(image_data: &[u8]) -> String {
        // Load settings (language selection).
        let settings = Settings::load();
        let config = sc_ocr::OcrConfig::new(sc_ocr::DEFAULT_MODELS_DIR, settings.ocr_language);

        if !sc_ocr::models_exist(&config) {
            return "OCR 引擎不可用（缺少模型文件）".to_string();
        }

        let engine = match sc_ocr::create_engine(&config) {
            Ok(engine) => engine,
            Err(e) => return format!("OCR 引擎启动失败: {e}"),
        };

        match sc_ocr::recognize_from_memory(&engine, image_data) {
            Ok(results) => {
                let text = sc_ocr::join_result_texts_trimmed(&results);
                if text.trim().is_empty() {
                    sc_ocr::OCR_NO_TEXT_PLACEHOLDER.to_string()
                } else {
                    text
                }
            }
            Err(e) => format!("OCR识别失败: {e}"),
        }
    }

    pub(super) fn handle_ocr_done_message(&mut self, request_id: u64) {
        let Some(resp) = Self::take_ocr_response(request_id) else {
            return;
        };

        // Ignore stale results (window content may have changed).
        if request_id != self.ocr_request_id {
            return;
        }

        self.ocr_in_flight = false;
        self.ocr_cached_text = Some(resp.text.clone());
        self.text_content = resp.text;

        // Reset selection/scroll when new OCR text arrives.
        self.scroll_offset = 0;
        self.reset_text_selection();

        if self.show_text_area {
            self.refresh_text_lines();

            let window_id = self.window_id();
            let rect = self.text_area_rect;
            let _ = WindowsHostPlatform::new().request_redraw_rect(
                window_id,
                rect.left,
                rect.top,
                rect.right,
                rect.bottom,
            );
        }
    }

    pub(super) fn save_image_to_file(&mut self) {
        let window_id = self.window_id();
        let platform = WindowsHostPlatform::new();

        // NOTE: current export format is BMP bytes.
        let file_path = match platform.show_image_save_dialog(window_id, "screenshot.bmp") {
            Ok(path) => path,
            Err(e) => {
                let msg = format!("无法打开保存对话框: {e}");
                platform.show_error_message(window_id, "保存失败", &msg);
                return;
            }
        };

        let Some(file_path) = file_path else {
            // User cancelled.
            return;
        };

        let image_area_rect = self
            .drawing_state
            .as_ref()
            .map(|ds| ds.image_area_rect)
            .unwrap_or(RectI32 {
                left: 0,
                top: TITLE_BAR_HEIGHT,
                right: self.image_width,
                bottom: TITLE_BAR_HEIGHT + self.image_height,
            });

        let Some(renderer) = self.renderer.as_mut() else {
            platform.show_error_message(window_id, "保存失败", "渲染器未初始化");
            return;
        };

        if let Err(e) =
            renderer.set_image_from_pixels(&self.image_pixels, self.image_width, self.image_height)
        {
            let msg = format!("初始化位图失败: {e:?}");
            platform.show_error_message(window_id, "保存失败", &msg);
            return;
        }

        let bmp_data =
            match renderer.render_image_area_to_bmp(image_area_rect, self.drawing_state.as_mut()) {
                Ok(data) => data,
                Err(e) => {
                    let msg = format!("导出图片失败: {e:?}");
                    platform.show_error_message(window_id, "保存失败", &msg);
                    return;
                }
            };

        if let Err(e) = std::fs::write(&file_path, &bmp_data) {
            let msg = format!("写入文件失败: {e}");
            platform.show_error_message(window_id, "保存失败", &msg);
        }
    }

    fn center_icons_with<F>(&mut self, should_center: F)
    where
        F: Fn(&SvgIcon) -> bool,
    {
        let icon_y = (TITLE_BAR_HEIGHT - ICON_SIZE) / 2;

        for icon in &mut self.svg_icons {
            if !should_center(icon) {
                continue;
            }
            let icon_height = icon.rect.bottom - icon.rect.top;
            icon.rect.top = icon_y;
            icon.rect.bottom = icon_y + icon_height;
        }
    }

    fn center_icons(&mut self) {
        self.center_icons_with(|_| true);
    }

    pub(super) fn update_title_bar_buttons(&mut self) {
        let window_width = self.window_width;

        // 移除旧的标题栏按钮，保留左侧图标
        self.svg_icons.retain(|icon| !icon.is_title_bar_button);

        // 确保有左侧图标
        let has_left_icons = self.svg_icons.iter().any(|icon| !icon.is_title_bar_button);
        if !has_left_icons {
            let mut left_icons = Self::create_left_icons();
            self.svg_icons.append(&mut left_icons);
        }

        // 更新所有左侧图标的位置
        self.center_icons_with(|icon| !icon.is_title_bar_button);

        // 创建新的标题栏按钮
        let mut title_bar_buttons = Self::create_title_bar_buttons(window_width, self.is_maximized);
        self.svg_icons.append(&mut title_bar_buttons);

        // Keep selected states in sync after rebuilding icons.
        self.update_tool_icons();
    }

    fn create_title_bar_buttons(window_width: i32, is_maximized: bool) -> Vec<SvgIcon> {
        preview_layout::create_title_bar_buttons(window_width, is_maximized)
            .into_iter()
            .map(|icon| SvgIcon {
                name: icon.name.to_string(),
                rect: icon.rect,
                hovered: false,
                selected: false,
                is_title_bar_button: icon.is_title_bar_button(),
            })
            .collect()
    }

    fn build_icon_set(window_width: i32, is_maximized: bool) -> Vec<SvgIcon> {
        let mut icons = Self::create_left_icons();
        let mut title_bar_buttons = Self::create_title_bar_buttons(window_width, is_maximized);
        icons.append(&mut title_bar_buttons);
        icons
    }

    unsafe fn update_existing_window(
        existing_hwnd: HWND,
        image_data: Vec<u8>,
        ocr_results: Vec<OcrResult>,
        selection_rect: RectI32,
        is_pin_mode: bool,
        drawing_config: DrawingConfig,
        ocr_source_bmp_data: Option<Vec<u8>>,
    ) -> Result<()> {
        if existing_hwnd.0.is_null() || !unsafe { IsWindow(Some(existing_hwnd)).as_bool() } {
            return Err(anyhow::anyhow!("existing preview hwnd is invalid"));
        }

        let window_ptr =
            unsafe { GetWindowLongPtrW(existing_hwnd, GWLP_USERDATA) as *mut PreviewWindowState };
        if window_ptr.is_null() {
            return Err(anyhow::anyhow!("existing preview window state is null"));
        }

        // 解析图片与计算尺寸
        let (image_pixels, actual_width, actual_height) = Self::parse_bmp_data(&image_data)?;

        let (text_content, show_text_area, ocr_cached_text) =
            Self::derive_text_panel_state(&ocr_results, is_pin_mode);

        // 布局计算
        let (mut window_width, window_height) =
            Self::compute_window_size(actual_width, actual_height, show_text_area);

        window_width = window_width.max(Self::min_window_width_for_title_bar());

        // 计算位置
        let screen_size = WindowsHostPlatform::new().screen_size();
        let (window_x, window_y) = Self::compute_window_position(
            selection_rect,
            window_width,
            window_height,
            is_pin_mode,
            screen_size,
        );

        let window_id = sc_platform_windows::windows::window_id(existing_hwnd);
        let platform = WindowsHostPlatform::new();

        // Update topmost flag for pin mode.
        let _ = platform.set_window_topmost_flag(window_id, is_pin_mode);

        // Resize/move the window.
        let _ = unsafe {
            SetWindowPos(
                existing_hwnd,
                None,
                window_x,
                window_y,
                window_width,
                window_height,
                SWP_NOZORDER | SWP_NOACTIVATE,
            )
        };

        // Update in-place.
        // SAFETY: we validated `window_ptr` is non-null above and it is owned by this window.
        let window = unsafe { &mut *window_ptr };

        window.image_pixels = image_pixels;
        window.image_width = actual_width;
        window.image_height = actual_height;

        window.text_area_rect = RectI32 {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        window.window_width = window_width;
        window.window_height = window_height;
        window.scroll_offset = 0;
        window.text_lines.clear();
        window.reset_text_selection();

        window.is_pinned = is_pin_mode;
        window.show_text_area = show_text_area;

        // OCR state (new content resets cache).
        window.ocr_request_id = next_ocr_request_id();
        window.ocr_in_flight = false;
        window.ocr_source_bmp_data = ocr_source_bmp_data.unwrap_or(image_data);
        window.ocr_cached_text = ocr_cached_text;

        window.text_content = text_content;

        // Reset icons.
        window.svg_icons = Self::build_icon_set(window_width, window.is_maximized);
        window.update_tool_icons();

        // Reset drawing state.
        window.drawing_state = PreviewDrawingState::new(window_id, drawing_config).ok();

        // Re-init renderer state for the new image.
        if let Some(renderer) = &mut window.renderer {
            renderer.image_bitmap = None;
            let _ = renderer.initialize(window_id, window_width, window_height);
            let _ = renderer.set_image_from_pixels(
                &window.image_pixels,
                window.image_width,
                window.image_height,
            );
        }

        // Recompute layout and text wrapping.
        window.center_icons();
        window.recalculate_layout();

        if window.show_text_area {
            window.refresh_text_lines();
        }

        let _ = platform.show_window(window_id);
        let _ = platform.update_window(window_id);
        let _ = platform.request_redraw(window_id);

        Ok(())
    }

    pub(super) fn show(
        image_data: Vec<u8>,
        ocr_results: Vec<OcrResult>,
        selection_rect: RectI32,
        is_pin_mode: bool,
        drawing_config: DrawingConfig,
        ocr_source_bmp_data: Option<Vec<u8>>,
    ) -> Result<()> {
        unsafe {
            let _ = SetProcessDpiAwareness(PROCESS_PER_MONITOR_DPI_AWARE);

            // If we already have a live preview window, update it in-place.
            if let Some(existing_hwnd) = Self::existing_hwnd() {
                if !existing_hwnd.0.is_null() && IsWindow(Some(existing_hwnd)).as_bool() {
                    let window_ptr =
                        GetWindowLongPtrW(existing_hwnd, GWLP_USERDATA) as *mut PreviewWindowState;
                    if !window_ptr.is_null() {
                        Self::update_existing_window(
                            existing_hwnd,
                            image_data,
                            ocr_results,
                            selection_rect,
                            is_pin_mode,
                            drawing_config,
                            ocr_source_bmp_data,
                        )?;
                        return Ok(());
                    }
                }

                // Stale handle.
                Self::clear_singleton_hwnd(existing_hwnd);
            }

            // Register class.
            let class_name = windows::core::w!("PreviewWindow");
            let instance = GetModuleHandleW(None)?;

            let bg_brush = CreateSolidBrush(COLORREF(0));
            let cursor = LoadCursorW(None, IDC_ARROW)?;

            let window_class = WNDCLASSW {
                lpfnWndProc: Some(Self::window_proc),
                hInstance: instance.into(),
                lpszClassName: class_name,
                hCursor: cursor,
                // 关键：使用黑色背景刷，防止调整大小时出现白色闪烁
                hbrBackground: bg_brush,
                style: CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS,
                hIcon: HICON::default(),
                ..Default::default()
            };

            let atom = RegisterClassW(&window_class);
            if atom == 0 {
                let err_code = GetLastError();
                // ERROR_CLASS_ALREADY_EXISTS = 1410
                if err_code.0 != 1410 {
                    eprintln!("RegisterClassW failed: {:?}", err_code);
                }
            }

            // 解析图片与计算尺寸
            let (image_pixels, actual_width, actual_height) = Self::parse_bmp_data(&image_data)?;

            let (text_content, show_text_area, ocr_cached_text) =
                Self::derive_text_panel_state(&ocr_results, is_pin_mode);

            // Choose OCR source image: if not provided, default to the display image.
            let ocr_source_bmp_data = ocr_source_bmp_data.unwrap_or(image_data);

            // 布局计算
            let (mut window_width, window_height) =
                Self::compute_window_size(actual_width, actual_height, show_text_area);

            window_width = window_width.max(Self::min_window_width_for_title_bar());

            // 计算位置
            let screen_size = WindowsHostPlatform::new().screen_size();
            let (window_x, window_y) = Self::compute_window_position(
                selection_rect,
                window_width,
                window_height,
                is_pin_mode,
                screen_size,
            );

            // Create window.
            let dw_style = WS_THICKFRAME
                | WS_SYSMENU
                | WS_MAXIMIZEBOX
                | WS_MINIMIZEBOX
                | WS_VISIBLE
                | WS_CLIPCHILDREN;

            let mut dw_ex_style = WS_EX_APPWINDOW;
            if is_pin_mode {
                dw_ex_style |= WS_EX_TOPMOST;
            }

            let hwnd = CreateWindowExW(
                dw_ex_style,
                class_name,
                windows::core::w!("预览"),
                dw_style,
                window_x,
                window_y,
                window_width,
                window_height,
                None,
                None,
                Some(instance.into()),
                None,
            )?;

            // DWM settings.
            let dark_mode = 1_i32;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWINDOWATTRIBUTE(20),
                &dark_mode as *const _ as *const _,
                std::mem::size_of::<i32>() as u32,
            );

            let round_preference = 2_i32;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWINDOWATTRIBUTE(33),
                &round_preference as *const _ as *const _,
                std::mem::size_of::<i32>() as u32,
            );

            let margins = Margins {
                cxLeftWidth: -1,
                cxRightWidth: -1,
                cyTopHeight: -1,
                cyBottomHeight: -1,
            };
            let _ = DwmExtendFrameIntoClientArea(hwnd, &margins as *const Margins as *const _);

            SetWindowPos(
                hwnd,
                None,
                0,
                0,
                0,
                0,
                SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            )?;

            let svg_icons = Self::build_icon_set(window_width, false);

            let window_id = sc_platform_windows::windows::window_id(hwnd);
            let drawing_state = PreviewDrawingState::new(window_id, drawing_config).ok();

            let mut window = Self {
                hwnd,
                image_pixels,
                image_width: actual_width,
                image_height: actual_height,
                text_area_rect: RectI32 {
                    left: 0,
                    top: 0,
                    right: 0,
                    bottom: 0,
                },
                window_width,
                window_height,
                is_maximized: false,
                svg_icons,
                text_content,
                scroll_offset: 0,
                line_height: OCR_TEXT_LINE_HEIGHT,
                text_lines: Vec::new(),
                is_selecting: false,
                selection_start: None,
                selection_end: None,
                is_pinned: is_pin_mode,
                show_text_area,
                ocr_source_bmp_data,
                ocr_cached_text,
                ocr_in_flight: false,
                ocr_request_id: 0,
                renderer: PreviewRenderer::new().ok(),
                drawing_state,
            };


            window.update_tool_icons();

            // Store window pointer
            let window_ptr = Box::into_raw(Box::new(window));
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, window_ptr as isize);

            // Track singleton hwnd so future `show()` calls update this window.
            Self::set_singleton_hwnd(hwnd);

            // Initialize layout and renderer
            let window = &mut *window_ptr;
            window.center_icons();
            window.recalculate_layout();

            if let Some(renderer) = &mut window.renderer {
                let _ = renderer.initialize(window_id, window_width, window_height);
                let _ = renderer.set_image_from_pixels(
                    &window.image_pixels,
                    window.image_width,
                    window.image_height,
                );
            }

            if window.show_text_area {
                window.refresh_text_lines();
            }

            let platform = WindowsHostPlatform::new();
            let _ = platform.show_window(window_id);
            let _ = platform.update_window(window_id);

            Ok(())
        }
    }
}
