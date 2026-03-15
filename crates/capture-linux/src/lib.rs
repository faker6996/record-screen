pub mod native_audio_backend;
mod native_encoder_backend;
pub mod wayland_portal;

use std::{
    env, fs,
    io::{Read, Write},
    os::unix::process::ExitStatusExt,
    os::{fd::AsRawFd, unix::process::CommandExt},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{Arc, Mutex, OnceLock},
    thread,
    time::{Duration, SystemTime},
};

use capture::{
    ActiveRecording, AudioBackendAvailability, AudioBackendDescriptor, AudioBackendFactory,
    AudioBackendFamily, AudioBackendRuntimeReport, AudioBackendStatus, AudioInputKind,
    AudioInputOption, CUSTOM_REGION_TARGET_ID, CaptureBackendAvailability,
    CaptureBackendDescriptor, CaptureBackendFactory, CaptureBackendFamily,
    CaptureBackendRuntimeReport, CaptureBackendRuntimeSnapshot, CaptureBackendStatus,
    CaptureController, CaptureError, CaptureTargetOption, DEFAULT_AUDIO_INPUT_ID,
    EncoderBackendAvailability, EncoderBackendDescriptor, EncoderBackendFactory,
    EncoderBackendFamily, EncoderBackendRuntimeReport, EncoderBackendRuntimeSnapshot,
    EncoderBackendStatus, FULL_DESKTOP_TARGET_ID, RecordingArtifact, RecordingOptions,
    audio_backend_runtime_snapshot, audio_backend_statuses as shared_audio_backend_statuses,
    backend_statuses as shared_backend_statuses, capture_backend_runtime_snapshot,
    default_audio_input, encoder_backend_runtime_snapshot,
    encoder_backend_statuses as shared_encoder_backend_statuses, explain_audio_backend_selection,
    explain_capture_backend_selection, explain_encoder_backend_selection, full_desktop_target,
    preferred_system_audio_input, resolve_audio_input_id, select_audio_backend, select_backend,
    select_encoder_backend,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LinuxCaptureProcessKind {
    FfmpegX11,
    GstreamerWayland,
}

pub struct FfmpegLinuxCapture {
    active_recording: ActiveRecording,
    process_kind: LinuxCaptureProcessKind,
    child: Child,
    stdin: Option<ChildStdin>,
    stderr_buffer: Arc<Mutex<String>>,
    finished_artifact: Option<RecordingArtifact>,
    paused: bool,
}

pub struct FfmpegLinuxBackend;
pub struct PortalPipewireLinuxBackend;
static FFMPEG_LINUX_BACKEND: FfmpegLinuxBackend = FfmpegLinuxBackend;
static PORTAL_PIPEWIRE_LINUX_BACKEND: PortalPipewireLinuxBackend = PortalPipewireLinuxBackend;
static FFMPEG_LINUX_AUDIO_BACKEND: FfmpegLinuxAudioBackend = FfmpegLinuxAudioBackend;
static FFMPEG_LINUX_ENCODER_BACKEND: FfmpegLinuxEncoderBackend = FfmpegLinuxEncoderBackend;
pub struct FfmpegLinuxAudioBackend;
pub struct FfmpegLinuxEncoderBackend;

pub fn selected_backend() -> &'static dyn CaptureBackendFactory {
    select_backend(&backend_candidates())
}

fn backend_candidates() -> [&'static dyn CaptureBackendFactory; 2] {
    [&PORTAL_PIPEWIRE_LINUX_BACKEND, &FFMPEG_LINUX_BACKEND]
}

pub fn backend_statuses() -> Vec<CaptureBackendStatus> {
    shared_backend_statuses(&backend_candidates())
}

pub fn selected_audio_backend() -> &'static dyn AudioBackendFactory {
    select_audio_backend(&audio_backend_candidates())
}

fn audio_backend_candidates() -> [&'static dyn AudioBackendFactory; 2] {
    [native_audio_backend::backend(), &FFMPEG_LINUX_AUDIO_BACKEND]
}

pub fn audio_backend_statuses() -> Vec<AudioBackendStatus> {
    shared_audio_backend_statuses(&audio_backend_candidates())
}

pub fn selected_encoder_backend() -> &'static dyn EncoderBackendFactory {
    select_encoder_backend(&encoder_backend_candidates())
}

