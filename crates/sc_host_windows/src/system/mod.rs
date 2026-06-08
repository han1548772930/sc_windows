use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
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

pub struct SystemManager {
    /// Shared settings snapshot (used for OCR config, hotkeys, etc.).
    settings: Arc<RwLock<Settings>>,
    ocr_engine: Arc<Mutex<Option<OcrEngine>>>,
    ocr_generation: Arc<AtomicU64>,

    /// Window-thread event sender for background tasks.
    events: UserEventSender<HostEvent>,
}

impl SystemManager {
    pub fn new(
        settings: Arc<RwLock<Settings>>,
        events: UserEventSender<HostEvent>,
    ) -> Result<Self, SystemError> {
        Ok(Self {
            settings,
            ocr_engine: Arc::new(Mutex::new(None)),
            ocr_generation: Arc::new(AtomicU64::new(1)),
            events,
        })
    }

    pub fn handle_key_input(&mut self, _key: u32) -> Vec<Command> {
        vec![]
    }

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

    fn ocr_config(&self) -> OcrConfig {
        let settings = self.settings.read().unwrap_or_else(|e| e.into_inner());
        OcrConfig::new(
            PathBuf::from(sc_ocr::DEFAULT_MODELS_DIR),
            settings.ocr_language.clone(),
        )
    }

    pub fn start_ocr_engine_async(&self) {
        let engine_arc = Arc::clone(&self.ocr_engine);
        let generation = self.ocr_generation.load(Ordering::Acquire);
        let config = self.ocr_config();
        let events = self.events.clone();

        std::thread::spawn(move || {
            let success = Self::start_ocr_engine_sync_inner(&engine_arc, &config);
            let _ = events.send(HostEvent::OcrAvailabilityChanged {
                generation,
                available: success,
            });
        });
    }

    fn start_ocr_engine_sync_inner(
        engine_arc: &Arc<Mutex<Option<OcrEngine>>>,
        config: &OcrConfig,
    ) -> bool {
        let mut engine_guard = match engine_arc.lock() {
            Ok(guard) => guard,
            Err(_) => return false,
        };

        if engine_guard.is_some() {
            return true;
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

    fn stop_ocr_engine_sync(&self) {
        self.ocr_generation.fetch_add(1, Ordering::AcqRel);
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

    pub fn stop_ocr_engine_async(&self) {
        let engine_arc = Arc::clone(&self.ocr_engine);
        self.ocr_generation.fetch_add(1, Ordering::AcqRel);
        std::thread::spawn(move || {
            if let Ok(mut engine_guard) = engine_arc.lock()
                && let Some(engine) = engine_guard.take()
            {
                drop(engine);
            }
        });
    }

    pub fn is_ocr_engine_ready(&self) -> bool {
        if let Ok(engine_guard) = self.ocr_engine.try_lock() {
            return engine_guard.is_some();
        }
        false
    }

    pub fn ocr_is_available(&self) -> bool {
        sc_ocr::models_exist(&self.ocr_config()) && self.is_ocr_engine_ready()
    }

    pub fn start_async_ocr_check(&self) {
        let engine_arc = Arc::clone(&self.ocr_engine);
        let generation = self.ocr_generation.load(Ordering::Acquire);
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

            let _ = events.send(HostEvent::OcrAvailabilityChanged {
                generation,
                available,
            });
        });
    }

    pub fn reload_settings(&mut self) {
        // Currently no-op. Hotkey updates happen via `reregister_hotkey`.
    }

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

    pub fn recognize_text_from_selection(
        &self,
        selection_rect: core_selection::RectI32,
        window: WindowId,
        screenshot_manager: &mut ScreenshotManager,
        host_platform: &dyn HostPlatform<WindowHandle = WindowId>,
    ) -> Result<(), SystemError> {
        if !self.ocr_is_available() {
            host_platform.show_error_message(
                window,
                "OCR错误",
                "OCR引擎不可用。\n\n请确保 OCR 引擎正常运行。",
            );
            return Ok(());
        }

        let cached_image = screenshot_manager
            .get_current_image_data()
            .map(|d| d.to_vec());

        let engine_arc = Arc::clone(&self.ocr_engine);
        let generation = self.ocr_generation.load(Ordering::Acquire);

        let events = self.events.clone();

        std::thread::spawn(move || {
            let width = selection_rect.right - selection_rect.left;
            let height = selection_rect.bottom - selection_rect.top;
            if width <= 0 || height <= 0 {
                let _ = events.send(HostEvent::OcrCancelled { generation });
                return;
            }

            let Some(ref data) = cached_image else {
                eprintln!("没有缓存图像数据");
                let _ = events.send(HostEvent::OcrCancelled { generation });
                return;
            };

            let crop_rect: Rect = selection_rect.into();
            let cropped = match crop_bmp(data, &crop_rect) {
                Ok(cropped) => cropped,
                Err(e) => {
                    eprintln!("裁剪图像失败: {:?}", e);
                    let _ = events.send(HostEvent::OcrCancelled { generation });
                    return;
                }
            };

            let line_results = {
                let engine_guard = match engine_arc.lock() {
                    Ok(guard) => guard,
                    Err(_) => {
                        eprintln!("OCR 引擎锁已中毒");
                        let _ = events.send(HostEvent::OcrCancelled { generation });
                        return;
                    }
                };

                let Some(ref engine) = *engine_guard else {
                    eprintln!("OCR 引擎未就绪");
                    let _ = events.send(HostEvent::OcrCancelled { generation });
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

            let _ = events.send(HostEvent::OcrCompleted {
                generation,
                data: completion_data,
            });
        });

        Ok(())
    }

    pub fn ocr_generation(&self) -> u64 {
        self.ocr_generation.load(Ordering::Acquire)
    }
}

impl Drop for SystemManager {
    fn drop(&mut self) {
        self.cleanup();
    }
}

#[derive(Debug)]
pub enum SystemError {
    TrayError(String),
    HotkeyError(String),
    WindowDetectionError(String),
    OcrError(String),
    InitError(String),
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
