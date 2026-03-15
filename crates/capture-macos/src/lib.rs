use std::{
    fs,
    io::{Read, Write},
    os::unix::process::ExitStatusExt,
    process::{Child, ChildStdin, Command, Stdio},
    sync::{Arc, Mutex, OnceLock},
    thread,
    time::{Duration, SystemTime},
};

use capture::{
    ActiveRecording, AudioInputKind, AudioInputOption, CUSTOM_REGION_TARGET_ID, CaptureController,
    CaptureError, CaptureTargetOption, DEFAULT_AUDIO_INPUT_ID, FULL_DESKTOP_TARGET_ID,
    RecordingArtifact, RecordingOptions, default_audio_input, full_desktop_target,
    resolve_audio_input_id,
};

const MONITOR_TARGET_PREFIX: &str = "monitor:";
const STARTUP_POLL_INTERVAL: Duration = Duration::from_millis(50);
const STARTUP_POLL_ATTEMPTS: usize = 6;

#[derive(Clone, Copy)]
struct VideoEncoderProfile {
    codec: &'static str,
    preset: Option<&'static str>,
}

pub struct FfmpegMacosCapture {
    active_recording: ActiveRecording,
    child: Child,
    stdin: Option<ChildStdin>,
    stderr_buffer: Arc<Mutex<String>>,
    finished_artifact: Option<RecordingArtifact>,
    paused: bool,
}

impl FfmpegMacosCapture {
    pub fn start(options: RecordingOptions) -> Result<Self, CaptureError> {
        if options.system_audio_enabled {
            return Err(CaptureError::BackendUnavailable(
                "System-audio mixing is not wired into the macOS backend yet.".to_string(),
            ));
        }

        let screen_input = resolve_screen_input_for_recording(&options)?;
        let video_device = screen_input.id.clone();
        let audio_device = discover_audio_device(&options.audio_input_id, options.mic_enabled)?;
        let input = format!("{video_device}:{audio_device}");
        let (width, height, fps) = quality_settings(&options.quality_preset);
        let encoder = encoder_for_quality(&options.quality_preset);
        let started_at = SystemTime::now();
        let stderr_buffer = Arc::new(Mutex::new(String::new()));

        let mut command = Command::new("ffmpeg");
        command
            .arg("-y")
            .arg("-f")
            .arg("avfoundation")
            .arg("-capture_cursor")
            .arg("1")
            .arg("-capture_mouse_clicks")
            .arg("1")
            .arg("-framerate")
            .arg(fps.to_string());

        if options.capture_target_id != CUSTOM_REGION_TARGET_ID {
            command.arg("-video_size").arg(format!("{width}x{height}"));
        }

        command.arg("-i").arg(input).arg("-c:v").arg(encoder.codec);

        if let Some(preset) = encoder.preset {
            command.arg("-preset").arg(preset);
        }

        command.arg("-pix_fmt").arg("yuv420p");
        if let Some(filter) = video_filter(&options, width, height) {
            command.arg("-vf").arg(filter);
        }

        if options.mic_enabled {
            command.arg("-c:a").arg("aac").arg("-b:a").arg("192k");
        } else {
            command.arg("-an");
        }

        command
            .arg("-movflags")
            .arg("+faststart")
            .arg(options.output_path.as_os_str())
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());

        let mut child = command
            .spawn()
            .map_err(|error| CaptureError::SpawnFailed(error.to_string()))?;

        let stdin = child.stdin.take();
        if let Some(mut stderr) = child.stderr.take() {
            let stderr_buffer = Arc::clone(&stderr_buffer);
            thread::spawn(move || {
                let mut buffer = String::new();
                let _ = stderr.read_to_string(&mut buffer);
                if let Ok(mut log) = stderr_buffer.lock() {
                    *log = buffer;
                }
            });
        }

        verify_process_started(&mut child, &stderr_buffer)?;

