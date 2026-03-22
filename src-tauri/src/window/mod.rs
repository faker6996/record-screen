use std::{
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::Duration,
};

#[cfg(target_os = "linux")]
use std::sync::mpsc;

use app_core::{RecorderSnapshot, RecorderStatus};
use tauri::{
    AppHandle, Emitter, Error as TauriError, Manager, PhysicalPosition, PhysicalSize, Position,
    Size, WebviewUrl, WebviewWindowBuilder,
};

use crate::{emit_recorder_state, runtime_log, with_core};

pub const MAIN_WINDOW_LABEL: &str = "main";
pub const HUD_WINDOW_LABEL: &str = "hud";
pub const REGION_SELECTOR_WINDOW_LABEL: &str = "region-selector";
pub const TARGET_PREVIEW_WINDOW_LABEL: &str = "target-preview";
const HUD_DEFAULT_WIDTH: u32 = 294;
const HUD_DEFAULT_HEIGHT: u32 = 62;
const HUD_MARGIN_PX: i32 = 24;

static TARGET_PREVIEW_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[cfg(target_os = "linux")]
static EXPORTED_PORTAL_PARENT_WINDOW: std::sync::OnceLock<Option<String>> =
    std::sync::OnceLock::new();

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

#[cfg(target_os = "linux")]
pub fn portal_parent_window_handle(app: &AppHandle) -> Option<String> {
    let handle = EXPORTED_PORTAL_PARENT_WINDOW
        .get_or_init(|| export_wayland_parent_window_handle(app).ok().flatten())
        .clone();
    if let Some(handle) = &handle {
        eprintln!("[wayland-parent] exported parent window handle: {handle}");
    } else {
        eprintln!("[wayland-parent] no exported parent window handle was available");
    }
    handle
}

#[cfg(not(target_os = "linux"))]
pub fn portal_parent_window_handle(_app: &AppHandle) -> Option<String> {
    None
}

#[cfg(target_os = "linux")]
fn export_wayland_parent_window_handle(app: &AppHandle) -> Result<Option<String>, String> {
    use glib::translate::ToGlibPtr;
    use gtk::prelude::WidgetExt;
    use std::{
        ffi::CStr,
        os::raw::{c_char, c_void},
    };

    unsafe extern "C" fn on_exported_handle(
        _window: *mut gdk_wayland_sys::GdkWaylandWindow,
        handle: *const c_char,
        user_data: *mut c_void,
    ) {
        let sender = unsafe { &*(user_data as *const mpsc::SyncSender<Result<String, String>>) };
        let result = if handle.is_null() {
            Err("Wayland exported-handle callback returned a null handle".to_string())
        } else {
            let handle = unsafe { CStr::from_ptr(handle) }
                .to_string_lossy()
                .into_owned();
            Ok(format!("wayland:{handle}"))
        };
        let _ = sender.send(result);
    }

    unsafe extern "C" fn drop_export_sender(user_data: *mut c_void) {
        unsafe {
            drop(Box::<mpsc::SyncSender<Result<String, String>>>::from_raw(
                user_data as *mut _,
            ));
        }
    }

    let window = app
        .get_webview_window(MAIN_WINDOW_LABEL)
        .ok_or_else(|| "main window is not available".to_string())?;
    let window_for_main_thread = window.clone();
    let (result_tx, result_rx) = mpsc::sync_channel(1);

    window
        .run_on_main_thread(move || {
            let result = (|| -> Result<Option<String>, String> {
                let gtk_window = window_for_main_thread
                    .gtk_window()
                    .map_err(|error| error.to_string())?;
                let Some(gdk_window) = gtk_window.window() else {
                    return Err("main GTK window is not realized yet".to_string());
                };
                let (handle_tx, handle_rx) = mpsc::sync_channel(1);
                let gdk_window_ptr: *mut gtk::gdk::ffi::GdkWindow = gdk_window.to_glib_none().0;

                let sender_ptr = Box::into_raw(Box::new(handle_tx)) as *mut c_void;
                let exported = unsafe {
                    gdk_wayland_sys::gdk_wayland_window_export_handle(
                        gdk_window_ptr.cast(),
                        Some(on_exported_handle),
                        sender_ptr,
                        Some(drop_export_sender),
                    )
                };
                if exported == 0 {
                    unsafe {
                        drop_export_sender(sender_ptr);
                    }
                    return Ok(None);
                }

                let main_context = glib::MainContext::default();
                let started_at = std::time::Instant::now();
                loop {
                    match handle_rx.try_recv() {
                        Ok(Ok(handle)) => return Ok(Some(handle)),
                        Ok(Err(error)) => return Err(error),
                        Err(mpsc::TryRecvError::Empty) => {
                            if started_at.elapsed() >= Duration::from_secs(2) {
                                unsafe {
                                    gdk_wayland_sys::gdk_wayland_window_unexport_handle(
                                        gdk_window_ptr.cast(),
                                    );
                                }
                                return Err(
                                    "timed out while exporting the Wayland parent window handle"
                                        .to_string(),
                                );
                            }
                            main_context.iteration(true);
                        }
                        Err(mpsc::TryRecvError::Disconnected) => {
                            unsafe {
                                gdk_wayland_sys::gdk_wayland_window_unexport_handle(
                                    gdk_window_ptr.cast(),
                                );
                            }
                            return Err(
                                "the Wayland exported-handle callback disconnected unexpectedly"
                                    .to_string(),
                            );
                        }
                    }
                }
            })();

            let _ = result_tx.send(result);
        })
        .map_err(|error| error.to_string())?;

    match result_rx.recv_timeout(Duration::from_secs(3)) {
        Ok(Ok(handle)) => Ok(handle),
        Ok(Err(error)) => Err(error),
        Err(mpsc::RecvTimeoutError::Timeout) => {
            Err("timed out while waiting to export the Wayland parent window handle".to_string())
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            Err("the main-thread Wayland handle exporter disconnected unexpectedly".to_string())
        }
    }
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

#[cfg(target_os = "macos")]
pub fn schedule_hud_window_prewarm(app: &AppHandle) {
    let should_prewarm =
        with_core(app, |core| core.settings().show_hud_during_recording).unwrap_or(true);
    if !should_prewarm || app.get_webview_window(HUD_WINDOW_LABEL).is_some() {
        return;
    }

    let app_handle = app.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(900));
        let Some(main_window) = app_handle.get_webview_window(MAIN_WINDOW_LABEL) else {
            return;
        };
        let app_for_main_thread = app_handle.clone();
        let _ = main_window.run_on_main_thread(move || {
            if let Err(error) = ensure_hud_window(&app_for_main_thread) {
                runtime_log::log_runtime_error(&format!(
                    "unable to prewarm HUD window on macOS: {}",
                    error
                ));
            }
        });
    });
}