fn encoder_backend_candidates() -> [&'static dyn EncoderBackendFactory; 2] {
    [
        native_encoder_backend::backend(),
        &FFMPEG_LINUX_ENCODER_BACKEND,
    ]
}

pub fn encoder_backend_statuses() -> Vec<EncoderBackendStatus> {
    shared_encoder_backend_statuses(&encoder_backend_candidates())
}

pub fn capture_selection_note() -> String {
    explain_capture_backend_selection(&backend_candidates()).note
}

pub fn capture_runtime_snapshot() -> CaptureBackendRuntimeSnapshot {
    capture_backend_runtime_snapshot(&backend_candidates())
}

pub fn audio_selection_note() -> String {
    explain_audio_backend_selection(&audio_backend_candidates()).note
}

pub fn audio_runtime_snapshot() -> capture::AudioBackendRuntimeSnapshot {
    audio_backend_runtime_snapshot(&audio_backend_candidates())
}

pub fn encoder_selection_note() -> String {
    explain_encoder_backend_selection(&encoder_backend_candidates()).note
}

pub fn encoder_runtime_snapshot() -> EncoderBackendRuntimeSnapshot {
    encoder_backend_runtime_snapshot(&encoder_backend_candidates())
}

impl CaptureBackendFactory for PortalPipewireLinuxBackend {
    fn descriptor(&self) -> CaptureBackendDescriptor {
        CaptureBackendDescriptor {
            id: "linux-portal-pipewire",
            label: "Linux ScreenCast Portal / PipeWire",
            family: CaptureBackendFamily::Native,
        }
    }

    fn availability(&self) -> CaptureBackendAvailability {
        CaptureBackendAvailability::Unavailable {
            reason: "The Linux Wayland-native portal / PipeWire backend is still experimental and is not yet the default recorder runtime.".to_string(),
        }
    }

    fn runtime_report(&self) -> CaptureBackendRuntimeReport {
        CaptureBackendRuntimeReport {
            summary: Some(
                "Linux native capture candidate negotiates ScreenCast Portal / PipeWire, but the production runtime is not ready yet."
                    .to_string(),
            ),
            preferred_target_label: Some("Full desktop".to_string()),
        }
    }

    fn start(
        &self,
        _options: RecordingOptions,
    ) -> Result<Box<dyn CaptureController>, CaptureError> {
        Err(CaptureError::BackendUnavailable(
            "The Linux Wayland-native portal / PipeWire backend is not ready as the default runtime yet.".to_string(),
        ))
    }
}

impl CaptureBackendFactory for FfmpegLinuxBackend {
    fn descriptor(&self) -> CaptureBackendDescriptor {
        CaptureBackendDescriptor {
            id: "linux-ffmpeg-capture",
            label: "Linux ffmpeg recorder",
            family: CaptureBackendFamily::FallbackFfmpeg,
        }
    }

    fn availability(&self) -> CaptureBackendAvailability {
        CaptureBackendAvailability::Available
    }

    fn runtime_report(&self) -> CaptureBackendRuntimeReport {
        CaptureBackendRuntimeReport {
            summary: Some(
                "Current Linux capture runtime uses ffmpeg on X11/XWayland, with an experimental GStreamer path for pure Wayland sessions."
                    .to_string(),
            ),
            preferred_target_label: Some("Full desktop".to_string()),
        }
    }

    fn start(&self, options: RecordingOptions) -> Result<Box<dyn CaptureController>, CaptureError> {
        Ok(Box::new(FfmpegLinuxCapture::start(options)?))
    }
}

impl AudioBackendFactory for FfmpegLinuxAudioBackend {
    fn descriptor(&self) -> AudioBackendDescriptor {
        AudioBackendDescriptor {
            id: "linux-ffmpeg-pulse-audio",
            label: "Linux PulseAudio / ffmpeg audio",
            family: AudioBackendFamily::FallbackFfmpeg,
        }
    }

    fn availability(&self) -> AudioBackendAvailability {
        AudioBackendAvailability::Available
    }

    fn runtime_report(&self) -> AudioBackendRuntimeReport {
        let audio_inputs = query_audio_inputs().unwrap_or_default();
        let preferred_input = capture::preferred_audio_input(&audio_inputs)
            .map(|input| input.label.clone())
            .or_else(native_preferred_input_label);
        let preferred_system = preferred_system_audio_input(&audio_inputs)
            .map(|input| input.label.clone())
            .or_else(native_preferred_system_label);

        AudioBackendRuntimeReport {
            summary: Some(audio_input_support_summary()),
            preferred_input_id: native_preferred_input_label(),
            preferred_input_label: preferred_input,
            preferred_system_id: native_preferred_system_label(),
            preferred_system_label: preferred_system,
        }
    }
}

