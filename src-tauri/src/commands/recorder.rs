use app_core::RecorderSnapshot;
use tauri::{AppHandle, State};

use crate::{AppState, emit_recorder_state, window};

#[tauri::command]
pub fn toggle_recording(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<RecorderSnapshot, String> {
    let snapshot = {
        let mut core = state
            .core
            .lock()
            .map_err(|_| "failed to lock app state".to_string())?;
        core.toggle_recording()
    };

    emit_recorder_state(&app, &snapshot);
    let _ = window::sync_hud_visibility(&app, &snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub fn pause_resume(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<RecorderSnapshot>, String> {
    let snapshot = {
        let mut core = state
            .core
            .lock()
            .map_err(|_| "failed to lock app state".to_string())?;
        core.pause_resume()
    };

    if let Some(recorder) = snapshot.as_ref() {
        emit_recorder_state(&app, recorder);
        let _ = window::sync_hud_visibility(&app, recorder);
    }

    Ok(snapshot)
}

#[tauri::command]
pub fn toggle_microphone(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<RecorderSnapshot, String> {
    let snapshot = {
        let mut core = state
            .core
            .lock()
            .map_err(|_| "failed to lock app state".to_string())?;
        core.toggle_microphone()
    };

    emit_recorder_state(&app, &snapshot);
    Ok(snapshot)
}