        Ok(Self {
            active_recording: ActiveRecording {
                backend_name: "macOS ffmpeg / AVFoundation".to_string(),
                encoder_label: encoder_label(&encoder),
                output_path: options.output_path,
                started_at,
                target_label: if options.capture_target_id == CUSTOM_REGION_TARGET_ID {
                    format!(
                        "Custom region · {}, {} · {} x {}",
                        options.region_x,
                        options.region_y,
                        options.region_width,
                        options.region_height
                    )
                } else {
                    screen_input.label
                },
            },
            child,
            stdin,
            stderr_buffer,
            finished_artifact: None,
            paused: false,
        })
    }

    fn build_artifact(&self, finished_at: SystemTime) -> Result<RecordingArtifact, CaptureError> {
        let metadata = fs::metadata(&self.active_recording.output_path)
            .map_err(|error| CaptureError::OutputInspectionFailed(error.to_string()))?;

        let duration = finished_at
            .duration_since(self.active_recording.started_at)
            .unwrap_or_default();

        Ok(RecordingArtifact {
            output_path: self.active_recording.output_path.clone(),
            started_at: self.active_recording.started_at,
            finished_at,
            duration,
            bytes_written: metadata.len(),
        })
    }
}

pub fn list_capture_targets() -> Vec<CaptureTargetOption> {
    list_device_options().0
}

pub fn list_audio_inputs() -> Vec<AudioInputOption> {
    list_device_options().1
}

pub fn list_device_options() -> (Vec<CaptureTargetOption>, Vec<AudioInputOption>) {
    let Ok(listing) = load_avfoundation_listing() else {
        return (vec![full_desktop_target()], vec![default_audio_input()]);
    };

    let screen_inputs = parse_screen_inputs(&listing);
    let mut targets = if screen_inputs.is_empty() {
        vec![full_desktop_target()]
    } else {
        let mut targets = vec![CaptureTargetOption {
            id: FULL_DESKTOP_TARGET_ID.to_string(),
            label: "Primary display".to_string(),
            description: "Use the first available macOS screen capture source.".to_string(),
        }];
        targets.extend(screen_inputs.into_iter().map(|screen| CaptureTargetOption {
            id: format!("{MONITOR_TARGET_PREFIX}{}", screen.id),
            label: screen.label,
            description: screen.description,
        }));
        targets
    };

    if targets.is_empty() {
        targets.push(full_desktop_target());
    }

    let mut inputs = vec![default_audio_input()];
    inputs.extend(parse_audio_inputs(&listing));

    (targets, inputs)
}

impl CaptureController for FfmpegMacosCapture {
    fn active_recording(&self) -> &ActiveRecording {
        &self.active_recording
    }

    fn pause(&mut self) -> Result<(), CaptureError> {
        if self.paused {
            return Ok(());
        }

        let result = unsafe { libc::kill(self.child.id() as i32, libc::SIGSTOP) };
        if result != 0 {
            return Err(CaptureError::SignalFailed(
                "failed to send SIGSTOP".to_string(),
            ));
        }

        self.paused = true;
        Ok(())
    }

    fn resume(&mut self) -> Result<(), CaptureError> {
        if !self.paused {
            return Ok(());
        }

        let result = unsafe { libc::kill(self.child.id() as i32, libc::SIGCONT) };
        if result != 0 {
            return Err(CaptureError::SignalFailed(
                "failed to send SIGCONT".to_string(),
            ));
        }

        self.paused = false;
        Ok(())
    }

    fn stop(&mut self) -> Result<RecordingArtifact, CaptureError> {
        if let Some(artifact) = self.finished_artifact.clone() {
            return Ok(artifact);
        }

        if self.paused {
            self.resume()?;
        }

        if let Some(stdin) = self.stdin.as_mut() {
            stdin
                .write_all(b"q\n")
                .and_then(|_| stdin.flush())
                .map_err(|error| CaptureError::StopFailed(error.to_string()))?;
        }

        let status = self
            .child
            .wait()
            .map_err(|error| CaptureError::StopFailed(error.to_string()))?;

        if !status.success() && status.signal() != Some(libc::SIGTERM) {
            return Err(CaptureError::StopFailed(format!(
                "ffmpeg exited with status {status}: {}",
                describe_ffmpeg_failure(status.code(), &read_stderr_buffer(&self.stderr_buffer))
            )));
        }

        let finished_at = SystemTime::now();
        let artifact = self.build_artifact(finished_at)?;
        self.finished_artifact = Some(artifact.clone());
        Ok(artifact)
    }

