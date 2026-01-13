pub use sc_host_windows::{
    app, command_executor, constants, core_bridge, error, screenshot, system,
};

// Public compatibility modules (re-exporting the new crates).
pub use sc_drawing_host as drawing;
pub use sc_host_protocol as message;
pub use sc_ocr as ocr;
pub use sc_settings as settings;
pub use sc_ui_windows as ui;

pub use sc_host_windows::constants::WINDOW_CLASS_NAME;
pub use sc_host_windows::{App, Command, CommandExecutor};
