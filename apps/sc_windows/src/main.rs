#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

fn main() {
    if let Err(e) = sc_host_windows::run() {
        eprintln!("{e}");
    }
}
