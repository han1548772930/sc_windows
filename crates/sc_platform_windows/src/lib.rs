#![cfg(target_os = "windows")]

mod event_converter;
pub mod win32;
pub mod win_api;
pub mod windows;

pub(crate) use event_converter::EventConverter;
pub use windows::*;
