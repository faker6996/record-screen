use std::sync::OnceLock;

use app_core::{RecorderSnapshot, RecorderStatus};
use tauri::{
    AppHandle, Wry,
    menu::{MenuBuilder, MenuItem},
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

static TRAY_MENU_ITEMS: OnceLock<TrayMenuItems> = OnceLock::new();

#[derive(Clone)]
struct TrayMenuItems {
    toggle_recording: MenuItem<Wry>,
    pause_resume: MenuItem<Wry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TrayMenuPresentation {
    toggle_recording_label: String,
    toggle_recording_enabled: bool,
    pause_resume_label: String,
    pause_resume_enabled: bool,
}

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

    let toggle_recording = menu
        .get(MENU_TOGGLE_RECORDING)
        .and_then(|item| item.as_menuitem().cloned())
        .ok_or_else(|| "failed to resolve tray toggle-recording menu item".to_string())?;
    let pause_resume = menu
        .get(MENU_PAUSE_RESUME)
        .and_then(|item| item.as_menuitem().cloned())
        .ok_or_else(|| "failed to resolve tray pause-resume menu item".to_string())?;
    let _ = TRAY_MENU_ITEMS.set(TrayMenuItems {
        toggle_recording,
        pause_resume,
    });

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
    if let Ok(snapshot) = crate::with_core(app, |core| core.snapshot()) {
        let _ = sync_recorder_state(app, &snapshot);
    }
    Ok(())
}

pub fn sync_recorder_state(_app: &AppHandle, snapshot: &RecorderSnapshot) -> Result<(), String> {
    let Some(menu_items) = TRAY_MENU_ITEMS.get() else {
        return Ok(());
    };

    let presentation = tray_menu_presentation(snapshot);
    menu_items
        .toggle_recording
        .set_text(&presentation.toggle_recording_label)
        .map_err(|error| error.to_string())?;
    menu_items
        .toggle_recording
        .set_enabled(presentation.toggle_recording_enabled)
        .map_err(|error| error.to_string())?;
    menu_items
        .pause_resume
        .set_text(&presentation.pause_resume_label)
        .map_err(|error| error.to_string())?;
    menu_items
        .pause_resume
        .set_enabled(presentation.pause_resume_enabled)
        .map_err(|error| error.to_string())?;

    Ok(())
}

fn tray_menu_presentation(snapshot: &RecorderSnapshot) -> TrayMenuPresentation {
    let toggle_recording_label = match snapshot.status {
        RecorderStatus::Idle => "Start recording".to_string(),
        RecorderStatus::Recording | RecorderStatus::Paused => "Stop recording".to_string(),
        RecorderStatus::Finalizing => "Finalizing recording...".to_string(),
    };
    let toggle_recording_enabled = snapshot.status != RecorderStatus::Finalizing;

    let (pause_resume_label, pause_resume_enabled) = match snapshot.status {
        RecorderStatus::Idle => ("Pause recording".to_string(), false),
        RecorderStatus::Recording if snapshot.can_pause => ("Pause recording".to_string(), true),
        RecorderStatus::Paused if snapshot.can_pause => ("Resume recording".to_string(), true),
        RecorderStatus::Finalizing => ("Finalizing output".to_string(), false),
        RecorderStatus::Recording | RecorderStatus::Paused => (
            snapshot
                .pause_note
                .as_deref()
                .map(|_| "Pause unavailable".to_string())
                .unwrap_or_else(|| "Pause / resume unavailable".to_string()),
            false,
        ),
    };

    TrayMenuPresentation {
        toggle_recording_label,
        toggle_recording_enabled,
        pause_resume_label,
        pause_resume_enabled,
    }
}

#[cfg(test)]
mod tests {
    use super::tray_menu_presentation;
    use app_core::{RecorderSnapshot, RecorderStatus};

    fn snapshot(
        status: RecorderStatus,
        can_pause: bool,
        pause_note: Option<&str>,
    ) -> RecorderSnapshot {
        RecorderSnapshot {
            status,
            elapsed_label: "00:00:00".to_string(),
            active_target: "Full desktop".to_string(),
            active_output_path: None,
            active_encoder_label: None,
            can_pause,
            pause_note: pause_note.map(str::to_string),
            quality_preset: "1080p / 30 fps".to_string(),
            output_directory: "~/Movies/Record Screen".to_string(),
            mic_enabled: true,
        }
    }

    #[test]
    fn idle_snapshot_disables_pause_in_tray() {
        let presentation = tray_menu_presentation(&snapshot(RecorderStatus::Idle, true, None));
        assert_eq!(presentation.toggle_recording_label, "Start recording");
        assert!(presentation.toggle_recording_enabled);
        assert_eq!(presentation.pause_resume_label, "Pause recording");
        assert!(!presentation.pause_resume_enabled);
    }

    #[test]
    fn recording_snapshot_enables_pause_when_supported() {
        let presentation = tray_menu_presentation(&snapshot(RecorderStatus::Recording, true, None));
        assert_eq!(presentation.toggle_recording_label, "Stop recording");
        assert!(presentation.toggle_recording_enabled);
        assert_eq!(presentation.pause_resume_label, "Pause recording");
        assert!(presentation.pause_resume_enabled);
    }

    #[test]
    fn unsupported_pause_surface_is_explicit_in_tray() {
        let presentation = tray_menu_presentation(&snapshot(
            RecorderStatus::Recording,
            false,
            Some("Pause/resume is not available for the active recording backend."),
        ));
        assert_eq!(presentation.toggle_recording_label, "Stop recording");
        assert!(presentation.toggle_recording_enabled);
        assert_eq!(presentation.pause_resume_label, "Pause unavailable");
        assert!(!presentation.pause_resume_enabled);
    }

    #[test]
    fn finalizing_snapshot_disables_stop_and_pause_in_tray() {
        let presentation = tray_menu_presentation(&snapshot(
            RecorderStatus::Finalizing,
            false,
            Some("Recording is finalizing the output file."),
        ));
        assert_eq!(
            presentation.toggle_recording_label,
            "Finalizing recording..."
        );
        assert!(!presentation.toggle_recording_enabled);
        assert_eq!(presentation.pause_resume_label, "Finalizing output");
        assert!(!presentation.pause_resume_enabled);
    }
}