    fn poll_finished(&mut self) -> Result<Option<RecordingArtifact>, CaptureError> {
        if let Some(artifact) = self.finished_artifact.clone() {
            return Ok(Some(artifact));
        }

        let Some(status) = self
            .child
            .try_wait()
            .map_err(|error| CaptureError::StopFailed(error.to_string()))?
        else {
            return Ok(None);
        };

        if !status.success() && status.signal() != Some(libc::SIGTERM) {
            return Err(CaptureError::StopFailed(describe_ffmpeg_failure(
                status.code(),
                &read_stderr_buffer(&self.stderr_buffer),
            )));
        }

        let artifact = self.build_artifact(SystemTime::now())?;
        self.finished_artifact = Some(artifact.clone());
        Ok(Some(artifact))
    }
}

fn resolve_screen_input(capture_target_id: &str) -> Result<ScreenInput, CaptureError> {
    let listing = load_avfoundation_listing()?;
    let screen_inputs = parse_screen_inputs(&listing);
    if screen_inputs.is_empty() {
        return Err(CaptureError::BackendUnavailable(
            "ffmpeg did not expose any avfoundation screen device".to_string(),
        ));
    }

    if capture_target_id == FULL_DESKTOP_TARGET_ID {
        return screen_inputs.first().cloned().ok_or_else(|| {
            CaptureError::BackendUnavailable(
                "ffmpeg did not expose any avfoundation screen device".to_string(),
            )
        });
    }

    let Some(screen_id) = capture_target_id.strip_prefix(MONITOR_TARGET_PREFIX) else {
        return Err(CaptureError::BackendUnavailable(format!(
            "unknown macOS capture target: {capture_target_id}"
        )));
    };

    screen_inputs
        .into_iter()
        .find(|screen| screen.id == screen_id)
        .ok_or_else(|| {
            CaptureError::BackendUnavailable(format!(
                "the selected display `{capture_target_id}` is no longer available"
            ))
        })
}

fn discover_audio_device(
    selected_audio_input_id: &str,
    mic_enabled: bool,
) -> Result<String, CaptureError> {
    if !mic_enabled {
        return Ok("none".to_string());
    }

    let audio_inputs = parse_audio_inputs(&load_avfoundation_listing()?);
    if audio_inputs.is_empty() {
        return Err(CaptureError::BackendUnavailable(
            "ffmpeg did not expose any avfoundation microphone device".to_string(),
        ));
    }

    if selected_audio_input_id == DEFAULT_AUDIO_INPUT_ID {
        return resolve_audio_input_id(selected_audio_input_id, &audio_inputs).ok_or_else(|| {
            CaptureError::BackendUnavailable(
                "ffmpeg did not expose any avfoundation microphone device".to_string(),
            )
        });
    }

    resolve_audio_input_id(selected_audio_input_id, &audio_inputs).ok_or_else(|| {
        CaptureError::BackendUnavailable(format!(
            "the selected microphone input `{selected_audio_input_id}` is no longer available"
        ))
    })
}

fn load_avfoundation_listing() -> Result<String, CaptureError> {
    let output = Command::new("ffmpeg")
        .arg("-f")
        .arg("avfoundation")
        .arg("-list_devices")
        .arg("true")
        .arg("-i")
        .arg("")
        .output()
        .map_err(|error| CaptureError::BackendUnavailable(error.to_string()))?;

    Ok(String::from_utf8_lossy(&output.stderr).into_owned())
}

