mod wayland_portal;

use std::{
    env, fs,
    io::{Read, Write},
    os::unix::process::ExitStatusExt,
    process::{Child, ChildStdin, Command, Stdio},
    sync::{Arc, Mutex, OnceLock},
    thread,
    time::{Duration, SystemTime},
};

use capture::{
    ActiveRecording, AudioInputOption, CaptureController, CaptureError, CaptureTargetOption,
    DEFAULT_AUDIO_INPUT_ID, FULL_DESKTOP_TARGET_ID, RecordingArtifact, RecordingOptions,
    default_audio_input, full_desktop_target, resolve_audio_input_id,
};

const MONITOR_TARGET_PREFIX: &str = "monitor:";
const WINDOW_TARGET_PREFIX: &str = "window:";
const STARTUP_POLL_INTERVAL: Duration = Duration::from_millis(50);
const STARTUP_POLL_ATTEMPTS: usize = 6;

#[derive(Debug, Clone, PartialEq, Eq)]
enum LinuxDesktopSession {
    X11 {
        display: String,
    },
    WaylandWithX11 {
        wayland_display: String,
        x11_display: String,
    },
    WaylandOnly {
        wayland_display: String,
    },
    Headless,
}

#[derive(Clone)]
struct VideoEncoderProfile {
    codec: &'static str,
    preset: Option<&'static str>,
    vaapi_device: Option<String>,
}

