mod bootstrap;
mod commands;
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
            }
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState {
            core: Mutex::new(AppCore::default()),
            recorder: Mutex::new(None),
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
                {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::bootstrap::get_bootstrap,
            commands::bootstrap::reset_shortcuts,
            commands::permissions::get_permissions,
            commands::permissions::open_permission_settings,
            commands::permissions::request_permission,
            commands::recorder::pause_resume,
            commands::recorder::toggle_microphone,
            commands::recorder::toggle_recording,
            commands::settings::update_launch_on_login,
            commands::settings::update_output_directory,
            commands::settings::update_quality_preset,
            commands::window::focus_launcher,
            commands::window::hide_hud,
            commands::window::show_hud
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
