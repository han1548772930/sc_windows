// Application State Management
//
// Thread-safe state management using Arc<Mutex> instead of unsafe global static

use crate::App;
use anyhow::Result;
use parking_lot::Mutex;
use std::sync::{Arc, OnceLock};

/// Global application state holder using std::sync::OnceLock (Rust 1.70+)
static APP_STATE: OnceLock<Arc<Mutex<App>>> = OnceLock::new();

/// Initialize the application state
pub fn initialize_app(app: App) -> Result<()> {
    APP_STATE
        .set(Arc::new(Mutex::new(app)))
        .map_err(|_| anyhow::anyhow!("Application state already initialized"))
}

/// Get a reference to the application state
pub fn with_app<F, R>(f: F) -> Result<R>
where
    F: FnOnce(&mut App) -> R,
{
    let state = APP_STATE
        .get()
        .ok_or_else(|| anyhow::anyhow!("Application state not initialized"))?;

    let mut app = state.lock();
    Ok(f(&mut *app))
}

/// Check if the application state is initialized
pub fn is_initialized() -> bool {
    APP_STATE.get().is_some()
}

/// Clean up the application state (for shutdown)
pub fn cleanup() {
    // OnceCell doesn't support taking the value out,
    // but the App's Drop implementation will handle cleanup
    // when the process exits
}
