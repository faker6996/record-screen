use std::{
    io::Read,
    os::{fd::AsRawFd, unix::process::CommandExt, unix::process::ExitStatusExt},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::SystemTime,
};

use capture::{
    ActiveRecording, CaptureController, CaptureError, FULL_DESKTOP_TARGET_ID, RecordingArtifact,
    RecordingOptions,
};

use crate::native_encoder_backend::{self, GstreamerEncoderPlan};
use crate::{
    LinuxCaptureProcessKind, LinuxDesktopSession, build_recording_artifact,
    current_desktop_session, describe_process_failure, gst_audio_input_device, normalize_display,
    quality_settings, query_monitors, read_stderr_buffer, request_process_stop,
    resolve_audio_input, resolve_system_audio_input, resolve_target_with_monitors,
    verify_process_started, wayland_portal,
};

pub(crate) struct GstreamerWaylandCapture {
    active_recording: ActiveRecording,
    child: Child,
    stderr_buffer: Arc<Mutex<String>>,
    finished_artifact: Option<RecordingArtifact>,
    paused: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum X11GstreamerSupport {
    Available,
    Missing,
}

pub(crate) struct GstreamerX11Capture {
    active_recording: ActiveRecording,
    child: Child,
    stderr_buffer: Arc<Mutex<String>>,
    finished_artifact: Option<RecordingArtifact>,
    paused: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WaylandPortalRuntimePlan {
    pub target_label: String,
    pub encoder_label: String,
    pub encoder_plan: GstreamerEncoderPlan,
    pub stream_node_id: u32,
    pub stream_target_id: Option<String>,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub microphone_device: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct X11GstreamerRuntimePlan {
    pub target_label: String,
    pub encoder_label: String,
    pub encoder_plan: GstreamerEncoderPlan,
    pub display_name: String,
    pub origin_x: i32,
    pub origin_y: i32,
    pub source_width: Option<u32>,
    pub source_height: Option<u32>,
    pub output_width: u32,
    pub output_height: u32,
    pub fps: u32,
    pub microphone_device: Option<String>,
    pub system_audio_device: Option<String>,
}

impl GstreamerWaylandCapture {
    pub(crate) fn start(options: RecordingOptions) -> Result<Self, CaptureError> {
        let session = current_desktop_session();
        let wayland_display = match &session {
            LinuxDesktopSession::WaylandOnly { wayland_display }
            | LinuxDesktopSession::WaylandWithX11 {
                wayland_display, ..
            } => wayland_display.clone(),
            _ => return Err(CaptureError::BackendUnavailable(
                "The Linux ScreenCast portal / PipeWire backend is only used in Wayland sessions."
                    .to_string(),
            )),
        };

        if matches!(
            session,
            LinuxDesktopSession::X11 { .. } | LinuxDesktopSession::Headless
        ) {
            return Err(CaptureError::BackendUnavailable(
                "The Linux ScreenCast portal / PipeWire backend is only used in Wayland sessions."
                    .to_string(),
            ));
        }

        if options.capture_target_id != FULL_DESKTOP_TARGET_ID {
            return Err(CaptureError::BackendUnavailable(format!(
                "Wayland session {wayland_display} currently records through the ScreenCast portal chooser. Window and monitor targeting from the launcher is not wired into the pure Wayland path yet."
            )));
        }
        if options.system_audio_enabled {
            return Err(CaptureError::BackendUnavailable(format!(
                "Wayland session {wayland_display} currently uses the ScreenCast portal + GStreamer path, and system-audio mixing is not wired into that runtime yet."
            )));
        }

        let runtime_session =
            wayland_portal::negotiate_runtime_session(
                options.portal_parent_window.as_deref(),
                options.portal_restore_token.as_deref(),
            )
            .map_err(|error| {
            CaptureError::BackendUnavailable(format!(
                "Wayland session {wayland_display} could reach the ScreenCast portal path, but session negotiation did not complete: {error}"
            ))
        })?;
        let plan = build_wayland_runtime_plan(&options, &runtime_session)?;
        let started_at = SystemTime::now();
        let stderr_buffer = Arc::new(Mutex::new(String::new()));
        let child = spawn_wayland_gstreamer(
            &options,
            &runtime_session,
            &plan,
            Arc::clone(&stderr_buffer),
        )?;

        Ok(Self {
            active_recording: ActiveRecording {
                backend_name: "Linux ScreenCast portal / PipeWire".to_string(),
                encoder_label: plan.encoder_label.clone(),
                output_path: options.output_path,
                started_at,
                target_label: plan.target_label,
            },
            child,
            stderr_buffer,
            finished_artifact: None,
            paused: false,
        })
    }

    fn build_artifact(&self, finished_at: SystemTime) -> Result<RecordingArtifact, CaptureError> {
        build_recording_artifact(
            &self.active_recording.output_path,
            self.active_recording.started_at,
            finished_at,
        )
    }

    fn ensure_nonempty_artifact(
        &self,
        artifact: RecordingArtifact,
    ) -> Result<RecordingArtifact, CaptureError> {
        if artifact.bytes_written > 0 {
            return Ok(artifact);
        }

        let stderr_log = read_stderr_buffer(&self.stderr_buffer);
        Err(CaptureError::StopFailed(format!(
            "the Wayland GStreamer pipeline exited without writing media data. {}",
            describe_process_failure(LinuxCaptureProcessKind::GstreamerWayland, &stderr_log)
        )))
    }
}

impl GstreamerX11Capture {
    pub(crate) fn start(
        options: RecordingOptions,
    ) -> Result<Box<dyn CaptureController>, CaptureError> {
        match current_desktop_session() {
            LinuxDesktopSession::X11 { .. } | LinuxDesktopSession::WaylandWithX11 { .. } => {}
            _ => {
                return Err(CaptureError::BackendUnavailable(
                    "The Linux GStreamer X11 lane is only used in X11/XWayland sessions."
                        .to_string(),
                ));
            }
        }

        let plan = build_x11_runtime_plan(&options)?;

        let started_at = SystemTime::now();
        let stderr_buffer = Arc::new(Mutex::new(String::new()));
        let child = spawn_x11_gstreamer(&options, &plan, Arc::clone(&stderr_buffer))?;

        Ok(Box::new(Self {
            active_recording: ActiveRecording {
                backend_name: "Linux GStreamer / ximagesrc".to_string(),
                encoder_label: plan.encoder_label.clone(),
                output_path: options.output_path,
                started_at,
                target_label: plan.target_label,
            },
            child,
            stderr_buffer,
            finished_artifact: None,
            paused: false,
        }))
    }

    fn build_artifact(&self, finished_at: SystemTime) -> Result<RecordingArtifact, CaptureError> {
        build_recording_artifact(
            &self.active_recording.output_path,
            self.active_recording.started_at,
            finished_at,
        )
    }

    fn ensure_nonempty_artifact(
        &self,
        artifact: RecordingArtifact,
    ) -> Result<RecordingArtifact, CaptureError> {
        if artifact.bytes_written > 0 {
            return Ok(artifact);
        }

        let stderr_log = read_stderr_buffer(&self.stderr_buffer);
        Err(CaptureError::StopFailed(format!(
            "the X11 GStreamer pipeline exited without writing media data. {}",
            describe_process_failure(LinuxCaptureProcessKind::GstreamerX11, &stderr_log)
        )))
    }
}

pub(crate) fn x11_gstreamer_support() -> X11GstreamerSupport {
    if gst_element_available("ximagesrc")
        && gst_element_available("mp4mux")
        && native_encoder_backend::encoder_plan_for_quality("1080p / 30 fps").is_some()
    {
        X11GstreamerSupport::Available
    } else {
        X11GstreamerSupport::Missing
    }
}

impl CaptureController for GstreamerWaylandCapture {
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

        request_process_stop(LinuxCaptureProcessKind::GstreamerX11, self.child.id(), None)?;

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
                    LinuxCaptureProcessKind::GstreamerX11,
                    &read_stderr_buffer(&self.stderr_buffer)
                )
            )));
        }

        let artifact = self.ensure_nonempty_artifact(self.build_artifact(SystemTime::now())?)?;
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
                LinuxCaptureProcessKind::GstreamerX11,
                &read_stderr_buffer(&self.stderr_buffer),
            )));
        }

        let artifact = self.ensure_nonempty_artifact(self.build_artifact(SystemTime::now())?)?;
        self.finished_artifact = Some(artifact.clone());
        Ok(Some(artifact))
    }
}