impl EncoderBackendFactory for FfmpegLinuxEncoderBackend {
    fn descriptor(&self) -> EncoderBackendDescriptor {
        EncoderBackendDescriptor {
            id: "linux-ffmpeg-encoder",
            label: "Linux ffmpeg encoder",
            family: EncoderBackendFamily::FallbackFfmpeg,
        }
    }

    fn availability(&self) -> EncoderBackendAvailability {
        EncoderBackendAvailability::Available
    }

    fn runtime_report(&self) -> EncoderBackendRuntimeReport {
        EncoderBackendRuntimeReport {
            summary: Some(format!(
                "Current Linux output pipeline uses ffmpeg with preferred encoder `{}`.",
                encoder_label(&preferred_video_encoder())
            )),
            preferred_encoder_label: Some(encoder_label(&preferred_video_encoder())),
        }
    }
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
        let started_at = SystemTime::now();
        let stderr_buffer = Arc::new(Mutex::new(String::new()));
        let (target_label, encoder_label, process_kind, child, stdin) = match &session {
            LinuxDesktopSession::X11 { display } => {
                let target = resolve_target(&options)?;
                let encoder = encoder_for_quality(&options.quality_preset);
                let (child, stdin) =
                    spawn_ffmpeg(&options, display, &target, Arc::clone(&stderr_buffer))?;
                (
                    target.label,
                    encoder_label(&encoder),
                    LinuxCaptureProcessKind::FfmpegX11,
                    child,
                    stdin,
                )
            }
            LinuxDesktopSession::WaylandWithX11 { x11_display, .. } => {
                let target = resolve_target(&options)?;
                let encoder = encoder_for_quality(&options.quality_preset);
                let (child, stdin) =
                    spawn_ffmpeg(&options, x11_display, &target, Arc::clone(&stderr_buffer))?;
                (
                    target.label,
                    encoder_label(&encoder),
                    LinuxCaptureProcessKind::FfmpegX11,
                    child,
                    stdin,
                )
            }
            LinuxDesktopSession::WaylandOnly { wayland_display } => {
                if options.capture_target_id != FULL_DESKTOP_TARGET_ID {
                    return Err(CaptureError::BackendUnavailable(format!(
                        "Wayland session {wayland_display} currently records through the ScreenCast portal chooser. Window and monitor targeting from the launcher is not wired into the pure Wayland path yet."
                    )));
                }
                if options.system_audio_enabled {
                    return Err(CaptureError::BackendUnavailable(format!(
                        "Wayland session {wayland_display} currently uses the experimental ScreenCast portal + GStreamer path, and system-audio mixing is not wired into that runtime yet."
                    )));
                }

                let runtime_session = wayland_portal::negotiate_runtime_session().map_err(|error| {
                    CaptureError::BackendUnavailable(format!(
                        "Wayland session {wayland_display} could reach the ScreenCast portal path, but session negotiation did not complete: {error}"
                    ))
                })?;
                let (child, stdin) = spawn_wayland_gstreamer(
                    &options,
                    &runtime_session,
                    Arc::clone(&stderr_buffer),
                )?;
                (
                    "Wayland ScreenCast selection".to_string(),
                    wayland_encoder_label(&options.quality_preset),
                    LinuxCaptureProcessKind::GstreamerWayland,
                    child,
                    stdin,
                )
            }
            LinuxDesktopSession::Headless => {
                return Err(CaptureError::BackendUnavailable(session.capture_guidance()));
            }
        };

