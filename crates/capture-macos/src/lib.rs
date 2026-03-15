pub mod native_audio_backend;
mod native_backend;
mod native_encoder_backend;

use std::{
    fs,
    io::{Read, Write},
    os::unix::process::ExitStatusExt,
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant, SystemTime},
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
    explain_capture_backend_selection, explain_encoder_backend_selection, ffmpeg_command,
    full_desktop_target, resolve_audio_input_id, select_audio_backend, select_backend,
    select_encoder_backend,
};
#[cfg(target_os = "macos")]
use screencapturekit::{
    cg::CGRect,
    cm::SCFrameStatus,
    cv::CVPixelBufferLockFlags,
    recording_output::{
        RecordingCallbacks, SCRecordingOutput, SCRecordingOutputCodec,
        SCRecordingOutputConfiguration, SCRecordingOutputFileType,
    },
    shareable_content::SCShareableContent,
    stream::{
        SCStream,
        configuration::{PixelFormat, SCStreamConfiguration},
        content_filter::SCContentFilter,
        output_type::SCStreamOutputType,
    },
};

const MONITOR_TARGET_PREFIX: &str = "monitor:";
const STARTUP_POLL_INTERVAL: Duration = Duration::from_millis(50);
const STARTUP_POLL_ATTEMPTS: usize = 6;
const AVFOUNDATION_LISTING_TTL: Duration = Duration::from_secs(12);

#[derive(Clone)]
struct CachedAvfoundationListing {
    listing: String,
    refreshed_at: Instant,
}

#[derive(Clone, Copy)]
struct VideoEncoderProfile {
    codec: &'static str,
    preset: Option<&'static str>,
}

#[derive(Debug, Clone)]
struct MacosRecorderRuntimePlan {
    capture: native_backend::ScreenCaptureKitStartPlan,
    audio: native_audio_backend::MacosAudioStartPlan,
    encoder: native_encoder_backend::AvAssetWriterOutputPlan,
    discovered_audio_inputs: Vec<AudioInputOption>,
}

#[derive(Debug, Clone)]
struct ScreenCaptureKitBridgeCropPlan {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
}

pub struct FfmpegMacosCapture {
    active_recording: ActiveRecording,
    child: Child,
    stdin: Option<ChildStdin>,
    stderr_buffer: Arc<Mutex<String>>,
    finished_artifact: Option<RecordingArtifact>,
    paused: bool,
}

#[cfg(target_os = "macos")]
pub struct ScreenCaptureKitBridgeCapture {
    active_recording: ActiveRecording,
    child: Child,
    stream: Option<SCStream>,
    stderr_buffer: Arc<Mutex<String>>,
    finished_artifact: Option<RecordingArtifact>,
    paused: Arc<AtomicBool>,
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

pub struct FfmpegMacosBackend;
static FFMPEG_MACOS_BACKEND: FfmpegMacosBackend = FfmpegMacosBackend;
static FFMPEG_MACOS_AUDIO_BACKEND: FfmpegMacosAudioBackend = FfmpegMacosAudioBackend;
static FFMPEG_MACOS_ENCODER_BACKEND: FfmpegMacosEncoderBackend = FfmpegMacosEncoderBackend;
pub struct FfmpegMacosAudioBackend;
pub struct FfmpegMacosEncoderBackend;

pub fn selected_backend() -> &'static dyn CaptureBackendFactory {
    select_backend(&backend_candidates())
}

fn backend_candidates() -> [&'static dyn CaptureBackendFactory; 2] {
    [native_backend::backend(), &FFMPEG_MACOS_BACKEND]
}

pub fn backend_statuses() -> Vec<CaptureBackendStatus> {
    shared_backend_statuses(&backend_candidates())
}

pub fn selected_audio_backend() -> &'static dyn AudioBackendFactory {
    select_audio_backend(&audio_backend_candidates())
}