#[cfg(not(target_os = "macos"))]
pub fn schedule_hud_window_prewarm(_app: &AppHandle) {}

fn position_hud_window(app: &AppHandle, window: &tauri::WebviewWindow) -> Result<(), String> {
    let monitor = app
        .get_webview_window(MAIN_WINDOW_LABEL)
        .and_then(|main_window| main_window.current_monitor().ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten())
        .ok_or_else(|| "no monitor is available for HUD placement".to_string())?;

    let hud_size = window
        .outer_size()
        .unwrap_or(PhysicalSize::new(HUD_DEFAULT_WIDTH, HUD_DEFAULT_HEIGHT));
    let monitor_size = monitor.size();
    let monitor_position = monitor.position();
    let x = monitor_position.x + monitor_size.width as i32 - hud_size.width as i32 - HUD_MARGIN_PX;
    let y = monitor_position.y + HUD_MARGIN_PX;

    window
        .set_position(Position::Physical(PhysicalPosition::new(
            x.max(monitor_position.x),
            y,
        )))
        .map_err(|error| error.to_string())
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
    let monitors = match app.available_monitors() {
        Ok(monitors) => monitors,
        Err(_) => return capture::FULL_DESKTOP_TARGET_ID.to_string(),
    };

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
    preview: crate::target_preview::PreviewPresentation,
) -> Result<(), String> {
    ensure_target_preview_window(app)?;

    let sequence = TARGET_PREVIEW_SEQUENCE.fetch_add(1, Ordering::Relaxed) + 1;
    if let Some(window) = app.get_webview_window(TARGET_PREVIEW_WINDOW_LABEL) {
        let payload = serde_json::json!({
            "title": preview.title,
            "sequence": sequence,
        });
        window
            .set_position(Position::Physical(PhysicalPosition::new(
                preview.bounds.x,
                preview.bounds.y,
            )))
            .map_err(|error| error.to_string())?;
        window
            .set_size(Size::Physical(PhysicalSize::new(
                preview.bounds.width.max(64),
                preview.bounds.height.max(64),
            )))
            .map_err(|error| error.to_string())?;
        let script = format!(
            "window.__RECORD_SCREEN_TARGET_PREVIEW_CONTEXT__ = {payload}; window.dispatchEvent(new Event('record-screen:target-preview'));"
        );
        let _ = window.eval(&script);
        window.show().map_err(|error| error.to_string())?;
    }

    let app_handle = app.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(3000));
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
        let should_reposition = !window.is_visible().unwrap_or(false);
        runtime_log::log_runtime_info(&format!(
            "hud show requested | repositioned={should_reposition}"
        ));
        if window.is_minimized().map_err(|error| error.to_string())? {
            window.unminimize().map_err(|error| error.to_string())?;
        }
        if should_reposition {
            position_hud_window(app, &window)?;
        }
        window
            .set_always_on_top(true)
            .map_err(|error| error.to_string())?;
        window.show().map_err(|error| error.to_string())?;
        runtime_log::log_runtime_info(&format!("hud shown | repositioned={should_reposition}"));
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
        runtime_log::log_runtime_info("hud hidden");
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
        RecorderStatus::Recording | RecorderStatus::Paused | RecorderStatus::Finalizing => {
            if show_hud_during_recording {
                show_hud(app)
            } else {
                hide_hud(app)
            }
        }
    }
}
