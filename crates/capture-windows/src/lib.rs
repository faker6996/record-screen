#[cfg(target_os = "windows")]
pub mod native_audio_backend;
#[cfg(target_os = "windows")]
pub mod native_encoder_backend;

#[cfg(target_os = "windows")]
mod platform {
    use std::{
        fs,
        io::{Read, Write},
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
        encoder_backend_statuses as shared_encoder_backend_statuses,
        explain_audio_backend_selection, explain_capture_backend_selection,
        explain_encoder_backend_selection, full_desktop_target, preferred_system_audio_input,
        resolve_audio_input_id, select_audio_backend, select_backend, select_encoder_backend,
    };
    use serde::Deserialize;

    const MONITOR_TARGET_PREFIX: &str = "monitor:";
    const WINDOW_TARGET_PREFIX: &str = "window:";
    const STARTUP_POLL_INTERVAL: Duration = Duration::from_millis(50);
    const STARTUP_POLL_ATTEMPTS: usize = 6;

    #[derive(Clone, Copy)]
    struct VideoEncoderProfile {
        codec: &'static str,
        preset: Option<&'static str>,
    }

    pub struct FfmpegWindowsCapture {
        active_recording: ActiveRecording,
        child: Child,
        stdin: Option<ChildStdin>,
        stderr_buffer: Arc<Mutex<String>>,
        finished_artifact: Option<RecordingArtifact>,
        paused: bool,
    }

    pub struct FfmpegWindowsBackend;
    pub struct WindowsGraphicsCaptureBackend;
    pub struct FfmpegWindowsAudioBackend;
    pub struct FfmpegWindowsEncoderBackend;
    static FFMPEG_WINDOWS_BACKEND: FfmpegWindowsBackend = FfmpegWindowsBackend;
    static WINDOWS_GRAPHICS_CAPTURE_BACKEND: WindowsGraphicsCaptureBackend =
        WindowsGraphicsCaptureBackend;
    static FFMPEG_WINDOWS_AUDIO_BACKEND: FfmpegWindowsAudioBackend = FfmpegWindowsAudioBackend;
    static FFMPEG_WINDOWS_ENCODER_BACKEND: FfmpegWindowsEncoderBackend =
        FfmpegWindowsEncoderBackend;

