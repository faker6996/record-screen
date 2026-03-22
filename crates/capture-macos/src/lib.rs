pub mod native_audio_backend;
mod native_backend;
mod native_encoder_backend;

use std::{
    fs,
    process::Command,
    sync::{Arc, Mutex, OnceLock},
    thread,
    time::{Duration, SystemTime},
};

use capture::{
    ActiveRecording, AudioBackendFactory, AudioBackendStatus, AudioInputOption,
    CUSTOM_REGION_TARGET_ID, CaptureBackendFactory, CaptureBackendRuntimeSnapshot,
    CaptureBackendStatus, CaptureController, CaptureError, CaptureTargetOption,
    DEFAULT_AUDIO_INPUT_ID, EncoderBackendFactory, EncoderBackendRuntimeSnapshot,
    EncoderBackendStatus, FULL_DESKTOP_TARGET_ID, RecordingArtifact, RecordingOptions,
    audio_backend_runtime_snapshot, audio_backend_statuses as shared_audio_backend_statuses,
    backend_statuses as shared_backend_statuses, capture_backend_runtime_snapshot,
    default_audio_input, encoder_backend_runtime_snapshot,
    encoder_backend_statuses as shared_encoder_backend_statuses, explain_audio_backend_selection,
    explain_capture_backend_selection, explain_encoder_backend_selection, full_desktop_target,
    select_audio_backend, select_backend, select_encoder_backend,
};
#[cfg(target_os = "macos")]
use core_graphics::display::CGDisplay;
#[cfg(target_os = "macos")]
use screencapturekit::{
    cg::CGRect,
    recording_output::{
        RecordingCallbacks, SCRecordingOutput, SCRecordingOutputCodec,
        SCRecordingOutputConfiguration, SCRecordingOutputFileType,
    },
    shareable_content::SCShareableContent,
    stream::{
        SCStream,
        configuration::{PixelFormat, SCStreamConfiguration},
        content_filter::SCContentFilter,
    },
};

const MONITOR_TARGET_PREFIX: &str = "monitor:";
const RECORDING_OUTPUT_STARTUP_POLL_INTERVAL: Duration = Duration::from_millis(50);
const RECORDING_OUTPUT_STARTUP_POLL_ATTEMPTS: usize = 80;

#[derive(Clone)]
struct MacosRecorderRuntimePlan {
    capture: native_backend::ScreenCaptureKitStartPlan,
    audio: native_audio_backend::MacosAudioStartPlan,
    encoder: native_encoder_backend::AvAssetWriterOutputPlan,
}

#[cfg(target_os = "macos")]
pub struct ScreenCaptureKitRecordingOutputCapture {
    active_recording: ActiveRecording,
    stream: Option<SCStream>,
    recording_output: Option<SCRecordingOutput>,
    delegate_state: Arc<Mutex<ScreenCaptureKitRecordingOutputState>>,
    finished_artifact: Option<RecordingArtifact>,
}

#[cfg(target_os = "macos")]
#[derive(Debug, Default, Clone)]
struct ScreenCaptureKitRecordingOutputState {
    started: bool,
    finished: bool,
    error: Option<String>,
}

pub fn selected_backend() -> &'static dyn CaptureBackendFactory {
    let candidates = backend_candidates();
    select_backend(&candidates)
}

fn backend_candidates() -> Vec<&'static dyn CaptureBackendFactory> {
    vec![native_backend::backend()]
}

pub fn backend_statuses() -> Vec<CaptureBackendStatus> {
    let candidates = backend_candidates();
    shared_backend_statuses(&candidates)
}

pub fn selected_audio_backend() -> &'static dyn AudioBackendFactory {
    let candidates = audio_backend_candidates();
    select_audio_backend(&candidates)
}

fn audio_backend_candidates() -> Vec<&'static dyn AudioBackendFactory> {
    vec![native_audio_backend::backend()]
}

pub fn audio_backend_statuses() -> Vec<AudioBackendStatus> {
    let candidates = audio_backend_candidates();
    shared_audio_backend_statuses(&candidates)
}

