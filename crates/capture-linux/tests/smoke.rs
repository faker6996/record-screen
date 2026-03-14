use std::{
    env, fs,
    path::PathBuf,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use capture::{CaptureController, RecordingOptions};
use capture_linux::FfmpegLinuxCapture;

#[test]
#[ignore = "requires a live Linux X11 desktop session with ffmpeg access"]
fn linux_smoke_recording_creates_output_file() {
    let display = env::var("DISPLAY").unwrap_or_default();
    assert!(
        !display.trim().is_empty(),
        "DISPLAY must be set for Linux X11 capture smoke tests",
    );

    let output_path = unique_output_path();
    let options = RecordingOptions {
        output_path: output_path.clone(),
        quality_preset: "720p / 30 fps".to_string(),
        mic_enabled: env::var("RECORD_SCREEN_SMOKE_WITH_MIC")
            .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
            .unwrap_or(false),
        system_audio_enabled: false,
        capture_target_id: env::var("RECORD_SCREEN_SMOKE_TARGET_ID")
            .unwrap_or_else(|_| "full-desktop".to_string()),
        audio_input_id: capture::DEFAULT_AUDIO_INPUT_ID.to_string(),
        region_x: 160,
        region_y: 120,
        region_width: 1280,
        region_height: 720,
    };

    let mut controller =
        FfmpegLinuxCapture::start(options).expect("linux capture backend should start");

    thread::sleep(Duration::from_secs(3));

    let artifact = controller
        .stop()
        .expect("linux capture backend should stop");
    assert_eq!(artifact.output_path, output_path);
    assert!(artifact.duration >= Duration::from_secs(2));
    assert!(artifact.bytes_written > 0);
    assert!(output_path.exists(), "expected recording output to exist");

    let metadata = fs::metadata(&output_path).expect("expected output metadata");
    assert!(metadata.len() > 0, "expected non-empty recording output");

    let _ = fs::remove_file(output_path);
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
    let options = RecordingOptions {
        output_path: output_path.clone(),
        quality_preset: "720p / 30 fps".to_string(),
        mic_enabled: env::var("RECORD_SCREEN_SMOKE_WITH_MIC")
            .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
            .unwrap_or(false),
        system_audio_enabled: false,
        capture_target_id: "full-desktop".to_string(),
        audio_input_id: capture::DEFAULT_AUDIO_INPUT_ID.to_string(),
        region_x: 160,
        region_y: 120,
        region_width: 1280,
        region_height: 720,
    };

    let mut controller =
        FfmpegLinuxCapture::start(options).expect("linux wayland capture backend should start");

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