fn audio_backend_candidates() -> [&'static dyn AudioBackendFactory; 2] {
    [native_audio_backend::backend(), &FFMPEG_MACOS_AUDIO_BACKEND]
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
        &FFMPEG_MACOS_ENCODER_BACKEND,
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
    explain_audio_backend_selection(&audio_backend_candidates()).note
}

pub fn audio_runtime_snapshot() -> capture::AudioBackendRuntimeSnapshot {
    audio_backend_runtime_snapshot(&audio_backend_candidates())
}

pub fn audio_start_plan_summary(options: &RecordingOptions) -> Option<String> {
    Some(build_runtime_plan(options).audio.summary)
}

pub fn encoder_selection_note() -> String {
    explain_encoder_backend_selection(&encoder_backend_candidates()).note
}

pub fn encoder_runtime_snapshot() -> EncoderBackendRuntimeSnapshot {
    encoder_backend_runtime_snapshot(&encoder_backend_candidates())
}

pub fn encoder_output_plan_summary(options: &RecordingOptions) -> Option<String> {
    Some(build_runtime_plan(options).encoder.summary)
}

impl CaptureBackendFactory for FfmpegMacosBackend {
    fn descriptor(&self) -> CaptureBackendDescriptor {
        CaptureBackendDescriptor {
            id: "macos-ffmpeg-avfoundation",
            label: "macOS ffmpeg / AVFoundation",
            family: CaptureBackendFamily::FallbackFfmpeg,
        }
    }

    fn availability(&self) -> CaptureBackendAvailability {
        CaptureBackendAvailability::Available
    }

    fn runtime_report(&self) -> CaptureBackendRuntimeReport {
        CaptureBackendRuntimeReport {
            summary: Some(
                "Current macOS capture runtime uses ffmpeg with AVFoundation screen sources."
                    .to_string(),
            ),
            preferred_target_label: Some("Primary display".to_string()),
        }
    }

