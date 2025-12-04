//! Settings module
//!
//! This module provides application settings management including:
//! - Core settings data structure and persistence
//! - Default value functions for serde
//! - Settings window UI implementation

mod core;
mod defaults;
mod window;

// Re-export main types
pub use core::Settings;
pub use window::{show_settings_window, SettingsWindow};
