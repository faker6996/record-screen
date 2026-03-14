use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

use storage::{AppSettings, expand_home_path};
use tauri::{AppHandle, State};

use crate::{
    AppState, audio_inputs, capture_targets, emit_recorder_state, launch_on_login, persist_settings,
};

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
    persist_settings(&app)?;
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
    persist_settings(&app)?;
    Ok(settings)
}

#[tauri::command]
pub fn update_launch_on_login(
    app: AppHandle,
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

    launch_on_login::sync_launch_on_login(launch_on_login)?;
    persist_settings(&app)?;
    Ok(settings)
}

#[tauri::command]
pub fn update_show_hud_during_recording(
    app: AppHandle,
    state: State<'_, AppState>,
    show_hud_during_recording: bool,
) -> Result<AppSettings, String> {
    let (settings, recorder) = {
        let mut core = state
            .core
            .lock()
            .map_err(|_| "failed to lock app state".to_string())?;
        let settings = core.update_show_hud_during_recording(show_hud_during_recording);
        let recorder = core.snapshot();
        (settings, recorder)
    };

    emit_recorder_state(&app, &recorder);
    let _ = crate::window::sync_hud_visibility(&app, &recorder, settings.show_hud_during_recording);
    persist_settings(&app)?;
    Ok(settings)
}

#[tauri::command]
pub fn update_capture_target(
    app: AppHandle,
    state: State<'_, AppState>,
    capture_target_id: String,
) -> Result<AppSettings, String> {
    let available_targets = {
        let core = state
            .core
            .lock()
            .map_err(|_| "failed to lock app state".to_string())?;
        capture_targets::available_capture_targets(&core.settings())
    };

    let capture_target = available_targets
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
    persist_settings(&app)?;
    if let Some(bounds) =
        crate::target_preview::preview_bounds_for_target(&app, &capture_target_id)?
    {
        let _ = crate::window::show_target_preview(&app, bounds);
    }
    Ok(settings)
}

#[tauri::command]
pub fn update_system_audio_enabled(
    app: AppHandle,
    state: State<'_, AppState>,
    system_audio_enabled: bool,
) -> Result<AppSettings, String> {
    let capabilities = crate::capture_capabilities::current_capture_capabilities();
    if system_audio_enabled && !capabilities.supports_system_audio {
        return Err(capabilities.system_audio_note);
    }

    let (settings, recorder) = {
        let mut core = state
            .core
            .lock()
            .map_err(|_| "failed to lock app state".to_string())?;
        let settings = core.update_system_audio_enabled(system_audio_enabled);
        let recorder = core.snapshot();
        (settings, recorder)
    };

    emit_recorder_state(&app, &recorder);
    persist_settings(&app)?;
    Ok(settings)
}

#[tauri::command]
pub fn update_audio_input(
    app: AppHandle,
    state: State<'_, AppState>,
    audio_input_id: String,
) -> Result<AppSettings, String> {
    let audio_input = audio_inputs::available_audio_inputs()
        .into_iter()
        .find(|input| input.id == audio_input_id)
        .ok_or_else(|| "selected microphone input is not available".to_string())?;

    if audio_input.kind == capture::AudioInputKind::System {
        return Err(
            "system-audio loopback sources are controlled by the `Include system audio` toggle, not the microphone selector."
                .to_string(),
        );
    }

    let (settings, recorder) = {
        let mut core = state
            .core
            .lock()
            .map_err(|_| "failed to lock app state".to_string())?;
        let settings = core.update_audio_input(audio_input.id);
        let recorder = core.snapshot();
        (settings, recorder)
    };

    emit_recorder_state(&app, &recorder);
    persist_settings(&app)?;
    Ok(settings)
}

#[tauri::command]
pub fn update_custom_region(
    app: AppHandle,
    state: State<'_, AppState>,
    region_x: u32,
    region_y: u32,
    region_width: u32,
    region_height: u32,
) -> Result<AppSettings, String> {
    let (settings, recorder) = {
        let mut core = state
            .core
            .lock()
            .map_err(|_| "failed to lock app state".to_string())?;
        let settings = core.update_custom_region(region_x, region_y, region_width, region_height);
        let recorder = core.snapshot();
        (settings, recorder)
    };

    emit_recorder_state(&app, &recorder);
    persist_settings(&app)?;
    Ok(settings)
}