    fn start(&self, options: RecordingOptions) -> Result<Box<dyn CaptureController>, CaptureError> {
        Ok(Box::new(FfmpegMacosCapture::start(options)?))
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn start_native_capture_bridge(
    options: RecordingOptions,
) -> Result<Box<dyn CaptureController>, CaptureError> {
    if supports_native_recording_output(&options) {
        if native_recording_output_is_supported() {
            match ScreenCaptureKitRecordingOutputCapture::start(options.clone()) {
                Ok(capture) => return Ok(Box::new(capture)),
                Err(error) if options.system_audio_enabled => return Err(error),
                Err(_) => {}
            }
        } else if options.system_audio_enabled {
            return Err(CaptureError::BackendUnavailable(
                native_recording_output_support_note(),
            ));
        }
    }

    if options.system_audio_enabled || options.mic_enabled {
        return Ok(Box::new(FfmpegMacosCapture::start(options)?));
    }

    Ok(Box::new(ScreenCaptureKitBridgeCapture::start(options)?))
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn start_native_capture_bridge(
    _options: RecordingOptions,
) -> Result<Box<dyn CaptureController>, CaptureError> {
    Err(CaptureError::BackendUnavailable(
        "ScreenCaptureKit bridge capture only runs on macOS hosts.".to_string(),
    ))
}

impl AudioBackendFactory for FfmpegMacosAudioBackend {
    fn descriptor(&self) -> AudioBackendDescriptor {
        AudioBackendDescriptor {
            id: "macos-ffmpeg-avfoundation-audio",
            label: "macOS ffmpeg / AVFoundation audio",
            family: AudioBackendFamily::FallbackFfmpeg,
        }
    }

    fn availability(&self) -> AudioBackendAvailability {
        AudioBackendAvailability::Available
    }

    fn runtime_report(&self) -> AudioBackendRuntimeReport {
        let audio_inputs = load_avfoundation_listing()
            .map(|listing| parse_audio_inputs(&listing))
            .unwrap_or_default();
        let preferred_input = capture::preferred_audio_input(&audio_inputs)
            .map(|input| input.label.clone())
            .or_else(native_preferred_input_label);

        AudioBackendRuntimeReport {
            summary: Some(audio_input_support_summary()),
            preferred_input_id: native_preferred_input_label(),
            preferred_input_label: preferred_input,
            preferred_system_id: native_preferred_output_label(),
            preferred_system_label: native_preferred_output_label(),
        }
    }
}

impl EncoderBackendFactory for FfmpegMacosEncoderBackend {
    fn descriptor(&self) -> EncoderBackendDescriptor {
        EncoderBackendDescriptor {
            id: "macos-ffmpeg-videotoolbox",
            label: "macOS ffmpeg / VideoToolbox",
            family: EncoderBackendFamily::FallbackFfmpeg,
        }
    }

    fn availability(&self) -> EncoderBackendAvailability {
        EncoderBackendAvailability::Available
    }

    fn runtime_report(&self) -> EncoderBackendRuntimeReport {
        EncoderBackendRuntimeReport {
            summary: Some(format!(
                "Current macOS output pipeline uses ffmpeg with preferred encoder `{}`.",
                encoder_label(&preferred_video_encoder())
            )),
            preferred_encoder_label: Some(encoder_label(&preferred_video_encoder())),
        }
    }
}

impl FfmpegMacosCapture {
    pub fn start(options: RecordingOptions) -> Result<Self, CaptureError> {
        if options.system_audio_enabled {
            return Err(CaptureError::BackendUnavailable(
                "System-audio mixing is not wired into the macOS backend yet.".to_string(),
            ));
        }

        let runtime_plan = build_runtime_plan(&options);
        let screen_input = resolve_screen_input_for_recording(&runtime_plan.capture)?;
        let video_device = screen_input.id.clone();
        let audio_device = discover_audio_device(
            &options.audio_input_id,
            options.mic_enabled,
            &runtime_plan.discovered_audio_inputs,
            &runtime_plan.audio,
        )?;
        let input = format!("{video_device}:{audio_device}");
        let width = runtime_plan.capture.output_width;
        let height = runtime_plan.capture.output_height;
        let fps = runtime_plan.capture.fps;
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

        command
            .arg("-i")
            .arg(input)
            .arg("-c:v")
            .arg(&runtime_plan.encoder.codec_name);

        if let Some(preset) = runtime_plan.encoder.codec_preset.as_deref() {
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
                encoder_label: runtime_plan.encoder.encoder_label,
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
        build_recording_artifact(&self.active_recording, finished_at)
    }
}

#[cfg(target_os = "macos")]
impl ScreenCaptureKitBridgeCapture {
    fn start(options: RecordingOptions) -> Result<Self, CaptureError> {
        let runtime_plan = build_runtime_plan(&options);
        let start_plan = &runtime_plan.capture;
        let content = SCShareableContent::create()
            .with_on_screen_windows_only(true)
            .with_exclude_desktop_windows(true)
            .get()
            .map_err(|error| CaptureError::BackendUnavailable(error.to_string()))?;
        let display =
            native_backend::resolve_native_display(&content, &start_plan.resolved_source_target_id)
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

        let width = start_plan.output_width as usize;
        let height = start_plan.output_height as usize;
        let crop_plan = build_bridge_crop_plan(&options, &display, start_plan);
        let output_width = crop_plan.as_ref().map(|plan| plan.width).unwrap_or(width);
        let output_height = crop_plan.as_ref().map(|plan| plan.height).unwrap_or(height);
        let fps = start_plan.fps;
        let started_at = SystemTime::now();
        let stderr_buffer = Arc::new(Mutex::new(String::new()));

        let mut command = ffmpeg_command();
        command
            .arg("-y")
            .arg("-f")
            .arg("rawvideo")
            .arg("-pixel_format")
            .arg("bgra")
            .arg("-video_size")
            .arg(format!("{}x{}", output_width, output_height))
            .arg("-framerate")
            .arg(fps.to_string())
            .arg("-i")
            .arg("pipe:0")
            .arg("-c:v")
            .arg(&runtime_plan.encoder.codec_name);

        if let Some(preset) = runtime_plan.encoder.codec_preset.as_deref() {
            command.arg("-preset").arg(preset);
        }

        command
            .arg("-pix_fmt")
            .arg("yuv420p")
            .arg("-an")
            .arg("-movflags")
            .arg("+faststart")
            .arg(options.output_path.as_os_str())
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());

        let mut child = command.spawn().map_err(|error| {
            CaptureError::SpawnFailed(capture::ffmpeg_launch_error_message(&error, "macOS"))
        })?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| CaptureError::SpawnFailed("failed to open ffmpeg stdin".to_string()))?;
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

        let (frame_tx, frame_rx) = mpsc::sync_channel::<Vec<u8>>(3);
        let writer_stdin = Arc::new(Mutex::new(Some(stdin)));
        let writer_stdin_for_thread = Arc::clone(&writer_stdin);
        thread::spawn(move || {
            while let Ok(frame) = frame_rx.recv() {
                let Ok(mut stdin_guard) = writer_stdin_for_thread.lock() else {
                    break;
                };
                let Some(stdin) = stdin_guard.as_mut() else {
                    break;
                };
                if stdin.write_all(&frame).and_then(|_| stdin.flush()).is_err() {
                    break;
                }
            }
            if let Ok(mut stdin_guard) = writer_stdin_for_thread.lock() {
                let _ = stdin_guard.take();
            }
        });

        let paused = Arc::new(AtomicBool::new(false));
        let paused_for_handler = Arc::clone(&paused);
        let crop_plan_for_handler = crop_plan.clone();
        let mut stream = SCStream::new(&filter, &config);
        let handler_registered = stream
            .add_output_handler(
                move |sample: screencapturekit::cm::CMSampleBuffer, _type| {
                    if paused_for_handler.load(Ordering::Relaxed) {
                        return;
                    }
                    if !matches!(sample.frame_status(), Some(SCFrameStatus::Complete)) {
                        return;
                    }
                    let Some(pixel_buffer) = sample.image_buffer() else {
                        return;
                    };
                    let Ok(guard) = pixel_buffer.lock(CVPixelBufferLockFlags::READ_ONLY) else {
                        return;
                    };
                    let expected_bytes_per_row = width.saturating_mul(4);
                    let frame = if guard.bytes_per_row() == expected_bytes_per_row {
                        guard.as_slice().to_vec()
                    } else {
                        let mut packed =
                            Vec::with_capacity(height.saturating_mul(expected_bytes_per_row));
                        for row_index in 0..height {
                            let Some(row) = guard.row(row_index) else {
                                return;
                            };
                            let copy_len = expected_bytes_per_row.min(row.len());
                            packed.extend_from_slice(&row[..copy_len]);
                        }
                        packed
                    };
                    let output_frame = if let Some(crop_plan) = crop_plan_for_handler.as_ref() {
                        crop_bgra_frame(&frame, width, crop_plan)
                    } else {
                        frame
                    };
                    let _ = frame_tx.try_send(output_frame);
                },
                SCStreamOutputType::Screen,
            )
            .is_some();

        if !handler_registered {
            return Err(CaptureError::BackendUnavailable(
                "ScreenCaptureKit bridge could not register a screen output handler.".to_string(),
            ));
        }

        stream
            .start_capture()
            .map_err(|error| CaptureError::SpawnFailed(error.to_string()))?;

        Ok(Self {
            active_recording: ActiveRecording {
                backend_name: "macOS ScreenCaptureKit / ffmpeg bridge".to_string(),
                encoder_label: runtime_plan.encoder.encoder_label,
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
                    start_plan
                        .resolved_native_target_label
                        .clone()
                        .unwrap_or_else(|| start_plan.resolved_source_target_id.clone())
                },
            },
            child,
            stream: Some(stream),
            stderr_buffer,
            finished_artifact: None,
            paused,
        })
    }

    fn build_artifact(&self, finished_at: SystemTime) -> Result<RecordingArtifact, CaptureError> {
        build_recording_artifact(&self.active_recording, finished_at)
    }
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
            native_backend::resolve_native_display(&content, &start_plan.resolved_source_target_id)
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
        let metadata = fs::metadata(&self.active_recording.output_path)
            .map_err(|error| CaptureError::OutputInspectionFailed(error.to_string()))?;
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

pub fn list_audio_inputs() -> Vec<AudioInputOption> {
    list_device_options().1
}

pub fn list_device_options() -> (Vec<CaptureTargetOption>, Vec<AudioInputOption>) {
    let Ok(listing) = load_avfoundation_listing() else {
        let mut default_input = default_audio_input();
        if let Some(preferred_input) = native_preferred_input_label() {
            default_input.description =
                format!("Use the macOS default input device: {preferred_input}.");
        }

        return (vec![full_desktop_target()], vec![default_input]);
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

    let mut default_input = default_audio_input();
    if let Some(preferred_input) = native_preferred_input_label() {
        default_input.description =
            format!("Use the macOS default input device: {preferred_input}.");
    }

    let mut inputs = vec![default_input];
    inputs.extend(parse_audio_inputs(&listing));

    (targets, inputs)
}

pub fn audio_input_support_summary() -> String {
    match load_avfoundation_listing().map(|listing| parse_audio_inputs(&listing)) {
        Ok(audio_inputs) if !audio_inputs.is_empty() => {
            let base = format!(
                "AVFoundation microphone discovery is ready. Found {} input{}.",
                audio_inputs.len(),
                if audio_inputs.len() == 1 { "" } else { "s" }
            );
            match native_audio_backend::runtime_summary() {
                Some(summary) => format!("{base} {summary}"),
                None => base,
            }
        }
        Ok(_) => "AVFoundation did not expose any microphone device yet.".to_string(),
        Err(error) => match native_audio_backend::runtime_summary() {
            Some(summary) => format!("AVFoundation microphone discovery failed. {summary} {error}"),
            None => format!("AVFoundation microphone discovery failed. {error}"),
        },
    }
}

pub fn custom_region_support_summary() -> (bool, String) {
    (
        true,
        if native_recording_output_is_supported() {
            "Custom region capture is available on macOS through the ScreenCaptureKit native lane."
                .to_string()
        } else {
            "Custom region capture is available on macOS, but older runtimes use the fallback AVFoundation or ScreenCaptureKit bridge path instead of direct native file output."
                .to_string()
        },
    )
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

fn native_preferred_output_label() -> Option<String> {
    native_audio_backend::preferred_output_device_name()
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

#[cfg(target_os = "macos")]
impl CaptureController for ScreenCaptureKitBridgeCapture {
    fn active_recording(&self) -> &ActiveRecording {
        &self.active_recording
    }

    fn pause(&mut self) -> Result<(), CaptureError> {
        self.paused.store(true, Ordering::Relaxed);
        Ok(())
    }

    fn resume(&mut self) -> Result<(), CaptureError> {
        self.paused.store(false, Ordering::Relaxed);
        Ok(())
    }

    fn stop(&mut self) -> Result<RecordingArtifact, CaptureError> {
        if let Some(artifact) = self.finished_artifact.clone() {
            return Ok(artifact);
        }

        if let Some(stream) = self.stream.take() {
            stream
                .stop_capture()
                .map_err(|error| CaptureError::StopFailed(error.to_string()))?;
            drop(stream);
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

        let artifact = self.build_artifact(SystemTime::now())?;
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

        if let Some(stream) = self.stream.take() {
            let _ = stream.stop_capture();
            drop(stream);
        }

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
    discovered_audio_inputs: &[AudioInputOption],
    audio_start_plan: &native_audio_backend::MacosAudioStartPlan,
) -> Result<String, CaptureError> {
    if !mic_enabled {
        return Ok("none".to_string());
    }

    if discovered_audio_inputs.is_empty() {
        return audio_start_plan
            .microphone_device_name
            .clone()
            .ok_or_else(|| {
                CaptureError::BackendUnavailable(
                    "ffmpeg did not expose any avfoundation microphone device".to_string(),
                )
            });
    }

    if selected_audio_input_id == DEFAULT_AUDIO_INPUT_ID {
        return audio_start_plan
            .microphone_device_name
            .clone()
            .or_else(|| resolve_audio_input_id(selected_audio_input_id, discovered_audio_inputs))
            .ok_or_else(|| {
                CaptureError::BackendUnavailable(
                    "ffmpeg did not expose any avfoundation microphone device".to_string(),
                )
            });
    }

    resolve_audio_input_id(selected_audio_input_id, discovered_audio_inputs).ok_or_else(|| {
        CaptureError::BackendUnavailable(format!(
            "the selected microphone input `{selected_audio_input_id}` is no longer available"
        ))
    })
}

fn load_avfoundation_listing() -> Result<String, CaptureError> {
    let cache = avfoundation_listing_cache();
    if let Some(cached) = cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .as_ref()
    {
        if cached.refreshed_at.elapsed() < AVFOUNDATION_LISTING_TTL {
            return Ok(cached.listing.clone());
        }
    }

    let output = Command::new("ffmpeg")
        .arg("-f")
        .arg("avfoundation")
        .arg("-list_devices")
        .arg("true")
        .arg("-i")
        .arg("")
        .output()
        .map_err(|error| CaptureError::BackendUnavailable(error.to_string()))?;

    let listing = String::from_utf8_lossy(&output.stderr).into_owned();
    let mut cache = cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *cache = Some(CachedAvfoundationListing {
        listing: listing.clone(),
        refreshed_at: Instant::now(),
    });

    Ok(listing)
}

fn avfoundation_listing_cache() -> &'static Mutex<Option<CachedAvfoundationListing>> {
    static CACHE: OnceLock<Mutex<Option<CachedAvfoundationListing>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
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
    start_plan: &native_backend::ScreenCaptureKitStartPlan,
) -> Result<ScreenInput, CaptureError> {
    resolve_screen_input(&start_plan.resolved_source_target_id)
}

fn build_runtime_plan(options: &RecordingOptions) -> MacosRecorderRuntimePlan {
    let discovered_audio_inputs = load_avfoundation_listing()
        .map(|listing| parse_audio_inputs(&listing))
        .unwrap_or_default();

    MacosRecorderRuntimePlan {
        capture: native_backend::start_plan(options),
        audio: native_audio_backend::start_plan(
            &options.audio_input_id,
            options.mic_enabled,
            options.system_audio_enabled,
            &discovered_audio_inputs,
        ),
        encoder: native_encoder_backend::output_plan(options),
        discovered_audio_inputs,
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
    let output = Command::new("sw_vers")
        .args(["-productVersion"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    parse_macos_version(String::from_utf8_lossy(&output.stdout).trim())
}

fn parse_macos_version(value: &str) -> Option<(u64, u64, u64)> {
    let mut parts = value.trim().split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().unwrap_or("0").parse().ok()?;
    let patch = parts.next().unwrap_or("0").parse().ok()?;
    Some((major, minor, patch))
}

fn build_recording_artifact(
    active_recording: &ActiveRecording,
    finished_at: SystemTime,
) -> Result<RecordingArtifact, CaptureError> {
    let metadata = fs::metadata(&active_recording.output_path)
        .map_err(|error| CaptureError::OutputInspectionFailed(error.to_string()))?;

    let duration = finished_at
        .duration_since(active_recording.started_at)
        .unwrap_or_default();

    Ok(RecordingArtifact {
        output_path: active_recording.output_path.clone(),
        started_at: active_recording.started_at,
        finished_at,
        duration,
        bytes_written: metadata.len(),
    })
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
    for _ in 0..STARTUP_POLL_ATTEMPTS {
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
        thread::sleep(STARTUP_POLL_INTERVAL);
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
        "native recording output did not materialize `{}` in time",
        path.display()
    )))
}

#[cfg(target_os = "macos")]
fn build_bridge_crop_plan(
    options: &RecordingOptions,
    display: &screencapturekit::shareable_content::SCDisplay,
    start_plan: &native_backend::ScreenCaptureKitStartPlan,
) -> Option<ScreenCaptureKitBridgeCropPlan> {
    if options.capture_target_id != CUSTOM_REGION_TARGET_ID {
        return None;
    }

    let source_scale = (options.region_source_scale_factor_milli.max(1) as f64) / 1000.0;
    let display_points_width = (display.width() as f64) / source_scale;
    let display_points_height = (display.height() as f64) / source_scale;
    if display_points_width <= 0.0 || display_points_height <= 0.0 {
        return None;
    }

    let origin_points_x =
        ((options.region_x as i32 - options.region_source_origin_x).max(0) as f64) / source_scale;
    let origin_points_y =
        ((options.region_y as i32 - options.region_source_origin_y).max(0) as f64) / source_scale;
    let region_points_width = ((options.region_width.max(1) as f64) / source_scale).max(1.0);
    let region_points_height = ((options.region_height.max(1) as f64) / source_scale).max(1.0);

    let scale_x = (start_plan.output_width as f64) / display_points_width;
    let scale_y = (start_plan.output_height as f64) / display_points_height;

    let x = (origin_points_x * scale_x).round().max(0.0) as usize;
    let y = (origin_points_y * scale_y).round().max(0.0) as usize;
    let max_width = start_plan.output_width as usize;
    let max_height = start_plan.output_height as usize;
    if x >= max_width || y >= max_height {
        return None;
    }

    let width = ((region_points_width * scale_x).round().max(1.0) as usize).min(max_width - x);
    let height = ((region_points_height * scale_y).round().max(1.0) as usize).min(max_height - y);

    Some(ScreenCaptureKitBridgeCropPlan {
        x,
        y,
        width,
        height,
    })
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

fn crop_bgra_frame(
    frame: &[u8],
    source_width: usize,
    crop_plan: &ScreenCaptureKitBridgeCropPlan,
) -> Vec<u8> {
    let bytes_per_pixel = 4;
    let source_row_bytes = source_width.saturating_mul(bytes_per_pixel);
    let crop_row_start = crop_plan.x.saturating_mul(bytes_per_pixel);
    let crop_row_end =
        crop_row_start.saturating_add(crop_plan.width.saturating_mul(bytes_per_pixel));
    let mut output = Vec::with_capacity(
        crop_plan
            .width
            .saturating_mul(crop_plan.height)
            .saturating_mul(bytes_per_pixel),
    );

    for row in crop_plan.y..crop_plan.y.saturating_add(crop_plan.height) {
        let row_start = row.saturating_mul(source_row_bytes);
        let row_end = row_start.saturating_add(source_row_bytes);
        if row_end > frame.len() {
            break;
        }
        let row_slice = &frame[row_start..row_end];
        if crop_row_end > row_slice.len() {
            break;
        }
        output.extend_from_slice(&row_slice[crop_row_start..crop_row_end]);
    }

    output
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

#[cfg(test)]
mod tests {
    use super::{
        ScreenCaptureKitBridgeCropPlan, ScreenCaptureKitRecordingOutputState,
        build_native_source_rect_from_dimensions, crop_bgra_frame,
        native_recording_output_support_note_for, parse_macos_version,
    };
    use capture::{CUSTOM_REGION_TARGET_ID, RecordingOptions};
    #[cfg(target_os = "macos")]
    use screencapturekit::cm::CMTime;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    #[test]
    fn crops_bgra_frame_rows_correctly() {
        let frame = vec![
            1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, // row 0, 3 pixels
            13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, // row 1
        ];
        let crop_plan = ScreenCaptureKitBridgeCropPlan {
            x: 1,
            y: 0,
            width: 2,
            height: 2,
        };

        let cropped = crop_bgra_frame(&frame, 3, &crop_plan);
        assert_eq!(
            cropped,
            vec![
                5, 6, 7, 8, 9, 10, 11, 12, //
                17, 18, 19, 20, 21, 22, 23, 24,
            ]
        );
    }

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