impl CaptureController for GstreamerX11Capture {
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

        request_process_stop(
            LinuxCaptureProcessKind::GstreamerX11,
            self.child.id(),
            None,
        )?;

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
                    LinuxCaptureProcessKind::GstreamerX11,
                    &read_stderr_buffer(&self.stderr_buffer)
                )
            )));
        }

        let artifact = self.ensure_nonempty_artifact(self.build_artifact(SystemTime::now())?)?;
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
                LinuxCaptureProcessKind::GstreamerX11,
                &read_stderr_buffer(&self.stderr_buffer),
            )));
        }

        let artifact = self.ensure_nonempty_artifact(self.build_artifact(SystemTime::now())?)?;
        self.finished_artifact = Some(artifact.clone());
        Ok(Some(artifact))
    }
}

pub(crate) fn build_wayland_runtime_plan(
    options: &RecordingOptions,
    runtime_session: &wayland_portal::ScreenCastPortalRuntimeSession,
) -> Result<WaylandPortalRuntimePlan, CaptureError> {
    let stream_node_id = runtime_session
        .stream_node_ids
        .first()
        .copied()
        .ok_or_else(|| {
            CaptureError::BackendUnavailable(
                "the ScreenCast portal did not return any PipeWire stream node IDs".to_string(),
            )
        })?;
    let (width, height, fps) = quality_settings(&options.quality_preset);
    let encoder_plan = native_encoder_backend::encoder_plan_for_quality(&options.quality_preset)
        .ok_or_else(|| {
            CaptureError::BackendUnavailable(
                "the Linux native GStreamer lane could not resolve a usable H.264 encoder"
                    .to_string(),
            )
        })?;
    let stream_target_id = runtime_session.stream_target_ids.first().cloned();
    let microphone_device = if options.mic_enabled {
        gst_audio_input_device(&options.audio_input_id)?
    } else {
        None
    };

    Ok(WaylandPortalRuntimePlan {
        target_label: "Wayland ScreenCast selection".to_string(),
        encoder_label: format!("gstreamer / pipewiresrc · {}", encoder_plan.label),
        encoder_plan,
        stream_node_id,
        stream_target_id,
        width,
        height,
        fps,
        microphone_device,
    })
}

