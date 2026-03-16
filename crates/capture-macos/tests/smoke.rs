use std::{
    env, fs,
    path::PathBuf,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use capture::{
    CUSTOM_REGION_TARGET_ID, DEFAULT_AUDIO_INPUT_ID, FULL_DESKTOP_TARGET_ID, RecordingOptions,
};

#[test]
#[ignore = "requires a live macOS desktop session with screen-recording permission"]
fn macos_smoke_full_desktop_recording_creates_output_file() {
    run_smoke_recording_test(SmokeScenario {
        capture_target_id: FULL_DESKTOP_TARGET_ID.to_string(),
        extension: "mp4",
        mic_enabled: false,
        system_audio_enabled: false,
        output_name: "full-desktop",
        ..SmokeScenario::default()
    });
}

#[test]
#[ignore = "requires a live macOS desktop session with screen-recording permission"]
fn macos_smoke_custom_region_recording_creates_output_file() {
    run_smoke_recording_test(SmokeScenario {
        capture_target_id: CUSTOM_REGION_TARGET_ID.to_string(),
        extension: "mp4",
        mic_enabled: false,
        system_audio_enabled: false,
        output_name: "custom-region",
        region_x: 160,
        region_y: 120,
        region_width: 960,
        region_height: 540,
        ..SmokeScenario::default()
    });
}

#[test]
#[ignore = "requires a live macOS desktop session with screen-recording and microphone permissions"]
fn macos_smoke_full_desktop_with_microphone_creates_output_file() {
    run_smoke_recording_test(SmokeScenario {
        capture_target_id: FULL_DESKTOP_TARGET_ID.to_string(),
        extension: "mp4",
        mic_enabled: true,
        system_audio_enabled: false,
        output_name: "full-desktop-mic",
        ..SmokeScenario::default()
    });
}

#[test]
#[ignore = "requires macOS 15+ direct recording-output support plus screen-recording permission"]
fn macos_smoke_full_desktop_with_system_audio_creates_output_file() {
    run_smoke_recording_test(SmokeScenario {
        capture_target_id: FULL_DESKTOP_TARGET_ID.to_string(),
        extension: "mp4",
        mic_enabled: false,
        system_audio_enabled: true,
        output_name: "full-desktop-system-audio",
        ..SmokeScenario::default()
    });
}

#[derive(Debug, Clone)]
struct SmokeScenario {
    capture_target_id: String,
    extension: &'static str,
    mic_enabled: bool,
    system_audio_enabled: bool,
    output_name: &'static str,
    region_x: u32,
    region_y: u32,
    region_width: u32,
    region_height: u32,
    region_source_capture_target_id: String,
    region_source_origin_x: i32,
    region_source_origin_y: i32,
    region_source_scale_factor_milli: u32,
}

impl Default for SmokeScenario {
    fn default() -> Self {
        Self {
            capture_target_id: FULL_DESKTOP_TARGET_ID.to_string(),
            extension: "mp4",
            mic_enabled: false,
            system_audio_enabled: false,
            output_name: "default",
            region_x: 160,
            region_y: 120,
            region_width: 1280,
            region_height: 720,
            region_source_capture_target_id: FULL_DESKTOP_TARGET_ID.to_string(),
            region_source_origin_x: 0,
            region_source_origin_y: 0,
            region_source_scale_factor_milli: 1000,
        }
    }
}

fn run_smoke_recording_test(scenario: SmokeScenario) {
    let output_path = unique_output_path(scenario.output_name, scenario.extension);
    let options = RecordingOptions {
        output_path: output_path.clone(),
        quality_preset: env::var("RECORD_SCREEN_MACOS_SMOKE_QUALITY")
            .unwrap_or_else(|_| "720p / 30 fps".to_string()),
        mic_enabled: scenario.mic_enabled,
        system_audio_enabled: scenario.system_audio_enabled,
        capture_target_id: scenario.capture_target_id,
        audio_input_id: env::var("RECORD_SCREEN_SMOKE_AUDIO_INPUT_ID")
            .unwrap_or_else(|_| DEFAULT_AUDIO_INPUT_ID.to_string()),
        region_x: scenario.region_x,
        region_y: scenario.region_y,
        region_width: scenario.region_width,
        region_height: scenario.region_height,
        region_source_capture_target_id: scenario.region_source_capture_target_id,
        region_source_origin_x: scenario.region_source_origin_x,
        region_source_origin_y: scenario.region_source_origin_y,
        region_source_scale_factor_milli: scenario.region_source_scale_factor_milli,
    };

    let mut controller = capture_macos::selected_backend()
        .start(options)
        .expect("macOS capture backend should start");
    let backend_name = controller.active_recording().backend_name.clone();

    if scenario.mic_enabled || scenario.system_audio_enabled {
        assert!(
            backend_name.contains("ScreenCaptureKit / SCRecordingOutput"),
            "expected native macOS recording-output lane, got `{backend_name}`"
        );
    }

    thread::sleep(Duration::from_secs(3));

    let artifact = controller
        .stop()
        .expect("macOS capture backend should stop");
    assert_eq!(artifact.output_path, output_path);
    assert!(artifact.duration >= Duration::from_secs(2));
    assert!(artifact.bytes_written > 0);
    assert!(output_path.exists(), "expected recording output to exist");

    let metadata = fs::metadata(&output_path).expect("expected output metadata");
    assert!(metadata.len() > 0, "expected non-empty recording output");

    let _ = fs::remove_file(output_path);
}

fn unique_output_path(name: &str, extension: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    env::temp_dir().join(format!(
        "record-screen-macos-smoke-{name}-{stamp}.{extension}"
    ))
}