pub fn selected_encoder_backend() -> &'static dyn EncoderBackendFactory {
    let candidates = encoder_backend_candidates();
    select_encoder_backend(&candidates)
}

fn encoder_backend_candidates() -> Vec<&'static dyn EncoderBackendFactory> {
    vec![native_encoder_backend::backend()]
}

pub fn encoder_backend_statuses() -> Vec<EncoderBackendStatus> {
    let candidates = encoder_backend_candidates();
    shared_encoder_backend_statuses(&candidates)
}

pub fn capture_selection_note() -> String {
    let candidates = backend_candidates();
    explain_capture_backend_selection(&candidates).note
}

pub fn capture_runtime_snapshot() -> CaptureBackendRuntimeSnapshot {
    let candidates = backend_candidates();
    capture_backend_runtime_snapshot(&candidates)
}

pub fn capture_start_plan_summary(options: &RecordingOptions) -> Option<String> {
    Some(build_runtime_plan(options).capture.summary)
}

pub fn capture_execution_plan_summary(options: &RecordingOptions) -> Option<String> {
    native_backend::execution_plan(options)
        .ok()
        .map(|plan| plan.summary)
}

pub fn capture_runtime_foundation_summary(options: &RecordingOptions) -> Option<String> {
    native_backend::runtime_foundation_summary(options)
}

pub fn capture_prepared_runtime_summary(options: &RecordingOptions) -> Option<String> {
    native_backend::prepared_runtime_summary(options)
}

pub fn capture_smoke_lifecycle_summary(options: &RecordingOptions) -> Option<String> {
    native_backend::smoke_lifecycle_summary(options)
}

pub fn audio_selection_note() -> String {
    let candidates = audio_backend_candidates();
    explain_audio_backend_selection(&candidates).note
}

pub fn audio_runtime_snapshot() -> capture::AudioBackendRuntimeSnapshot {
    let candidates = audio_backend_candidates();
    audio_backend_runtime_snapshot(&candidates)
}

pub fn audio_start_plan_summary(options: &RecordingOptions) -> Option<String> {
    Some(build_runtime_plan(options).audio.summary)
}

pub fn encoder_selection_note() -> String {
    let candidates = encoder_backend_candidates();
    explain_encoder_backend_selection(&candidates).note
}

pub fn encoder_runtime_snapshot() -> EncoderBackendRuntimeSnapshot {
    let candidates = encoder_backend_candidates();
    encoder_backend_runtime_snapshot(&candidates)
}

pub fn encoder_output_plan_summary(options: &RecordingOptions) -> Option<String> {
    Some(build_runtime_plan(options).encoder.summary)
}

