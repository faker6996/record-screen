use app_core::RecorderSnapshot;
use tauri::{AppHandle, State};

use crate::{AppState, emit_recorder_state};

#[tauri::command]
pub fn toggle_recording(app: AppHandle) -> Result<RecorderSnapshot, String> {
    crate::recording::toggle_recording(&app)
}

#[tauri::command]
pub fn pause_resume(app: AppHandle) -> Result<Option<RecorderSnapshot>, String> {
    crate::recording::pause_resume(&app)
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