#[tauri::command]
pub fn pick_output_directory(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<AppSettings>, String> {
    let current_directory = {
        let core = state
            .core
            .lock()
            .map_err(|_| "failed to lock app state".to_string())?;
        core.settings().output_directory
    };

    let Some(chosen_directory) = open_directory_picker(&current_directory)? else {
        return Ok(None);
    };

    let normalized_directory = normalize_output_directory(&chosen_directory);
    let (settings, recorder) = {
        let mut core = state
            .core
            .lock()
            .map_err(|_| "failed to lock app state".to_string())?;
        let settings = core.update_output_directory(normalized_directory);
        let recorder = core.snapshot();
        (settings, recorder)
    };

    emit_recorder_state(&app, &recorder);
    persist_settings(&app)?;
    Ok(Some(settings))
}

fn normalize_output_directory(directory: &Path) -> String {
    let display = directory.display().to_string();

    if let Ok(home) = env::var("HOME") {
        if let Some(stripped) = display.strip_prefix(&home) {
            return format!("~{stripped}");
        }
    }

    display
}

fn open_directory_picker(current_directory: &str) -> Result<Option<PathBuf>, String> {
    let starting_directory = expand_home_path(current_directory);

    #[cfg(target_os = "linux")]
    {
        let start = format!("{}/", starting_directory.display());

        if command_exists("zenity") {
            let output = Command::new("zenity")
                .args([
                    "--file-selection",
                    "--directory",
                    "--title=Choose output folder",
                    "--filename",
                    &start,
                ])
                .output()
                .map_err(|error| format!("failed to open directory picker: {error}"))?;

            if output.status.success() {
                let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if value.is_empty() {
                    return Ok(None);
                }
                return Ok(Some(PathBuf::from(value)));
            }

            return Ok(None);
        }

        if command_exists("kdialog") {
            let output = Command::new("kdialog")
                .args(["--getexistingdirectory", &start])
                .output()
                .map_err(|error| format!("failed to open directory picker: {error}"))?;

            if output.status.success() {
                let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if value.is_empty() {
                    return Ok(None);
                }
                return Ok(Some(PathBuf::from(value)));
            }

            return Ok(None);
        }

        return Err("no supported directory picker found. Install zenity or kdialog.".to_string());
    }

    #[cfg(target_os = "macos")]
    {
        let script = format!(
            "POSIX path of (choose folder with prompt \"Choose output folder\" default location POSIX file \"{}\")",
            escape_applescript_path(&starting_directory)
        );
        let output = Command::new("osascript")
            .args(["-e", &script])
            .output()
            .map_err(|error| format!("failed to open directory picker: {error}"))?;

        if output.status.success() {
            let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if value.is_empty() {
                return Ok(None);
            }
            return Ok(Some(PathBuf::from(value)));
        }

        return Ok(None);
    }

    #[cfg(target_os = "windows")]
    {
        let script = format!(
            "$dialog = New-Object System.Windows.Forms.FolderBrowserDialog; \
             $dialog.Description = 'Choose output folder'; \
             $dialog.SelectedPath = '{}'; \
             if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {{ \
               Write-Output $dialog.SelectedPath \
             }}",
            starting_directory.display().to_string().replace('\'', "''")
        );

        let output = Command::new("powershell")
            .args(["-NoProfile", "-Command", &format!(
                "Add-Type -AssemblyName System.Windows.Forms; Add-Type -AssemblyName System.Drawing; {script}"
            )])
            .output()
            .map_err(|error| format!("failed to open directory picker: {error}"))?;

        if output.status.success() {
            let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if value.is_empty() {
                return Ok(None);
            }
            return Ok(Some(PathBuf::from(value)));
        }

        return Ok(None);
    }

    #[allow(unreachable_code)]
    Err("directory picking is not supported on this platform".to_string())
}

#[cfg(target_os = "linux")]
fn command_exists(command_name: &str) -> bool {
    env::var_os("PATH")
        .map(|path| env::split_paths(&path).any(|directory| directory.join(command_name).is_file()))
        .unwrap_or(false)
}

#[cfg(target_os = "macos")]
fn escape_applescript_path(path: &Path) -> String {
    path.display()
        .to_string()
        .replace('\\', "\\\\")
        .replace('\"', "\\\"")
}
