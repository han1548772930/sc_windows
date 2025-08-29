// Unified Error Handling Module
//
// Centralized error types for consistent error management across the application

use std::io;
use thiserror::Error;

/// Main application error type
#[derive(Debug, Error)]
pub enum AppError {
    #[error("Platform error: {0}")]
    Platform(#[from] PlatformError),

    #[error("Screenshot error: {0}")]
    Screenshot(#[from] ScreenshotError),

    #[error("Drawing error: {0}")]
    Drawing(#[from] DrawingError),

    #[error("OCR error: {0}")]
    Ocr(#[from] OcrError),

    #[error("UI error: {0}")]
    UI(#[from] UIError),

    #[error("System error: {0}")]
    System(#[from] SystemError),

    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("Windows API error: {0}")]
    Windows(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("State error: {0}")]
    State(String),

    #[error("Other error: {0}")]
    Other(#[from] anyhow::Error),
}

/// Platform-specific errors
#[derive(Debug, Error)]
pub enum PlatformError {
    #[error("Initialization failed: {0}")]
    InitializationError(String),

    #[error("Rendering failed: {0}")]
    RenderingError(String),

    #[error("Resource creation failed: {0}")]
    ResourceCreationError(String),

    #[error("Platform not supported: {0}")]
    UnsupportedPlatform(String),
}

/// Screenshot-related errors
#[derive(Debug, Error)]
pub enum ScreenshotError {
    #[error("Screen capture failed: {0}")]
    CaptureError(String),

    #[error("Invalid selection area")]
    InvalidSelection,

    #[error("Save failed: {0}")]
    SaveError(String),

    #[error("Clipboard operation failed: {0}")]
    ClipboardError(String),
}

/// Drawing-related errors
#[derive(Debug, Error)]
pub enum DrawingError {
    #[error("Invalid drawing tool")]
    InvalidTool,

    #[error("Drawing operation failed: {0}")]
    OperationError(String),

    #[error("Element not found")]
    ElementNotFound,

    #[error("History operation failed: {0}")]
    HistoryError(String),
}

/// OCR-related errors
#[derive(Debug, Error)]
pub enum OcrError {
    #[error("OCR engine not available")]
    EngineNotAvailable,

    #[error("OCR processing failed: {0}")]
    ProcessingError(String),

    #[error("Invalid language: {0}")]
    InvalidLanguage(String),

    #[error("OCR initialization failed: {0}")]
    InitializationError(String),
}

/// UI-related errors
#[derive(Debug, Error)]
pub enum UIError {
    #[error("Window creation failed: {0}")]
    WindowCreationError(String),

    #[error("Dialog operation failed: {0}")]
    DialogError(String),

    #[error("Invalid UI state: {0}")]
    InvalidState(String),

    #[error("Icon loading failed: {0}")]
    IconError(String),
}

/// System-related errors
#[derive(Debug, Error)]
pub enum SystemError {
    #[error("Hotkey registration failed: {0}")]
    HotkeyError(String),

    #[error("System tray operation failed: {0}")]
    TrayError(String),

    #[error("Window detection failed: {0}")]
    WindowDetectionError(String),

    #[error("Timer operation failed: {0}")]
    TimerError(String),
}

/// Result type alias for convenience
pub type AppResult<T> = Result<T, AppError>;

/// Convert Windows HRESULT to AppError
impl From<windows::core::Error> for AppError {
    fn from(err: windows::core::Error) -> Self {
        AppError::Windows(format!("Windows API error: {:?}", err))
    }
}

/// Helper trait for converting Windows results
pub trait IntoAppResult<T> {
    fn into_app_result(self) -> AppResult<T>;
}

impl<T> IntoAppResult<T> for windows::core::Result<T> {
    fn into_app_result(self) -> AppResult<T> {
        self.map_err(|e| AppError::Windows(format!("{:?}", e)))
    }
}
