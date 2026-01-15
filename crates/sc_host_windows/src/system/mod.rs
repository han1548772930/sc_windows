use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use sc_app::selection as core_selection;
use sc_drawing::Rect;
use sc_host_protocol::Command;
use sc_ocr::{self, BoundingBox, OcrCompletionData, OcrConfig, OcrEngine, OcrResult};
use sc_platform::{HostPlatform, PlatformServicesError, TrayEvent, WindowId};
use sc_platform_windows::windows::UserEventSender;
use sc_platform_windows::windows::bmp::crop_bmp;
use sc_settings::Settings;

use crate::HostEvent;

use crate::constants::HOTKEY_SCREENSHOT_ID;
use crate::screenshot::ScreenshotManager;

/// 系统管理器
pub struct SystemManager {
    /// Shared settings snapshot (used for OCR config, hotkeys, etc.).
    settings: Arc<RwLock<Settings>>,
    /// OCR 引擎实例
    ocr_engine: Arc<Mutex<Option<OcrEngine>>>,

    /// Window-thread event sender for background tasks.
    events: UserEventSender<HostEvent>,
}

impl SystemManager {
    /// 创建新的系统管理器
    ///
    /// # 参数
    /// - `settings`: 共享的配置引用
    pub fn new(
        settings: Arc<RwLock<Settings>>,
        events: UserEventSender<HostEvent>,
    ) -> Result<Self, SystemError> {
        Ok(Self {
            settings,
            ocr_engine: Arc::new(Mutex::new(None)),
            events,
        })
    }

    /// 处理键盘输入（全局快捷键）
    pub fn handle_key_input(&mut self, _key: u32) -> Vec<Command> {
        // 全局热键通过 Win32 `WM_HOTKEY` 处理（不走普通 KeyDown 输入）。
        vec![]
    }

    /// 初始化系统集成
    pub fn initialize(
        &mut self,
        window: WindowId,
        host_platform: &dyn HostPlatform<WindowHandle = WindowId>,
    ) -> Result<(), SystemError> {
        // Tray init should not block hotkey registration.
        let (hotkey_modifiers, hotkey_key, hotkey_display) = self
            .settings
            .read()
            .map(|s| (s.hotkey_modifiers, s.hotkey_key, s.get_hotkey_string()))
            .unwrap_or((0x0003, 'S' as u32, "Ctrl+Alt+S".to_string())); // Default Ctrl+Alt+S

        let tooltip = format!("截图工具 - {hotkey_display} 截图，右键查看菜单");
        if let Err(e) = host_platform.init_tray(window, &tooltip) {
            eprintln!("Failed to initialize system tray: {e}");
        }

        if let Err(e) = host_platform.set_global_hotkey(
            window,
            HOTKEY_SCREENSHOT_ID,
            hotkey_modifiers,
            hotkey_key,
        ) {
            eprintln!("Failed to register hotkey: {e}");
        }

        // 异步启动 OCR 引擎
        self.start_ocr_engine_async();

        Ok(())
    }

    /// Cleanup platform integrations (tray/hotkeys).
    pub fn cleanup_platform(&mut self, host_platform: &dyn HostPlatform<WindowHandle = WindowId>) {
        if let Err(e) = host_platform.cleanup_tray() {
            eprintln!("Failed to cleanup system tray: {e}");
        }

        if let Err(e) = host_platform.clear_global_hotkeys() {
            eprintln!("Failed to cleanup global hotkeys: {e}");
        }
    }

    /// 清理系统资源
    pub fn cleanup(&mut self) {
        // Keep Drop-safe cleanup here (no HostPlatform reference).
        self.stop_ocr_engine_sync();
    }

    pub fn handle_tray_event(&mut self, event: TrayEvent) -> Vec<Command> {
        match event {
            TrayEvent::MenuCommand(cmd) => match cmd {
                1001 => vec![Command::TakeScreenshot],
                1002 => vec![Command::ShowSettings],
                1003 => vec![Command::QuitApp],
                _ => vec![],
            },
            TrayEvent::DoubleClick => vec![Command::ShowSettings],
        }
    }

    // ========== OCR 引擎管理 ==========

