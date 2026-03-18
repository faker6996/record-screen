#![cfg(target_os = "windows")]

use std::{
    env, fs,
    path::PathBuf,
    process::{Child, Command},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use capture::{
    CaptureTargetOption, DEFAULT_AUDIO_INPUT_ID, RecordingOptions,
};

const WINDOW_TARGET_PREFIX: &str = "window:";

#[test]
#[ignore = "requires a live Windows desktop session with another visible app window"]
fn windows_smoke_window_recording_creates_output_file() {
    let fixture = spawn_window_fixture();
    let capture_target = resolve_window_target(fixture.as_ref().map(|fixture| fixture.title_fragment.as_str()));
    run_smoke_recording_test(SmokeScenario {
        capture_target_id: capture_target.id,
        output_name: "window",
        mic_enabled: false,
        system_audio_enabled: false,
        window_title_fragment: fixture.as_ref().map(|fixture| fixture.title_fragment.clone()),
    });
    drop(fixture);
}

#[test]
#[ignore = "requires a live Windows desktop session with screen capture available"]
fn windows_smoke_full_desktop_recording_creates_output_file() {
    run_smoke_recording_test(SmokeScenario {
        capture_target_id: capture::FULL_DESKTOP_TARGET_ID.to_string(),
        output_name: "full-desktop",
        mic_enabled: false,
        system_audio_enabled: false,
        window_title_fragment: None,
    });
}

#[test]
#[ignore = "requires a live Windows desktop session with a usable microphone input"]
fn windows_smoke_window_recording_with_microphone_creates_output_file() {
    let fixture = spawn_window_fixture();
    let capture_target = resolve_window_target(fixture.as_ref().map(|fixture| fixture.title_fragment.as_str()));
    run_smoke_recording_test(SmokeScenario {
        capture_target_id: capture_target.id,
        output_name: "window-mic",
        mic_enabled: true,
        system_audio_enabled: false,
        window_title_fragment: fixture.as_ref().map(|fixture| fixture.title_fragment.clone()),
    });
    drop(fixture);
}

#[test]
#[ignore = "requires a live Windows session with a usable microphone endpoint"]
fn windows_smoke_microphone_worker_receives_packets() {
    let audio_input_id = env::var("RECORD_SCREEN_SMOKE_AUDIO_INPUT_ID")
        .unwrap_or_else(|_| DEFAULT_AUDIO_INPUT_ID.to_string());
    let available_audio_inputs = capture_windows::list_audio_inputs();
    let worker = capture_windows::native_audio_backend::start_microphone_worker_for_input(
        &audio_input_id,
        &available_audio_inputs,
    )
    .expect("Windows microphone worker should start");

    thread::sleep(Duration::from_millis(900));

    let snapshot = worker.snapshot();
    let final_stats = worker.stop().expect("Windows microphone worker should stop");
    assert!(
        snapshot.packets_observed > 0
            || final_stats.packets_observed > 0
            || snapshot.next_packet_frames > 0
            || final_stats.next_packet_frames > 0,
        "expected the Windows microphone worker to observe at least one packet"
    );
}

#[derive(Debug, Clone)]
struct SmokeScenario {
    capture_target_id: String,
    output_name: &'static str,
    mic_enabled: bool,
    system_audio_enabled: bool,
    window_title_fragment: Option<String>,
}

struct WindowFixture {
    child: Child,
    path: PathBuf,
    title_fragment: String,
}

impl Drop for WindowFixture {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_file(&self.path);
    }
}

fn run_smoke_recording_test(scenario: SmokeScenario) {
    let output_path = unique_output_path(scenario.output_name, "mp4");
    let options = RecordingOptions {
        output_path: output_path.clone(),
        quality_preset: env::var("RECORD_SCREEN_WINDOWS_SMOKE_QUALITY")
            .unwrap_or_else(|_| "720p / 30 fps".to_string()),
        mic_enabled: scenario.mic_enabled,
        system_audio_enabled: scenario.system_audio_enabled,
        capture_target_id: scenario.capture_target_id.clone(),
        audio_input_id: env::var("RECORD_SCREEN_SMOKE_AUDIO_INPUT_ID")
            .unwrap_or_else(|_| DEFAULT_AUDIO_INPUT_ID.to_string()),
        portal_parent_window: None,
        portal_restore_token: None,
        region_x: 0,
        region_y: 0,
        region_width: 1280,
        region_height: 720,
        region_source_capture_target_id: scenario.capture_target_id,
        region_source_origin_x: 0,
        region_source_origin_y: 0,
        region_source_scale_factor_milli: 1000,
    };

    let mut controller = capture_windows::selected_backend()
        .start(options)
        .expect("Windows capture backend should start");
    let backend_name = controller.active_recording().backend_name.clone();

    assert!(
        backend_name.contains("Windows Graphics Capture"),
        "expected the Windows native backend, got `{backend_name}`"
    );
    assert!(
        !controller.supports_pause_resume(),
        "current Windows native controller should still report pause/resume unsupported",
    );

    if let Some(title_fragment) = scenario.window_title_fragment.as_ref() {
        nudge_window_activity(title_fragment);
    }

    thread::sleep(Duration::from_secs(3));

    let artifact = controller
        .stop()
        .expect("Windows capture backend should stop");
    assert_eq!(artifact.output_path, output_path);
    assert!(artifact.duration >= Duration::from_secs(2));
    assert!(artifact.bytes_written > 0);
    assert!(output_path.exists(), "expected recording output to exist");

    let metadata = fs::metadata(&output_path).expect("expected output metadata");
    assert!(metadata.len() > 0, "expected non-empty recording output");

    let _ = fs::remove_file(output_path);
}