fn parse_audio_inputs(listing: &str) -> Vec<AudioInputOption> {
    let mut in_audio_section = false;
    let mut inputs = Vec::new();

    for line in listing.lines() {
        if line.contains("AVFoundation audio devices") {
            in_audio_section = true;
            continue;
        }

        if line.contains("AVFoundation video devices") {
            in_audio_section = false;
            continue;
        }

        if !in_audio_section {
            continue;
        }

        let Ok(id) = parse_device_index(line) else {
            continue;
        };
        let label = parse_device_name(line);
        if label.is_empty() {
            continue;
        }

        inputs.push(AudioInputOption {
            id,
            description: format!("AVFoundation input: {label}"),
            label,
            kind: AudioInputKind::Microphone,
        });
    }

    inputs
}

#[derive(Debug, Clone)]
struct ScreenInput {
    id: String,
    label: String,
    description: String,
}

fn resolve_screen_input_for_recording(
    options: &RecordingOptions,
) -> Result<ScreenInput, CaptureError> {
    if options.capture_target_id == CUSTOM_REGION_TARGET_ID {
        let region_source = options.region_source_capture_target_id.trim();
        let target_id = if region_source.is_empty() {
            FULL_DESKTOP_TARGET_ID
        } else {
            region_source
        };

        return resolve_screen_input(target_id);
    }

    resolve_screen_input(&options.capture_target_id)
}

fn video_filter(options: &RecordingOptions, width: u32, height: u32) -> Option<String> {
    if options.capture_target_id != CUSTOM_REGION_TARGET_ID {
        return None;
    }

    let source_scale = (options.region_source_scale_factor_milli.max(1) as f64) / 1000.0;
    let crop_x = (((options.region_x as i32 - options.region_source_origin_x).max(0)) as f64
        / source_scale)
        .round() as u32;
    let crop_y = (((options.region_y as i32 - options.region_source_origin_y).max(0)) as f64
        / source_scale)
        .round() as u32;
    let crop_width = ((options.region_width.max(64) as f64) / source_scale)
        .round()
        .max(64.0) as u32;
    let crop_height = ((options.region_height.max(64) as f64) / source_scale)
        .round()
        .max(64.0) as u32;
    let crop = format!("crop={crop_width}:{crop_height}:{crop_x}:{crop_y}");
    let scale = scale_filter(width, height);

    Some(format!("{crop},{scale}"))
}

fn parse_screen_inputs(listing: &str) -> Vec<ScreenInput> {
    let mut in_video_section = false;
    let mut screens = Vec::new();

    for line in listing.lines() {
        if line.contains("AVFoundation video devices") {
            in_video_section = true;
            continue;
        }

        if line.contains("AVFoundation audio devices") {
            in_video_section = false;
            continue;
        }

        if !in_video_section || !line.contains("Capture screen") {
            continue;
        }

        let Ok(id) = parse_device_index(line) else {
            continue;
        };
        let raw_label = parse_device_name(line);
        if raw_label.is_empty() {
            continue;
        }

        screens.push(ScreenInput {
            id,
            label: raw_label.replacen("Capture screen", "Display", 1),
            description: format!("AVFoundation source: {raw_label}"),
        });
    }

    screens
}

fn parse_device_index(line: &str) -> Result<String, CaptureError> {
    let start = line
        .rfind('[')
        .ok_or_else(|| CaptureError::BackendUnavailable(format!("invalid device line: {line}")))?;
    let end = line[start + 1..]
        .find(']')
        .map(|index| index + start + 1)
        .ok_or_else(|| CaptureError::BackendUnavailable(format!("invalid device line: {line}")))?;

    Ok(line[start + 1..end].to_string())
}

fn parse_device_name(line: &str) -> String {
    let Some(index_end) = line.rfind(']') else {
        return String::new();
    };

    line[index_end + 1..].trim().to_string()
}

