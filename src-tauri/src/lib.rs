mod audio_inputs;
mod bootstrap;
mod capture_capabilities;
mod capture_targets;
mod commands;
mod device_discovery;
mod diagnostics;
mod launch_on_login;
mod mic_check;
mod recording;
mod runtime_log;
mod target_preview;
mod tray;
mod window;

use std::str::FromStr;
use std::sync::Mutex;

use app_core::{AppCore, RecorderSnapshot};
use capture::CaptureController;
use shortcuts::{ShortcutAction, ShortcutBinding};
use tauri::{AppHandle, Emitter, Manager, RunEvent, WindowEvent};
use tauri_plugin_global_shortcut::{
    Builder as GlobalShortcutBuilder, GlobalShortcutExt, Shortcut, ShortcutState,
};

pub struct AppState {
    core: Mutex<AppCore>,
    recorder: Mutex<Option<Box<dyn CaptureController>>>,
    mic_check: Mutex<Option<mic_check::MicCheckProcess>>,
}

pub(crate) fn with_core<T>(
    app: &AppHandle,
    handler: impl FnOnce(&mut AppCore) -> T,
) -> Result<T, String> {
    let state = app.state::<AppState>();
    let mut core = state
        .core
        .lock()
        .map_err(|_| "failed to lock app state".to_string())?;

    Ok(handler(&mut core))
}

pub(crate) fn emit_recorder_state(app: &AppHandle, snapshot: &RecorderSnapshot) {
    let _ = tray::sync_recorder_state(app, snapshot);
    let _ = app.emit("recorder://state-changed", snapshot);
}

pub(crate) fn emit_runtime_error(app: &AppHandle, message: &str) {
    runtime_log::log_runtime_error(message);
    let _ = app.emit("recorder://runtime-error", message.to_string());
}

pub(crate) fn emit_recent_sessions_refresh_request(app: &AppHandle) {
    let _ = app.emit("recorder://recent-sessions-refresh-requested", ());
}

pub(crate) fn persist_settings(app: &AppHandle) -> Result<(), String> {
    let settings = with_core(app, |core| core.settings())?;
    storage::save_app_settings(&settings).map_err(|error| error.to_string())
}

pub(crate) fn persist_shortcuts(app: &AppHandle) -> Result<(), String> {
    let shortcuts = with_core(app, |core| core.shortcuts())?;
    storage::save_shortcuts(&shortcuts).map_err(|error| error.to_string())
}

fn load_initial_settings() -> storage::AppSettings {
    match storage::load_app_settings() {
        Ok(settings) => settings,
        Err(error) => {
            eprintln!("failed to load persisted settings, falling back to defaults: {error}");
            storage::AppSettings::default()
        }
    }
}

fn load_initial_shortcuts() -> Vec<ShortcutBinding> {
    match storage::load_shortcuts() {
        Ok(shortcuts) => shortcuts,
        Err(error) => {
            eprintln!("failed to load persisted shortcuts, falling back to defaults: {error}");
            shortcuts::default_shortcuts()
        }
    }
}

pub(crate) fn parse_shortcut_accelerator(accelerator: &str) -> Result<Shortcut, String> {
    let mut normalized_tokens = Vec::new();

    for token in accelerator.split('+') {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            continue;
        }

        let normalized = match trimmed.to_ascii_lowercase().as_str() {
            "cmdorctrl" => {
                if cfg!(target_os = "macos") {
                    "Super".to_string()
                } else {
                    "Control".to_string()
                }
            }
            "cmd" | "command" => "Super".to_string(),
            "ctrl" | "control" => "Control".to_string(),
            "alt" | "option" => "Alt".to_string(),
            "shift" => "Shift".to_string(),
            "space" => "Space".to_string(),
            "enter" | "return" => "Enter".to_string(),
            "escape" | "esc" => "Escape".to_string(),
            "tab" => "Tab".to_string(),
            "backspace" => "Backspace".to_string(),
            "delete" => "Delete".to_string(),
            "up" => "ArrowUp".to_string(),
            "down" => "ArrowDown".to_string(),
            "left" => "ArrowLeft".to_string(),
            "right" => "ArrowRight".to_string(),
            value
                if value.len() == 1
                    && value
                        .chars()
                        .all(|character| character.is_ascii_alphabetic()) =>
            {
                format!("Key{}", value.to_ascii_uppercase())
            }
            value
                if value.len() == 1
                    && value.chars().all(|character| character.is_ascii_digit()) =>
            {
                format!("Digit{value}")
            }
            value
                if value.starts_with('f')
                    && value.len() <= 3
                    && value[1..]
                        .chars()
                        .all(|character| character.is_ascii_digit()) =>
            {
                value.to_ascii_uppercase()
            }
            _ => trimmed.to_string(),
        };

        normalized_tokens.push(normalized);
    }

    if normalized_tokens.is_empty() {
        return Err("shortcut accelerator is empty".to_string());
    }

    Shortcut::from_str(&normalized_tokens.join("+")).map_err(|error| {
        format!(
            "shortcut `{accelerator}` is invalid. Use a combination like CmdOrCtrl+Shift+R. {error}"
        )
    })
}