#[cfg(target_os = "macos")]
pub(crate) fn start_native_capture_bridge(
    options: RecordingOptions,
) -> Result<Box<dyn CaptureController>, CaptureError> {
    if !supports_native_recording_output(&options) || !native_recording_output_is_supported() {
        return Err(CaptureError::BackendUnavailable(
            native_recording_output_support_note(),
        ));
    }

    ScreenCaptureKitRecordingOutputCapture::start(options)
        .map(|capture| Box::new(capture) as Box<dyn CaptureController>)
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn start_native_capture_bridge(
    _options: RecordingOptions,
) -> Result<Box<dyn CaptureController>, CaptureError> {
    Err(CaptureError::BackendUnavailable(
        "ScreenCaptureKit recording output only runs on macOS hosts.".to_string(),
    ))
}

#[cfg(target_os = "macos")]
impl ScreenCaptureKitRecordingOutputCapture {
    fn start(options: RecordingOptions) -> Result<Self, CaptureError> {
        let runtime_plan = build_runtime_plan(&options);
        let start_plan = &runtime_plan.capture;
        let content = SCShareableContent::create()
            .with_on_screen_windows_only(true)
            .with_exclude_desktop_windows(true)
            .get()
            .map_err(|error| CaptureError::BackendUnavailable(error.to_string()))?;
        let display =
            native_backend::resolve_native_display(&content, &start_plan.resolved_native_target_id)
                .ok_or_else(|| {
                    CaptureError::BackendUnavailable(format!(
                        "ScreenCaptureKit could not resolve native display for `{}`.",
                        start_plan.resolved_source_target_id
                    ))
                })?;

        let filter = SCContentFilter::create()
            .with_display(&display)
            .with_excluding_windows(&[])
            .build();
        let source_rect = build_native_source_rect(&options, &display);
        let mut config = SCStreamConfiguration::new()
            .with_width(start_plan.output_width)
            .with_height(start_plan.output_height)
            .with_fps(start_plan.fps)
            .with_pixel_format(PixelFormat::BGRA)
            .with_shows_cursor(true)
            .with_captures_audio(options.system_audio_enabled)
            .with_captures_microphone(options.mic_enabled)
            .with_excludes_current_process_audio(options.system_audio_enabled);
        if let Some(microphone_device_id) = runtime_plan.audio.microphone_device_id.as_deref() {
            config = config.with_microphone_capture_device_id(microphone_device_id);
        }
        if let Some(source_rect) = source_rect {
            config = config.with_source_rect(source_rect);
        }

        let recording_config = SCRecordingOutputConfiguration::new()
            .with_output_url(&options.output_path)
            .with_output_file_type(recording_output_file_type(&options))
            .with_video_codec(recording_output_codec(&runtime_plan.encoder.codec_name));
        let delegate_state = Arc::new(Mutex::new(ScreenCaptureKitRecordingOutputState::default()));
        let recording_delegate = build_recording_output_callbacks(&delegate_state);
        let recording_output =
            SCRecordingOutput::new_with_delegate(&recording_config, recording_delegate)
                .ok_or_else(|| {
                    CaptureError::BackendUnavailable(
                        "ScreenCaptureKit recording output is not available on this macOS runtime."
                            .to_string(),
                    )
                })?;

        let stream = SCStream::new(&filter, &config);
        stream
            .add_recording_output(&recording_output)
            .map_err(|error| CaptureError::SpawnFailed(error.to_string()))?;
        stream
            .start_capture()
            .map_err(|error| CaptureError::SpawnFailed(error.to_string()))?;
        if let Err(error) = wait_for_recording_output_start(&delegate_state, &options.output_path) {
            let _ = shutdown_recording_output_stream(&stream, Some(&recording_output));
            return Err(error);
        }

        Ok(Self {
            active_recording: ActiveRecording {
                backend_name: "macOS ScreenCaptureKit / SCRecordingOutput".to_string(),
                encoder_label: runtime_plan.encoder.encoder_label,
                output_path: options.output_path,
                started_at: SystemTime::now(),
                target_label: if options.capture_target_id == CUSTOM_REGION_TARGET_ID {
                    format!(
                        "Custom region · {}, {} · {} x {}",
                        options.region_x,
                        options.region_y,
                        options.region_width,
                        options.region_height
                    )
                } else {
                    start_plan
                        .resolved_native_target_label
                        .clone()
                        .unwrap_or_else(|| start_plan.resolved_source_target_id.clone())
                },
            },
            stream: Some(stream),
            recording_output: Some(recording_output),
            delegate_state,
            finished_artifact: None,
        })
    }

    fn build_artifact(&self, finished_at: SystemTime) -> Result<RecordingArtifact, CaptureError> {
        let metadata = fs::metadata(&self.active_recording.output_path).map_err(|error| {
            CaptureError::OutputInspectionFailed(describe_output_path_error(
                &self.active_recording.output_path,
                &error,
            ))
        })?;
        let bytes_written = self
            .recording_output
            .as_ref()
            .map(recording_output_file_size)
            .unwrap_or(0)
            .max(metadata.len());
        let duration = self
            .recording_output
            .as_ref()
            .and_then(recording_output_duration)
            .unwrap_or_else(|| {
                finished_at
                    .duration_since(self.active_recording.started_at)
                    .unwrap_or_default()
            });

        Ok(RecordingArtifact {
            output_path: self.active_recording.output_path.clone(),
            started_at: self.active_recording.started_at,
            finished_at,
            duration,
            bytes_written,
        })
    }

    fn current_state(&self) -> Result<ScreenCaptureKitRecordingOutputState, CaptureError> {
        self.delegate_state
            .lock()
            .map(|state| state.clone())
            .map_err(|_| {
                CaptureError::StopFailed(
                    "failed to inspect ScreenCaptureKit recording-output state".to_string(),
                )
            })
    }

    fn stop_stream(&mut self) -> Result<(), CaptureError> {
        if let Some(stream) = self.stream.take() {
            shutdown_recording_output_stream(&stream, self.recording_output.as_ref())?;
            drop(stream);
        }
        Ok(())
    }

    fn wait_for_completion(&self) -> Result<(), CaptureError> {
        for _ in 0..30 {
            let state = self.current_state()?;
            if let Some(error) = state.error {
                return Err(CaptureError::StopFailed(error));
            }
            if state.finished {
                return Ok(());
            }
            if self.active_recording.output_path.is_file()
                && self
                    .recording_output
                    .as_ref()
                    .map(recording_output_file_size)
                    .unwrap_or(0)
                    > 0
            {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(100));
        }

        wait_for_recording_output_file(&self.active_recording.output_path)
    }
}

pub fn list_capture_targets() -> Vec<CaptureTargetOption> {
    list_device_options().0
}

#[cfg(target_os = "macos")]
pub fn preview_target_bounds(
    capture_target_id: &str,
    region_x: u32,
    region_y: u32,
    region_width: u32,
    region_height: u32,
) -> Result<(i32, i32, u32, u32), CaptureError> {
    if capture_target_id == CUSTOM_REGION_TARGET_ID {
        return Ok((
            region_x as i32,
            region_y as i32,
            region_width.max(64),
            region_height.max(64),
        ));
    }

    let start_plan = native_backend::start_plan(&RecordingOptions {
        output_path: "/tmp/record-screen-preview.mp4".into(),
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
        region_source_capture_target_id: capture_target_id.to_string(),
        region_source_origin_x: 0,
        region_source_origin_y: 0,
        region_source_scale_factor_milli: 1000,
    });

    if start_plan.resolved_native_target_id == FULL_DESKTOP_TARGET_ID {
        let displays = CGDisplay::active_displays()
            .map_err(|error| CaptureError::BackendUnavailable(error.to_string()))?;
        let mut bounds_iter = displays
            .into_iter()
            .map(|display_id| CGDisplay::new(display_id).bounds());
        let Some(first) = bounds_iter.next() else {
            return Err(CaptureError::BackendUnavailable(
                "CoreGraphics did not expose any active displays.".to_string(),
            ));
        };

        let mut min_x = first.origin.x;
        let mut min_y = first.origin.y;
        let mut max_x = first.origin.x + first.size.width;
        let mut max_y = first.origin.y + first.size.height;

        for bounds in bounds_iter {
            min_x = min_x.min(bounds.origin.x);
            min_y = min_y.min(bounds.origin.y);
            max_x = max_x.max(bounds.origin.x + bounds.size.width);
            max_y = max_y.max(bounds.origin.y + bounds.size.height);
        }

        return Ok((
            min_x.round() as i32,
            min_y.round() as i32,
            (max_x - min_x).max(64.0).round() as u32,
            (max_y - min_y).max(64.0).round() as u32,
        ));
    }

    let content = SCShareableContent::create()
        .with_on_screen_windows_only(true)
        .with_exclude_desktop_windows(true)
        .get()
        .map_err(|error| CaptureError::BackendUnavailable(error.to_string()))?;
    let display =
        native_backend::resolve_native_display(&content, &start_plan.resolved_native_target_id)
            .ok_or_else(|| {
                CaptureError::BackendUnavailable(format!(
                    "ScreenCaptureKit could not resolve native display for `{}`.",
                    start_plan.resolved_source_target_id
                ))
            })?;
    let bounds = CGDisplay::new(display.display_id()).bounds();
    let cg_display = CGDisplay::new(display.display_id());

    Ok((
        bounds.origin.x.round() as i32,
        bounds.origin.y.round() as i32,
        cg_display.pixels_wide().max(64) as u32,
        cg_display.pixels_high().max(64) as u32,
    ))
}

pub fn list_audio_inputs() -> Vec<AudioInputOption> {
    list_device_options().1
}

pub fn list_device_options() -> (Vec<CaptureTargetOption>, Vec<AudioInputOption>) {
    let native_targets = native_backend::capture_target_options();
    let native_audio_inputs = native_audio_backend::selectable_audio_inputs();
    let mut targets = native_targets.unwrap_or_else(|| vec![full_desktop_target()]);

    if targets.is_empty() {
        targets.push(full_desktop_target());
    }

    let mut default_input = default_audio_input();
    if let Some(preferred_input) = native_preferred_input_label() {
        default_input.description =
            format!("Use the macOS default input device: {preferred_input}.");
    }

    let mut inputs = vec![default_input];
    inputs.extend(native_audio_inputs);

    (targets, inputs)
}

pub fn audio_input_support_summary() -> String {
    let native_audio_inputs = native_audio_backend::selectable_audio_inputs();
    if !native_audio_inputs.is_empty() {
        let base = format!(
            "Native microphone discovery is ready. Found {} input{}.",
            native_audio_inputs.len(),
            if native_audio_inputs.len() == 1 {
                ""
            } else {
                "s"
            }
        );
        return match native_audio_backend::runtime_summary() {
            Some(summary) => format!("{base} {summary}"),
            None => base,
        };
    }

    match native_audio_backend::runtime_summary() {
        Some(summary) => {
            format!("Native microphone discovery did not expose a selectable input yet. {summary}")
        }
        None => "Native microphone discovery did not expose a selectable input yet.".to_string(),
    }
}

pub fn custom_region_support_summary() -> (bool, String) {
    if native_recording_output_is_supported() {
        (
            true,
            "Custom region capture is available on macOS through the ScreenCaptureKit native lane."
                .to_string(),
        )
    } else {
        (false, native_recording_output_support_note())
    }
}

pub fn system_audio_support_summary() -> (bool, String) {
    if native_recording_output_is_supported() {
        (
            true,
            "System audio capture is available on macOS through the ScreenCaptureKit recording-output lane."
                .to_string(),
        )
    } else {
        (false, native_recording_output_support_note())
    }
}

fn native_preferred_input_label() -> Option<String> {
    native_audio_backend::preferred_input_device_name()
}

#[cfg(target_os = "macos")]
impl CaptureController for ScreenCaptureKitRecordingOutputCapture {
    fn active_recording(&self) -> &ActiveRecording {
        &self.active_recording
    }

    fn supports_pause_resume(&self) -> bool {
        false
    }

    fn pause_resume_note(&self) -> Option<String> {
        Some(
            "Pause/resume is not available for the macOS ScreenCaptureKit recording-output lane yet."
                .to_string(),
        )
    }

    fn pause(&mut self) -> Result<(), CaptureError> {
        Err(CaptureError::SignalFailed(
            "Pause/resume is not wired into the ScreenCaptureKit recording-output lane yet."
                .to_string(),
        ))
    }

    fn resume(&mut self) -> Result<(), CaptureError> {
        Err(CaptureError::SignalFailed(
            "Pause/resume is not wired into the ScreenCaptureKit recording-output lane yet."
                .to_string(),
        ))
    }

    fn stop(&mut self) -> Result<RecordingArtifact, CaptureError> {
        if let Some(artifact) = self.finished_artifact.clone() {
            return Ok(artifact);
        }

        self.stop_stream()?;
        self.wait_for_completion()?;
        let artifact = self.build_artifact(SystemTime::now())?;
        self.finished_artifact = Some(artifact.clone());
        Ok(artifact)
    }

    fn poll_finished(&mut self) -> Result<Option<RecordingArtifact>, CaptureError> {
        if let Some(artifact) = self.finished_artifact.clone() {
            return Ok(Some(artifact));
        }

        let state = self.current_state()?;
        if let Some(error) = state.error {
            return Err(CaptureError::StopFailed(error));
        }

        if !state.finished {
            return Ok(None);
        }

        let _ = self.stop_stream();
        let artifact = self.build_artifact(SystemTime::now())?;
        self.finished_artifact = Some(artifact.clone());
        Ok(Some(artifact))
    }
}

fn build_runtime_plan(options: &RecordingOptions) -> MacosRecorderRuntimePlan {
    let discovered_audio_inputs = native_audio_backend::selectable_audio_inputs();

    MacosRecorderRuntimePlan {
        capture: native_backend::start_plan(options),
        audio: native_audio_backend::start_plan(
            &options.audio_input_id,
            options.mic_enabled,
            options.system_audio_enabled,
            &discovered_audio_inputs,
        ),
        encoder: native_encoder_backend::output_plan(options),
    }
}

fn native_recording_output_is_supported() -> bool {
    matches!(current_macos_version(), Some((major, _, _)) if major >= 15)
}

fn native_recording_output_support_note() -> String {
    native_recording_output_support_note_for(current_macos_version())
}

fn native_recording_output_support_note_for(version: Option<(u64, u64, u64)>) -> String {
    match version {
        Some((major, minor, patch)) if major < 15 => format!(
            "System audio capture requires macOS 15.0 or newer for the ScreenCaptureKit recording-output lane. Current runtime is {major}.{minor}.{patch}."
        ),
        Some((_major, _minor, _patch)) => "ScreenCaptureKit recording output should be available on this macOS runtime, but the direct native lane could not be used.".to_string(),
        None => "The app could not confirm the macOS version for ScreenCaptureKit recording-output support.".to_string(),
    }
}

fn current_macos_version() -> Option<(u64, u64, u64)> {
    static CURRENT_MACOS_VERSION: OnceLock<Option<(u64, u64, u64)>> = OnceLock::new();

    CURRENT_MACOS_VERSION
        .get_or_init(|| {
            let output = Command::new("sw_vers")
                .args(["-productVersion"])
                .output()
                .ok()?;

            if !output.status.success() {
                return None;
            }

            parse_macos_version(String::from_utf8_lossy(&output.stdout).trim())
        })
        .to_owned()
}

fn parse_macos_version(value: &str) -> Option<(u64, u64, u64)> {
    let mut parts = value.trim().split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().unwrap_or("0").parse().ok()?;
    let patch = parts.next().unwrap_or("0").parse().ok()?;
    Some((major, minor, patch))
}

#[cfg(target_os = "macos")]
fn supports_native_recording_output(options: &RecordingOptions) -> bool {
    options.capture_target_id == FULL_DESKTOP_TARGET_ID
        || options.capture_target_id.starts_with(MONITOR_TARGET_PREFIX)
        || options.capture_target_id == CUSTOM_REGION_TARGET_ID
}

#[cfg(target_os = "macos")]
fn recording_output_codec(codec_name: &str) -> SCRecordingOutputCodec {
    if codec_name.to_ascii_lowercase().contains("hevc") {
        SCRecordingOutputCodec::HEVC
    } else {
        SCRecordingOutputCodec::H264
    }
}

#[cfg(target_os = "macos")]
fn recording_output_file_type(options: &RecordingOptions) -> SCRecordingOutputFileType {
    match options
        .output_path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("mov") => SCRecordingOutputFileType::MOV,
        _ => SCRecordingOutputFileType::MP4,
    }
}

