mod encoding_support;
pub mod native_audio_backend;
mod native_capture_backend;
mod native_encoder_backend;
mod runtime_support;
pub mod wayland_portal;

use std::{env, fs, process::Command, time::SystemTime};

use capture::{
    AudioBackendFactory, AudioBackendStatus, AudioInputKind, AudioInputOption,
    CUSTOM_REGION_TARGET_ID, CaptureBackendAvailability, CaptureBackendDescriptor,
    CaptureBackendFactory, CaptureBackendRuntimeReport, CaptureBackendRuntimeSnapshot,
    CaptureBackendStatus, CaptureController, CaptureError, CaptureTargetOption,
    DEFAULT_AUDIO_INPUT_ID, EncoderBackendFactory, EncoderBackendRuntimeSnapshot,
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LinuxDesktopSession {
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

pub(crate) use encoding_support::{
    cpu_preset_for_quality, gst_bitrate_for_quality, quality_settings,
};
pub(crate) use runtime_support::{
    LinuxCaptureProcessKind, describe_process_failure, read_stderr_buffer, request_process_stop,
    verify_process_started,
};

pub struct PortalPipewireLinuxBackend;
pub struct GstreamerX11LinuxBackend;
static PORTAL_PIPEWIRE_LINUX_BACKEND: PortalPipewireLinuxBackend = PortalPipewireLinuxBackend;
static GSTREAMER_X11_LINUX_BACKEND: GstreamerX11LinuxBackend = GstreamerX11LinuxBackend;

pub fn selected_backend() -> &'static dyn CaptureBackendFactory {
    select_backend(&backend_candidates())
}

fn backend_candidates() -> [&'static dyn CaptureBackendFactory; 2] {
    [&PORTAL_PIPEWIRE_LINUX_BACKEND, &GSTREAMER_X11_LINUX_BACKEND]
}

pub fn backend_statuses() -> Vec<CaptureBackendStatus> {
    shared_backend_statuses(&backend_candidates())
}

pub fn selected_audio_backend() -> &'static dyn AudioBackendFactory {
    select_audio_backend(&audio_backend_candidates())
}

fn audio_backend_candidates() -> [&'static dyn AudioBackendFactory; 1] {
    [native_audio_backend::backend()]
}

pub fn audio_backend_statuses() -> Vec<AudioBackendStatus> {
    shared_audio_backend_statuses(&audio_backend_candidates())
}

pub fn selected_encoder_backend() -> &'static dyn EncoderBackendFactory {
    select_encoder_backend(&encoder_backend_candidates())
}

fn encoder_backend_candidates() -> [&'static dyn EncoderBackendFactory; 1] {
    [native_encoder_backend::backend()]
}

pub fn encoder_backend_statuses() -> Vec<EncoderBackendStatus> {
    shared_encoder_backend_statuses(&encoder_backend_candidates())
}

pub fn capture_selection_note() -> String {
    explain_capture_backend_selection(&backend_candidates()).note
}

pub fn current_wayland_restore_token() -> Option<String> {
    wayland_portal::current_restore_token()
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
        }
    }

    fn availability(&self) -> CaptureBackendAvailability {
        portal_backend_availability_for(
            &current_desktop_session(),
            wayland_portal::probe_screen_cast_portal(),
            wayland_portal::gstreamer_pipewire_support(),
        )
    }

    fn runtime_report(&self) -> CaptureBackendRuntimeReport {
        CaptureBackendRuntimeReport {
            summary: Some(
                "Pure Wayland capture can use the ScreenCast portal and PipeWire through the GStreamer-native path."
                    .to_string(),
            ),
            preferred_target_label: Some("Full desktop".to_string()),
        }
    }

    fn start(&self, options: RecordingOptions) -> Result<Box<dyn CaptureController>, CaptureError> {
        Ok(Box::new(
            native_capture_backend::GstreamerWaylandCapture::start(options)?,
        ))
    }
}

impl CaptureBackendFactory for GstreamerX11LinuxBackend {
    fn descriptor(&self) -> CaptureBackendDescriptor {
        CaptureBackendDescriptor {
            id: "linux-gstreamer-x11-capture",
            label: "Linux GStreamer X11 recorder",
        }
    }

    fn availability(&self) -> CaptureBackendAvailability {
        x11_gstreamer_backend_availability_for(
            &current_desktop_session(),
            native_capture_backend::x11_gstreamer_support(),
        )
    }

    fn runtime_report(&self) -> CaptureBackendRuntimeReport {
        CaptureBackendRuntimeReport {
            summary: Some(
                "X11 and XWayland sessions can use the Linux native GStreamer ximagesrc path."
                    .to_string(),
            ),
            preferred_target_label: Some("Full desktop".to_string()),
        }
    }