pub(crate) fn build_x11_runtime_plan(
    options: &RecordingOptions,
) -> Result<X11GstreamerRuntimePlan, CaptureError> {
    let session = current_desktop_session();
    let display_name = match &session {
        LinuxDesktopSession::X11 { display } => normalize_display(display),
        LinuxDesktopSession::WaylandWithX11 { x11_display, .. } => normalize_display(x11_display),
        _ => {
            return Err(CaptureError::BackendUnavailable(
                "The Linux GStreamer X11 lane is only used in X11/XWayland sessions.".to_string(),
            ));
        }
    };

    let monitors = query_monitors().unwrap_or_default();
    let target = resolve_target_with_monitors(options, &monitors)?;
    let desktop_origin_x = monitors.iter().map(|monitor| monitor.x).min().unwrap_or(0);
    let desktop_origin_y = monitors.iter().map(|monitor| monitor.y).min().unwrap_or(0);

    let (output_width, output_height, fps) = quality_settings(&options.quality_preset);
    let encoder_plan = native_encoder_backend::encoder_plan_for_quality(&options.quality_preset)
        .ok_or_else(|| {
            CaptureError::BackendUnavailable(
                "the Linux native GStreamer lane could not resolve a usable H.264 encoder"
                    .to_string(),
            )
        })?;
    let microphone_device = if options.mic_enabled {
        Some(resolve_audio_input(&options.audio_input_id)?)
    } else {
        None
    };
    let system_audio_device = if options.system_audio_enabled {
        Some(resolve_system_audio_input()?)
    } else {
        None
    };

    Ok(X11GstreamerRuntimePlan {
        target_label: target.label,
        encoder_label: format!("gstreamer / ximagesrc · {}", encoder_plan.label),
        encoder_plan,
        display_name,
        origin_x: target.origin_x - desktop_origin_x,
        origin_y: target.origin_y - desktop_origin_y,
        source_width: target.video_size.map(|(width, _)| width),
        source_height: target.video_size.map(|(_, height)| height),
        output_width,
        output_height,
        fps,
        microphone_device,
        system_audio_device,
    })
}