#[cfg(target_os = "macos")]
fn build_recording_output_callbacks(
    state: &Arc<Mutex<ScreenCaptureKitRecordingOutputState>>,
) -> RecordingCallbacks {
    let start_state = Arc::clone(state);
    let fail_state = Arc::clone(state);
    let finish_state = Arc::clone(state);

    RecordingCallbacks::new()
        .on_start(move || {
            if let Ok(mut state) = start_state.lock() {
                state.started = true;
            }
        })
        .on_fail(move |error| {
            if let Ok(mut state) = fail_state.lock() {
                state.error = Some(error);
            }
        })
        .on_finish(move || {
            if let Ok(mut state) = finish_state.lock() {
                state.finished = true;
            }
        })
}

#[cfg(target_os = "macos")]
fn wait_for_recording_output_start(
    state: &Arc<Mutex<ScreenCaptureKitRecordingOutputState>>,
    output_path: &std::path::Path,
) -> Result<(), CaptureError> {
    for _ in 0..RECORDING_OUTPUT_STARTUP_POLL_ATTEMPTS {
        let snapshot = state.lock().map(|state| state.clone()).map_err(|_| {
            CaptureError::SpawnFailed(
                "failed to inspect ScreenCaptureKit recording-output startup state".to_string(),
            )
        })?;
        if let Some(error) = snapshot.error {
            return Err(CaptureError::SpawnFailed(error));
        }
        if snapshot.started {
            return Ok(());
        }
        if output_path.is_file() {
            return Ok(());
        }
        thread::sleep(RECORDING_OUTPUT_STARTUP_POLL_INTERVAL);
    }

    Err(CaptureError::SpawnFailed(
        "ScreenCaptureKit recording output did not confirm startup in time.".to_string(),
    ))
}