    fn ocr_config(&self) -> OcrConfig {
        let settings = self.settings.read().unwrap_or_else(|e| e.into_inner());
        OcrConfig::new(
            PathBuf::from(sc_ocr::DEFAULT_MODELS_DIR),
            settings.ocr_language.clone(),
        )
    }

    /// 异步启动 OCR 引擎
    pub fn start_ocr_engine_async(&self) {
        let engine_arc = Arc::clone(&self.ocr_engine);
        let config = self.ocr_config();
        let events = self.events.clone();

        std::thread::spawn(move || {
            let success = Self::start_ocr_engine_sync_inner(&engine_arc, &config);
            let _ = events.send(HostEvent::OcrAvailabilityChanged { available: success });
        });
    }

    /// 同步启动 OCR 引擎（内部实现）
    fn start_ocr_engine_sync_inner(
        engine_arc: &Arc<Mutex<Option<OcrEngine>>>,
        config: &OcrConfig,
    ) -> bool {
        let mut engine_guard = match engine_arc.lock() {
            Ok(guard) => guard,
            Err(_) => return false,
        };

        if engine_guard.is_some() {
            return true; // 已经启动
        }

        #[cfg(debug_assertions)]
        println!("正在启动 OCR 引擎...");

        #[cfg(debug_assertions)]
        let start_time = std::time::Instant::now();

        match sc_ocr::create_engine(config) {
            Ok(engine) => {
                *engine_guard = Some(engine);
                #[cfg(debug_assertions)]
                println!("OCR 引擎启动成功，耗时: {:?}", start_time.elapsed());
                true
            }
            Err(e) => {
                eprintln!("OCR 引擎启动失败: {}", e);
                false
            }
        }
    }

    /// 同步停止 OCR 引擎
    fn stop_ocr_engine_sync(&self) {
        if let Ok(mut engine_guard) = self.ocr_engine.lock()
            && let Some(engine) = engine_guard.take()
        {
            #[cfg(debug_assertions)]
            println!("正在停止 OCR 引擎...");
            drop(engine);
            #[cfg(debug_assertions)]
            println!("OCR 引擎已停止");
        }
    }

    /// 异步停止 OCR 引擎
    pub fn stop_ocr_engine_async(&self) {
        let engine_arc = Arc::clone(&self.ocr_engine);
        std::thread::spawn(move || {
            if let Ok(mut engine_guard) = engine_arc.lock()
                && let Some(engine) = engine_guard.take()
            {
                drop(engine);
            }
        });
    }

    /// 检查 OCR 引擎是否已准备就绪
    pub fn is_ocr_engine_ready(&self) -> bool {
        if let Ok(engine_guard) = self.ocr_engine.try_lock() {
            return engine_guard.is_some();
        }
        false
    }

    /// 检查 OCR 引擎是否可用（模型存在且引擎就绪）
    pub fn ocr_is_available(&self) -> bool {
        sc_ocr::models_exist(&self.ocr_config()) && self.is_ocr_engine_ready()
    }

    /// 启动异步 OCR 引擎状态检查
    pub fn start_async_ocr_check(&self) {
        let engine_arc = Arc::clone(&self.ocr_engine);
        let config = self.ocr_config();
        let events = self.events.clone();

        std::thread::spawn(move || {
            let models_exist = sc_ocr::models_exist(&config);
            let engine_ready = if let Ok(guard) = engine_arc.lock() {
                guard.is_some()
            } else {
                false
            };
            let available = models_exist && engine_ready;

            let _ = events.send(HostEvent::OcrAvailabilityChanged { available });
        });
    }

    /// 重新加载设置
    pub fn reload_settings(&mut self) {
        // Currently no-op. Hotkey updates happen via `reregister_hotkey`.
    }

    /// 重新注册热键
    pub fn reregister_hotkey(
        &mut self,
        window: WindowId,
        host_platform: &dyn HostPlatform<WindowHandle = WindowId>,
    ) -> Result<(), PlatformServicesError> {
        let (hotkey_modifiers, hotkey_key) = self
            .settings
            .read()
            .map(|s| (s.hotkey_modifiers, s.hotkey_key))
            .unwrap_or((0x0003, 'S' as u32));

        host_platform.clear_global_hotkeys()?;
        host_platform.set_global_hotkey(window, HOTKEY_SCREENSHOT_ID, hotkey_modifiers, hotkey_key)
    }

