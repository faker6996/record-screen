use app_core::RecorderSnapshot;
use tauri::{AppHandle, State};

use crate::{AppState, emit_recorder_state, persist_settings};

#[cfg(target_os = "macos")]
fn blocked_microphone_message() -> Option<String> {
    if permissions::microphone_permission_blocked("macos") {
        return Some(
            permissions::microphone_permission_guidance("macos")
                .unwrap_or_else(|| "Microphone access is blocked in macOS settings.".to_string()),
        );
    }

    None
}

#[tauri::command]
pub fn get_recorder_snapshot(state: State<'_, AppState>) -> Result<RecorderSnapshot, String> {
    let core = state
        .core
        .lock()
        .map_err(|_| "failed to lock app state".to_string())?;
    Ok(core.snapshot())
}

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
    #[cfg(target_os = "macos")]
    {
        let should_enable_microphone = {
            let core = state
                .core
                .lock()
                .map_err(|_| "failed to lock app state".to_string())?;
            !core.settings().mic_enabled
        };
        if should_enable_microphone {
            let capture_target_id = {
                let core = state
                    .core
                    .lock()
                    .map_err(|_| "failed to lock app state".to_string())?;
                let settings = core.settings();
                settings.capture_target_id
            };
            if let Some(message) = blocked_microphone_message() {
                return Err(message);
            }
            let (supported, note) =
                capture_macos::microphone_support_summary_for_target(&capture_target_id);
            if !supported {
                return Err(note);
            }
        }
    }

    let snapshot = {
        let mut core = state
            .core
            .lock()
            .map_err(|_| "failed to lock app state".to_string())?;
        core.toggle_microphone()
    };

    emit_recorder_state(&app, &snapshot);
    persist_settings(&app)?;
    Ok(snapshot)
}

#[tauri::command]
pub fn start_mic_check(app: AppHandle) -> Result<crate::mic_check::MicCheckSnapshot, String> {
    crate::mic_check::start_mic_check(&app)
}

#[tauri::command]
pub fn stop_mic_check(app: AppHandle) -> Result<crate::mic_check::MicCheckSnapshot, String> {
    crate::mic_check::stop_mic_check(&app)
}