fn quality_settings(preset: &str) -> (u32, u32, u32) {
    match preset {
        "720p / 30 fps" => (1280, 720, 30),
        "1080p / 30 fps" => (1920, 1080, 30),
        "1440p / 60 fps" => (2560, 1440, 60),
        "4K / 60 fps" => (3840, 2160, 60),
        _ => (1920, 1080, 60),
    }
}

fn preferred_video_encoder() -> VideoEncoderProfile {
    static ENCODER: OnceLock<VideoEncoderProfile> = OnceLock::new();
    *ENCODER.get_or_init(|| {
        let encoders = load_ffmpeg_encoders().unwrap_or_default();
        if encoders.contains("h264_videotoolbox") {
            VideoEncoderProfile {
                codec: "h264_videotoolbox",
                preset: None,
            }
        } else {
            VideoEncoderProfile {
                codec: "libx264",
                preset: None,
            }
        }
    })
}

fn encoder_for_quality(preset: &str) -> VideoEncoderProfile {
    let preferred = preferred_video_encoder();
    if preferred.codec == "libx264" {
        VideoEncoderProfile {
            codec: "libx264",
            preset: Some(cpu_preset_for_quality(preset)),
        }
    } else {
        preferred
    }
}

fn cpu_preset_for_quality(preset: &str) -> &'static str {
    match preset {
        "4K / 60 fps" | "1440p / 60 fps" => "ultrafast",
        "1080p / 60 fps" => "superfast",
        _ => "veryfast",
    }
}

fn scale_filter(width: u32, height: u32) -> String {
    format!(
        "scale={width}:{height}:force_original_aspect_ratio=decrease,pad={width}:{height}:(ow-iw)/2:(oh-ih)/2"
    )
}

fn encoder_label(profile: &VideoEncoderProfile) -> String {
    match profile.preset {
        Some(preset) => format!("{} · {}", profile.codec, preset),
        None => profile.codec.to_string(),
    }
}

fn load_ffmpeg_encoders() -> Result<String, CaptureError> {
    let output = Command::new("ffmpeg")
        .args(["-hide_banner", "-encoders"])
        .output()
        .map_err(|error| CaptureError::BackendUnavailable(error.to_string()))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    Ok(format!("{stdout}\n{stderr}").to_ascii_lowercase())
}

fn verify_process_started(
    child: &mut Child,
    stderr_buffer: &Arc<Mutex<String>>,
) -> Result<(), CaptureError> {
    for _ in 0..STARTUP_POLL_ATTEMPTS {
        thread::sleep(STARTUP_POLL_INTERVAL);
        if let Some(status) = child
            .try_wait()
            .map_err(|error| CaptureError::SpawnFailed(error.to_string()))?
        {
            let stderr_log = read_stderr_buffer(stderr_buffer);
            return Err(CaptureError::SpawnFailed(describe_ffmpeg_failure(
                status.code(),
                &stderr_log,
            )));
        }
    }

    Ok(())
}

fn read_stderr_buffer(buffer: &Arc<Mutex<String>>) -> String {
    buffer.lock().map(|log| log.clone()).unwrap_or_default()
}

fn describe_ffmpeg_failure(exit_code: Option<i32>, stderr_log: &str) -> String {
    let stderr_lower = stderr_log.to_lowercase();

    if stderr_lower.contains("not authorized")
        || stderr_lower.contains("permission denied")
        || stderr_lower.contains("operation not permitted")
        || stderr_lower.contains("screen recording")
    {
        return "macOS blocked screen capture. Open System Settings > Privacy & Security > Screen & System Audio Recording, allow this app or Terminal, then try again.".to_string();
    }

    if stderr_lower.contains("no such file or directory")
        || stderr_lower.contains("command not found")
    {
        return "ffmpeg is not available on this Mac. Install ffmpeg first, then retry."
            .to_string();
    }

    let tail = stderr_log
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("ffmpeg exited before capture could start.")
        .trim();

    match exit_code {
        Some(code) => format!("ffmpeg failed to start (exit code {code}). {tail}"),
        None => tail.to_string(),
    }
}
