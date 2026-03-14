mod audio_inputs;
mod bootstrap;
mod capture_targets;
mod commands;
mod device_discovery;
mod diagnostics;
mod launch_on_login;
mod mic_check;
mod recording;
mod tray;
mod window;

use std::sync::Mutex;

use app_core::{AppCore, RecorderSnapshot};
use capture::CaptureController;
use shortcuts::ShortcutAction;
use tauri::{AppHandle, Emitter, Manager, WindowEvent};
use tauri_plugin_global_shortcut::{
    Builder as GlobalShortcutBuilder, Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState,
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
    let _ = app.emit("recorder://state-changed", snapshot);
}

pub(crate) fn emit_runtime_error(app: &AppHandle, message: &str) {
    let _ = app.emit("recorder://runtime-error", message.to_string());
}

pub(crate) fn emit_recent_sessions_refresh_request(app: &AppHandle) {
    let _ = app.emit("recorder://recent-sessions-refresh-requested", ());
}

pub(crate) fn persist_settings(app: &AppHandle) -> Result<(), String> {
    let settings = with_core(app, |core| core.settings())?;
    storage::save_app_settings(&settings).map_err(|error| error.to_string())
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

fn command_or_control_modifier() -> Modifiers {
    if cfg!(target_os = "macos") {
        Modifiers::SUPER
    } else {
        Modifiers::CONTROL
    }
}

fn shortcut_for(action: ShortcutAction) -> Shortcut {
    let modifiers = Some(command_or_control_modifier() | Modifiers::SHIFT);

    match action {
        ShortcutAction::ToggleRecording => Shortcut::new(modifiers, Code::KeyR),
        ShortcutAction::PauseRecording => Shortcut::new(modifiers, Code::KeyP),
        ShortcutAction::OpenLauncher => Shortcut::new(modifiers, Code::KeyL),
        ShortcutAction::ToggleMicrophone => Shortcut::new(modifiers, Code::KeyM),
    }
}

fn action_for(shortcut: &Shortcut) -> Option<ShortcutAction> {
    ShortcutAction::ALL
        .into_iter()
        .find(|action| shortcut == &shortcut_for(*action))
}

pub(crate) fn register_shortcuts(app: &AppHandle) -> Result<(), String> {
    let shortcut_manager = app.global_shortcut();

    shortcut_manager
        .unregister_all()
        .map_err(|error| error.to_string())?;

    for action in ShortcutAction::ALL {
        shortcut_manager
            .register(shortcut_for(action))
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

    tauri::Builder::default()
        .manage(AppState {
            core: Mutex::new(AppCore::new(initial_settings)),
            recorder: Mutex::new(None),
            mic_check: Mutex::new(None),
        })
        .plugin(
            GlobalShortcutBuilder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        if let Some(action) = action_for(&shortcut) {
                            handle_shortcut_action(app, action);
                        }
                    }
                })
                .build(),
        )
        .setup(|app| {
            let launch_on_login_enabled =
                with_core(app.handle(), |core| core.settings().launch_on_login).unwrap_or(false);
            if let Err(error) = launch_on_login::sync_launch_on_login(launch_on_login_enabled) {
                eprintln!("failed to sync launch-on-login state: {error}");
            }
            register_shortcuts(app.handle())?;
            tray::create(app.handle())?;
            window::focus_launcher(app.handle())?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == window::MAIN_WINDOW_LABEL
                    || window.label() == window::HUD_WINDOW_LABEL
                {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::bootstrap::get_bootstrap,
            commands::bootstrap::get_audio_inputs,
            commands::bootstrap::get_capture_targets,
            commands::bootstrap::reset_shortcuts,
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
            commands::settings::pick_output_directory,
            commands::settings::update_output_directory,
            commands::settings::update_quality_preset,
            commands::window::focus_launcher,
            commands::window::hide_hud,
            commands::window::show_hud,
            commands::window::start_hud_drag
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