        Ok(Self {
            active_recording: ActiveRecording {
                backend_name: session.backend_name(),
                encoder_label,
                output_path: options.output_path,
                started_at,
                target_label,
            },
            process_kind,
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

        request_process_stop(self.process_kind, self.child.id(), self.stdin.as_mut())?;

        let status = self
            .child
            .wait()
            .map_err(|error| CaptureError::StopFailed(error.to_string()))?;

        if !status.success()
            && status.signal() != Some(libc::SIGTERM)
            && status.signal() != Some(libc::SIGINT)
        {
            return Err(CaptureError::StopFailed(format!(
                "capture process exited with status {status}: {}",
                describe_process_failure(
                    self.process_kind,
                    &read_stderr_buffer(&self.stderr_buffer)
                )
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

        if !status.success()
            && status.signal() != Some(libc::SIGTERM)
            && status.signal() != Some(libc::SIGINT)
        {
            return Err(CaptureError::StopFailed(describe_process_failure(
                self.process_kind,
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
    let mut default_input = default_audio_input();
    if let Some(preferred_input) = native_preferred_input_label() {
        default_input.description =
            format!("Use the preferred Linux input source: {preferred_input}.");
    }

    let mut inputs = vec![default_input];
    inputs.extend(query_audio_inputs().unwrap_or_default());
    inputs
}

pub fn audio_input_support_summary() -> String {
    match query_audio_inputs() {
        Ok(audio_inputs) => {
            let microphone_count = audio_inputs
                .iter()
                .filter(|input| input.kind == AudioInputKind::Microphone)
                .count();
            let system_count = audio_inputs
                .iter()
                .filter(|input| input.kind == AudioInputKind::System)
                .count();

            let base = format!(
                "Linux audio discovery is ready. Found {} microphone source{} and {} system-audio source{}.",
                microphone_count,
                if microphone_count == 1 { "" } else { "s" },
                system_count,
                if system_count == 1 { "" } else { "s" }
            );

            match native_audio_backend::pipewire_runtime_summary() {
                Some(summary) => format!("{base} {summary}"),
                None => base,
            }
        }
        Err(error) => match native_audio_backend::pipewire_runtime_summary() {
            Some(summary) => format!("Linux audio discovery failed. {summary} {error}"),
            None => format!("Linux audio discovery failed. {error}"),
        },
    }
}

fn native_preferred_input_label() -> Option<String> {
    native_audio_backend::preferred_input_source_name()
}

fn native_preferred_system_label() -> Option<String> {
    native_audio_backend::preferred_monitor_source_name()
}

pub fn preview_target_bounds(
    capture_target_id: &str,
    region_x: u32,
    region_y: u32,
    region_width: u32,
    region_height: u32,
) -> Result<(i32, i32, u32, u32), CaptureError> {
    let target = resolve_target(&RecordingOptions {
        output_path: std::env::temp_dir().join("record-screen-preview.mp4"),
        quality_preset: "1080p / 30 fps".to_string(),
        mic_enabled: false,
        system_audio_enabled: false,
        capture_target_id: capture_target_id.to_string(),
        audio_input_id: DEFAULT_AUDIO_INPUT_ID.to_string(),
        region_x,
        region_y,
        region_width,
        region_height,
        region_source_capture_target_id: FULL_DESKTOP_TARGET_ID.to_string(),
        region_source_origin_x: 0,
        region_source_origin_y: 0,
        region_source_scale_factor_milli: 1000,
    })?;
    let (width, height) = target.video_size.unwrap_or((640, 360));
    Ok((target.origin_x, target.origin_y, width, height))
}

pub fn custom_region_support_summary() -> (bool, String) {
    match current_desktop_session() {
        LinuxDesktopSession::X11 { .. } => (
            true,
            "Custom region capture is available through the native X11 path.".to_string(),
        ),
        LinuxDesktopSession::WaylandWithX11 { .. } => (
            true,
            "Custom region capture is available through the XWayland compatibility path."
                .to_string(),
        ),
        LinuxDesktopSession::WaylandOnly { .. } => (
            false,
            "Pure Wayland recording currently uses the ScreenCast portal chooser, so launcher-defined custom regions are not wired in yet."
                .to_string(),
        ),
        LinuxDesktopSession::Headless => (
            false,
            "Custom region capture needs an active desktop session.".to_string(),
        ),
    }
}

pub fn system_audio_support_summary() -> (bool, String) {
    match current_desktop_session() {
        LinuxDesktopSession::X11 { .. } | LinuxDesktopSession::WaylandWithX11 { .. } => {
            match resolve_system_audio_input() {
                Ok(_) => (
                    true,
                    "A PulseAudio/PipeWire monitor source is available for system-audio mixing."
                        .to_string(),
                ),
                Err(error) => (
                    false,
                    format!(
                        "Linux could not find a usable PulseAudio/PipeWire monitor source. {error}"
                    ),
                ),
            }
        }
        LinuxDesktopSession::WaylandOnly { .. } => (
            false,
            "Pure Wayland recording currently uses the experimental ScreenCast portal + GStreamer path, and system-audio mixing is not wired into that runtime yet."
                .to_string(),
        ),
        LinuxDesktopSession::Headless => (
            false,
            "System-audio mixing needs an active desktop session.".to_string(),
        ),
    }
}

fn resolve_target(options: &RecordingOptions) -> Result<ResolvedTarget, CaptureError> {
    let target_id = options.capture_target_id.as_str();
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

        if target_id == CUSTOM_REGION_TARGET_ID {
            return Ok(ResolvedTarget {
                label: format!(
                    "Custom region · {}, {} · {} x {}",
                    options.region_x, options.region_y, options.region_width, options.region_height
                ),
                origin_x: options.region_x as i32,
                origin_y: options.region_y as i32,
                video_size: Some((options.region_width.max(64), options.region_height.max(64))),
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

    let mut audio_input_count = 0;
    if options.mic_enabled {
        let audio_input = resolve_audio_input(&options.audio_input_id)?;
        command
            .arg("-f")
            .arg("pulse")
            .arg("-thread_queue_size")
            .arg("1024")
            .arg("-i")
            .arg(audio_input);
        audio_input_count += 1;
    }

    if options.system_audio_enabled {
        let system_audio_input = resolve_system_audio_input()?;
        command
            .arg("-f")
            .arg("pulse")
            .arg("-thread_queue_size")
            .arg("1024")
            .arg("-i")
            .arg(system_audio_input);
        audio_input_count += 1;
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

    match audio_input_count {
        0 => {
            command.arg("-an");
        }
        1 => {
            command
                .arg("-map")
                .arg("0:v")
                .arg("-map")
                .arg("1:a")
                .arg("-c:a")
                .arg("aac")
                .arg("-b:a")
                .arg("192k");
        }
        _ => {
            command
                .arg("-filter_complex")
                .arg("[1:a][2:a]amix=inputs=2:normalize=0[aout]")
                .arg("-map")
                .arg("0:v")
                .arg("-map")
                .arg("[aout]")
                .arg("-c:a")
                .arg("aac")
                .arg("-b:a")
                .arg("192k");
        }
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

    verify_process_started(
        &mut child,
        &stderr_buffer,
        LinuxCaptureProcessKind::FfmpegX11,
    )?;

    Ok((child, stdin))
}

fn spawn_wayland_gstreamer(
    options: &RecordingOptions,
    runtime_session: &wayland_portal::ScreenCastPortalRuntimeSession,
    stderr_buffer: Arc<Mutex<String>>,
) -> Result<(Child, Option<ChildStdin>), CaptureError> {
    let stream_node_id = runtime_session
        .stream_node_ids
        .first()
        .copied()
        .ok_or_else(|| {
            CaptureError::BackendUnavailable(
                "the ScreenCast portal did not return any PipeWire stream node IDs".to_string(),
            )
        })?;
    let remote_fd = runtime_session.pipewire_remote_fd.as_raw_fd();
    let args = build_wayland_gstreamer_args(options, stream_node_id)?;

    let mut command = Command::new("gst-launch-1.0");
    command
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    unsafe {
        command.pre_exec(move || {
            if libc::dup2(remote_fd, 3) == -1 {
                return Err(std::io::Error::last_os_error());
            }

            if libc::fcntl(3, libc::F_SETFD, 0) == -1 {
                return Err(std::io::Error::last_os_error());
            }

            Ok(())
        });
    }

    let mut child = command
        .spawn()
        .map_err(|error| CaptureError::SpawnFailed(error.to_string()))?;

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

    verify_process_started(
        &mut child,
        &stderr_buffer,
        LinuxCaptureProcessKind::GstreamerWayland,
    )?;

    Ok((child, None))
}

fn build_wayland_gstreamer_args(
    options: &RecordingOptions,
    stream_node_id: u32,
) -> Result<Vec<String>, CaptureError> {
    let (width, height, fps) = quality_settings(&options.quality_preset);
    let speed_preset = cpu_preset_for_quality(&options.quality_preset);
    let bitrate_kbps = gst_bitrate_for_quality(&options.quality_preset);
    let target_object = stream_node_id.to_string();
    let output_location = options.output_path.display().to_string();
    let mut args = vec![
        "-e".to_string(),
        "pipewiresrc".to_string(),
        "fd=3".to_string(),
        format!("target-object={target_object}"),
        "do-timestamp=true".to_string(),
        "keepalive-time=1000".to_string(),
        "!".to_string(),
        "queue".to_string(),
        "!".to_string(),
        "videoconvert".to_string(),
        "!".to_string(),
        "videoscale".to_string(),
        "!".to_string(),
        "videorate".to_string(),
        "!".to_string(),
        format!("video/x-raw,width={width},height={height},framerate={fps}/1"),
        "!".to_string(),
        "x264enc".to_string(),
        format!("speed-preset={speed_preset}"),
        "tune=zerolatency".to_string(),
        format!("bitrate={bitrate_kbps}"),
        format!("key-int-max={fps}"),
        "!".to_string(),
        "h264parse".to_string(),
        "config-interval=-1".to_string(),
        "!".to_string(),
        "queue".to_string(),
        "!".to_string(),
        "mux.video_0".to_string(),
    ];

    if options.mic_enabled {
        args.push("pulsesrc".to_string());
        args.push("do-timestamp=true".to_string());

        if let Some(audio_device) = gst_audio_input_device(&options.audio_input_id)? {
            args.push(format!("device={audio_device}"));
        }

        args.extend([
            "!".to_string(),
            "queue".to_string(),
            "!".to_string(),
            "audioconvert".to_string(),
            "!".to_string(),
            "audioresample".to_string(),
            "!".to_string(),
            "voaacenc".to_string(),
            "bitrate=192000".to_string(),
            "!".to_string(),
            "aacparse".to_string(),
            "!".to_string(),
            "queue".to_string(),
            "!".to_string(),
            "mux.audio_0".to_string(),
        ]);
    }

    args.extend([
        "mp4mux".to_string(),
        "name=mux".to_string(),
        "faststart=true".to_string(),
        "!".to_string(),
        "filesink".to_string(),
        format!("location={output_location}"),
    ]);

    Ok(args)
}

fn resolve_audio_input(audio_input_id: &str) -> Result<String, CaptureError> {
    resolve_audio_input_from_snapshot(audio_input_id, query_audio_inputs().ok().as_deref())
}

fn resolve_system_audio_input() -> Result<String, CaptureError> {
    let audio_inputs = query_audio_inputs()?;
    preferred_system_audio_input(&audio_inputs)
        .map(|input| input.id.clone())
        .ok_or_else(|| {
            CaptureError::BackendUnavailable(
                "Linux could not find a usable system-audio monitor source. Disable system audio and try again."
                    .to_string(),
            )
        })
}

fn gst_audio_input_device(audio_input_id: &str) -> Result<Option<String>, CaptureError> {
    if audio_input_id == DEFAULT_AUDIO_INPUT_ID {
        return Ok(None);
    }

    Ok(Some(resolve_audio_input(audio_input_id)?))
}

fn resolve_audio_input_from_snapshot(
    audio_input_id: &str,
    audio_inputs: Option<&[AudioInputOption]>,
) -> Result<String, CaptureError> {
    if audio_input_id == DEFAULT_AUDIO_INPUT_ID {
        return audio_inputs
            .and_then(|inputs| resolve_audio_input_id(audio_input_id, inputs))
            .or_else(|| {
                audio_inputs.and_then(|inputs| {
                    inputs
                        .iter()
                        .find(|input| input.id == DEFAULT_AUDIO_INPUT_ID)
                        .map(|input| input.id.clone())
                })
            })
            .or_else(|| Some("default".to_string()))
            .ok_or_else(|| {
                CaptureError::BackendUnavailable(
                    "ffmpeg could not find any usable PulseAudio microphone source".to_string(),
                )
            });
    }

    if let Some(audio_inputs) = audio_inputs {
        return resolve_audio_input_id(audio_input_id, audio_inputs).ok_or_else(|| {
            CaptureError::BackendUnavailable(format!(
                "the selected microphone input `{audio_input_id}` is no longer available"
            ))
        });
    }

    Ok(audio_input_id.to_string())
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

        let kind = if name.ends_with(".monitor") || line.to_ascii_lowercase().contains("monitor") {
            AudioInputKind::System
        } else {
            AudioInputKind::Microphone
        };

        let label = if kind == AudioInputKind::System {
            format!("System audio · {name}")
        } else {
            name.to_string()
        };

        inputs.push(AudioInputOption {
            id: name.to_string(),
            label,
            description: if kind == AudioInputKind::System {
                format!("PulseAudio/PipeWire monitor source: {name}")
            } else {
                format!("PulseAudio source: {name}")
            },
            kind,
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
                "Linux ScreenCast portal / PipeWire".to_string()
            }
            LinuxDesktopSession::Headless => "Linux capture backend".to_string(),
        }
    }

    fn capture_guidance(&self) -> String {
        match self {
            LinuxDesktopSession::WaylandOnly { wayland_display } => format!(
                "Wayland session {wayland_display} was detected without an X11 DISPLAY. The recorder now tries a native ScreenCast portal plus GStreamer PipeWire path, but it still depends on portal approval, PipeWire, and the required GStreamer plugins being available."
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
        "1080p / 60 fps" => (1920, 1080, 60),
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

fn wayland_encoder_label(preset: &str) -> String {
    format!(
        "gstreamer / pipewiresrc · x264enc · {}",
        cpu_preset_for_quality(preset)
    )
}

fn gst_bitrate_for_quality(preset: &str) -> u32 {
    match preset {
        "720p / 30 fps" => 4_000,
        "1080p / 30 fps" => 8_000,
        "1080p / 60 fps" => 12_000,
        "1440p / 60 fps" => 18_000,
        "4K / 60 fps" => 30_000,
        _ => 8_000,
    }
}

fn verify_process_started(
    child: &mut Child,
    stderr_buffer: &Arc<Mutex<String>>,
    process_kind: LinuxCaptureProcessKind,
) -> Result<(), CaptureError> {
    for _ in 0..STARTUP_POLL_ATTEMPTS {
        thread::sleep(STARTUP_POLL_INTERVAL);
        if child
            .try_wait()
            .map_err(|error| CaptureError::SpawnFailed(error.to_string()))?
            .is_some()
        {
            return Err(CaptureError::SpawnFailed(describe_process_failure(
                process_kind,
                &read_stderr_buffer(stderr_buffer),
            )));
        }
    }

    Ok(())
}

fn read_stderr_buffer(buffer: &Arc<Mutex<String>>) -> String {
    buffer.lock().map(|log| log.clone()).unwrap_or_default()
}

fn describe_process_failure(process_kind: LinuxCaptureProcessKind, stderr_log: &str) -> String {
    match process_kind {
        LinuxCaptureProcessKind::FfmpegX11 => describe_ffmpeg_failure(stderr_log),
        LinuxCaptureProcessKind::GstreamerWayland => describe_gstreamer_failure(stderr_log),
    }
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

fn describe_gstreamer_failure(stderr_log: &str) -> String {
    let stderr_lower = stderr_log.to_lowercase();

    if stderr_lower.contains("no element \"pipewiresrc\"") {
        return "GStreamer PipeWire support is missing on this machine. Install the PipeWire GStreamer plugin first.".to_string();
    }

    if stderr_lower.contains("could not open resource for reading")
        || stderr_lower.contains("failed to connect")
        || stderr_lower.contains("pipewire")
    {
        return "The ScreenCast portal returned a PipeWire stream, but GStreamer could not attach to it. Check that PipeWire and xdg-desktop-portal are running in this Wayland session.".to_string();
    }

    if stderr_lower.contains("pulsesrc") || stderr_lower.contains("pulse") {
        return "GStreamer could not open the selected microphone source. Disable microphone capture and try again.".to_string();
    }

    stderr_log
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("the Wayland GStreamer capture process exited before capture could start.")
        .trim()
        .to_string()
}

fn request_process_stop(
    process_kind: LinuxCaptureProcessKind,
    pid: u32,
    stdin: Option<&mut ChildStdin>,
) -> Result<(), CaptureError> {
    match process_kind {
        LinuxCaptureProcessKind::FfmpegX11 => {
            if let Some(stdin) = stdin {
                stdin
                    .write_all(b"q\n")
                    .and_then(|_| stdin.flush())
                    .map_err(|error| CaptureError::StopFailed(error.to_string()))?;
            }

            Ok(())
        }
        LinuxCaptureProcessKind::GstreamerWayland => {
            let result = unsafe { libc::kill(pid as i32, libc::SIGINT) };
            if result != 0 {
                return Err(CaptureError::StopFailed(
                    "failed to send SIGINT to gst-launch".to_string(),
                ));
            }

            Ok(())
        }
    }
}

fn missing_display() -> String {
    current_desktop_session().capture_guidance()
}

#[cfg(test)]
mod tests {
    use std::{env, path::PathBuf};

    use capture::{DEFAULT_AUDIO_INPUT_ID, FULL_DESKTOP_TARGET_ID, RecordingOptions};

    use super::{
        LinuxDesktopSession, build_wayland_gstreamer_args, classify_desktop_session,
        gst_bitrate_for_quality, parse_monitors, parse_windows,
    };

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

    #[test]
    fn default_audio_input_falls_back_without_discovery() {
        assert_eq!(
            super::resolve_audio_input_from_snapshot(capture::DEFAULT_AUDIO_INPUT_ID, None)
                .expect("default input should still resolve"),
            "default"
        );
    }

    #[test]
    fn explicit_audio_input_is_preserved_without_discovery() {
        assert_eq!(
            super::resolve_audio_input_from_snapshot("alsa_input.usb-mic", None)
                .expect("explicit input should be preserved"),
            "alsa_input.usb-mic"
        );
    }

    #[test]
    fn builds_wayland_gstreamer_args_without_microphone_branch() {
        let output_path = env::temp_dir().join("record-screen-wayland-no-mic.mp4");
        let options = RecordingOptions {
            output_path: output_path.clone(),
            quality_preset: "1080p / 30 fps".to_string(),
            mic_enabled: false,
            system_audio_enabled: false,
            capture_target_id: "full-desktop".to_string(),
            audio_input_id: DEFAULT_AUDIO_INPUT_ID.to_string(),
            region_x: 160,
            region_y: 120,
            region_width: 1280,
            region_height: 720,
            region_source_capture_target_id: FULL_DESKTOP_TARGET_ID.to_string(),
            region_source_origin_x: 0,
            region_source_origin_y: 0,
            region_source_scale_factor_milli: 1000,
        };

        let args = build_wayland_gstreamer_args(&options, 77).expect("wayland args should build");
        let joined = args.join(" ");

        assert!(joined.contains("pipewiresrc fd=3 target-object=77"));
        assert!(joined.contains("video/x-raw,width=1920,height=1080,framerate=30/1"));
        assert!(joined.contains("x264enc speed-preset=veryfast"));
        assert!(joined.contains("bitrate=8000"));
        assert!(joined.contains("mp4mux name=mux faststart=true"));
        assert!(joined.contains(&format!("location={}", output_path.display())));
        assert!(!joined.contains("pulsesrc"));
        assert!(!joined.contains("mux.audio_0"));
    }

    #[test]
    fn builds_wayland_gstreamer_args_with_explicit_microphone_branch() {
        let options = RecordingOptions {
            output_path: PathBuf::from("/tmp/record-screen-wayland-with-mic.mp4"),
            quality_preset: "1080p / 60 fps".to_string(),
            mic_enabled: true,
            system_audio_enabled: false,
            capture_target_id: "full-desktop".to_string(),
            audio_input_id: "alsa_input.usb-Blue_Yeti".to_string(),
            region_x: 160,
            region_y: 120,
            region_width: 1280,
            region_height: 720,
            region_source_capture_target_id: FULL_DESKTOP_TARGET_ID.to_string(),
            region_source_origin_x: 0,
            region_source_origin_y: 0,
            region_source_scale_factor_milli: 1000,
        };

        let args = build_wayland_gstreamer_args(&options, 9).expect("wayland args should build");
        let joined = args.join(" ");

        assert!(joined.contains("target-object=9"));
        assert!(joined.contains("framerate=60/1"));
        assert!(joined.contains("speed-preset=superfast"));
        assert!(joined.contains("bitrate=12000"));
        assert!(joined.contains("pulsesrc do-timestamp=true device=alsa_input.usb-Blue_Yeti"));
        assert!(joined.contains("voaacenc bitrate=192000"));
        assert!(joined.contains("mux.audio_0"));
    }

    #[test]
    fn maps_quality_to_expected_wayland_bitrate() {
        assert_eq!(gst_bitrate_for_quality("720p / 30 fps"), 4_000);
        assert_eq!(gst_bitrate_for_quality("1080p / 30 fps"), 8_000);
        assert_eq!(gst_bitrate_for_quality("1080p / 60 fps"), 12_000);
        assert_eq!(gst_bitrate_for_quality("1440p / 60 fps"), 18_000);
        assert_eq!(gst_bitrate_for_quality("4K / 60 fps"), 30_000);
    }
}