    fn start(&self, options: RecordingOptions) -> Result<Box<dyn CaptureController>, CaptureError> {
        native_capture_backend::GstreamerX11Capture::start(options)
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
pub(crate) struct ResolvedTarget {
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

pub(crate) fn build_recording_artifact(
    output_path: &std::path::Path,
    started_at: SystemTime,
    finished_at: SystemTime,
) -> Result<RecordingArtifact, CaptureError> {
    let metadata = fs::metadata(output_path)
        .map_err(|error| CaptureError::OutputInspectionFailed(error.to_string()))?;
    let duration = finished_at.duration_since(started_at).unwrap_or_default();

    Ok(RecordingArtifact {
        output_path: output_path.to_path_buf(),
        started_at,
        finished_at,
        duration,
        bytes_written: metadata.len(),
    })
}

fn portal_backend_availability_for(
    session: &LinuxDesktopSession,
    portal_probe: wayland_portal::ScreenCastPortalProbe,
    gstreamer_support: wayland_portal::PipeWireGstreamerSupport,
) -> CaptureBackendAvailability {
    match session {
        LinuxDesktopSession::WaylandOnly { wayland_display } => match (portal_probe, gstreamer_support) {
            (
                wayland_portal::ScreenCastPortalProbe::Available(_),
                wayland_portal::PipeWireGstreamerSupport::Available,
            ) => CaptureBackendAvailability::Available,
            (
                wayland_portal::ScreenCastPortalProbe::Available(_),
                wayland_portal::PipeWireGstreamerSupport::Missing,
            ) => CaptureBackendAvailability::Unavailable {
                reason: format!(
                    "Wayland session {wayland_display} can reach the ScreenCast portal, but the required GStreamer PipeWire plugins are missing."
                ),
            },
            (
                wayland_portal::ScreenCastPortalProbe::Available(_),
                wayland_portal::PipeWireGstreamerSupport::Unknown,
            ) => CaptureBackendAvailability::Unavailable {
                reason: format!(
                    "Wayland session {wayland_display} can reach the ScreenCast portal, but GStreamer/PipeWire runtime support could not be confirmed."
                ),
            },
            (probe, _) => CaptureBackendAvailability::Unavailable {
                reason: match probe {
                    wayland_portal::ScreenCastPortalProbe::MissingPortal => format!(
                        "Wayland session {wayland_display} has no reachable ScreenCast portal."
                    ),
                    wayland_portal::ScreenCastPortalProbe::MissingDbusTools => format!(
                        "Wayland session {wayland_display} could not inspect portal readiness because neither gdbus nor busctl is available."
                    ),
                    wayland_portal::ScreenCastPortalProbe::Unreachable => format!(
                        "Wayland session {wayland_display} appears to have a portal installed, but the ScreenCast interface could not be reached on the session bus."
                    ),
                    wayland_portal::ScreenCastPortalProbe::Available(_) => {
                        "The Linux ScreenCast portal backend is unavailable.".to_string()
                    }
                },
            },
        },
        LinuxDesktopSession::WaylandWithX11 { .. } => {
            CaptureBackendAvailability::Unavailable {
                reason: "Wayland sessions with XWayland should use the native X11 GStreamer lane. The pure portal/PipeWire lane is reserved for Wayland-only sessions.".to_string(),
            }
        }
        LinuxDesktopSession::X11 { .. } => {
            CaptureBackendAvailability::Unavailable {
                reason: "The Linux ScreenCast portal / PipeWire backend is reserved for Wayland sessions. This session can use the native X11 recorder lane.".to_string(),
            }
        }
        LinuxDesktopSession::Headless => CaptureBackendAvailability::Unavailable {
            reason: session.capture_guidance(),
        },
    }
}

fn x11_gstreamer_backend_availability_for(
    session: &LinuxDesktopSession,
    support: native_capture_backend::X11GstreamerSupport,
) -> CaptureBackendAvailability {
    match session {
        LinuxDesktopSession::X11 { .. } | LinuxDesktopSession::WaylandWithX11 { .. } => match support {
            native_capture_backend::X11GstreamerSupport::Available => {
                CaptureBackendAvailability::Available
            }
            native_capture_backend::X11GstreamerSupport::Missing => {
                CaptureBackendAvailability::Unavailable {
                    reason: "X11 native capture needs `ximagesrc`, `mp4mux`, and at least one usable native H.264 GStreamer encoder."
                        .to_string(),
                }
            }
        },
        LinuxDesktopSession::WaylandOnly { .. } => CaptureBackendAvailability::Unavailable {
            reason: "Pure Wayland sessions do not use the X11 GStreamer lane.".to_string(),
        },
        LinuxDesktopSession::Headless => CaptureBackendAvailability::Unavailable {
            reason: session.capture_guidance(),
        },
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
        portal_parent_window: None,
        portal_restore_token: None,
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
            "Custom region capture is available through the XWayland-backed X11 recorder lane."
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
            "Pure Wayland recording currently uses the ScreenCast portal + GStreamer path, and system-audio mixing is not wired into that runtime yet."
                .to_string(),
        ),
        LinuxDesktopSession::Headless => (
            false,
            "System-audio mixing needs an active desktop session.".to_string(),
        ),
    }
}

pub(crate) fn resolve_target(options: &RecordingOptions) -> Result<ResolvedTarget, CaptureError> {
    resolve_target_with_monitors(options, &query_monitors().unwrap_or_default())
}

pub(crate) fn resolve_target_with_monitors(
    options: &RecordingOptions,
    monitors: &[MonitorDescriptor],
) -> Result<ResolvedTarget, CaptureError> {
    let target_id = options.capture_target_id.as_str();

    if target_id == FULL_DESKTOP_TARGET_ID {
        return Ok(resolve_full_desktop_target(monitors));
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
        label: monitor.label.clone(),
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

pub(crate) fn resolve_audio_input(audio_input_id: &str) -> Result<String, CaptureError> {
    if audio_input_id != DEFAULT_AUDIO_INPUT_ID {
        return Ok(audio_input_id.to_string());
    }

    resolve_audio_input_from_snapshot(audio_input_id, query_audio_inputs().ok().as_deref())
}

pub(crate) fn resolve_system_audio_input() -> Result<String, CaptureError> {
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
                    "Linux audio discovery could not find any usable microphone source".to_string(),
                )
            });
    }

