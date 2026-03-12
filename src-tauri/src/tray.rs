use tauri::{
    AppHandle,
    menu::MenuBuilder,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

use crate::{handle_shortcut_action, window};
use shortcuts::ShortcutAction;

const TRAY_ID: &str = "main-tray";
const MENU_SHOW_LAUNCHER: &str = "show-launcher";
const MENU_TOGGLE_RECORDING: &str = "toggle-recording";
const MENU_PAUSE_RESUME: &str = "pause-resume";
const MENU_TOGGLE_MICROPHONE: &str = "toggle-microphone";
const MENU_SHOW_HUD: &str = "show-hud";
const MENU_HIDE_HUD: &str = "hide-hud";
const MENU_QUIT: &str = "quit";

pub fn create(app: &AppHandle) -> Result<(), String> {
    let menu = MenuBuilder::new(app)
        .text(MENU_SHOW_LAUNCHER, "Show launcher")
        .separator()
        .text(MENU_TOGGLE_RECORDING, "Start / stop recording")
        .text(MENU_PAUSE_RESUME, "Pause / resume")
        .text(MENU_TOGGLE_MICROPHONE, "Mute / unmute microphone")
        .separator()
        .text(MENU_SHOW_HUD, "Show HUD")
        .text(MENU_HIDE_HUD, "Hide HUD")
        .separator()
        .text(MENU_QUIT, "Quit")
        .build()
        .map_err(|error| error.to_string())?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .tooltip("Record Screen")
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            MENU_SHOW_LAUNCHER => {
                let _ = window::focus_launcher(app);
            }
            MENU_TOGGLE_RECORDING => handle_shortcut_action(app, ShortcutAction::ToggleRecording),
            MENU_PAUSE_RESUME => handle_shortcut_action(app, ShortcutAction::PauseRecording),
            MENU_TOGGLE_MICROPHONE => handle_shortcut_action(app, ShortcutAction::ToggleMicrophone),
            MENU_SHOW_HUD => {
                let _ = window::show_hud(app);
            }
            MENU_HIDE_HUD => {
                let _ = window::hide_hud(app);
            }
            MENU_QUIT => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let _ = window::focus_launcher(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    }

    builder.build(app).map_err(|error| error.to_string())?;
    Ok(())
}