fn resolve_window_target(preferred_title_fragment: Option<&str>) -> CaptureTargetOption {
    let capture_targets = capture_windows::list_capture_targets();
    let window_targets: Vec<_> = capture_targets
        .iter()
        .filter(|target| target.id.starts_with(WINDOW_TARGET_PREFIX))
        .cloned()
        .collect();

    if let Ok(explicit_target_id) = env::var("RECORD_SCREEN_WINDOWS_SMOKE_TARGET_ID") {
        return window_targets
            .iter()
            .find(|target| target.id == explicit_target_id)
            .cloned()
            .unwrap_or_else(|| {
                panic!(
                    "RECORD_SCREEN_WINDOWS_SMOKE_TARGET_ID=`{explicit_target_id}` was not found in the current Windows capture target list"
                )
            });
    }

    if let Some(title_fragment) = preferred_title_fragment {
        let lowered_fragment = title_fragment.to_ascii_lowercase();
        if let Some(target) = window_targets
            .iter()
            .find(|target| target.label.to_ascii_lowercase().contains(&lowered_fragment))
        {
            return target.clone();
        }
    }

    if let Ok(title_fragment) = env::var("RECORD_SCREEN_WINDOWS_SMOKE_WINDOW_TITLE") {
        let lowered_fragment = title_fragment.to_ascii_lowercase();
        if let Some(target) = window_targets
            .iter()
            .find(|target| target.label.to_ascii_lowercase().contains(&lowered_fragment))
        {
            return target.clone();
        }

        panic!(
            "No window capture target matched RECORD_SCREEN_WINDOWS_SMOKE_WINDOW_TITLE=`{title_fragment}`"
        );
    }

    window_targets
        .into_iter()
        .max_by_key(score_window_target)
        .unwrap_or_else(|| {
            panic!(
                "No `window:*` capture target is available. Open another visible app window or set RECORD_SCREEN_WINDOWS_SMOKE_TARGET_ID."
            )
        })
}

fn score_window_target(target: &CaptureTargetOption) -> i32 {
    let lowered = target.label.to_ascii_lowercase();
    let mut score = 0;

    if lowered.contains("notepad") {
        score += 500;
    }
    if lowered.contains("visual studio code") || lowered.contains("code") {
        score += 200;
    }
    if lowered.contains("settings") || lowered.contains("program manager") {
        score -= 300;
    }
    if lowered.contains("record screen") {
        score -= 400;
    }
    if lowered.trim().is_empty() {
        score -= 500;
    } else {
        score += lowered.len() as i32;
    }

    score
}

fn spawn_window_fixture() -> Option<WindowFixture> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let title_fragment = format!("record-screen-smoke-window-{stamp}.txt");
    let path = env::temp_dir().join(&title_fragment);
    fs::write(&path, b"record-screen smoke fixture").ok()?;
    let child = Command::new("notepad").arg(&path).spawn().ok()?;
    thread::sleep(Duration::from_secs(2));
    Some(WindowFixture {
        child,
        path,
        title_fragment,
    })
}

fn nudge_window_activity(title_fragment: &str) {
    let title_fragment = title_fragment.to_string();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(500));
        let escaped_title = title_fragment.replace('\'', "''");
        let script = format!(
            "Add-Type -AssemblyName Microsoft.VisualBasic; \
             Add-Type -AssemblyName System.Windows.Forms; \
             if ([Microsoft.VisualBasic.Interaction]::AppActivate('{escaped_title}')) {{ \
               [System.Windows.Forms.SendKeys]::SendWait('record screen smoke'); \
               [System.Windows.Forms.SendKeys]::SendWait('{{ENTER}}'); \
             }}"
        );
        let _ = Command::new("powershell")
            .args(["-NoProfile", "-Command", &script])
            .status();
    });
}

fn unique_output_path(name: &str, extension: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    env::temp_dir().join(format!(
        "record-screen-windows-smoke-{name}-{stamp}.{extension}"
    ))
}