#[cfg(target_os = "macos")]
fn shutdown_recording_output_stream(
    stream: &SCStream,
    recording_output: Option<&SCRecordingOutput>,
) -> Result<(), CaptureError> {
    let stop_error = stream.stop_capture().err().map(|error| error.to_string());
    let remove_error = recording_output
        .and_then(|recording_output| stream.remove_recording_output(recording_output).err())
        .map(|error| error.to_string());

    match (stop_error, remove_error) {
        (None, None) => Ok(()),
        (Some(stop_error), None) => Err(CaptureError::StopFailed(stop_error)),
        (None, Some(remove_error)) => Err(CaptureError::StopFailed(remove_error)),
        (Some(stop_error), Some(remove_error)) => Err(CaptureError::StopFailed(format!(
            "failed to stop ScreenCaptureKit recording output cleanly: stop={stop_error}; remove={remove_error}"
        ))),
    }
}

#[cfg(target_os = "macos")]
fn recording_output_duration(recording_output: &SCRecordingOutput) -> Option<Duration> {
    duration_from_cmtime(recording_output.recorded_duration())
}

#[cfg(target_os = "macos")]
fn duration_from_cmtime(duration: screencapturekit::cm::CMTime) -> Option<Duration> {
    if duration.timescale <= 0 || duration.value < 0 {
        return None;
    }

    let seconds = duration.value / i64::from(duration.timescale);
    let nanos = ((duration.value % i64::from(duration.timescale)) * 1_000_000_000)
        / i64::from(duration.timescale);
    Some(Duration::new(seconds as u64, nanos.max(0) as u32))
}