fn action_for(app: &AppHandle, shortcut: &Shortcut) -> Option<ShortcutAction> {
    let bindings = with_core(app, |core| core.shortcuts()).ok()?;
    bindings
        .into_iter()
        .filter(|binding| binding.enabled)
        .find_map(|binding| {
            parse_shortcut_accelerator(&binding.accelerator)
                .ok()
                .filter(|candidate| candidate == shortcut)
                .map(|_| binding.action)
        })
}

pub(crate) fn register_shortcuts(app: &AppHandle) -> Result<(), String> {
    let shortcut_manager = app.global_shortcut();
    let bindings = with_core(app, |core| core.shortcuts())?;

    shortcut_manager
        .unregister_all()
        .map_err(|error| error.to_string())?;

    for binding in bindings.into_iter().filter(|binding| binding.enabled) {
        shortcut_manager
            .register(parse_shortcut_accelerator(&binding.accelerator)?)
            .map_err(|error| error.to_string())?;
    }

    Ok(())
}

pub(crate) fn handle_shortcut_action(app: &AppHandle, action: ShortcutAction) {
    match action {
        ShortcutAction::ToggleRecording => {
            if let Err(error) = recording::toggle_recording(app) {
                emit_runtime_error(app, &error);
            }
        }
        ShortcutAction::PauseRecording => {
            if let Err(error) = recording::pause_resume(app) {
                emit_runtime_error(app, &error);
            }
        }
        ShortcutAction::OpenLauncher => {
            let _ = window::focus_launcher(app);
        }
        ShortcutAction::ToggleMicrophone => {
            if let Ok(snapshot) = with_core(app, AppCore::toggle_microphone) {
                emit_recorder_state(app, &snapshot);
                if let Err(error) = persist_settings(app) {
                    emit_runtime_error(app, &error);
                }
            }
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let initial_settings = load_initial_settings();
    let initial_shortcuts = load_initial_shortcuts();

    tauri::Builder::default()
        .manage(AppState {
            core: Mutex::new(AppCore::new(initial_settings, initial_shortcuts)),
            recorder: Mutex::new(None),
            mic_check: Mutex::new(None),
        })
        .plugin(
            GlobalShortcutBuilder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        if let Some(action) = action_for(app, &shortcut) {
                            handle_shortcut_action(app, action);
                        }
                    }
                })
                .build(),
        )
        .setup(|app| {
            runtime_log::init(&app.package_info().version.to_string());
            runtime_log::log_runtime_diagnostics(&diagnostics::initial_runtime_diagnostics());
            let launch_on_login_enabled =
                with_core(app.handle(), |core| core.settings().launch_on_login).unwrap_or(false);
            if let Err(error) = launch_on_login::sync_launch_on_login(launch_on_login_enabled) {
                eprintln!("failed to sync launch-on-login state: {error}");
            }
            register_shortcuts(app.handle())?;
            tray::create(app.handle())?;
            window::ensure_hud_window(app.handle())?;
            window::focus_launcher(app.handle())?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == window::MAIN_WINDOW_LABEL
                    || window.label() == window::HUD_WINDOW_LABEL
                    || window.label() == window::REGION_SELECTOR_WINDOW_LABEL
                    || window.label() == window::TARGET_PREVIEW_WINDOW_LABEL
                {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::bootstrap::get_bootstrap,
            commands::bootstrap::get_runtime_diagnostics,
            commands::bootstrap::get_audio_inputs,
            commands::bootstrap::get_capture_targets,
            commands::bootstrap::reset_shortcuts,
            commands::bootstrap::update_shortcut,
            commands::library::get_recent_recordings,
            commands::library::open_recording,
            commands::library::reveal_recording_in_folder,
            commands::library::save_recording_copy,
            commands::library::trash_recordings,
            commands::permissions::get_permissions,
            commands::permissions::open_permission_settings,
            commands::permissions::request_permission,
            commands::recorder::get_recorder_snapshot,
            commands::recorder::pause_resume,
            commands::recorder::start_mic_check,
            commands::recorder::stop_mic_check,
            commands::recorder::toggle_microphone,
            commands::recorder::toggle_recording,
            commands::settings::update_launch_on_login,
            commands::settings::update_show_hud_during_recording,
            commands::settings::update_capture_target,
            commands::settings::update_audio_input,
            commands::settings::update_system_audio_enabled,
            commands::settings::update_custom_region,
            commands::settings::pick_output_directory,
            commands::settings::update_output_directory,
            commands::settings::update_quality_preset,
            commands::window::focus_launcher,
            commands::window::hide_hud,
            commands::window::hide_region_selector,
            commands::window::show_hud,
            commands::window::show_region_selector,
            commands::window::start_hud_drag
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let RunEvent::ExitRequested { .. } = event {
                recording::finalize_active_recording_before_exit(app);
            }
        });
}
