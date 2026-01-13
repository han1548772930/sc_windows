use sc_platform::{HostPlatform, WindowId};
use sc_platform_windows::win32::{CS_DBLCLKS, CS_HREDRAW, CS_OWNDC, Result};
use sc_platform_windows::windows::{
    Direct2DRenderer, UserEventSender, WindowsHostPlatform, run_fullscreen_toolwindow_app,
};

use crate::error::{AppError, AppResult};
use crate::{App, HostEvent, WINDOW_CLASS_NAME};

fn create_app(
    window: WindowId,
    screen_size: (i32, i32),
    events: UserEventSender<HostEvent>,
) -> AppResult<App> {
    let (screen_width, screen_height) = screen_size;

    let mut renderer = Direct2DRenderer::new().map_err(|e| {
        AppError::Init(format!(
            "图形引擎创建失败: {e:?}\n\n请检查显卡驱动是否正常安装。"
        ))
    })?;

    renderer
        .initialize(window, screen_width, screen_height)
        .map_err(|_| AppError::Init("图形引擎初始化失败，请检查显卡驱动是否正常。".to_string()))?;

    let host_platform: Box<dyn HostPlatform<WindowHandle = WindowId>> =
        Box::new(WindowsHostPlatform::new());

    let mut app = App::new(renderer, events, host_platform)?;
    let _ = app.init_system_tray(window);
    app.start_async_ocr_check();

    Ok(app)
}

pub fn run() -> Result<()> {
    run_fullscreen_toolwindow_app(
        WINDOW_CLASS_NAME,
        CS_DBLCLKS | CS_OWNDC | CS_HREDRAW,
        create_app,
    )?;
    Ok(())
}
