use std::{
    env, fs,
    path::PathBuf,
    sync::mpsc,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use capture::RecordingOptions;
use capture_linux::selected_backend;
use glib::translate::ToGlibPtr;
use gtk::prelude::{GtkWindowExt, WidgetExt};

struct WaylandPortalParent {
    handle: String,
    #[allow(dead_code)]
    window: gtk::Window,
}

#[test]
#[ignore = "requires a live Linux X11 desktop session with screen-capture access"]
fn linux_smoke_recording_creates_output_file() {
    let display = env::var("DISPLAY").unwrap_or_default();
    assert!(
        !display.trim().is_empty(),
        "DISPLAY must be set for Linux X11 capture smoke tests",
    );

    let output_path = unique_output_path();
    let quality_preset = env::var("RECORD_SCREEN_SMOKE_QUALITY_PRESET")
        .unwrap_or_else(|_| "720p / 30 fps".to_string());
    let duration_secs = env::var("RECORD_SCREEN_SMOKE_DURATION_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(3);
    let keep_output = env::var("RECORD_SCREEN_SMOKE_KEEP_OUTPUT")
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let options = RecordingOptions {
        output_path: output_path.clone(),
        quality_preset,
        mic_enabled: env::var("RECORD_SCREEN_SMOKE_WITH_MIC")
            .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
            .unwrap_or(false),
        system_audio_enabled: false,
        capture_target_id: env::var("RECORD_SCREEN_SMOKE_TARGET_ID")
            .unwrap_or_else(|_| "full-desktop".to_string()),
        audio_input_id: capture::DEFAULT_AUDIO_INPUT_ID.to_string(),
        portal_parent_window: None,
        portal_restore_token: None,
        region_x: 160,
        region_y: 120,
        region_width: 1280,
        region_height: 720,
        region_source_capture_target_id: capture::FULL_DESKTOP_TARGET_ID.to_string(),
        region_source_origin_x: 0,
        region_source_origin_y: 0,
        region_source_scale_factor_milli: 1000,
    };

    let mut controller = selected_backend()
        .start(options)
        .expect("linux capture backend should start");

    thread::sleep(Duration::from_secs(duration_secs));

    let artifact = controller
        .stop()
        .expect("linux capture backend should stop");
    assert_eq!(artifact.output_path, output_path);
    assert!(artifact.duration >= Duration::from_secs(duration_secs.saturating_sub(1).max(1)));
    assert!(artifact.bytes_written > 0);
    assert!(output_path.exists(), "expected recording output to exist");

    let metadata = fs::metadata(&output_path).expect("expected output metadata");
    assert!(metadata.len() > 0, "expected non-empty recording output");

    if keep_output {
        println!("smoke-output={}", output_path.display());
    } else {
        let _ = fs::remove_file(output_path);
    }
}

#[test]
#[ignore = "requires a live Linux Wayland-only session with ScreenCast portal access"]
fn linux_wayland_smoke_recording_creates_output_file() {
    let wayland_display = env::var("WAYLAND_DISPLAY").unwrap_or_default();
    assert!(
        !wayland_display.trim().is_empty(),
        "WAYLAND_DISPLAY must be set for Linux Wayland capture smoke tests",
    );
    assert!(
        env::var("DISPLAY").unwrap_or_default().trim().is_empty(),
        "DISPLAY must be unset for the pure Wayland smoke test",
    );

    let output_path = unique_output_path_with_extension("mp4");
    let portal_parent = exported_wayland_parent_window()
        .expect("expected a Wayland portal parent window for the smoke test");
    let options = RecordingOptions {
        output_path: output_path.clone(),
        quality_preset: "720p / 30 fps".to_string(),
        mic_enabled: env::var("RECORD_SCREEN_SMOKE_WITH_MIC")
            .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
            .unwrap_or(false),
        system_audio_enabled: false,
        capture_target_id: "full-desktop".to_string(),
        audio_input_id: capture::DEFAULT_AUDIO_INPUT_ID.to_string(),
        portal_parent_window: Some(portal_parent.handle.clone()),
        portal_restore_token: None,
        region_x: 160,
        region_y: 120,
        region_width: 1280,
        region_height: 720,
        region_source_capture_target_id: capture::FULL_DESKTOP_TARGET_ID.to_string(),
        region_source_origin_x: 0,
        region_source_origin_y: 0,
        region_source_scale_factor_milli: 1000,
    };

    let mut controller = selected_backend()
        .start(options)
        .expect("linux wayland capture backend should start");

    thread::sleep(Duration::from_secs(3));

    let artifact = controller
        .stop()
        .expect("linux wayland capture backend should stop");
    assert_eq!(artifact.output_path, output_path);
    assert!(artifact.duration >= Duration::from_secs(2));
    assert!(artifact.bytes_written > 0);
    assert!(output_path.exists(), "expected recording output to exist");

    let metadata = fs::metadata(&output_path).expect("expected output metadata");
    assert!(metadata.len() > 0, "expected non-empty recording output");

    let _ = fs::remove_file(output_path);
}

#[test]
#[ignore = "requires a live Linux Wayland session and ScreenCast portal access"]
fn linux_wayland_session_smoke_recording_creates_output_file() {
    let wayland_display = env::var("WAYLAND_DISPLAY").unwrap_or_default();
    assert!(
        !wayland_display.trim().is_empty(),
        "WAYLAND_DISPLAY must be set for Linux Wayland capture smoke tests",
    );

    let output_path = unique_output_path_with_extension("mp4");
    let portal_parent = exported_wayland_parent_window()
        .expect("expected a Wayland portal parent window for the smoke test");
    let options = RecordingOptions {
        output_path: output_path.clone(),
        quality_preset: "720p / 30 fps".to_string(),
        mic_enabled: env::var("RECORD_SCREEN_SMOKE_WITH_MIC")
            .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
            .unwrap_or(false),
        system_audio_enabled: false,
        capture_target_id: "full-desktop".to_string(),
        audio_input_id: capture::DEFAULT_AUDIO_INPUT_ID.to_string(),
        portal_parent_window: Some(portal_parent.handle.clone()),
        portal_restore_token: None,
        region_x: 160,
        region_y: 120,
        region_width: 1280,
        region_height: 720,
        region_source_capture_target_id: capture::FULL_DESKTOP_TARGET_ID.to_string(),
        region_source_origin_x: 0,
        region_source_origin_y: 0,
        region_source_scale_factor_milli: 1000,
    };

    let mut controller = selected_backend()
        .start(options)
        .expect("linux wayland-session capture backend should start");

    thread::sleep(Duration::from_secs(3));

    let artifact = controller
        .stop()
        .expect("linux wayland-session capture backend should stop");
    assert_eq!(artifact.output_path, output_path);
    assert!(artifact.duration >= Duration::from_secs(2));
    assert!(artifact.bytes_written > 0);
    assert!(output_path.exists(), "expected recording output to exist");

    let metadata = fs::metadata(&output_path).expect("expected output metadata");
    assert!(metadata.len() > 0, "expected non-empty recording output");

    let _ = fs::remove_file(output_path);
}

fn unique_output_path() -> PathBuf {
    unique_output_path_with_extension("mp4")
}

fn unique_output_path_with_extension(extension: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    env::temp_dir().join(format!("record-screen-linux-smoke-{stamp}.{extension}"))
}

fn exported_wayland_parent_window() -> Option<WaylandPortalParent> {
    unsafe extern "C" fn on_exported_handle(
        _window: *mut gdk_wayland_sys::GdkWaylandWindow,
        handle: *const std::os::raw::c_char,
        user_data: *mut std::os::raw::c_void,
    ) {
        let sender = unsafe { &*(user_data as *const mpsc::SyncSender<Option<String>>) };
        let handle = if handle.is_null() {
            None
        } else {
            Some(format!(
                "wayland:{}",
                unsafe { std::ffi::CStr::from_ptr(handle) }.to_string_lossy()
            ))
        };
        let _ = sender.send(handle);
    }

    unsafe extern "C" fn drop_export_sender(user_data: *mut std::os::raw::c_void) {
        unsafe {
            drop(Box::<mpsc::SyncSender<Option<String>>>::from_raw(
                user_data as *mut _,
            ));
        }
    }

    if env::var("WAYLAND_DISPLAY")
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        return None;
    }

    if !gtk::is_initialized() {
        gtk::init().ok()?;
    }

    let window = gtk::Window::new(gtk::WindowType::Toplevel);
    window.set_title("Record Screen Wayland Smoke");
    window.set_default_size(32, 32);
    window.realize();
    window.show_all();

    let gdk_window = window.window()?;
    let gdk_window_ptr: *mut gtk::gdk::ffi::GdkWindow = gdk_window.to_glib_none().0;
    let (handle_tx, handle_rx) = mpsc::sync_channel(1);
    let sender_ptr = Box::into_raw(Box::new(handle_tx)) as *mut std::os::raw::c_void;
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
        return None;
    }

    let main_context = glib::MainContext::default();
    let started_at = std::time::Instant::now();
    loop {
        match handle_rx.try_recv() {
            Ok(Some(handle)) => return Some(WaylandPortalParent { handle, window }),
            Ok(None) => return None,
            Err(mpsc::TryRecvError::Empty) => {
                if started_at.elapsed() >= Duration::from_secs(2) {
                    return None;
                }
                main_context.iteration(true);
            }
            Err(mpsc::TryRecvError::Disconnected) => return None,
        }
    }
}
