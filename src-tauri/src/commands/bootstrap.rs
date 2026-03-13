use app_core::BootstrapSnapshot;
use capture::CaptureTargetOption;
use shortcuts::ShortcutBinding;
use tauri::{AppHandle, State};

use crate::{AppState, bootstrap, capture_targets, register_shortcuts};

#[tauri::command]
pub fn get_bootstrap(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<BootstrapSnapshot, String> {
    let capture_targets = capture_targets::initial_capture_targets();
    let core = state
        .core
        .lock()
        .map_err(|_| "failed to lock app state".to_string())?;
    let platform = bootstrap::platform_name();
    let mut snapshot = core.bootstrap(platform, capture_targets);
    snapshot.permissions = permissions::probe_permissions(platform);
    snapshot.recent_sessions =
        crate::commands::library::scan_recent_recordings(&snapshot.settings.output_directory);

    let _ = app;
    Ok(snapshot)
}

#[tauri::command]
pub fn get_capture_targets() -> Result<Vec<CaptureTargetOption>, String> {
    Ok(capture_targets::available_capture_targets())
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