#[cfg(target_os = "macos")]
fn recording_output_file_size(recording_output: &SCRecordingOutput) -> u64 {
    recording_output.recorded_file_size().max(0) as u64
}

#[cfg(target_os = "macos")]
fn wait_for_recording_output_file(path: &std::path::Path) -> Result<(), CaptureError> {
    for _ in 0..20 {
        if path.is_file() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(100));
    }

    Err(CaptureError::OutputInspectionFailed(format!(
        "native recording output did not materialize `{}` in time. The file may still be finalizing, the output directory may be unavailable, or the volume may be full.",
        path.display(),
    )))
}

fn describe_output_path_error(path: &std::path::Path, error: &std::io::Error) -> String {
    use std::io::ErrorKind;

    match error.kind() {
        ErrorKind::NotFound => format!(
            "recording output `{}` is missing. The destination may have been removed or the recording never finalized cleanly.",
            path.display()
        ),
        ErrorKind::PermissionDenied => format!(
            "recording output `{}` is not readable. Check folder permissions and macOS privacy access for the chosen output location.",
            path.display()
        ),
        ErrorKind::StorageFull => format!(
            "recording output `{}` could not be finalized because the volume is full.",
            path.display()
        ),
        _ => format!("{} ({})", error, path.display()),
    }
}