pub struct FfmpegLinuxCapture {
    active_recording: ActiveRecording,
    child: Child,
    stdin: Option<ChildStdin>,
    stderr_buffer: Arc<Mutex<String>>,
    finished_artifact: Option<RecordingArtifact>,
    paused: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MonitorDescriptor {
    connector: String,
    label: String,
    width: u32,
    height: u32,
    x: i32,
    y: i32,
    is_primary: bool,
}

#[derive(Debug, Clone)]
struct ResolvedTarget {
    label: String,
    origin_x: i32,
    origin_y: i32,
    video_size: Option<(u32, u32)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WindowDescriptor {
    id: String,
    title: String,
    width: u32,
    height: u32,
    x: i32,
    y: i32,
}

impl FfmpegLinuxCapture {
    pub fn start(options: RecordingOptions) -> Result<Self, CaptureError> {
        let session = current_desktop_session();
        let display = match &session {
            LinuxDesktopSession::X11 { display } => display.clone(),
            LinuxDesktopSession::WaylandWithX11 { x11_display, .. } => x11_display.clone(),
            LinuxDesktopSession::WaylandOnly { wayland_display } => {
                return match wayland_portal::negotiate_runtime_session() {
                    Ok(runtime_session) => Err(CaptureError::BackendUnavailable(
                        wayland_portal::runtime_pending_copy(&runtime_session),
                    )),
                    Err(error) => Err(CaptureError::BackendUnavailable(format!(
                        "Wayland session {wayland_display} could reach the ScreenCast portal path, but session negotiation did not complete: {error}"
                    ))),
                };
            }
            LinuxDesktopSession::Headless => {
                return Err(CaptureError::BackendUnavailable(session.capture_guidance()));
            }
        };
        let started_at = SystemTime::now();
        let stderr_buffer = Arc::new(Mutex::new(String::new()));
        let target = resolve_target(&options.capture_target_id)?;
        let encoder = encoder_for_quality(&options.quality_preset);
        let (child, stdin) = spawn_ffmpeg(&options, &display, &target, Arc::clone(&stderr_buffer))?;

        Ok(Self {
            active_recording: ActiveRecording {
                backend_name: session.backend_name(),
                encoder_label: encoder_label(&encoder),
                output_path: options.output_path,
                started_at,
                target_label: target.label,
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

impl CaptureController for FfmpegLinuxCapture {
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
                describe_ffmpeg_failure(&read_stderr_buffer(&self.stderr_buffer))
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
                &read_stderr_buffer(&self.stderr_buffer),
            )));
        }

        let artifact = self.build_artifact(SystemTime::now())?;
        self.finished_artifact = Some(artifact.clone());
        Ok(Some(artifact))
    }
}

pub fn list_capture_targets() -> Vec<CaptureTargetOption> {
    let mut targets = vec![full_desktop_target()];

    if let Ok(monitors) = query_monitors() {
        targets.extend(monitors.into_iter().map(|monitor| CaptureTargetOption {
            id: format!("{MONITOR_TARGET_PREFIX}{}", monitor.connector),
            label: monitor.label,
            description: format!(
                "{} x {} at {}, {}",
                monitor.width, monitor.height, monitor.x, monitor.y
            ),
        }));
    }

    if let Ok(windows) = query_windows() {
        targets.extend(windows.into_iter().map(|window| CaptureTargetOption {
            id: format!("{WINDOW_TARGET_PREFIX}{}", window.id),
            label: format!("Window · {}", window.title),
            description: format!(
                "{} x {} at {}, {}",
                window.width, window.height, window.x, window.y
            ),
        }));
    }

    targets
}

pub fn list_audio_inputs() -> Vec<AudioInputOption> {
    let mut inputs = vec![default_audio_input()];
    inputs.extend(query_audio_inputs().unwrap_or_default());
    inputs
}

fn resolve_target(target_id: &str) -> Result<ResolvedTarget, CaptureError> {
    let monitors = query_monitors().unwrap_or_default();

    if target_id == FULL_DESKTOP_TARGET_ID {
        return Ok(resolve_full_desktop_target(&monitors));
    }

    let Some(connector) = target_id.strip_prefix(MONITOR_TARGET_PREFIX) else {
        if let Some(window_id) = target_id.strip_prefix(WINDOW_TARGET_PREFIX) {
            let window = query_windows()?
                .into_iter()
                .find(|item| item.id.eq_ignore_ascii_case(window_id))
                .ok_or_else(|| {
                    CaptureError::BackendUnavailable(format!(
                        "the selected window `{window_id}` is no longer available"
                    ))
                })?;

            return Ok(ResolvedTarget {
                label: format!("Window · {}", window.title),
                origin_x: window.x,
                origin_y: window.y,
                video_size: Some((window.width, window.height)),
            });
        }

        return Err(CaptureError::BackendUnavailable(format!(
            "unknown Linux capture target: {target_id}"
        )));
    };

    let monitor = monitors
        .into_iter()
        .find(|item| item.connector == connector)
        .ok_or_else(|| {
            CaptureError::BackendUnavailable(format!(
                "the selected monitor `{connector}` is no longer available"
            ))
        })?;

    Ok(ResolvedTarget {
        label: monitor.label,
        origin_x: monitor.x,
        origin_y: monitor.y,
        video_size: Some((monitor.width, monitor.height)),
    })
}

fn resolve_full_desktop_target(monitors: &[MonitorDescriptor]) -> ResolvedTarget {
    if monitors.is_empty() {
        return ResolvedTarget {
            label: "Full desktop".to_string(),
            origin_x: 0,
            origin_y: 0,
            video_size: None,
        };
    }

    let min_x = monitors.iter().map(|monitor| monitor.x).min().unwrap_or(0);
    let min_y = monitors.iter().map(|monitor| monitor.y).min().unwrap_or(0);
    let max_x = monitors
        .iter()
        .map(|monitor| monitor.x + monitor.width as i32)
        .max()
        .unwrap_or(0);
    let max_y = monitors
        .iter()
        .map(|monitor| monitor.y + monitor.height as i32)
        .max()
        .unwrap_or(0);

    ResolvedTarget {
        label: "Full desktop".to_string(),
        origin_x: min_x,
        origin_y: min_y,
        video_size: Some(((max_x - min_x) as u32, (max_y - min_y) as u32)),
    }
}

fn spawn_ffmpeg(
    options: &RecordingOptions,
    display: &str,
    target: &ResolvedTarget,
    stderr_buffer: Arc<Mutex<String>>,
) -> Result<(Child, Option<ChildStdin>), CaptureError> {
    let input = normalize_display(display);
    let (width, height, fps) = quality_settings(&options.quality_preset);
    let encoder = encoder_for_quality(&options.quality_preset);
    let mut command = Command::new("ffmpeg");
    command
        .arg("-y")
        .arg("-f")
        .arg("x11grab")
        .arg("-draw_mouse")
        .arg("1")
        .arg("-framerate")
        .arg(fps.to_string())
        .arg("-thread_queue_size")
        .arg("1024");

    if let Some(device) = encoder.vaapi_device.as_deref() {
        command.arg("-vaapi_device").arg(device);
    }

    if let Some((source_width, source_height)) = target.video_size {
        command
            .arg("-video_size")
            .arg(format!("{source_width}x{source_height}"));
    }

    command
        .arg("-i")
        .arg(format!("{input}+{},{}", target.origin_x, target.origin_y));

    if options.mic_enabled {
        let audio_input = resolve_audio_input(&options.audio_input_id)?;
        command
            .arg("-f")
            .arg("pulse")
            .arg("-thread_queue_size")
            .arg("1024")
            .arg("-i")
            .arg(audio_input);
    }

    command.arg("-c:v").arg(encoder.codec);

    if encoder.codec != "h264_vaapi" {
        command.arg("-pix_fmt").arg("yuv420p");
    }

    if let Some(preset) = encoder.preset {
        command.arg("-preset").arg(preset);
    }

    if let Some(filter) = video_filter(target.video_size, width, height, &encoder) {
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

    Ok((child, stdin))
}

fn resolve_audio_input(audio_input_id: &str) -> Result<String, CaptureError> {
    let audio_inputs = query_audio_inputs()?;

    if audio_input_id == DEFAULT_AUDIO_INPUT_ID {
        return resolve_audio_input_id(audio_input_id, &audio_inputs)
            .or_else(|| {
                audio_inputs
                    .iter()
                    .find(|input| input.id == DEFAULT_AUDIO_INPUT_ID)
                    .map(|input| input.id.clone())
            })
            .or_else(|| Some("default".to_string()))
            .ok_or_else(|| {
                CaptureError::BackendUnavailable(
                    "ffmpeg could not find any usable PulseAudio microphone source".to_string(),
                )
            });
    }

    resolve_audio_input_id(audio_input_id, &audio_inputs).ok_or_else(|| {
        CaptureError::BackendUnavailable(format!(
            "the selected microphone input `{audio_input_id}` is no longer available"
        ))
    })
}

fn query_audio_inputs() -> Result<Vec<AudioInputOption>, CaptureError> {
    let output = Command::new("pactl")
        .args(["list", "short", "sources"])
        .output()
        .map_err(|error| CaptureError::BackendUnavailable(error.to_string()))?;

    if !output.status.success() {
        return Err(CaptureError::BackendUnavailable(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }

    let listing = String::from_utf8_lossy(&output.stdout);
    let mut inputs = Vec::new();

    for line in listing.lines() {
        let mut columns = line.split('\t');
        let _index = columns.next();
        let Some(name) = columns.next() else {
            continue;
        };

        if name.ends_with(".monitor") {
            continue;
        }

        inputs.push(AudioInputOption {
            id: name.to_string(),
            label: name.to_string(),
            description: format!("PulseAudio source: {name}"),
        });
    }

    Ok(inputs)
}

fn query_monitors() -> Result<Vec<MonitorDescriptor>, CaptureError> {
    let output = Command::new("xrandr")
        .arg("--listmonitors")
        .output()
        .map_err(|error| CaptureError::BackendUnavailable(error.to_string()))?;

    if !output.status.success() {
        return Err(CaptureError::BackendUnavailable(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }

    let listing = String::from_utf8_lossy(&output.stdout);
    Ok(parse_monitors(&listing))
}

fn query_windows() -> Result<Vec<WindowDescriptor>, CaptureError> {
    let output = Command::new("xwininfo")
        .args(["-root", "-tree"])
        .output()
        .map_err(|error| CaptureError::BackendUnavailable(error.to_string()))?;

    if !output.status.success() {
        return Err(CaptureError::BackendUnavailable(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }

    let listing = String::from_utf8_lossy(&output.stdout);
    Ok(parse_windows(&listing))
}

fn parse_monitors(listing: &str) -> Vec<MonitorDescriptor> {
    let mut monitors = Vec::new();

    for (index, line) in listing.lines().enumerate() {
        if index == 0 || line.trim().is_empty() {
            continue;
        }

        let tokens: Vec<&str> = line.split_whitespace().collect();
        if tokens.len() < 4 {
            continue;
        }

        let flags_token = tokens[1];
        let geometry_token = tokens[2];
        let connector = tokens.last().unwrap_or(&"").to_string();
        let Some((width, height, x, y)) = parse_monitor_geometry(geometry_token) else {
            continue;
        };
        let position = monitors.len() + 1;
        let primary_label = if flags_token.contains('*') {
            format!("Display {position} · Primary ({connector})")
        } else {
            format!("Display {position} · {connector}")
        };

        monitors.push(MonitorDescriptor {
            connector,
            label: primary_label,
            width,
            height,
            x,
            y,
            is_primary: flags_token.contains('*'),
        });
    }

    monitors
}

fn parse_windows(listing: &str) -> Vec<WindowDescriptor> {
    listing
        .lines()
        .filter_map(parse_window_line)
        .filter(|window| {
            window.width >= 240
                && window.height >= 160
                && !window.title.starts_with("Record Screen")
                && !window.title.starts_with("Desktop Icons")
        })
        .collect()
}

fn parse_window_line(line: &str) -> Option<WindowDescriptor> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with("0x") {
        return None;
    }

    let id_end = trimmed.find(' ')?;
    let id = trimmed[..id_end].to_string();

    let title_start = trimmed[id_end..].find('"')? + id_end + 1;
    let title_end = trimmed[title_start..].find('"')? + title_start;
    let title = trimmed[title_start..title_end].trim().to_string();
    if title.is_empty() || title == "(has no name)" {
        return None;
    }

    let after_title = &trimmed[title_end + 1..];
    let class_end = after_title.find(')')?;
    let geometry_section = after_title[class_end + 1..].trim();
    let geometry_token = geometry_section.split_whitespace().next()?;
    let (width, height, x, y) = parse_window_geometry(geometry_token)?;

    Some(WindowDescriptor {
        id,
        title,
        width,
        height,
        x,
        y,
    })
}

fn parse_monitor_geometry(input: &str) -> Option<(u32, u32, i32, i32)> {
    let (resolution, offsets) = input.split_once('+')?;
    let (width_token, height_token) = resolution.split_once('x')?;
    let width = width_token.split('/').next()?.parse().ok()?;
    let height = height_token.split('/').next()?.parse().ok()?;
    let (x_token, y_token) = offsets.split_once('+')?;
    let x = x_token.parse().ok()?;
    let y = y_token.parse().ok()?;
    Some((width, height, x, y))
}

fn parse_window_geometry(input: &str) -> Option<(u32, u32, i32, i32)> {
    let (size, offsets) = input.split_once('+')?;
    let (width_token, height_token) = size.split_once('x')?;
    let width = width_token.parse().ok()?;
    let height = height_token.parse().ok()?;
    let (x_token, y_token) = offsets.split_once('+')?;
    let x = x_token.parse().ok()?;
    let y = y_token.parse().ok()?;
    Some((width, height, x, y))
}

fn normalize_display(display: &str) -> String {
    if display.contains('.') {
        display.to_string()
    } else {
        format!("{display}.0")
    }
}

fn current_desktop_session() -> LinuxDesktopSession {
    classify_desktop_session(
        env::var("DISPLAY").ok().as_deref(),
        env::var("WAYLAND_DISPLAY").ok().as_deref(),
    )
}

fn classify_desktop_session(
    display: Option<&str>,
    wayland_display: Option<&str>,
) -> LinuxDesktopSession {
    let display = display
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let wayland_display = wayland_display
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);

    match (display, wayland_display) {
        (Some(display), Some(wayland_display)) => LinuxDesktopSession::WaylandWithX11 {
            wayland_display,
            x11_display: display,
        },
        (Some(display), None) => LinuxDesktopSession::X11 { display },
        (None, Some(wayland_display)) => LinuxDesktopSession::WaylandOnly { wayland_display },
        (None, None) => LinuxDesktopSession::Headless,
    }
}

impl LinuxDesktopSession {
    fn backend_name(&self) -> String {
        match self {
            LinuxDesktopSession::X11 { .. } => "Linux ffmpeg / x11grab".to_string(),
            LinuxDesktopSession::WaylandWithX11 { .. } => {
                "Linux ffmpeg / x11grab (XWayland)".to_string()
            }
            LinuxDesktopSession::WaylandOnly { .. } => {
                "Linux ScreenCast portal / PipeWire (session negotiation ready)".to_string()
            }
            LinuxDesktopSession::Headless => "Linux capture backend".to_string(),
        }
    }

    fn capture_guidance(&self) -> String {
        match self {
            LinuxDesktopSession::WaylandOnly { wayland_display } => format!(
                "Wayland session {wayland_display} was detected without an X11 DISPLAY. The repo now has a native ScreenCast portal lifecycle client, but the recorder still needs PipeWire stream ingestion before pure Wayland capture can run end-to-end."
            ),
            LinuxDesktopSession::Headless => {
                "No X11 display was detected. This Linux backend currently records through X11grab, so it needs DISPLAY to be set (for example :0 or :1).".to_string()
            }
            LinuxDesktopSession::X11 { .. } | LinuxDesktopSession::WaylandWithX11 { .. } => {
                "Linux capture could not resolve the current X11 display.".to_string()
            }
        }
    }
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

fn encoder_for_quality(preset: &str) -> VideoEncoderProfile {
    let preferred = preferred_video_encoder();
    if preferred.codec == "libx264" {
        VideoEncoderProfile {
            codec: "libx264",
            preset: Some(cpu_preset_for_quality(preset)),
            vaapi_device: None,
        }
    } else {
        preferred
    }
}

fn preferred_video_encoder() -> VideoEncoderProfile {
    static ENCODER: OnceLock<VideoEncoderProfile> = OnceLock::new();
    ENCODER
        .get_or_init(|| {
            let encoders = load_ffmpeg_encoders().unwrap_or_default();
            let has_nvidia = Command::new("nvidia-smi")
                .arg("--query-gpu=name")
                .arg("--format=csv,noheader")
                .output()
                .map(|output| output.status.success())
                .unwrap_or(false);

            if has_nvidia && encoders.contains("h264_nvenc") {
                VideoEncoderProfile {
                    codec: "h264_nvenc",
                    preset: None,
                    vaapi_device: None,
                }
            } else if encoders.contains("h264_vaapi") {
                if let Some(device) = preferred_vaapi_device() {
                    VideoEncoderProfile {
                        codec: "h264_vaapi",
                        preset: None,
                        vaapi_device: Some(device),
                    }
                } else {
                    VideoEncoderProfile {
                        codec: "libx264",
                        preset: None,
                        vaapi_device: None,
                    }
                }
            } else {
                VideoEncoderProfile {
                    codec: "libx264",
                    preset: None,
                    vaapi_device: None,
                }
            }
        })
        .clone()
}

fn cpu_preset_for_quality(preset: &str) -> &'static str {
    match preset {
        "4K / 60 fps" | "1440p / 60 fps" => "ultrafast",
        "1080p / 60 fps" => "superfast",
        _ => "veryfast",
    }
}

fn needs_scale_filter(source_size: Option<(u32, u32)>, width: u32, height: u32) -> bool {
    !matches!(source_size, Some((source_width, source_height)) if source_width == width && source_height == height)
}

fn video_filter(
    source_size: Option<(u32, u32)>,
    width: u32,
    height: u32,
    encoder: &VideoEncoderProfile,
) -> Option<String> {
    if encoder.codec == "h264_vaapi" {
        if needs_scale_filter(source_size, width, height) {
            return Some(format!(
                "scale={width}:{height}:force_original_aspect_ratio=decrease,pad={width}:{height}:(ow-iw)/2:(oh-ih)/2,format=nv12,hwupload"
            ));
        }

        return Some("format=nv12,hwupload".to_string());
    }

    if needs_scale_filter(source_size, width, height) {
        return Some(scale_filter(width, height));
    }

    None
}

fn scale_filter(width: u32, height: u32) -> String {
    format!(
        "scale={width}:{height}:force_original_aspect_ratio=decrease,pad={width}:{height}:(ow-iw)/2:(oh-ih)/2"
    )
}

fn preferred_vaapi_device() -> Option<String> {
    let render_directory = fs::read_dir("/dev/dri").ok()?;
    let mut candidates = render_directory
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let file_name = entry.file_name();
            let file_name = file_name.to_string_lossy();
            if file_name.starts_with("renderD") {
                Some(entry.path().display().to_string())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    candidates.sort();
    candidates.into_iter().next()
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

fn encoder_label(profile: &VideoEncoderProfile) -> String {
    match profile.preset {
        Some(preset) => format!("{} · {}", profile.codec, preset),
        None => profile.codec.to_string(),
    }
}

fn verify_process_started(
    child: &mut Child,
    stderr_buffer: &Arc<Mutex<String>>,
) -> Result<(), CaptureError> {
    for _ in 0..STARTUP_POLL_ATTEMPTS {
        thread::sleep(STARTUP_POLL_INTERVAL);
        if child
            .try_wait()
            .map_err(|error| CaptureError::SpawnFailed(error.to_string()))?
            .is_some()
        {
            return Err(CaptureError::SpawnFailed(describe_ffmpeg_failure(
                &read_stderr_buffer(stderr_buffer),
            )));
        }
    }

    Ok(())
}

fn read_stderr_buffer(buffer: &Arc<Mutex<String>>) -> String {
    buffer.lock().map(|log| log.clone()).unwrap_or_default()
}

fn describe_ffmpeg_failure(stderr_log: &str) -> String {
    let stderr_lower = stderr_log.to_lowercase();

    if stderr_lower.contains("cannot open display") || stderr_lower.contains("x11grab") {
        return format!(
            "ffmpeg could not access the X11 display. Make sure this app is started inside the desktop session and DISPLAY is exported. {}",
            missing_display()
        );
    }

    if stderr_lower.contains("default: no such process")
        || stderr_lower.contains("pulse")
        || stderr_lower.contains("alsa")
    {
        return "ffmpeg could not open the default microphone source. Disable microphone capture and try again.".to_string();
    }

    if stderr_lower.contains("no such file or directory")
        || stderr_lower.contains("command not found")
    {
        return "ffmpeg is not available on this machine. Install ffmpeg first, then retry."
            .to_string();
    }

    stderr_log
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("ffmpeg exited before capture could start.")
        .trim()
        .to_string()
}

fn missing_display() -> String {
    current_desktop_session().capture_guidance()
}

#[cfg(test)]
mod tests {
    use super::{LinuxDesktopSession, classify_desktop_session, parse_monitors, parse_windows};

    #[test]
    fn parses_xrandr_monitor_listing() {
        let monitors = parse_monitors(
            "Monitors: 2\n 0: +*DP-1 1920/600x1080/330+0+0  DP-1\n 1: +HDMI-0 1920/600x1080/330+1920+0  HDMI-0\n",
        );

        assert_eq!(monitors.len(), 2);
        assert_eq!(monitors[0].connector, "DP-1");
        assert_eq!(monitors[0].width, 1920);
        assert_eq!(monitors[0].height, 1080);
        assert_eq!(monitors[0].x, 0);
        assert_eq!(monitors[0].y, 0);
        assert!(monitors[0].is_primary);
        assert_eq!(monitors[1].connector, "HDMI-0");
        assert_eq!(monitors[1].x, 1920);
    }

    #[test]
    fn parses_x11_window_listing() {
        let windows = parse_windows(
            r#"
     0x2000007 "Tilix: demo": ("tilix" "Tilix")  1920x1080+1920+0  +1920+0
     0x7600003 "Record Screen": ("record-screen-desktop" "Record-screen-desktop")  1308x926+333+114  +333+114
     0x160001e "Google Chrome": ("google-chrome" "Google-chrome")  1865x1048+55+32  +55+32
"#,
        );

        assert_eq!(windows.len(), 2);
        assert_eq!(windows[0].id, "0x2000007");
        assert_eq!(windows[0].width, 1920);
        assert_eq!(windows[0].x, 1920);
        assert_eq!(windows[1].title, "Google Chrome");
    }

    #[test]
    fn classifies_pure_x11_session() {
        assert_eq!(
            classify_desktop_session(Some(":0"), None),
            LinuxDesktopSession::X11 {
                display: ":0".to_string()
            }
        );
    }

    #[test]
    fn classifies_xwayland_session() {
        assert_eq!(
            classify_desktop_session(Some(":1"), Some("wayland-0")),
            LinuxDesktopSession::WaylandWithX11 {
                wayland_display: "wayland-0".to_string(),
                x11_display: ":1".to_string()
            }
        );
    }

    #[test]
    fn classifies_pure_wayland_session() {
        assert_eq!(
            classify_desktop_session(None, Some("wayland-1")),
            LinuxDesktopSession::WaylandOnly {
                wayland_display: "wayland-1".to_string()
            }
        );
    }
}
