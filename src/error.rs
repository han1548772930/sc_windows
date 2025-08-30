// 统一的错误处理模块
use thiserror::Error;

/// 应用程序的统一错误类型
#[derive(Debug, Error)]
pub enum AppError {
    /// 渲染相关错误
    #[error("渲染错误: {0}")]
    Render(String),

    /// 初始化相关错误
    #[error("初始化错误: {0}")]
    Init(String),

    /// 截图相关错误
    #[error("截图错误: {0}")]
    Screenshot(String),

    /// 绘图相关错误
    #[error("绘图错误: {0}")]
    Drawing(String),

    /// UI相关错误
    #[error("UI错误: {0}")]
    UI(String),

    /// 系统相关错误（托盘、热键、OCR等）
    #[error("系统错误: {0}")]
    System(String),

    /// 平台相关错误
    #[error("平台错误: {0}")]
    Platform(String),

    /// 文件操作错误
    #[error("文件错误: {0}")]
    File(String),

    /// Windows API错误
    #[error("Windows API错误: {0}")]
    WinApi(String),

    /// IO错误
    #[error("IO错误: {0}")]
    Io(#[from] std::io::Error),

    /// Windows core错误
    #[error("Windows错误: {0}")]
    Windows(#[from] windows::core::Error),

    /// 其他错误
    #[error("错误: {0}")]
    Other(String),
}

// 便捷的错误创建宏
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

// 注意: From<std::io::Error> 和 From<windows::core::Error> 已经通过 #[from] 属性自动生成

// Result类型别名
pub type AppResult<T> = Result<T, AppError>;

// 错误处理辅助函数
pub fn log_error(error: &AppError) {
    eprintln!("[错误] {error}");
}

pub fn log_and_return_error<T>(error: AppError) -> Result<T, AppError> {
    log_error(&error);
    Err(error)
}

// 链式错误上下文添加
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