    pub fn selected_backend() -> &'static dyn CaptureBackendFactory {
        select_backend(&backend_candidates())
    }

    fn backend_candidates() -> [&'static dyn CaptureBackendFactory; 2] {
        [&WINDOWS_GRAPHICS_CAPTURE_BACKEND, &FFMPEG_WINDOWS_BACKEND]
    }

    pub fn backend_statuses() -> Vec<CaptureBackendStatus> {
        shared_backend_statuses(&backend_candidates())
    }

    pub fn selected_audio_backend() -> &'static dyn AudioBackendFactory {
        select_audio_backend(&audio_backend_candidates())
    }

    fn audio_backend_candidates() -> [&'static dyn AudioBackendFactory; 2] {
        [
            super::native_audio_backend::backend(),
            &FFMPEG_WINDOWS_AUDIO_BACKEND,
        ]
    }

    pub fn audio_backend_statuses() -> Vec<AudioBackendStatus> {
        shared_audio_backend_statuses(&audio_backend_candidates())
    }

    pub fn selected_encoder_backend() -> &'static dyn EncoderBackendFactory {
        select_encoder_backend(&encoder_backend_candidates())
    }

    fn encoder_backend_candidates() -> [&'static dyn EncoderBackendFactory; 2] {
        [
            super::native_encoder_backend::backend(),
            &FFMPEG_WINDOWS_ENCODER_BACKEND,
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

    impl CaptureBackendFactory for WindowsGraphicsCaptureBackend {
        fn descriptor(&self) -> CaptureBackendDescriptor {
            CaptureBackendDescriptor {
                id: "windows-graphics-capture",
                label: "Windows Graphics Capture",
                family: CaptureBackendFamily::Native,
            }
        }

        fn availability(&self) -> CaptureBackendAvailability {
            CaptureBackendAvailability::Unavailable {
                reason: "Windows.Graphics.Capture is planned for Phase 1, but the native frame pipeline is not implemented yet.".to_string(),
            }
        }

        fn runtime_report(&self) -> CaptureBackendRuntimeReport {
            CaptureBackendRuntimeReport {
                summary: Some(
                    "Windows native capture candidate targets Windows.Graphics.Capture, but the frame pipeline is not wired into the recorder runtime yet."
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
                "Windows.Graphics.Capture backend is not implemented yet.".to_string(),
            ))
        }
    }

    impl CaptureBackendFactory for FfmpegWindowsBackend {
        fn descriptor(&self) -> CaptureBackendDescriptor {
            CaptureBackendDescriptor {
                id: "windows-ffmpeg-gdigrab",
                label: "Windows ffmpeg / gdigrab",
                family: CaptureBackendFamily::FallbackFfmpeg,
            }
        }

        fn availability(&self) -> CaptureBackendAvailability {
            CaptureBackendAvailability::Available
        }

        fn runtime_report(&self) -> CaptureBackendRuntimeReport {
            CaptureBackendRuntimeReport {
                summary: Some(
                    "Current Windows capture runtime uses ffmpeg with gdigrab window/desktop sources."
                        .to_string(),
                ),
                preferred_target_label: Some("Full desktop".to_string()),
            }
        }

        fn start(
            &self,
            options: RecordingOptions,
        ) -> Result<Box<dyn CaptureController>, CaptureError> {
            Ok(Box::new(FfmpegWindowsCapture::start(options)?))
        }
    }

    impl AudioBackendFactory for FfmpegWindowsAudioBackend {
        fn descriptor(&self) -> AudioBackendDescriptor {
            AudioBackendDescriptor {
                id: "windows-ffmpeg-dshow-audio",
                label: "Windows ffmpeg / DirectShow audio",
                family: AudioBackendFamily::FallbackFfmpeg,
            }
        }

        fn availability(&self) -> AudioBackendAvailability {
            AudioBackendAvailability::Available
        }

        fn runtime_report(&self) -> AudioBackendRuntimeReport {
            let preferred_input = discover_audio_inputs()
                .ok()
                .and_then(|inputs| {
                    capture::preferred_audio_input(&inputs).map(|input| input.label.clone())
                })
                .or_else(native_preferred_input_label);
            let preferred_system = discover_audio_inputs()
                .ok()
                .and_then(|inputs| {
                    preferred_system_audio_input(&inputs).map(|input| input.label.clone())
                })
                .or_else(native_preferred_render_label);

            AudioBackendRuntimeReport {
                summary: Some(audio_input_support_summary()),
                preferred_input_id: super::native_audio_backend::preferred_capture_endpoint_id(),
                preferred_input_label: preferred_input,
                preferred_system_id: super::native_audio_backend::preferred_render_endpoint_id(),
                preferred_system_label: preferred_system,
            }
        }
    }

    impl EncoderBackendFactory for FfmpegWindowsEncoderBackend {
        fn descriptor(&self) -> EncoderBackendDescriptor {
            EncoderBackendDescriptor {
                id: "windows-ffmpeg-encoder",
                label: "Windows ffmpeg encoder",
                family: EncoderBackendFamily::FallbackFfmpeg,
            }
        }

        fn availability(&self) -> EncoderBackendAvailability {
            EncoderBackendAvailability::Available
        }

        fn runtime_report(&self) -> EncoderBackendRuntimeReport {
            EncoderBackendRuntimeReport {
                summary: Some(format!(
                    "Current Windows output pipeline uses ffmpeg with preferred encoder `{}`.",
                    encoder_label(&preferred_video_encoder())
                )),
                preferred_encoder_label: Some(encoder_label(&preferred_video_encoder())),
            }
        }
    }

    #[derive(Debug, Clone, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct MonitorDescriptor {
        device_name: String,
        label: String,
        width: u32,
        height: u32,
        x: i32,
        y: i32,
    }

    #[derive(Debug, Clone, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct WindowDescriptor {
        id: i64,
        title: String,
        process_name: String,
        width: u32,
        height: u32,
        x: i32,
        y: i32,
    }

    #[derive(Debug, Clone)]
    struct ResolvedTarget {
        label: String,
        source: String,
        offset_x: Option<i32>,
        offset_y: Option<i32>,
        video_size: Option<(u32, u32)>,
    }

    impl FfmpegWindowsCapture {
        pub fn start(options: RecordingOptions) -> Result<Self, CaptureError> {
            let started_at = SystemTime::now();
            let stderr_buffer = Arc::new(Mutex::new(String::new()));
            let target = resolve_target(&options)?;
            let encoder = encoder_for_quality(&options.quality_preset);
            let (child, stdin) = spawn_ffmpeg(&options, &target, Arc::clone(&stderr_buffer))?;

            Ok(Self {
                active_recording: ActiveRecording {
                    backend_name: "Windows ffmpeg / gdigrab".to_string(),
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

        fn build_artifact(
            &self,
            finished_at: SystemTime,
        ) -> Result<RecordingArtifact, CaptureError> {
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

    impl CaptureController for FfmpegWindowsCapture {
        fn active_recording(&self) -> &ActiveRecording {
            &self.active_recording
        }

        fn pause(&mut self) -> Result<(), CaptureError> {
            if self.paused {
                return Ok(());
            }

            run_powershell(&format!("Suspend-Process -Id {}", self.child.id()))
                .map_err(CaptureError::SignalFailed)?;
            self.paused = true;
            Ok(())
        }

        fn resume(&mut self) -> Result<(), CaptureError> {
            if !self.paused {
                return Ok(());
            }

            run_powershell(&format!("Resume-Process -Id {}", self.child.id()))
                .map_err(CaptureError::SignalFailed)?;
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

            if !status.success() {
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

            if !status.success() {
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
                id: format!("{MONITOR_TARGET_PREFIX}{}", monitor.device_name),
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
                    "{} · {} x {}",
                    window.process_name, window.width, window.height
                ),
            }));
        }

        targets
    }

    pub fn list_audio_inputs() -> Vec<AudioInputOption> {
        let mut default_input = default_audio_input();
        if let Some(route_plan) = super::native_audio_backend::route_plan() {
            default_input.description = route_plan.default_input_note;
        }

        match discover_audio_inputs() {
            Ok(discovered_inputs) => {
                let mut inputs = vec![default_input];
                inputs.extend(discovered_inputs);
                inputs
            }
            Err(error) => {
                default_input.description = match super::native_audio_backend::route_plan() {
                    Some(route_plan) => format!(
                        "Windows could not enumerate DirectShow microphone devices. {} {error}",
                        route_plan.default_input_note
                    ),
                    None => format!(
                        "Windows could not enumerate DirectShow microphone devices. {error}"
                    ),
                };
                vec![default_input]
            }
        }
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
        Ok((
            target.offset_x.unwrap_or(0),
            target.offset_y.unwrap_or(0),
            width,
            height,
        ))
    }

    pub fn audio_input_support_summary() -> String {
        match discover_audio_inputs() {
            Ok(audio_inputs) => format!(
                "DirectShow microphone discovery is ready. Found {} input{}.",
                audio_inputs.len(),
                if audio_inputs.len() == 1 { "" } else { "s" }
            ),
            Err(error) => match super::native_audio_backend::runtime_plan() {
                Some(plan) => {
                    let route_plan = super::native_audio_backend::route_plan();
                    let runtime_intent = super::native_audio_backend::runtime_intent(true, false);
                    let default_input = route_plan
                        .as_ref()
                        .and_then(|route_plan| route_plan.default_input_label.clone())
                        .or(plan.default_input_name)
                        .unwrap_or_else(|| "not detected".to_string());
                    let wasapi_summary = super::native_audio_backend::runtime_summary()
                        .unwrap_or_else(|| {
                            "Windows audio probing could not resolve a stable WASAPI candidate yet."
                                .to_string()
                        });
                    format!(
                        "DirectShow microphone discovery failed. Windows still reports default input `{default_input}`, {} capture endpoint{}, and {} render endpoint{}, so the app can keep using the Windows-default fallback today while Phase 2 moves to WASAPI. {} {} {wasapi_summary} {error}",
                        plan.capture_endpoint_count,
                        if plan.capture_endpoint_count == 1 {
                            ""
                        } else {
                            "s"
                        },
                        plan.render_endpoint_count,
                        if plan.render_endpoint_count == 1 {
                            ""
                        } else {
                            "s"
                        },
                        route_plan
                            .map(|route_plan| route_plan.default_input_note)
                            .unwrap_or_default(),
                        runtime_intent
                            .map(|intent| intent.summary)
                            .unwrap_or_default(),
                    )
                }
                None => format!("DirectShow microphone discovery failed. {error}"),
            },
        }
    }

    pub fn custom_region_support_summary() -> (bool, String) {
        (
            true,
            "Custom region capture is available on the Windows desktop path.".to_string(),
        )
    }

    pub fn system_audio_support_summary() -> (bool, String) {
        match discover_audio_inputs() {
            Ok(audio_inputs) => {
                if preferred_system_audio_input(&audio_inputs).is_some() {
                    (
                        true,
                        "A usable DirectShow loopback or Stereo Mix source is available for system-audio mixing."
                            .to_string(),
                    )
                } else {
                    (
                        false,
                        "Windows did not expose a usable system-audio loopback source. Look for Stereo Mix or a vendor loopback device to enable system-audio mixing."
                            .to_string(),
                    )
                }
            }
            Err(error) => match super::native_audio_backend::runtime_plan() {
                Some(plan) if plan.render_endpoint_count > 0 => {
                    let route_plan = super::native_audio_backend::route_plan();
                    let runtime_intent = super::native_audio_backend::runtime_intent(false, true);
                    let preferred_render = route_plan
                        .as_ref()
                        .and_then(|route_plan| route_plan.preferred_loopback_label.clone())
                        .unwrap_or_else(|| "not resolved".to_string());
                    (
                        false,
                        format!(
                            "Windows did not expose a usable DirectShow loopback source, but it does report {} render endpoint{} for a future WASAPI loopback path. {} {} The current preferred render candidate is `{preferred_render}`. {error}",
                            plan.render_endpoint_count,
                            if plan.render_endpoint_count == 1 {
                                ""
                            } else {
                                "s"
                            },
                            route_plan
                                .map(|route_plan| route_plan.loopback_note)
                                .unwrap_or_default(),
                            runtime_intent
                                .map(|intent| intent.summary)
                                .unwrap_or_default(),
                        ),
                    )
                }
                _ => (
                    false,
                    format!("Windows could not inspect DirectShow audio devices. {error}"),
                ),
            },
        }
    }

    pub fn audio_start_plan_summary(options: &RecordingOptions) -> Option<String> {
        let discovered_audio_inputs = discover_audio_inputs().unwrap_or_default();
        Some(
            super::native_audio_backend::start_plan(
                &options.audio_input_id,
                options.mic_enabled,
                options.system_audio_enabled,
                &discovered_audio_inputs,
            )
            .summary,
        )
    }

    fn resolve_target(options: &RecordingOptions) -> Result<ResolvedTarget, CaptureError> {
        let target_id = options.capture_target_id.as_str();
        let monitors = query_monitors().unwrap_or_default();

        if target_id == FULL_DESKTOP_TARGET_ID {
            return Ok(resolve_full_desktop_target(&monitors));
        }

        if let Some(device_name) = target_id.strip_prefix(MONITOR_TARGET_PREFIX) {
            let monitor = monitors
                .into_iter()
                .find(|item| item.device_name == device_name)
                .ok_or_else(|| {
                    CaptureError::BackendUnavailable(format!(
                        "the selected monitor `{device_name}` is no longer available"
                    ))
                })?;

            return Ok(ResolvedTarget {
                label: monitor.label,
                source: "desktop".to_string(),
                offset_x: Some(monitor.x),
                offset_y: Some(monitor.y),
                video_size: Some((monitor.width, monitor.height)),
            });
        }

        if let Some(window_id) = target_id.strip_prefix(WINDOW_TARGET_PREFIX) {
            let window = query_windows()?
                .into_iter()
                .find(|item| item.id.to_string() == window_id)
                .ok_or_else(|| {
                    CaptureError::BackendUnavailable(format!(
                        "the selected window `{window_id}` is no longer available"
                    ))
                })?;

            return Ok(ResolvedTarget {
                label: format!("Window · {}", window.title),
                source: "desktop".to_string(),
                offset_x: Some(window.x),
                offset_y: Some(window.y),
                video_size: Some((window.width, window.height)),
            });
        }

        if target_id == CUSTOM_REGION_TARGET_ID {
            return Ok(ResolvedTarget {
                label: format!(
                    "Custom region · {}, {} · {} x {}",
                    options.region_x, options.region_y, options.region_width, options.region_height
                ),
                source: "desktop".to_string(),
                offset_x: Some(options.region_x as i32),
                offset_y: Some(options.region_y as i32),
                video_size: Some((options.region_width.max(64), options.region_height.max(64))),
            });
        }

        Err(CaptureError::BackendUnavailable(format!(
            "unknown Windows capture target: {target_id}"
        )))
    }

    fn resolve_full_desktop_target(monitors: &[MonitorDescriptor]) -> ResolvedTarget {
        if monitors.is_empty() {
            return ResolvedTarget {
                label: "Full desktop".to_string(),
                source: "desktop".to_string(),
                offset_x: None,
                offset_y: None,
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
            source: "desktop".to_string(),
            offset_x: Some(min_x),
            offset_y: Some(min_y),
            video_size: Some(((max_x - min_x) as u32, (max_y - min_y) as u32)),
        }
    }

    fn spawn_ffmpeg(
        options: &RecordingOptions,
        target: &ResolvedTarget,
        stderr_buffer: Arc<Mutex<String>>,
    ) -> Result<(Child, Option<ChildStdin>), CaptureError> {
        let (width, height, fps) = quality_settings(&options.quality_preset);
        let encoder = encoder_for_quality(&options.quality_preset);
        let mut command = capture::ffmpeg_command();
        command
            .arg("-y")
            .arg("-f")
            .arg("gdigrab")
            .arg("-draw_mouse")
            .arg("1")
            .arg("-framerate")
            .arg(fps.to_string())
            .arg("-thread_queue_size")
            .arg("1024");

        if let Some(offset_x) = target.offset_x {
            command.arg("-offset_x").arg(offset_x.to_string());
        }

        if let Some(offset_y) = target.offset_y {
            command.arg("-offset_y").arg(offset_y.to_string());
        }

        if let Some((source_width, source_height)) = target.video_size {
            command
                .arg("-video_size")
                .arg(format!("{source_width}x{source_height}"));
        }

        command.arg("-i").arg(&target.source);

        let discovered_audio_inputs = discover_audio_inputs().unwrap_or_default();
        let windows_audio_start_plan = super::native_audio_backend::start_plan(
            &options.audio_input_id,
            options.mic_enabled,
            options.system_audio_enabled,
            &discovered_audio_inputs,
        );

        let mut audio_input_count = 0;
        if options.mic_enabled {
            if let Some(device_name) = discover_audio_device(
                &options.audio_input_id,
                &discovered_audio_inputs,
                &windows_audio_start_plan,
            )? {
                command
                    .arg("-f")
                    .arg("dshow")
                    .arg("-thread_queue_size")
                    .arg("1024")
                    .arg("-i")
                    .arg(format!("audio={device_name}"));
                audio_input_count += 1;
            }
        }

        if options.system_audio_enabled {
            let device_name =
                discover_system_audio_device(&discovered_audio_inputs, &windows_audio_start_plan)?;
            command
                .arg("-f")
                .arg("dshow")
                .arg("-thread_queue_size")
                .arg("1024")
                .arg("-i")
                .arg(format!("audio={device_name}"));
            audio_input_count += 1;
        }

        command
            .arg("-c:v")
            .arg(encoder.codec)
            .arg("-pix_fmt")
            .arg("yuv420p");

        if let Some(preset) = encoder.preset {
            command.arg("-preset").arg(preset);
        }

        if needs_scale_filter(target.video_size, width, height) {
            command.arg("-vf").arg(scale_filter(width, height));
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

        let mut child = command.spawn().map_err(|error| {
            CaptureError::SpawnFailed(capture::ffmpeg_launch_error_message(&error, "Windows"))
        })?;

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

    fn query_monitors() -> Result<Vec<MonitorDescriptor>, CaptureError> {
        parse_json_command(
            r#"Add-Type -AssemblyName System.Windows.Forms; [System.Windows.Forms.Screen]::AllScreens | ForEach-Object { [PSCustomObject]@{ deviceName = $_.DeviceName; label = if ($_.Primary) { "Display (Primary) · $($_.DeviceName)" } else { "Display · $($_.DeviceName)" }; width = $_.Bounds.Width; height = $_.Bounds.Height; x = $_.Bounds.X; y = $_.Bounds.Y; primary = $_.Primary } } | ConvertTo-Json -Compress"#,
        )
    }

    fn query_windows() -> Result<Vec<WindowDescriptor>, CaptureError> {
        parse_json_command(
            r#"$signature = @"
using System;
using System.Runtime.InteropServices;
public static class NativeWin {
  [StructLayout(LayoutKind.Sequential)]
  public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
  [DllImport("user32.dll")]
  public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
}
"@;
Add-Type $signature;
Get-Process | Where-Object { $_.MainWindowHandle -ne 0 -and $_.MainWindowTitle } | ForEach-Object {
  $rect = New-Object NativeWin+RECT
  if ([NativeWin]::GetWindowRect($_.MainWindowHandle, [ref]$rect)) {
    $width = $rect.Right - $rect.Left
    $height = $rect.Bottom - $rect.Top
    if ($width -gt 240 -and $height -gt 160 -and $_.MainWindowTitle -notlike 'Record Screen*') {
      [PSCustomObject]@{
        id = $_.MainWindowHandle.ToInt64()
        title = $_.MainWindowTitle
        processName = $_.ProcessName
        x = $rect.Left
        y = $rect.Top
        width = $width
        height = $height
      }
    }
  }
} | ConvertTo-Json -Compress"#,
        )
    }

    fn discover_audio_device(
        selected_audio_input_id: &str,
        discovered_audio_inputs: &[AudioInputOption],
        start_plan: &super::native_audio_backend::WindowsAudioStartPlan,
    ) -> Result<Option<String>, CaptureError> {
        if discovered_audio_inputs.is_empty() {
            if selected_audio_input_id == DEFAULT_AUDIO_INPUT_ID {
                return Ok(start_plan.microphone_device_name.clone());
            }

            return Err(CaptureError::BackendUnavailable(
                "ffmpeg did not expose any DirectShow audio input device".to_string(),
            ));
        }

        if selected_audio_input_id == DEFAULT_AUDIO_INPUT_ID {
            return Ok(start_plan.microphone_device_name.clone());
        }

        resolve_audio_input_id(selected_audio_input_id, discovered_audio_inputs)
            .map(Some)
            .ok_or_else(|| {
                CaptureError::BackendUnavailable(format!(
                    "the selected microphone input `{selected_audio_input_id}` is no longer available"
                ))
            })
    }

    fn native_preferred_input_label() -> Option<String> {
        super::native_audio_backend::route_plan().and_then(|plan| plan.default_input_label)
    }

    fn native_preferred_render_label() -> Option<String> {
        super::native_audio_backend::route_plan().and_then(|plan| plan.preferred_loopback_label)
    }

    fn discover_system_audio_device(
        discovered_audio_inputs: &[AudioInputOption],
        start_plan: &super::native_audio_backend::WindowsAudioStartPlan,
    ) -> Result<String, CaptureError> {
        start_plan
            .system_audio_device_name
            .clone()
            .or_else(|| {
                preferred_system_audio_input(discovered_audio_inputs).map(|input| input.id.clone())
            })
            .ok_or_else(|| {
                CaptureError::BackendUnavailable(
                    "Windows could not find a usable system-audio loopback source. Disable system audio and try again."
                        .to_string(),
                )
            })
    }

    fn discover_audio_inputs() -> Result<Vec<AudioInputOption>, CaptureError> {
        let output = capture::ffmpeg_command()
            .args(["-list_devices", "true", "-f", "dshow", "-i", "dummy"])
            .output()
            .map_err(|error| {
                CaptureError::BackendUnavailable(capture::ffmpeg_launch_error_message(
                    &error, "Windows",
                ))
            })?;

        let listing = String::from_utf8_lossy(&output.stderr);
        let mut in_audio_section = false;
        let mut audio_inputs = Vec::new();

        for line in listing.lines() {
            if line.contains("DirectShow audio devices") {
                in_audio_section = true;
                continue;
            }

            if in_audio_section && line.contains("DirectShow video devices") {
                in_audio_section = false;
            }

            if !in_audio_section {
                continue;
            }

            let Some(device_name) = parse_ffmpeg_quoted_device(line) else {
                continue;
            };

            audio_inputs.push(AudioInputOption {
                id: device_name.clone(),
                label: if classify_audio_input_kind(&device_name) == AudioInputKind::System {
                    format!("System audio · {device_name}")
                } else {
                    device_name.clone()
                },
                description: format!("DirectShow input: {device_name}"),
                kind: classify_audio_input_kind(&device_name),
            });
        }

        if audio_inputs.is_empty() {
            return Err(CaptureError::BackendUnavailable(format!(
                "ffmpeg did not expose any DirectShow audio input device. {}",
                extract_ffmpeg_context(&listing)
            )));
        }

        Ok(audio_inputs)
    }

    fn parse_ffmpeg_quoted_device(line: &str) -> Option<String> {
        let start = line.find('"')? + 1;
        let end = line[start..].find('"')? + start;
        Some(line[start..end].to_string())
    }

    fn classify_audio_input_kind(device_name: &str) -> AudioInputKind {
        let lowered = device_name.to_ascii_lowercase();
        if lowered.contains("stereo mix")
            || lowered.contains("what u hear")
            || lowered.contains("loopback")
            || lowered.contains("monitor")
            || lowered.contains("speaker")
            || lowered.contains("output")
        {
            AudioInputKind::System
        } else {
            AudioInputKind::Microphone
        }
    }

    fn parse_json_command<T>(script: &str) -> Result<Vec<T>, CaptureError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let output = Command::new("powershell")
            .args(["-NoProfile", "-Command", script])
            .output()
            .map_err(|error| CaptureError::BackendUnavailable(error.to_string()))?;

        if !output.status.success() {
            return Err(CaptureError::BackendUnavailable(
                String::from_utf8_lossy(&output.stderr).trim().to_string(),
            ));
        }

        parse_json_array_or_single(&output.stdout)
            .map_err(|error| CaptureError::BackendUnavailable(error.to_string()))
    }

    fn parse_json_array_or_single<T>(bytes: &[u8]) -> Result<Vec<T>, serde_json::Error>
    where
        T: for<'de> Deserialize<'de>,
    {
        if bytes.is_empty() {
            return Ok(Vec::new());
        }

        let value: serde_json::Value = serde_json::from_slice(bytes)?;
        match value {
            serde_json::Value::Array(_) => serde_json::from_value(value),
            serde_json::Value::Null => Ok(Vec::new()),
            other => serde_json::from_value(other).map(|item| vec![item]),
        }
    }

    fn run_powershell(script: &str) -> Result<(), String> {
        let output = Command::new("powershell")
            .args(["-NoProfile", "-Command", script])
            .output()
            .map_err(|error| error.to_string())?;

        if output.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
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
            }
        } else {
            preferred
        }
    }

    fn preferred_video_encoder() -> VideoEncoderProfile {
        static ENCODER: OnceLock<VideoEncoderProfile> = OnceLock::new();
        *ENCODER.get_or_init(|| {
            let encoders = load_ffmpeg_encoders().unwrap_or_default();

            for codec in ["h264_nvenc", "h264_qsv", "h264_amf", "h264_mf"] {
                if encoders.contains(codec) {
                    return VideoEncoderProfile {
                        codec,
                        preset: None,
                    };
                }
            }

            VideoEncoderProfile {
                codec: "libx264",
                preset: None,
            }
        })
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

    fn scale_filter(width: u32, height: u32) -> String {
        format!(
            "scale={width}:{height}:force_original_aspect_ratio=decrease,pad={width}:{height}:(ow-iw)/2:(oh-ih)/2"
        )
    }

    fn load_ffmpeg_encoders() -> Result<String, CaptureError> {
        let output = capture::ffmpeg_command()
            .args(["-hide_banner", "-encoders"])
            .output()
            .map_err(|error| {
                CaptureError::BackendUnavailable(capture::ffmpeg_launch_error_message(
                    &error, "Windows",
                ))
            })?;

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

        if stderr_lower.contains("gdigrab") || stderr_lower.contains("desktop") {
            return "ffmpeg could not access the Windows desktop capture source. Make sure ffmpeg is installed and the selected target is still visible.".to_string();
        }

        if stderr_lower.contains("dshow") || stderr_lower.contains("audio") {
            return "ffmpeg could not open the Windows microphone source. Disable microphone capture and try again.".to_string();
        }

        if stderr_lower.contains("no such file or directory")
            || stderr_lower.contains("not recognized as an internal or external command")
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

    fn extract_ffmpeg_context(stderr_log: &str) -> String {
        stderr_log
            .lines()
            .rev()
            .map(str::trim)
            .find(|line| {
                !line.is_empty()
                    && !line.starts_with('[')
                    && !line.eq_ignore_ascii_case("dummy: immediate exit requested")
            })
            .unwrap_or("Check ffmpeg installation, Windows microphone privacy settings, and whether another app is locking the input.")
            .to_string()
    }
}

#[cfg(target_os = "windows")]
pub use platform::{
    FfmpegWindowsCapture, audio_backend_statuses, audio_input_support_summary,
    audio_start_plan_summary, backend_statuses, custom_region_support_summary,
    encoder_backend_statuses, list_audio_inputs, list_capture_targets, preview_target_bounds,
    selected_audio_backend, selected_backend, selected_encoder_backend,
    system_audio_support_summary,
};

#[cfg(not(target_os = "windows"))]
pub struct FfmpegWindowsCapture;

#[cfg(not(target_os = "windows"))]
pub fn list_capture_targets() -> Vec<capture::CaptureTargetOption> {
    vec![capture::full_desktop_target()]
}

#[cfg(not(target_os = "windows"))]
pub fn list_audio_inputs() -> Vec<capture::AudioInputOption> {
    vec![capture::default_audio_input()]
}

#[cfg(not(target_os = "windows"))]
pub fn audio_input_support_summary() -> String {
    "DirectShow microphone discovery is only available on Windows.".to_string()
}

#[cfg(not(target_os = "windows"))]
pub fn selected_audio_backend() -> &'static dyn capture::AudioBackendFactory {
    panic!("selected_audio_backend is only available on Windows")
}

#[cfg(not(target_os = "windows"))]
pub fn audio_backend_statuses() -> Vec<capture::AudioBackendStatus> {
    vec![]
}

#[cfg(not(target_os = "windows"))]
pub fn selected_encoder_backend() -> &'static dyn capture::EncoderBackendFactory {
    panic!("selected_encoder_backend is only available on Windows")
}

#[cfg(not(target_os = "windows"))]
pub fn encoder_backend_statuses() -> Vec<capture::EncoderBackendStatus> {
    vec![]
}

#[cfg(not(target_os = "windows"))]
pub fn custom_region_support_summary() -> (bool, String) {
    (
        false,
        "Windows custom-region support is only available on Windows.".to_string(),
    )
}

#[cfg(not(target_os = "windows"))]
pub fn system_audio_support_summary() -> (bool, String) {
    (
        false,
        "Windows system-audio support is only available on Windows.".to_string(),
    )
}

#[cfg(not(target_os = "windows"))]
pub fn audio_start_plan_summary(_options: &capture::RecordingOptions) -> Option<String> {
    None
}

#[cfg(not(target_os = "windows"))]
pub fn preview_target_bounds(
    _capture_target_id: &str,
    _region_x: u32,
    _region_y: u32,
    _region_width: u32,
    _region_height: u32,
) -> Result<(i32, i32, u32, u32), capture::CaptureError> {
    Err(capture::CaptureError::UnsupportedPlatform)
}

pub fn backend_name() -> &'static str {
    "Windows ffmpeg / gdigrab backend"
}
