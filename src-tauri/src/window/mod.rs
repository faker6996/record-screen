use std::{
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::Duration,
};

use app_core::{RecorderSnapshot, RecorderStatus};
use tauri::{
    AppHandle, Emitter, Error as TauriError, Manager, PhysicalPosition, PhysicalSize, Position,
    Size, WebviewUrl, WebviewWindowBuilder,
};

use crate::{emit_recorder_state, with_core};

pub const MAIN_WINDOW_LABEL: &str = "main";
pub const HUD_WINDOW_LABEL: &str = "hud";
pub const REGION_SELECTOR_WINDOW_LABEL: &str = "region-selector";
pub const TARGET_PREVIEW_WINDOW_LABEL: &str = "target-preview";

static TARGET_PREVIEW_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct RegionSelectorContext {
    origin_x: i32,
    origin_y: i32,
    width: u32,
    height: u32,
    scale_factor: f64,
    capture_target_id: String,
}

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
            .inner_size(294.0, 62.0)
            .min_inner_size(264.0, 56.0)
            .visible(false)
            .resizable(false)
            .always_on_top(true)
            .decorations(false)
            .skip_taskbar(true)
            .shadow(false)
            .transparent(true);

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

fn region_selector_context(app: &AppHandle) -> Result<RegionSelectorContext, String> {
    let main_window = app
        .get_webview_window(MAIN_WINDOW_LABEL)
        .ok_or_else(|| "main window is not available".to_string())?;
    let monitor = main_window
        .current_monitor()
        .map_err(|error| error.to_string())?
        .or_else(|| app.primary_monitor().ok().flatten())
        .ok_or_else(|| "no active monitor is available for region selection".to_string())?;
    let size = monitor.size();
    let position = monitor.position();
    let capture_target_id = current_monitor_capture_target_id(app, &monitor);

    Ok(RegionSelectorContext {
        origin_x: position.x,
        origin_y: position.y,
        width: size.width,
        height: size.height,
        scale_factor: monitor.scale_factor(),
        capture_target_id,
    })
}

fn region_selector_init_script(context: &RegionSelectorContext) -> String {
    format!(
        "window.__RECORD_SCREEN_SURFACE__ = 'region-selector'; window.__RECORD_SCREEN_SELECTOR_CONTEXT__ = {{ originX: {}, originY: {}, width: {}, height: {}, scaleFactor: {}, captureTargetId: {:?} }};",
        context.origin_x,
        context.origin_y,
        context.width,
        context.height,
        context.scale_factor,
        context.capture_target_id
    )
}

#[cfg(target_os = "macos")]
fn current_monitor_capture_target_id(app: &AppHandle, monitor: &tauri::Monitor) -> String {
    let mut monitors = match app.available_monitors() {
        Ok(monitors) => monitors,
        Err(_) => return capture::FULL_DESKTOP_TARGET_ID.to_string(),
    };
    monitors.sort_by_key(|candidate| {
        let position = candidate.position();
        (position.y, position.x)
    });

    let current_index = monitors.iter().position(|candidate| {
        candidate.position() == monitor.position() && candidate.size() == monitor.size()
    });

    let monitor_targets: Vec<_> = capture_macos::list_capture_targets()
        .into_iter()
        .filter(|target| target.id != capture::FULL_DESKTOP_TARGET_ID)
        .collect();

    current_index
        .and_then(|index| monitor_targets.get(index))
        .map(|target| target.id.clone())
        .unwrap_or_else(|| capture::FULL_DESKTOP_TARGET_ID.to_string())
}

#[cfg(not(target_os = "macos"))]
fn current_monitor_capture_target_id(_app: &AppHandle, _monitor: &tauri::Monitor) -> String {
    capture::FULL_DESKTOP_TARGET_ID.to_string()
}

pub fn show_region_selector(app: &AppHandle) -> Result<(), String> {
    let context = region_selector_context(app)?;
    let init_script = region_selector_init_script(&context);
    let logical_width = context.width as f64 / context.scale_factor.max(1.0);
    let logical_height = context.height as f64 / context.scale_factor.max(1.0);
    let logical_x = context.origin_x as f64 / context.scale_factor.max(1.0);
    let logical_y = context.origin_y as f64 / context.scale_factor.max(1.0);

    if let Some(window) = app.get_webview_window(REGION_SELECTOR_WINDOW_LABEL) {
        let _ = window.eval(&init_script);
        if window.is_minimized().map_err(|error| error.to_string())? {
            window.unminimize().map_err(|error| error.to_string())?;
        }
        window.show().map_err(|error| error.to_string())?;
        window.set_focus().map_err(|error| error.to_string())?;
        return Ok(());
    }

    let mut builder = WebviewWindowBuilder::new(
        app,
        REGION_SELECTOR_WINDOW_LABEL,
        WebviewUrl::App("index.html".into()),
    )
    .title("Record Screen Region Selector")
    .initialization_script(&init_script)
    .inner_size(logical_width, logical_height)
    .position(logical_x, logical_y)
    .visible(true)
    .focused(true)
    .resizable(false)
    .always_on_top(true)
    .decorations(false)
    .skip_taskbar(true)
    .shadow(false)
    .transparent(true);

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

pub fn hide_region_selector(app: &AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(REGION_SELECTOR_WINDOW_LABEL) {
        window.hide().map_err(|error| error.to_string())?;
    }

    Ok(())
}

pub fn ensure_target_preview_window(app: &AppHandle) -> Result<(), String> {
    if app
        .get_webview_window(TARGET_PREVIEW_WINDOW_LABEL)
        .is_some()
    {
        return Ok(());
    }

    let mut builder = WebviewWindowBuilder::new(
        app,
        TARGET_PREVIEW_WINDOW_LABEL,
        WebviewUrl::App("index.html".into()),
    )
    .title("Record Screen Target Preview")
    .initialization_script("window.__RECORD_SCREEN_SURFACE__ = 'target-preview';")
    .inner_size(320.0, 180.0)
    .visible(false)
    .focused(false)
    .resizable(false)
    .always_on_top(true)
    .decorations(false)
    .skip_taskbar(true)
    .shadow(false)
    .transparent(true);

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

pub fn show_target_preview(
    app: &AppHandle,
    bounds: crate::target_preview::PreviewBounds,
) -> Result<(), String> {
    ensure_target_preview_window(app)?;

    let sequence = TARGET_PREVIEW_SEQUENCE.fetch_add(1, Ordering::Relaxed) + 1;
    if let Some(window) = app.get_webview_window(TARGET_PREVIEW_WINDOW_LABEL) {
        window
            .set_position(Position::Physical(PhysicalPosition::new(
                bounds.x, bounds.y,
            )))
            .map_err(|error| error.to_string())?;
        window
            .set_size(Size::Physical(PhysicalSize::new(
                bounds.width.max(64),
                bounds.height.max(64),
            )))
            .map_err(|error| error.to_string())?;
        window.show().map_err(|error| error.to_string())?;
    }

    let app_handle = app.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(1400));
        if TARGET_PREVIEW_SEQUENCE.load(Ordering::Relaxed) != sequence {
            return;
        }

        let _ = hide_target_preview(&app_handle);
    });

    Ok(())
}

pub fn hide_target_preview(app: &AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(TARGET_PREVIEW_WINDOW_LABEL) {
        window.hide().map_err(|error| error.to_string())?;
    }

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

pub fn start_hud_drag(app: &AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window(HUD_WINDOW_LABEL)
        .ok_or_else(|| "HUD window is not available.".to_string())?;

    window.start_dragging().map_err(|error| error.to_string())
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
