use app_core::BootstrapSnapshot;
use capture::{AudioInputOption, CaptureTargetOption, FULL_DESKTOP_TARGET_ID};
use shortcuts::{ShortcutAction, ShortcutBinding};
use tauri::{AppHandle, State};

use crate::{
    AppState, audio_inputs, bootstrap, capture_targets, diagnostics, parse_shortcut_accelerator,
    persist_shortcuts, register_shortcuts,
};

fn should_eagerly_refresh_capture_targets(settings: &storage::AppSettings) -> bool {
    settings.capture_target_id != FULL_DESKTOP_TARGET_ID
        || settings.region_source_capture_target_id != FULL_DESKTOP_TARGET_ID
}

#[tauri::command]
pub fn get_bootstrap(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<BootstrapSnapshot, String> {
    let mut core = state
        .core
        .lock()
        .map_err(|_| "failed to lock app state".to_string())?;
    let capabilities = crate::capture_capabilities::current_capture_capabilities();
    let mut settings = core.settings();
    let mut settings_changed = false;
    let capture_targets = if should_eagerly_refresh_capture_targets(&settings) {
        capture_targets::available_capture_targets(&settings)
    } else {
        capture_targets::initial_capture_targets(&settings)
    };
    let audio_inputs = audio_inputs::initial_audio_inputs();
    let available_capture_target_ids: Vec<&str> = capture_targets
        .iter()
        .map(|target| target.id.as_str())
        .collect();
    let current_audio_input_id = core.settings().audio_input_id;
    if let Some(next_audio_input_id) =
        audio_inputs::normalize_audio_input_selection(&current_audio_input_id, &audio_inputs)
    {
        if next_audio_input_id != current_audio_input_id {
            settings = core.update_audio_input(next_audio_input_id);
            settings_changed = true;
        }
    }
    if let Some(next_region_source_capture_target_id) =
        capture_targets::normalize_custom_region_source_target_id(
            &settings.region_source_capture_target_id,
            &capture_targets,
        )
    {
        if next_region_source_capture_target_id != settings.region_source_capture_target_id {
            let previous_target_id = settings.region_source_capture_target_id.clone();
            settings = core.update_custom_region(
                settings.region_x,
                settings.region_y,
                settings.region_width,
                settings.region_height,
                Some(next_region_source_capture_target_id.clone()),
                Some(settings.region_source_origin_x),
                Some(settings.region_source_origin_y),
                Some(settings.region_source_scale_factor_milli),
            );
            crate::runtime_log::log_runtime_info(&format!(
                "normalized custom-region source target during bootstrap | from={} | to={}",
                previous_target_id, next_region_source_capture_target_id
            ));
            settings_changed = true;
        }
    }
    if !available_capture_target_ids
        .iter()
        .any(|target_id| *target_id == settings.capture_target_id)
    {
        let previous_capture_target_id = settings.capture_target_id.clone();
        settings = core.update_capture_target(
            FULL_DESKTOP_TARGET_ID.to_string(),
            "Full desktop".to_string(),
        );
        crate::runtime_log::log_runtime_info(&format!(
            "normalized capture target during bootstrap | from={} | to={}",
            previous_capture_target_id, FULL_DESKTOP_TARGET_ID
        ));
        settings_changed = true;
    }
    if settings.system_audio_enabled && !capabilities.supports_system_audio {
        core.update_system_audio_enabled(false);
        settings_changed = true;
    }
    let platform = bootstrap::platform_name();
    let app_version = app.package_info().version.to_string();
    let snapshot = core.bootstrap(
        platform,
        &app_version,
        capture_targets,
        audio_inputs,
        diagnostics::initial_runtime_diagnostics(),
    );
    drop(core);

    if settings_changed {
        crate::persist_settings(&app)?;
    }

    Ok(snapshot)
}

#[cfg(test)]
mod tests {
    use super::should_eagerly_refresh_capture_targets;
    use storage::AppSettings;

    #[test]
    fn skips_eager_capture_target_refresh_for_full_desktop_defaults() {
        let settings = AppSettings::default();
        assert!(!should_eagerly_refresh_capture_targets(&settings));
    }

    #[test]
    fn eagerly_refreshes_when_a_monitor_target_is_persisted() {
        let mut settings = AppSettings::default();
        settings.capture_target_id = "monitor:\\\\.\\DISPLAY3".to_string();
        assert!(should_eagerly_refresh_capture_targets(&settings));
    }

    #[test]
    fn eagerly_refreshes_when_custom_region_uses_a_monitor_source() {
        let mut settings = AppSettings::default();
        settings.capture_target_id = "region:custom".to_string();
        settings.region_source_capture_target_id = "monitor:\\\\.\\DISPLAY1".to_string();
        assert!(should_eagerly_refresh_capture_targets(&settings));
    }
}

#[tauri::command]
pub fn get_runtime_diagnostics() -> Result<app_core::RuntimeDiagnostics, String> {
    Ok(diagnostics::current_runtime_diagnostics())
}

#[tauri::command]
pub fn get_capture_targets(state: State<'_, AppState>) -> Result<Vec<CaptureTargetOption>, String> {
    let settings = {
        let core = state
            .core
            .lock()
            .map_err(|_| "failed to lock app state".to_string())?;
        core.settings()
    };

    Ok(capture_targets::refreshed_capture_targets(&settings))
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
    persist_shortcuts(&app)?;
    Ok(shortcuts)
}

#[tauri::command]
pub fn update_shortcut(
    app: AppHandle,
    state: State<'_, AppState>,
    action: ShortcutAction,
    accelerator: String,
) -> Result<Vec<ShortcutBinding>, String> {
    let normalized_accelerator = accelerator.trim().to_string();
    let parsed_shortcut = parse_shortcut_accelerator(&normalized_accelerator)?;

    let shortcuts = {
        let mut core = state
            .core
            .lock()
            .map_err(|_| "failed to lock app state".to_string())?;
        let current_shortcuts = core.shortcuts();

        for binding in &current_shortcuts {
            if binding.action == action || !binding.enabled {
                continue;
            }

            if parse_shortcut_accelerator(&binding.accelerator)
                .map(|candidate| candidate == parsed_shortcut)
                .unwrap_or(false)
            {
                return Err(format!(
                    "shortcut conflict: `{}` is already assigned to {}.",
                    normalized_accelerator, binding.label
                ));
            }
        }

        core.update_shortcut(action, normalized_accelerator)
    };

    register_shortcuts(&app)?;
    persist_shortcuts(&app)?;
    Ok(shortcuts)
}