    /// 从选择区域识别文本
    pub fn recognize_text_from_selection(
        &self,
        selection_rect: core_selection::RectI32,
        window: WindowId,
        screenshot_manager: &mut ScreenshotManager,
        host_platform: &dyn HostPlatform<WindowHandle = WindowId>,
    ) -> Result<(), SystemError> {
        // 检查 OCR 引擎是否可用
        if !self.ocr_is_available() {
            host_platform.show_error_message(
                window,
                "OCR错误",
                "OCR引擎不可用。\n\n请确保 OCR 引擎正常运行。",
            );
            return Ok(());
        }

        // 获取缓存的图像数据
        let cached_image = screenshot_manager
            .get_current_image_data()
            .map(|d| d.to_vec());

        let engine_arc = Arc::clone(&self.ocr_engine);

        // 异步执行 OCR
        let events = self.events.clone();

        std::thread::spawn(move || {
            // 检查选区大小
            let width = selection_rect.right - selection_rect.left;
            let height = selection_rect.bottom - selection_rect.top;
            if width <= 0 || height <= 0 {
                let _ = events.send(HostEvent::OcrCancelled);
                return;
            }

            // 获取图像数据
            let Some(ref data) = cached_image else {
                eprintln!("没有缓存图像数据");
                let _ = events.send(HostEvent::OcrCancelled);
                return;
            };

            // 裁剪图像
            let crop_rect: Rect = selection_rect.into();
            let cropped = match crop_bmp(data, &crop_rect) {
                Ok(cropped) => cropped,
                Err(e) => {
                    eprintln!("裁剪图像失败: {:?}", e);
                    let _ = events.send(HostEvent::OcrCancelled);
                    return;
                }
            };

            // 执行 OCR 识别
            let line_results = {
                let engine_guard = match engine_arc.lock() {
                    Ok(guard) => guard,
                    Err(_) => {
                        eprintln!("OCR 引擎锁已中毒");
                        let _ = events.send(HostEvent::OcrCancelled);
                        return;
                    }
                };

                let Some(ref engine) = *engine_guard else {
                    eprintln!("OCR 引擎未就绪");
                    let _ = events.send(HostEvent::OcrCancelled);
                    return;
                };

                match sc_ocr::recognize_text_by_lines(engine, &cropped, selection_rect) {
                    Ok(results) => results,
                    Err(_) => vec![OcrResult {
                        text: sc_ocr::OCR_FAILED_PLACEHOLDER.to_string(),
                        confidence: 0.0,
                        bounding_box: BoundingBox {
                            x: 0,
                            y: 0,
                            width: 200,
                            height: 25,
                        },
                    }],
                }
            };

            let completion_data = OcrCompletionData {
                image_data: cropped,
                ocr_results: line_results,
                selection_rect,
            };

            let _ = events.send(HostEvent::OcrCompleted(completion_data));
        });

        Ok(())
    }
}

impl Drop for SystemManager {
    fn drop(&mut self) {
        self.cleanup();
    }
}

/// 系统错误类型
#[derive(Debug)]
pub enum SystemError {
    /// 托盘错误
    TrayError(String),
    /// 热键错误
    HotkeyError(String),
    /// 窗口检测错误
    WindowDetectionError(String),
    /// OCR错误
    OcrError(String),
    /// 初始化错误
    InitError(String),
    /// 窗口枚举失败
    WindowEnumerationFailed,
}

impl std::fmt::Display for SystemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SystemError::TrayError(msg) => write!(f, "Tray error: {msg}"),
            SystemError::HotkeyError(msg) => write!(f, "Hotkey error: {msg}"),
            SystemError::WindowDetectionError(msg) => write!(f, "Window detection error: {msg}"),
            SystemError::OcrError(msg) => write!(f, "OCR error: {msg}"),
            SystemError::InitError(msg) => write!(f, "System init error: {msg}"),
            SystemError::WindowEnumerationFailed => write!(f, "Window enumeration failed"),
        }
    }
}

impl std::error::Error for SystemError {}
