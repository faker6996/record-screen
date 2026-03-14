use app_core::BootstrapSnapshot;
use capture::{AudioInputOption, CaptureTargetOption};
use shortcuts::ShortcutBinding;
use tauri::{AppHandle, State};

use crate::{AppState, audio_inputs, bootstrap, capture_targets, diagnostics, register_shortcuts};

#[tauri::command]
pub fn get_bootstrap(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<BootstrapSnapshot, String> {
    let capture_targets = capture_targets::initial_capture_targets();
    let audio_inputs = audio_inputs::initial_audio_inputs();
    let mut core = state
        .core
        .lock()
        .map_err(|_| "failed to lock app state".to_string())?;
    let current_audio_input_id = core.settings().audio_input_id;
    if let Some(next_audio_input_id) =
        audio_inputs::normalize_audio_input_selection(&current_audio_input_id, &audio_inputs)
    {
        if next_audio_input_id != current_audio_input_id {
            core.update_audio_input(next_audio_input_id);
        }
    }
    let platform = bootstrap::platform_name();
    let app_version = app.package_info().version.to_string();
    let snapshot = core.bootstrap(
        platform,
        &app_version,
        capture_targets,
        audio_inputs,
        diagnostics::runtime_diagnostics(),
    );

    Ok(snapshot)
}

#[tauri::command]
pub fn get_capture_targets() -> Result<Vec<CaptureTargetOption>, String> {
    Ok(capture_targets::refreshed_capture_targets())
}

#[tauri::command]
pub fn get_audio_inputs() -> Result<Vec<AudioInputOption>, String> {
    Ok(audio_inputs::refreshed_audio_inputs())
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