#[cfg(target_os = "macos")]
fn build_native_source_rect(
    options: &RecordingOptions,
    display: &screencapturekit::shareable_content::SCDisplay,
) -> Option<CGRect> {
    build_native_source_rect_from_dimensions(
        options,
        display.width() as f64,
        display.height() as f64,
    )
}

#[cfg(target_os = "macos")]
fn build_native_source_rect_from_dimensions(
    options: &RecordingOptions,
    display_width: f64,
    display_height: f64,
) -> Option<CGRect> {
    if options.capture_target_id != CUSTOM_REGION_TARGET_ID {
        return None;
    }

    let source_scale = (options.region_source_scale_factor_milli.max(1) as f64) / 1000.0;
    let display_points_width = display_width / source_scale;
    let display_points_height = display_height / source_scale;
    if display_points_width <= 0.0 || display_points_height <= 0.0 {
        return None;
    }

    let x =
        ((options.region_x as i32 - options.region_source_origin_x).max(0) as f64) / source_scale;
    let y =
        ((options.region_y as i32 - options.region_source_origin_y).max(0) as f64) / source_scale;
    if x >= display_points_width || y >= display_points_height {
        return None;
    }

    let width = ((options.region_width.max(1) as f64) / source_scale)
        .max(1.0)
        .min(display_points_width - x);
    let height = ((options.region_height.max(1) as f64) / source_scale)
        .max(1.0)
        .min(display_points_height - y);

    Some(CGRect::new(x, y, width, height))
}