    if let Some(audio_inputs) = audio_inputs {
        if let Some(resolved) = resolve_audio_input_id(audio_input_id, audio_inputs) {
            return Ok(resolved);
        }

        return Ok(audio_input_id.to_string());
    }

    Ok(audio_input_id.to_string())
}

fn query_audio_inputs() -> Result<Vec<AudioInputOption>, CaptureError> {
    let native_inputs = native_audio_backend::discovered_audio_inputs();
    if !native_inputs.is_empty() {
        return Ok(native_inputs);
    }

    Err(CaptureError::BackendUnavailable(
        "Linux could not discover any usable microphone or monitor sources through PipeWire/PulseAudio.".to_string(),
    ))
}

pub(crate) fn query_monitors() -> Result<Vec<MonitorDescriptor>, CaptureError> {
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

pub(crate) fn normalize_display(display: &str) -> String {
    if display.contains('.') {
        display.to_string()
    } else {
        format!("{display}.0")
    }
}

pub(crate) fn current_desktop_session() -> LinuxDesktopSession {
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
    pub(crate) fn capture_guidance(&self) -> String {
        match self {
            LinuxDesktopSession::WaylandOnly { wayland_display } => format!(
                "Wayland session {wayland_display} was detected without an X11 DISPLAY. The recorder now tries a native ScreenCast portal plus GStreamer PipeWire path, but it still depends on portal approval, PipeWire, and the required GStreamer plugins being available."
            ),
            LinuxDesktopSession::WaylandWithX11 {
                wayland_display,
                x11_display,
            } => format!(
                "Wayland session {wayland_display} is active and also exposes XWayland display {x11_display}. The recorder prefers the native ScreenCast portal plus GStreamer PipeWire path, but it can still fall back to the native X11 GStreamer lane through XWayland."
            ),
            LinuxDesktopSession::Headless => {
                "No desktop display was detected. The Linux backend needs either a Wayland ScreenCast portal session or an X11/XWayland DISPLAY to start recording.".to_string()
            }
            LinuxDesktopSession::X11 { .. } => {
                "Linux capture could not resolve the current X11 display.".to_string()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{env, path::PathBuf};

    use capture::{DEFAULT_AUDIO_INPUT_ID, FULL_DESKTOP_TARGET_ID, RecordingOptions};

    use super::{
        LinuxDesktopSession, classify_desktop_session, gst_bitrate_for_quality,
        native_audio_backend, native_capture_backend, native_encoder_backend, parse_monitors,
        parse_windows, portal_backend_availability_for, x11_gstreamer_backend_availability_for,
    };
    use crate::wayland_portal::{
        PipeWireGstreamerSupport, ScreenCastPortalCapabilities, ScreenCastPortalProbe,
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
            portal_parent_window: None,
            portal_restore_token: None,
            region_x: 160,
            region_y: 120,
            region_width: 1280,
            region_height: 720,
            region_source_capture_target_id: FULL_DESKTOP_TARGET_ID.to_string(),
            region_source_origin_x: 0,
            region_source_origin_y: 0,
            region_source_scale_factor_milli: 1000,
        };

        let plan = native_capture_backend::WaylandPortalRuntimePlan {
            target_label: "Wayland ScreenCast selection".to_string(),
            encoder_label: "gstreamer / pipewiresrc · x264".to_string(),
            encoder_plan: native_encoder_backend::GstreamerEncoderPlan {
                element_name: "x264enc",
                label: "x264".to_string(),
                property_args: vec![
                    "speed-preset=veryfast".to_string(),
                    "tune=zerolatency".to_string(),
                    "bitrate=8000".to_string(),
                    "key-int-max=30".to_string(),
                ],
            },
            stream_node_id: 77,
            stream_target_id: Some("0".to_string()),
            width: 1920,
            height: 1080,
            fps: 30,
            microphone_device: None,
        };
        let args = native_capture_backend::build_wayland_gstreamer_args(&options, &plan)
            .expect("wayland args should build");
        let joined = args.join(" ");

        assert!(joined.contains("pipewiresrc fd=3 path=77 autoconnect=true"));
        assert!(joined.contains("always-copy=true"));
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
            portal_parent_window: None,
            portal_restore_token: None,
            region_x: 160,
            region_y: 120,
            region_width: 1280,
            region_height: 720,
            region_source_capture_target_id: FULL_DESKTOP_TARGET_ID.to_string(),
            region_source_origin_x: 0,
            region_source_origin_y: 0,
            region_source_scale_factor_milli: 1000,
        };

        let plan = native_capture_backend::WaylandPortalRuntimePlan {
            target_label: "Wayland ScreenCast selection".to_string(),
            encoder_label: "gstreamer / pipewiresrc · x264".to_string(),
            encoder_plan: native_encoder_backend::GstreamerEncoderPlan {
                element_name: "x264enc",
                label: "x264".to_string(),
                property_args: vec![
                    "speed-preset=superfast".to_string(),
                    "tune=zerolatency".to_string(),
                    "bitrate=12000".to_string(),
                    "key-int-max=60".to_string(),
                ],
            },
            stream_node_id: 9,
            stream_target_id: Some("0".to_string()),
            width: 1920,
            height: 1080,
            fps: 60,
            microphone_device: Some("alsa_input.usb-Blue_Yeti".to_string()),
        };
        let args = native_capture_backend::build_wayland_gstreamer_args(&options, &plan)
            .expect("wayland args should build");
        let joined = args.join(" ");

        assert!(joined.contains("autoconnect=true"));
        assert!(joined.contains("path=9"));
        assert!(joined.contains("always-copy=true"));
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

    #[test]
    fn native_portal_backend_is_available_for_pure_wayland_when_requirements_exist() {
        let availability = portal_backend_availability_for(
            &LinuxDesktopSession::WaylandOnly {
                wayland_display: "wayland-0".to_string(),
            },
            ScreenCastPortalProbe::Available(ScreenCastPortalCapabilities {
                available_source_types: 1,
                available_cursor_modes: 2,
            }),
            PipeWireGstreamerSupport::Available,
        );

        assert!(matches!(
            availability,
            capture::CaptureBackendAvailability::Available
        ));
    }

    #[test]
    fn native_portal_backend_is_unavailable_for_xwayland_sessions() {
        let availability = portal_backend_availability_for(
            &LinuxDesktopSession::WaylandWithX11 {
                wayland_display: "wayland-0".to_string(),
                x11_display: ":1".to_string(),
            },
            ScreenCastPortalProbe::Available(ScreenCastPortalCapabilities {
                available_source_types: 1,
                available_cursor_modes: 2,
            }),
            PipeWireGstreamerSupport::Available,
        );

        assert!(matches!(
            availability,
            capture::CaptureBackendAvailability::Unavailable { .. }
        ));
    }

    #[test]
    fn x11_gstreamer_backend_is_available_for_xwayland_sessions_when_requirements_exist() {
        let availability = x11_gstreamer_backend_availability_for(
            &LinuxDesktopSession::WaylandWithX11 {
                wayland_display: "wayland-0".to_string(),
                x11_display: ":1".to_string(),
            },
            native_capture_backend::X11GstreamerSupport::Available,
        );

        assert!(matches!(
            availability,
            capture::CaptureBackendAvailability::Available
        ));
    }

    #[test]
    fn pure_wayland_selection_prefers_native_audio_and_encoder_backends() {
        assert!(matches!(
            native_audio_backend::availability_for(
                &LinuxDesktopSession::WaylandOnly {
                    wayland_display: "wayland-0".to_string(),
                },
                true
            ),
            capture::AudioBackendAvailability::Available
        ));
        assert!(matches!(
            native_encoder_backend::availability_for(
                &LinuxDesktopSession::WaylandOnly {
                    wayland_display: "wayland-0".to_string(),
                },
                PipeWireGstreamerSupport::Available
            ),
            capture::EncoderBackendAvailability::Available
        ));
    }
}
