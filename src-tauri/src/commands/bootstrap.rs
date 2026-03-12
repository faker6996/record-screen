use app_core::BootstrapSnapshot;
use shortcuts::ShortcutBinding;
use tauri::{AppHandle, State};

use crate::{AppState, bootstrap, register_shortcuts};

#[tauri::command]
pub fn get_bootstrap(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<BootstrapSnapshot, String> {
    let core = state
        .core
        .lock()
        .map_err(|_| "failed to lock app state".to_string())?;

    let _ = app;
    Ok(core.bootstrap(bootstrap::platform_name()))
}

#[tauri::command]
pub fn reset_shortcuts(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<ShortcutBinding>, String> {
    let shortcuts = {
        let mut core = state
            .core
            .lock()
            .map_err(|_| "failed to lock app state".to_string())?;
        core.reset_shortcuts()
    };

    register_shortcuts(&app)?;
    Ok(shortcuts)
}