#[cfg(test)]
mod tests {
    use super::{
        ScreenCaptureKitRecordingOutputState, build_native_source_rect_from_dimensions,
        native_recording_output_support_note_for, parse_macos_version,
    };
    use capture::{CUSTOM_REGION_TARGET_ID, RecordingOptions};
    #[cfg(target_os = "macos")]
    use screencapturekit::cm::CMTime;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    #[test]
    #[cfg(target_os = "macos")]
    fn converts_recording_output_duration_from_cmtime() {
        let time = CMTime::new(2500, 1000);
        assert_eq!(
            super::duration_from_cmtime(time),
            Some(Duration::from_millis(2500))
        );
    }

    #[test]
    fn recording_output_state_defaults_are_idle() {
        let state = ScreenCaptureKitRecordingOutputState::default();
        assert!(!state.started);
        assert!(!state.finished);
        assert!(state.error.is_none());
    }

    #[test]
    fn recording_output_start_state_can_be_marked_started() {
        let state = Arc::new(Mutex::new(ScreenCaptureKitRecordingOutputState::default()));
        {
            let mut guard = state.lock().expect("state lock should work");
            guard.started = true;
        }
        assert!(state.lock().expect("state lock should work").started);
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn builds_native_source_rect_for_custom_region_shape() {
        let options = RecordingOptions {
            output_path: "/tmp/test.mp4".into(),
            quality_preset: "1080p".to_string(),
            mic_enabled: false,
            system_audio_enabled: false,
            capture_target_id: CUSTOM_REGION_TARGET_ID.to_string(),
            audio_input_id: "default".to_string(),
            portal_parent_window: None,
            portal_restore_token: None,
            region_x: 200,
            region_y: 100,
            region_width: 400,
            region_height: 300,
            region_source_capture_target_id: "monitor:1".to_string(),
            region_source_origin_x: 100,
            region_source_origin_y: 50,
            region_source_scale_factor_milli: 2000,
        };
        let rect = build_native_source_rect_from_dimensions(&options, 3024.0, 1964.0)
            .expect("custom region should map to a native source rect");

        assert_eq!(rect.x, 50.0);
        assert_eq!(rect.y, 25.0);
        assert_eq!(rect.width, 200.0);
        assert_eq!(rect.height, 150.0);
    }

    #[test]
    fn parses_macos_product_versions() {
        assert_eq!(parse_macos_version("15.0"), Some((15, 0, 0)));
        assert_eq!(parse_macos_version("14.6.1"), Some((14, 6, 1)));
        assert_eq!(parse_macos_version(""), None);
    }

    #[test]
    fn recording_output_support_note_mentions_runtime_requirement() {
        let note = native_recording_output_support_note_for(Some((14, 7, 0)));
        assert!(note.contains("macOS 15.0 or newer"));
        assert!(note.contains("14.7.0"));
    }
}
