use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("渲染错误: {0}")]
    Render(String),

    #[error("初始化错误: {0}")]
    Init(String),

    #[error("截图错误: {0}")]
    Screenshot(String),

    #[error("绘图错误: {0}")]
    Drawing(String),

    #[error("UI错误: {0}")]
    UI(String),

    #[error("系统错误: {0}")]
    System(String),

    #[error("平台错误: {0}")]
    Platform(String),

    #[error("文件错误: {0}")]
    File(String),

    #[error("Windows API错误: {0}")]
    WinApi(String),

    #[error("IO错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("Windows错误: {0}")]
    Windows(#[from] sc_platform_windows::win32::Error),

    #[error("错误: {0}")]
    Other(String),
}

#[macro_export]
macro_rules! app_error {
    (Render, $msg:expr) => {
        $crate::error::AppError::Render($msg.to_string())
    };
    (Init, $msg:expr) => {
        $crate::error::AppError::Init($msg.to_string())
    };
    (Screenshot, $msg:expr) => {
        $crate::error::AppError::Screenshot($msg.to_string())
    };
    (Drawing, $msg:expr) => {
        $crate::error::AppError::Drawing($msg.to_string())
    };
    (UI, $msg:expr) => {
        $crate::error::AppError::UI($msg.to_string())
    };
    (System, $msg:expr) => {
        $crate::error::AppError::System($msg.to_string())
    };
    (Platform, $msg:expr) => {
        $crate::error::AppError::Platform($msg.to_string())
    };
    (File, $msg:expr) => {
        $crate::error::AppError::File($msg.to_string())
    };
    (WinApi, $msg:expr) => {
        $crate::error::AppError::WinApi($msg.to_string())
    };
    ($msg:expr) => {
        $crate::error::AppError::Other($msg.to_string())
    };
}

pub type AppResult<T> = Result<T, AppError>;

pub fn log_error(error: &AppError) {
    eprintln!("[错误] {error}");
}

pub fn log_and_return_error<T>(error: AppError) -> Result<T, AppError> {
    log_error(&error);
    Err(error)
}

pub trait ErrorContext<T> {
    fn context(self, msg: &str) -> Result<T, AppError>;
    fn with_context<F>(self, f: F) -> Result<T, AppError>
    where
        F: FnOnce() -> String;
}

impl<T, E> ErrorContext<T> for Result<T, E>
where
    E: Into<AppError>,
{
    fn context(self, msg: &str) -> Result<T, AppError> {
        self.map_err(|e| {
            let base_error = e.into();
            AppError::Other(format!("{msg}: {base_error}"))
        })
    }

    fn with_context<F>(self, f: F) -> Result<T, AppError>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| {
            let base_error = e.into();
            AppError::Other(format!("{}: {}", f(), base_error))
        })
    }
}
