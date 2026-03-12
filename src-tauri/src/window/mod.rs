use app_core::{RecorderSnapshot, RecorderStatus};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

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
            .inner_size(340.0, 148.0)
            .min_inner_size(280.0, 116.0)
            .visible(false)
            .resizable(false)
            .always_on_top(true)
            .decorations(false)
            .skip_taskbar(true)
            .shadow(true);

    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon).map_err(|error| error.to_string())?;
    }

    builder.build().map_err(|error| error.to_string())?;
    Ok(())
}

pub fn show_hud(app: &AppHandle) -> Result<(), String> {
    ensure_hud_window(app)?;

    if let Some(window) = app.get_webview_window(HUD_WINDOW_LABEL) {
        window.show().map_err(|error| error.to_string())?;
        window.set_focus().map_err(|error| error.to_string())?;
    }

    Ok(())
}

pub fn hide_hud(app: &AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(HUD_WINDOW_LABEL) {
        window.hide().map_err(|error| error.to_string())?;
    }

    Ok(())
}

pub fn sync_hud_visibility(app: &AppHandle, snapshot: &RecorderSnapshot) -> Result<(), String> {
    match snapshot.status {
        RecorderStatus::Idle => hide_hud(app),
        RecorderStatus::Recording | RecorderStatus::Paused => show_hud(app),
    }
}
