use storage::AppSettings;
use tauri::{AppHandle, State};

use crate::{AppState, capture_targets, emit_recorder_state};

#[tauri::command]
pub fn update_quality_preset(
    app: AppHandle,
    state: State<'_, AppState>,
    quality_preset: String,
) -> Result<AppSettings, String> {
    let (settings, recorder) = {
        let mut core = state
            .core
            .lock()
            .map_err(|_| "failed to lock app state".to_string())?;
        let settings = core.update_quality_preset(quality_preset);
        let recorder = core.snapshot();
        (settings, recorder)
    };

    emit_recorder_state(&app, &recorder);
    Ok(settings)
}

#[tauri::command]
pub fn update_output_directory(
    app: AppHandle,
    state: State<'_, AppState>,
    output_directory: String,
) -> Result<AppSettings, String> {
    let (settings, recorder) = {
        let mut core = state
            .core
            .lock()
            .map_err(|_| "failed to lock app state".to_string())?;
        let settings = core.update_output_directory(output_directory);
        let recorder = core.snapshot();
        (settings, recorder)
    };

    emit_recorder_state(&app, &recorder);
    Ok(settings)
}

#[tauri::command]
pub fn update_launch_on_login(
    state: State<'_, AppState>,
    launch_on_login: bool,
) -> Result<AppSettings, String> {
    let settings = {
        let mut core = state
            .core
            .lock()
            .map_err(|_| "failed to lock app state".to_string())?;
        core.update_launch_on_login(launch_on_login)
    };

    Ok(settings)
}

#[tauri::command]
pub fn update_capture_target(
    app: AppHandle,
    state: State<'_, AppState>,
    capture_target_id: String,
) -> Result<AppSettings, String> {
    let capture_target = capture_targets::available_capture_targets()
        .into_iter()
        .find(|target| target.id == capture_target_id)
        .ok_or_else(|| "selected capture target is not available".to_string())?;

    let (settings, recorder) = {
        let mut core = state
            .core
            .lock()
            .map_err(|_| "failed to lock app state".to_string())?;
        let settings = core.update_capture_target(capture_target.id, capture_target.label);
        let recorder = core.snapshot();
        (settings, recorder)
    };

    emit_recorder_state(&app, &recorder);
    Ok(settings)
}
