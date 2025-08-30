// Command execution helpers to reduce code duplication
use crate::message::Command;
use crate::utils::win_api;
use windows::Win32::Foundation::HWND;

/// 执行操作并处理错误的辅助函数
pub fn execute_with_error_handling<F, E>(action: F, error_msg: &str) -> Vec<Command>
where
    F: FnOnce() -> Result<(), E>,
    E: std::fmt::Display,
{
    match action() {
        Ok(_) => vec![],
        Err(e) => {
            eprintln!("{error_msg}: {e}");
            vec![Command::ShowError(format!("{error_msg}: {e}"))]
        }
    }
}

/// 执行操作，成功后隐藏窗口并重置状态
pub fn execute_and_hide<F, E>(
    action: F,
    hwnd: HWND,
    reset_fn: impl FnOnce(),
    error_msg: &str,
) -> Vec<Command>
where
    F: FnOnce() -> Result<(), E>,
    E: std::fmt::Display,
{
    match action() {
        Ok(_) => {
            let _ = win_api::hide_window(hwnd);
            reset_fn();
            vec![]
        }
        Err(e) => {
            eprintln!("{error_msg}: {e}");
            vec![Command::ShowError(format!("{error_msg}: {e}"))]
        }
    }
}

/// 执行文件保存操作的辅助函数
pub fn execute_save_operation<F, E>(
    save_fn: F,
    hwnd: HWND,
    reset_fn: impl FnOnce(),
    error_msg: &str,
) -> Vec<Command>
where
    F: FnOnce() -> Result<bool, E>,
    E: std::fmt::Display,
{
    match save_fn() {
        Ok(true) => {
            let _ = win_api::hide_window(hwnd);
            reset_fn();
            vec![]
        }
        Ok(false) => {
            // 用户取消
            vec![]
        }
        Err(e) => {
            eprintln!("{error_msg}: {e}");
            vec![Command::ShowError(format!("{error_msg}: {e}"))]
        }
    }
}

/// 宏：简化命令匹配
#[macro_export]
macro_rules! handle_command {
    ($self:expr, $hwnd:expr, $command:expr, {
        $($pattern:pat => $handler:expr),* $(,)?
    }) => {
        match $command {
            $($pattern => $handler,)*
            _ => vec![]
        }
    };
}

/// 宏：简化错误处理
#[macro_export]
macro_rules! try_execute {
    ($action:expr, $error_msg:literal) => {
        match $action {
            Ok(result) => result,
            Err(e) => {
                eprintln!(concat!($error_msg, ": {}"), e);
                return vec![Command::ShowError(format!(concat!($error_msg, ": {}"), e))];
            }
        }
    };
}