fn spawn_wayland_gstreamer(
    options: &RecordingOptions,
    runtime_session: &wayland_portal::ScreenCastPortalRuntimeSession,
    plan: &WaylandPortalRuntimePlan,
    stderr_buffer: Arc<Mutex<String>>,
) -> Result<Child, CaptureError> {
    let remote_fd = runtime_session.pipewire_remote_fd.as_raw_fd();
    let args = build_wayland_gstreamer_args(options, plan)?;

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

    Ok(child)
}

fn spawn_x11_gstreamer(
    options: &RecordingOptions,
    plan: &X11GstreamerRuntimePlan,
    stderr_buffer: Arc<Mutex<String>>,
) -> Result<Child, CaptureError> {
    let args = build_x11_gstreamer_args(options, plan)?;
    let mut command = Command::new("gst-launch-1.0");
    command
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

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
        LinuxCaptureProcessKind::GstreamerX11,
    )?;

    Ok(child)
}

pub(crate) fn build_wayland_gstreamer_args(
    options: &RecordingOptions,
    plan: &WaylandPortalRuntimePlan,
) -> Result<Vec<String>, CaptureError> {
    let output_location = options.output_path.display().to_string();
    let mut args = vec![
        "-e".to_string(),
        "pipewiresrc".to_string(),
        "fd=3".to_string(),
        format!("path={}", plan.stream_node_id),
        "autoconnect=true".to_string(),
        "always-copy=true".to_string(),
        "do-timestamp=true".to_string(),
        "keepalive-time=1000".to_string(),
    ];

    args.extend([
        "!".to_string(),
        "queue".to_string(),
        "!".to_string(),
        "videoconvert".to_string(),
        "!".to_string(),
        "videoscale".to_string(),
        "!".to_string(),
        "videorate".to_string(),
        "!".to_string(),
        format!(
            "video/x-raw,width={},height={},framerate={}/1",
            plan.width, plan.height, plan.fps
        ),
        "!".to_string(),
        plan.encoder_plan.element_name.to_string(),
    ]);
    args.extend(plan.encoder_plan.property_args.iter().cloned());
    args.extend([
        "!".to_string(),
        "h264parse".to_string(),
        "config-interval=-1".to_string(),
        "!".to_string(),
        "queue".to_string(),
        "!".to_string(),
        "mux.video_0".to_string(),
    ]);

    if options.mic_enabled {
        args.push("pulsesrc".to_string());
        args.push("do-timestamp=true".to_string());

        if let Some(audio_device) = &plan.microphone_device {
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

pub(crate) fn build_x11_gstreamer_args(
    options: &RecordingOptions,
    plan: &X11GstreamerRuntimePlan,
) -> Result<Vec<String>, CaptureError> {
    let output_location = options.output_path.display().to_string();
    let mut args = vec![
        "-e".to_string(),
        "ximagesrc".to_string(),
        format!("display-name={}", plan.display_name),
        "show-pointer=true".to_string(),
        "use-damage=false".to_string(),
    ];

    if let (Some(source_width), Some(source_height)) = (plan.source_width, plan.source_height) {
        let end_x = plan.origin_x + source_width as i32 - 1;
        let end_y = plan.origin_y + source_height as i32 - 1;
        args.push(format!("startx={}", plan.origin_x));
        args.push(format!("starty={}", plan.origin_y));
        args.push(format!("endx={end_x}"));
        args.push(format!("endy={end_y}"));
    }

    args.extend([
        "!".to_string(),
        "queue".to_string(),
        "!".to_string(),
        "videoconvert".to_string(),
        "!".to_string(),
        "videoscale".to_string(),
        "!".to_string(),
        "videorate".to_string(),
        "!".to_string(),
        format!(
            "video/x-raw,width={},height={},framerate={}/1",
            plan.output_width, plan.output_height, plan.fps
        ),
        "!".to_string(),
        plan.encoder_plan.element_name.to_string(),
    ]);
    args.extend(plan.encoder_plan.property_args.iter().cloned());
    args.extend([
        "!".to_string(),
        "h264parse".to_string(),
        "config-interval=-1".to_string(),
        "!".to_string(),
        "queue".to_string(),
        "!".to_string(),
        "mux.video_0".to_string(),
    ]);

    append_x11_audio_args(&mut args, plan);

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

fn append_x11_audio_args(args: &mut Vec<String>, plan: &X11GstreamerRuntimePlan) {
    match (
        plan.microphone_device.as_deref(),
        plan.system_audio_device.as_deref(),
    ) {
        (None, None) => {}
        (Some(device), None) | (None, Some(device)) => {
            append_pulsesrc_branch(args, Some(device));
            args.extend([
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
        (Some(microphone), Some(system)) => {
            append_pulsesrc_branch(args, Some(microphone));
            args.extend([
                "!".to_string(),
                "audioconvert".to_string(),
                "!".to_string(),
                "audioresample".to_string(),
                "!".to_string(),
                "amix.".to_string(),
            ]);
            append_pulsesrc_branch(args, Some(system));
            args.extend([
                "!".to_string(),
                "audioconvert".to_string(),
                "!".to_string(),
                "audioresample".to_string(),
                "!".to_string(),
                "amix.".to_string(),
                "audiomixer".to_string(),
                "name=amix".to_string(),
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
    }
}

fn append_pulsesrc_branch(args: &mut Vec<String>, device: Option<&str>) {
    args.push("pulsesrc".to_string());
    args.push("do-timestamp=true".to_string());
    if let Some(device) = device {
        args.push(format!("device={device}"));
    }
    args.push("!".to_string());
    args.push("queue".to_string());
}

fn gst_element_available(element: &str) -> bool {
    Command::new("gst-inspect-1.0")
        .arg(element)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::{X11GstreamerRuntimePlan, build_x11_gstreamer_args};
    use crate::native_encoder_backend::GstreamerEncoderPlan;
    use capture::{FULL_DESKTOP_TARGET_ID, RecordingOptions};
    use std::path::PathBuf;

    #[test]
    fn builds_x11_gstreamer_args_with_audio_mix() {
        let options = RecordingOptions {
            output_path: PathBuf::from("/tmp/x11-gst.mp4"),
            quality_preset: "1080p / 60 fps".to_string(),
            mic_enabled: true,
            system_audio_enabled: true,
            capture_target_id: FULL_DESKTOP_TARGET_ID.to_string(),
            audio_input_id: "alsa_input.usb-Blue_Yeti".to_string(),
            portal_parent_window: None,
            portal_restore_token: None,
            region_x: 0,
            region_y: 0,
            region_width: 0,
            region_height: 0,
            region_source_capture_target_id: FULL_DESKTOP_TARGET_ID.to_string(),
            region_source_origin_x: 0,
            region_source_origin_y: 0,
            region_source_scale_factor_milli: 1000,
        };

        let plan = X11GstreamerRuntimePlan {
            target_label: "Full desktop".to_string(),
            encoder_label: "gstreamer / ximagesrc · x264".to_string(),
            encoder_plan: GstreamerEncoderPlan {
                element_name: "x264enc",
                label: "x264".to_string(),
                property_args: vec![
                    "speed-preset=superfast".to_string(),
                    "tune=zerolatency".to_string(),
                    "bitrate=12000".to_string(),
                    "key-int-max=60".to_string(),
                ],
            },
            display_name: ":1.0".to_string(),
            origin_x: 0,
            origin_y: 0,
            source_width: Some(1920),
            source_height: Some(1080),
            output_width: 1920,
            output_height: 1080,
            fps: 60,
            microphone_device: Some("alsa_input.usb-Blue_Yeti".to_string()),
            system_audio_device: Some("alsa_output.pci.monitor".to_string()),
        };

        let joined = build_x11_gstreamer_args(&options, &plan)
            .expect("x11 gst args should build")
            .join(" ");

        assert!(joined.contains("ximagesrc"));
        assert!(joined.contains("display-name=:1.0"));
        assert!(joined.contains("startx=0"));
        assert!(joined.contains("endx=1919"));
        assert!(joined.contains("framerate=60/1"));
        assert!(joined.contains("x264enc speed-preset=superfast"));
        assert!(joined.contains("pulsesrc do-timestamp=true device=alsa_input.usb-Blue_Yeti"));
        assert!(joined.contains("pulsesrc do-timestamp=true device=alsa_output.pci.monitor"));
        assert!(joined.contains("audiomixer name=amix"));
    }
}
