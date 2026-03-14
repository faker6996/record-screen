use app_core::{RecorderSnapshot, RecorderStatus};
use tauri::{AppHandle, Emitter, Error as TauriError, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::{emit_recorder_state, with_core};

pub const MAIN_WINDOW_LABEL: &str = "main";
pub const HUD_WINDOW_LABEL: &str = "hud";

pub fn focus_launcher(app: &AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        if window.is_minimized().map_err(|error| error.to_string())? {
            window.unminimize().map_err(|error| error.to_string())?;
        }
        window.show().map_err(|error| error.to_string())?;
        window.set_focus().map_err(|error| error.to_string())?;
    }

    Ok(())
}

pub fn ensure_hud_window(app: &AppHandle) -> Result<(), String> {
    if app.get_webview_window(HUD_WINDOW_LABEL).is_some() {
        return Ok(());
    }

    let mut builder =
        WebviewWindowBuilder::new(app, HUD_WINDOW_LABEL, WebviewUrl::App("index.html".into()))
            .title("Record Screen HUD")
            .initialization_script("window.__RECORD_SCREEN_SURFACE__ = 'hud';")
            .inner_size(328.0, 80.0)
            .min_inner_size(292.0, 72.0)
            .visible(false)
            .resizable(false)
            .always_on_top(true)
            .decorations(false)
            .skip_taskbar(true)
            .shadow(true);

    #[cfg(not(target_os = "macos"))]
    {
        builder = builder.transparent(true);
    }

    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder
            .icon(icon)
            .map_err(|error: TauriError| error.to_string())?;
    }

    builder
        .build()
        .map_err(|error: TauriError| error.to_string())?;
    Ok(())
}

pub fn show_hud(app: &AppHandle) -> Result<(), String> {
    ensure_hud_window(app)?;

    if let Some(window) = app.get_webview_window(HUD_WINDOW_LABEL) {
        if window.is_minimized().map_err(|error| error.to_string())? {
            window.unminimize().map_err(|error| error.to_string())?;
        }
        window
            .set_always_on_top(true)
            .map_err(|error| error.to_string())?;
        window.show().map_err(|error| error.to_string())?;
    }

    if let Ok(snapshot) = with_core(app, |core| core.snapshot()) {
        emit_recorder_state(app, &snapshot);
    }
    let _ = app.emit("recorder://hud-shown", ());

    Ok(())
}

pub fn hide_hud(app: &AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(HUD_WINDOW_LABEL) {
        window.hide().map_err(|error| error.to_string())?;
    }

    Ok(())
}

pub fn sync_hud_visibility(
    app: &AppHandle,
    snapshot: &RecorderSnapshot,
    show_hud_during_recording: bool,
) -> Result<(), String> {
    match snapshot.status {
        RecorderStatus::Idle => hide_hud(app),
        RecorderStatus::Recording | RecorderStatus::Paused => {
            if show_hud_during_recording {
                show_hud(app)
            } else {
                hide_hud(app)
            }
        }
    }
}
