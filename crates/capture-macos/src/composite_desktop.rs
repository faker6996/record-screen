#[cfg(target_os = "macos")]
use std::{
    alloc::{Layout, alloc_zeroed, dealloc},
    collections::VecDeque,
    ffi::c_void,
    fs,
    mem::offset_of,
    path::{Path, PathBuf},
    ptr::NonNull,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant, SystemTime},
};

#[cfg(target_os = "macos")]
use capture::{
    ActiveRecording, CaptureController, CaptureError, RecordingArtifact, RecordingOptions,
};
#[cfg(target_os = "macos")]
use objc2::runtime::AnyObject;
#[cfg(target_os = "macos")]
use objc2_av_foundation::{
    AVAssetWriter, AVAssetWriterInput, AVAssetWriterInputPixelBufferAdaptor, AVFileType,
    AVFileTypeMPEG4, AVFileTypeQuickTimeMovie, AVMediaTypeAudio, AVMediaTypeVideo, AVVideoCodecKey,
    AVVideoCodecTypeH264, AVVideoCodecTypeHEVC, AVVideoHeightKey, AVVideoWidthKey,
};
#[cfg(target_os = "macos")]
use objc2_avf_audio::{AVEncoderBitRateKey, AVFormatIDKey, AVNumberOfChannelsKey, AVSampleRateKey};
#[cfg(target_os = "macos")]
use objc2_core_audio_types::{
    AudioBuffer, AudioBufferList, AudioStreamBasicDescription, kAudioFormatFlagIsBigEndian,
    kAudioFormatFlagIsFloat, kAudioFormatFlagIsNonInterleaved, kAudioFormatFlagIsPacked,
    kAudioFormatFlagIsSignedInteger, kAudioFormatLinearPCM, kAudioFormatMPEG4AAC,
};
#[cfg(target_os = "macos")]
use objc2_core_foundation::CFRetained;
#[cfg(target_os = "macos")]
use objc2_core_media::{
    CMAudioFormatDescriptionCreate, CMFormatDescription, CMSampleBuffer as ObjcCMSampleBuffer,
    CMSampleTimingInfo as ObjcCMSampleTimingInfo, CMTime, CMTimeFlags,
    kCMSampleBufferFlag_AudioBufferList_Assure16ByteAlignment, kCMTimeZero,
};
#[cfg(target_os = "macos")]
use objc2_core_video::CVPixelBuffer as ObjcCVPixelBuffer;
#[cfg(target_os = "macos")]
use objc2_foundation::{NSDictionary, NSNumber, NSString, NSURL};
#[cfg(target_os = "macos")]
use screencapturekit::{
    cm::CMSampleBuffer,
    cv::{CVPixelBuffer, CVPixelBufferLockFlags},
    shareable_content::{SCDisplay, SCShareableContent},
    stream::{
        SCStream,
        configuration::{PixelFormat, SCStreamConfiguration},
        content_filter::SCContentFilter,
        output_type::SCStreamOutputType,
    },
};

#[cfg(target_os = "macos")]
const COMPOSITE_PIXEL_FORMAT_BGRA: u32 = 0x4247_5241;
#[cfg(target_os = "macos")]
const COMPOSITE_STARTUP_POLL_INTERVAL: Duration = Duration::from_millis(50);
#[cfg(target_os = "macos")]
const COMPOSITE_STARTUP_POLL_ATTEMPTS: usize = 40;
#[cfg(target_os = "macos")]
const MIXED_AUDIO_TARGET_SAMPLE_RATE: f64 = 48_000.0;
#[cfg(target_os = "macos")]
const MIXED_AUDIO_TARGET_CHANNEL_COUNT: u32 = 2;
#[cfg(target_os = "macos")]
const MIXED_AUDIO_TARGET_BITS_PER_CHANNEL: u32 = 32;
#[cfg(target_os = "macos")]
const MIXED_AUDIO_TARGET_BYTES_PER_FRAME: u32 = 8;
#[cfg(target_os = "macos")]
const MIXED_AUDIO_TARGET_FORMAT_FLAGS: u32 = kAudioFormatFlagIsFloat | kAudioFormatFlagIsPacked;

#[cfg(target_os = "macos")]
#[derive(Clone)]
struct CompositeDisplayPlan {
    display: SCDisplay,
    label: String,
    origin_x: usize,
    origin_y: usize,
    width: u32,
    height: u32,
}

#[cfg(target_os = "macos")]
#[derive(Default)]
struct CompositeFrameSlot {
    latest_buffer: Option<CVPixelBuffer>,
    observed_frames: usize,
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CompositeAudioKind {
    SystemAudio,
    Microphone,
    Mixed,
}

#[cfg(target_os = "macos")]
struct RetainedSampleBuffer {
    ptr: *mut std::ffi::c_void,
}

#[cfg(target_os = "macos")]
unsafe impl Send for RetainedSampleBuffer {}

#[cfg(target_os = "macos")]
unsafe impl Sync for RetainedSampleBuffer {}

#[cfg(target_os = "macos")]
impl RetainedSampleBuffer {
    fn from_sample(sample: &CMSampleBuffer) -> Self {
        let ptr = sample.as_ptr();
        unsafe {
            screencapturekit::cm::ffi::cm_sample_buffer_retain(ptr);
        }
        Self { ptr }
    }

    fn as_objc(&self) -> Result<&ObjcCMSampleBuffer, CaptureError> {
        unsafe { self.ptr.cast::<ObjcCMSampleBuffer>().as_ref() }.ok_or_else(|| {
            CaptureError::StopFailed(
                "failed to bridge a retained audio sample buffer into Core Media.".to_string(),
            )
        })
    }

    fn as_sample(&self) -> Result<CMSampleBuffer, CaptureError> {
        unsafe {
            screencapturekit::cm::ffi::cm_sample_buffer_retain(self.ptr);
        }
        CMSampleBuffer::from_raw(self.ptr).ok_or_else(|| {
            CaptureError::StopFailed(
                "failed to bridge a retained audio sample buffer into ScreenCaptureKit."
                    .to_string(),
            )
        })
    }
}

#[cfg(target_os = "macos")]
impl Drop for RetainedSampleBuffer {
    fn drop(&mut self) {
        unsafe {
            screencapturekit::cm::ffi::cm_sample_buffer_release(self.ptr);
        }
    }
}

#[cfg(target_os = "macos")]
impl CompositeAudioCapturePlan {
    fn captures_system_audio(&self) -> bool {
        matches!(
            self.kind,
            CompositeAudioKind::SystemAudio | CompositeAudioKind::Mixed
        )
    }

    fn captures_microphone(&self) -> bool {
        matches!(
            self.kind,
            CompositeAudioKind::Microphone | CompositeAudioKind::Mixed
        )
    }
}

#[cfg(target_os = "macos")]
impl OwnedAudioBufferList {
    fn new(buffer_channels: &[u32], payloads: Vec<Box<[u8]>>) -> Result<Self, CaptureError> {
        let buffer_count = payloads.len();
        if buffer_count == 0 || buffer_count != buffer_channels.len() {
            return Err(CaptureError::StopFailed(
                "mixed audio buffer list could not be constructed because its channel payloads were inconsistent."
                    .to_string(),
            ));
        }

        let offset = offset_of!(AudioBufferList, mBuffers);
        let layout = Layout::from_size_align(
            offset + std::mem::size_of::<AudioBuffer>() * buffer_count,
            std::mem::align_of::<AudioBufferList>(),
        )
        .map_err(|_| {
            CaptureError::StopFailed(
                "mixed audio buffer list could not allocate a valid memory layout.".to_string(),
            )
        })?;
        let raw = unsafe { alloc_zeroed(layout) };
        let ptr = NonNull::new(raw.cast::<AudioBufferList>()).ok_or_else(|| {
            CaptureError::StopFailed(
                "mixed audio buffer list allocation returned a null pointer.".to_string(),
            )
        })?;

        unsafe {
            (*ptr.as_ptr()).mNumberBuffers = buffer_count as u32;
            let buffers_ptr = raw.add(offset).cast::<AudioBuffer>();
            for (index, (payload, channel_count)) in
                payloads.iter().zip(buffer_channels.iter()).enumerate()
            {
                *buffers_ptr.add(index) = AudioBuffer {
                    mNumberChannels: *channel_count,
                    mDataByteSize: payload.len() as u32,
                    mData: payload.as_ptr().cast_mut().cast::<c_void>(),
                };
            }
        }

        Ok(Self {
            ptr,
            layout,
            _payloads: payloads,
        })
    }

    fn as_nonnull(&self) -> NonNull<AudioBufferList> {
        self.ptr
    }
}

#[cfg(target_os = "macos")]
impl Drop for OwnedAudioBufferList {
    fn drop(&mut self) {
        unsafe {
            dealloc(self.ptr.as_ptr().cast::<u8>(), self.layout);
        }
    }
}

#[cfg(target_os = "macos")]
impl MixedAudioSample {
    fn requeue(self, audio_writer_plan: &CompositeAudioWriterPlan) {
        match self {
            Self::Retained(sample) => {
                let mut queue_state = audio_writer_plan
                    .primary_queue_state
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                queue_state.pending_samples.push_front(sample);
            }
            Self::Mixed { primary, secondary } => {
                {
                    let mut queue_state = audio_writer_plan
                        .primary_queue_state
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    queue_state.pending_samples.push_front(primary);
                }
                if let Some(secondary) = secondary {
                    if let Some(secondary_queue_state) =
                        audio_writer_plan.secondary_queue_state.as_ref()
                    {
                        let mut queue_state = secondary_queue_state
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner());
                        queue_state.pending_samples.push_front(secondary);
                    }
                }
            }
        }
    }
}

#[cfg(target_os = "macos")]
#[derive(Default)]
struct CompositeAudioQueueState {
    pending_samples: VecDeque<RetainedSampleBuffer>,
    observed_samples: usize,
    sample_rate: Option<f64>,
    channel_count: Option<u32>,
    bits_per_channel: Option<u32>,
    bytes_per_frame: Option<u32>,
    format_flags: Option<u32>,
    buffer_count: Option<usize>,
}

#[cfg(target_os = "macos")]
struct CompositeAudioCapturePlan {
    kind: CompositeAudioKind,
    primary_queue_state: Arc<Mutex<CompositeAudioQueueState>>,
    secondary_queue_state: Option<Arc<Mutex<CompositeAudioQueueState>>>,
    microphone_device_id: Option<String>,
}

#[cfg(target_os = "macos")]
struct CompositeAudioWriterPlan {
    kind: CompositeAudioKind,
    primary_queue_state: Arc<Mutex<CompositeAudioQueueState>>,
    secondary_queue_state: Option<Arc<Mutex<CompositeAudioQueueState>>>,
    timestamp_origin: Arc<Mutex<Option<screencapturekit::cm::CMTime>>>,
    output_frame_cursor: Arc<Mutex<u64>>,
    sample_rate: f64,
    channel_count: u32,
    bits_per_channel: u32,
    _bytes_per_frame: u32,
    format_flags: u32,
}

#[cfg(target_os = "macos")]
#[derive(Default)]
struct CompositeAudioDrainStats {
    primary_dequeued_samples: usize,
    secondary_dequeued_samples: usize,
    appended_samples: usize,
    appended_frames: u64,
    silent_secondary_mixes: usize,
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, PartialEq)]
struct CompositeAudioFormatMetadata {
    sample_rate: f64,
    channel_count: u32,
    bits_per_channel: u32,
    bytes_per_frame: u32,
    format_flags: u32,
    buffer_count: usize,
}

#[cfg(target_os = "macos")]
struct DecodedAudioSample {
    timing: screencapturekit::cm::CMSampleTimingInfo,
    sample_rate: f64,
    channels: Vec<Vec<f32>>,
}

#[cfg(target_os = "macos")]
struct OwnedAudioBufferList {
    ptr: NonNull<AudioBufferList>,
    layout: Layout,
    _payloads: Vec<Box<[u8]>>,
}

#[cfg(target_os = "macos")]
enum MixedAudioSample {
    Retained(RetainedSampleBuffer),
    Mixed {
        primary: RetainedSampleBuffer,
        secondary: Option<RetainedSampleBuffer>,
    },
}

#[cfg(target_os = "macos")]
pub(crate) struct ScreenCaptureKitCompositeDesktopCapture {
    active_recording: ActiveRecording,
    streams: Vec<SCStream>,
    stop_flag: Arc<AtomicBool>,
    writer_handle: Option<JoinHandle<Result<RecordingArtifact, CaptureError>>>,
    finished_artifact: Option<RecordingArtifact>,
}

#[cfg(target_os = "macos")]
pub(crate) fn start_full_desktop_composite_capture(
    options: RecordingOptions,
) -> Result<Box<dyn CaptureController>, CaptureError> {
    ScreenCaptureKitCompositeDesktopCapture::start(options)
        .map(|capture| Box::new(capture) as Box<dyn CaptureController>)
}

#[cfg(target_os = "macos")]
impl ScreenCaptureKitCompositeDesktopCapture {
    fn start(options: RecordingOptions) -> Result<Self, CaptureError> {
        let runtime_plan = super::build_runtime_plan(&options);
        let content = SCShareableContent::create()
            .with_on_screen_windows_only(true)
            .with_exclude_desktop_windows(true)
            .get()
            .map_err(|error| CaptureError::BackendUnavailable(error.to_string()))?;
        let displays = content.displays();
        if displays.len() <= 1 {
            return Err(CaptureError::BackendUnavailable(
                "Native desktop composition on macOS only activates when more than one display is attached."
                    .to_string(),
            ));
        }

        let (display_plan, canvas_width, canvas_height) =
            build_display_plan(displays).map_err(CaptureError::BackendUnavailable)?;
        let display_count = display_plan.len();
        let audio_capture_plan = build_audio_capture_plan(&options, &runtime_plan.audio);
        let frame_slots = Arc::new(
            display_plan
                .iter()
                .map(|_| Mutex::new(CompositeFrameSlot::default()))
                .collect::<Vec<_>>(),
        );
        let stop_flag = Arc::new(AtomicBool::new(false));

        let mut streams = Vec::with_capacity(display_plan.len());
        for (display_index, display) in display_plan.iter().enumerate() {
            let filter = SCContentFilter::create()
                .with_display(&display.display)
                .with_excluding_windows(&[])
                .build();
            let is_audio_owner = display_index == 0;
            let captures_system_audio = is_audio_owner
                && audio_capture_plan
                    .as_ref()
                    .is_some_and(CompositeAudioCapturePlan::captures_system_audio);
            let captures_microphone = is_audio_owner
                && audio_capture_plan
                    .as_ref()
                    .is_some_and(CompositeAudioCapturePlan::captures_microphone);
            let mut config = SCStreamConfiguration::new()
                .with_width(display.width)
                .with_height(display.height)
                .with_fps(runtime_plan.capture.fps)
                .with_pixel_format(PixelFormat::BGRA)
                .with_shows_cursor(true)
                .with_captures_audio(captures_system_audio)
                .with_captures_microphone(captures_microphone);
            if captures_system_audio {
                config = config.with_sample_rate(48_000).with_channel_count(2);
            }
            if captures_microphone {
                if let Some(microphone_device_id) = audio_capture_plan
                    .as_ref()
                    .and_then(|plan| plan.microphone_device_id.as_deref())
                {
                    config = config.with_microphone_capture_device_id(microphone_device_id);
                }
            }
            let mut stream = SCStream::new(&filter, &config);
            let frame_slots_for_handler = Arc::clone(&frame_slots);
            if stream
                .add_output_handler(
                    move |sample: CMSampleBuffer, _type| {
                        if let Some(pixel_buffer) = sample.image_buffer() {
                            if let Some(slot) = frame_slots_for_handler.get(display_index) {
                                let mut slot =
                                    slot.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                                slot.latest_buffer = Some(pixel_buffer);
                                slot.observed_frames = slot.observed_frames.saturating_add(1);
                            }
                        }
                    },
                    SCStreamOutputType::Screen,
                )
                .is_none()
            {
                let _ = stop_streams(&mut streams);
                return Err(CaptureError::SpawnFailed(format!(
                    "ScreenCaptureKit composite desktop capture could not register a screen handler for {}.",
                    display.label
                )));
            }

            if is_audio_owner {
                if let Some(audio_capture_plan) = audio_capture_plan.as_ref() {
                    if captures_system_audio {
                        let queue_state = if captures_microphone {
                            audio_capture_plan
                                .secondary_queue_state
                                .as_ref()
                                .cloned()
                                .ok_or_else(|| {
                                    CaptureError::SpawnFailed(
                                        "ScreenCaptureKit composite desktop capture lost its secondary system-audio queue."
                                            .to_string(),
                                    )
                                })?
                        } else {
                            Arc::clone(&audio_capture_plan.primary_queue_state)
                        };
                        if stream
                            .add_output_handler(
                                move |sample: CMSampleBuffer, _type| {
                                    record_audio_sample(&queue_state, &sample);
                                },
                                SCStreamOutputType::Audio,
                            )
                            .is_none()
                        {
                            let _ = stop_streams(&mut streams);
                            return Err(CaptureError::SpawnFailed(format!(
                                "ScreenCaptureKit composite desktop capture could not register a system-audio handler for {}.",
                                display.label
                            )));
                        }
                    }

                    if captures_microphone {
                        let queue_state = Arc::clone(&audio_capture_plan.primary_queue_state);
                        if stream
                            .add_output_handler(
                                move |sample: CMSampleBuffer, _type| {
                                    record_audio_sample(&queue_state, &sample);
                                },
                                SCStreamOutputType::Microphone,
                            )
                            .is_none()
                        {
                            let _ = stop_streams(&mut streams);
                            return Err(CaptureError::SpawnFailed(format!(
                                "ScreenCaptureKit composite desktop capture could not register a microphone handler for {}.",
                                display.label
                            )));
                        }
                    }
                }
            }

            if let Err(error) = stream.start_capture() {
                let _ = stop_streams(&mut streams);
                return Err(CaptureError::SpawnFailed(error.to_string()));
            }
            streams.push(stream);
        }

        if let Err(error) = wait_for_first_composite_frame(&frame_slots) {
            let _ = stop_streams(&mut streams);
            return Err(error);
        }
        let audio_writer_plan = wait_for_audio_writer_plan(audio_capture_plan.as_ref())?;

        let output_path = options.output_path.clone();
        let fps = runtime_plan.capture.fps;
        let codec_name = runtime_plan.encoder.codec_name.clone();
        let stop_flag_for_worker = Arc::clone(&stop_flag);
        let frame_slots_for_worker = Arc::clone(&frame_slots);
        let display_plan_for_worker = display_plan.clone();
        let writer_handle = thread::spawn(move || {
            run_composite_writer(
                output_path,
                display_plan_for_worker,
                frame_slots_for_worker,
                stop_flag_for_worker,
                canvas_width,
                canvas_height,
                fps,
                &codec_name,
                audio_writer_plan,
            )
        });

        Ok(Self {
            active_recording: ActiveRecording {
                backend_name: "macOS ScreenCaptureKit / desktop composite".to_string(),
                encoder_label: runtime_plan.encoder.encoder_label,
                output_path: options.output_path,
                started_at: SystemTime::now(),
                target_label: format!("Full desktop · {} displays", display_count),
            },
            streams,
            stop_flag,
            writer_handle: Some(writer_handle),
            finished_artifact: None,
        })
    }
}

#[cfg(target_os = "macos")]
impl CaptureController for ScreenCaptureKitCompositeDesktopCapture {
    fn active_recording(&self) -> &ActiveRecording {
        &self.active_recording
    }

    fn supports_pause_resume(&self) -> bool {
        false
    }

    fn pause_resume_note(&self) -> Option<String> {
        Some(
            "Pause/resume is not available for the macOS multi-display desktop-composite lane yet."
                .to_string(),
        )
    }

    fn pause(&mut self) -> Result<(), CaptureError> {
        Err(CaptureError::SignalFailed(
            "Pause/resume is not wired into the macOS multi-display desktop-composite lane yet."
                .to_string(),
        ))
    }

    fn resume(&mut self) -> Result<(), CaptureError> {
        Err(CaptureError::SignalFailed(
            "Pause/resume is not wired into the macOS multi-display desktop-composite lane yet."
                .to_string(),
        ))
    }

    fn stop(&mut self) -> Result<RecordingArtifact, CaptureError> {
        if let Some(artifact) = self.finished_artifact.clone() {
            return Ok(artifact);
        }

        self.stop_flag.store(true, Ordering::Relaxed);
        let stop_streams_result = stop_streams(&mut self.streams);
        let handle = self.writer_handle.take().ok_or_else(|| {
            CaptureError::StopFailed(
                "macOS desktop-composite writer worker was not available during stop.".to_string(),
            )
        })?;
        let artifact = handle.join().map_err(|_| {
            CaptureError::StopFailed(
                "macOS desktop-composite writer worker panicked during stop.".to_string(),
            )
        })??;
        stop_streams_result?;
        self.finished_artifact = Some(artifact.clone());
        Ok(artifact)
    }
}

#[cfg(target_os = "macos")]
fn build_display_plan(
    displays: Vec<SCDisplay>,
) -> Result<(Vec<CompositeDisplayPlan>, u32, u32), String> {
    if displays.is_empty() {
        return Err(
            "ScreenCaptureKit did not expose any displays for desktop composition.".to_string(),
        );
    }

    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut raw_displays = Vec::with_capacity(displays.len());
    for display in displays {
        let frame = display.frame();
        min_x = min_x.min(frame.x);
        min_y = min_y.min(frame.y);
        raw_displays.push((display, frame));
    }

    let mut plan = Vec::with_capacity(raw_displays.len());
    let mut max_x = 0usize;
    let mut max_y = 0usize;
    for (display, frame) in raw_displays {
        let width = frame.width.round().max(64.0) as u32;
        let height = frame.height.round().max(64.0) as u32;
        let origin_x = (frame.x - min_x).round().max(0.0) as usize;
        let origin_y = (frame.y - min_y).round().max(0.0) as usize;
        max_x = max_x.max(origin_x.saturating_add(width as usize));
        max_y = max_y.max(origin_y.saturating_add(height as usize));

        plan.push(CompositeDisplayPlan {
            label: format!("Display {}", display.display_id()),
            display,
            origin_x,
            origin_y,
            width,
            height,
        });
    }

    if max_x == 0 || max_y == 0 {
        return Err(
            "Desktop composition could not resolve a non-empty canvas for the attached displays."
                .to_string(),
        );
    }

    Ok((plan, max_x as u32, max_y as u32))
}

#[cfg(target_os = "macos")]
fn build_audio_capture_plan(
    options: &RecordingOptions,
    audio_start_plan: &super::native_audio_backend::MacosAudioStartPlan,
) -> Option<CompositeAudioCapturePlan> {
    let kind = match (options.system_audio_enabled, options.mic_enabled) {
        (true, true) => CompositeAudioKind::Mixed,
        (true, false) => CompositeAudioKind::SystemAudio,
        (false, true) => CompositeAudioKind::Microphone,
        (false, false) => return None,
    };

    let primary_queue_state = Arc::new(Mutex::new(CompositeAudioQueueState::default()));
    let secondary_queue_state = if kind == CompositeAudioKind::Mixed {
        Some(Arc::new(Mutex::new(CompositeAudioQueueState::default())))
    } else {
        None
    };

    Some(CompositeAudioCapturePlan {
        kind,
        primary_queue_state,
        secondary_queue_state,
        microphone_device_id: audio_start_plan.microphone_device_id.clone(),
    })
}

#[cfg(target_os = "macos")]
fn wait_for_audio_writer_plan(
    audio_capture_plan: Option<&CompositeAudioCapturePlan>,
) -> Result<Option<CompositeAudioWriterPlan>, CaptureError> {
    let Some(audio_capture_plan) = audio_capture_plan else {
        return Ok(None);
    };

    for _ in 0..COMPOSITE_STARTUP_POLL_ATTEMPTS {
        let primary_metadata =
            current_audio_format_metadata(&audio_capture_plan.primary_queue_state);
        let secondary_metadata = audio_capture_plan
            .secondary_queue_state
            .as_ref()
            .and_then(current_audio_format_metadata);
        match audio_capture_plan.kind {
            CompositeAudioKind::SystemAudio | CompositeAudioKind::Microphone => {
                if let Some(primary_metadata) = primary_metadata {
                    return Ok(Some(CompositeAudioWriterPlan {
                        kind: audio_capture_plan.kind,
                        primary_queue_state: Arc::clone(&audio_capture_plan.primary_queue_state),
                        secondary_queue_state: None,
                        timestamp_origin: Arc::new(Mutex::new(None)),
                        output_frame_cursor: Arc::new(Mutex::new(0)),
                        sample_rate: primary_metadata.sample_rate,
                        channel_count: primary_metadata.channel_count.max(1),
                        bits_per_channel: primary_metadata.bits_per_channel,
                        _bytes_per_frame: primary_metadata.bytes_per_frame,
                        format_flags: primary_metadata.format_flags,
                    }));
                }
            }
            CompositeAudioKind::Mixed => {
                if let (Some(primary_metadata), Some(secondary_metadata)) =
                    (primary_metadata, secondary_metadata)
                {
                    ensure_dual_audio_formats_supported(primary_metadata, secondary_metadata)?;
                    return Ok(Some(CompositeAudioWriterPlan {
                        kind: CompositeAudioKind::Mixed,
                        primary_queue_state: Arc::clone(&audio_capture_plan.primary_queue_state),
                        secondary_queue_state: audio_capture_plan.secondary_queue_state.clone(),
                        timestamp_origin: Arc::new(Mutex::new(None)),
                        output_frame_cursor: Arc::new(Mutex::new(0)),
                        sample_rate: MIXED_AUDIO_TARGET_SAMPLE_RATE,
                        channel_count: MIXED_AUDIO_TARGET_CHANNEL_COUNT,
                        bits_per_channel: MIXED_AUDIO_TARGET_BITS_PER_CHANNEL,
                        _bytes_per_frame: MIXED_AUDIO_TARGET_BYTES_PER_FRAME,
                        format_flags: MIXED_AUDIO_TARGET_FORMAT_FLAGS,
                    }));
                }
            }
        }
        thread::sleep(COMPOSITE_STARTUP_POLL_INTERVAL);
    }

    Err(CaptureError::SpawnFailed(format!(
        "macOS desktop-composite capture did not receive its first {} sample in time.",
        match audio_capture_plan.kind {
            CompositeAudioKind::SystemAudio => "system-audio",
            CompositeAudioKind::Microphone => "microphone",
            CompositeAudioKind::Mixed => "microphone and system-audio",
        }
    )))
}

#[cfg(target_os = "macos")]
fn record_audio_sample(
    queue_state: &Arc<Mutex<CompositeAudioQueueState>>,
    sample: &CMSampleBuffer,
) {
    if let Some(format_description) = sample.format_description() {
        let mut queue_state = queue_state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if queue_state.sample_rate.is_none() {
            queue_state.sample_rate = format_description.audio_sample_rate();
        }
        if queue_state.channel_count.is_none() {
            queue_state.channel_count = format_description.audio_channel_count();
        }
        if queue_state.bits_per_channel.is_none() {
            queue_state.bits_per_channel = format_description.audio_bits_per_channel();
        }
        if queue_state.bytes_per_frame.is_none() {
            queue_state.bytes_per_frame = format_description.audio_bytes_per_frame();
        }
        if queue_state.format_flags.is_none() {
            queue_state.format_flags = format_description.audio_format_flags();
        }
        if queue_state.buffer_count.is_none() {
            queue_state.buffer_count = sample.audio_buffer_list().map(|list| list.num_buffers());
        }
        queue_state
            .pending_samples
            .push_back(RetainedSampleBuffer::from_sample(sample));
        queue_state.observed_samples = queue_state.observed_samples.saturating_add(1);
        while queue_state.pending_samples.len() > 512 {
            queue_state.pending_samples.pop_front();
        }
    }
}

#[cfg(target_os = "macos")]
fn current_audio_format_metadata(
    queue_state: &Arc<Mutex<CompositeAudioQueueState>>,
) -> Option<CompositeAudioFormatMetadata> {
    let queue_state = queue_state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if queue_state.observed_samples == 0 {
        return None;
    }

    Some(CompositeAudioFormatMetadata {
        sample_rate: queue_state.sample_rate.unwrap_or(48_000.0),
        channel_count: queue_state.channel_count.unwrap_or(2).max(1),
        bits_per_channel: queue_state.bits_per_channel.unwrap_or(32).max(1),
        bytes_per_frame: queue_state.bytes_per_frame.unwrap_or(4).max(1),
        format_flags: queue_state.format_flags.unwrap_or(kAudioFormatFlagIsFloat),
        buffer_count: queue_state.buffer_count.unwrap_or(1).max(1),
    })
}

#[cfg(target_os = "macos")]
fn ensure_dual_audio_formats_supported(
    primary: CompositeAudioFormatMetadata,
    secondary: CompositeAudioFormatMetadata,
) -> Result<(), CaptureError> {
    ensure_audio_format_supported(primary)?;
    ensure_audio_format_supported(secondary)?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn ensure_audio_format_supported(
    metadata: CompositeAudioFormatMetadata,
) -> Result<(), CaptureError> {
    if metadata.format_flags & kAudioFormatFlagIsBigEndian != 0 {
        return Err(CaptureError::BackendUnavailable(
            "Full desktop across multiple macOS displays can only combine microphone and system audio for little-endian PCM audio on the native composite lane right now."
                .to_string(),
        ));
    }

    let is_float32 =
        metadata.format_flags & kAudioFormatFlagIsFloat != 0 && metadata.bits_per_channel == 32;
    let is_i16 = metadata.format_flags & kAudioFormatFlagIsSignedInteger != 0
        && metadata.bits_per_channel == 16;
    if is_float32 || is_i16 {
        return Ok(());
    }

    Err(CaptureError::BackendUnavailable(
        "Full desktop across multiple macOS displays can only combine microphone and system audio for 32-bit float PCM or 16-bit signed PCM sources right now."
            .to_string(),
    ))
}

#[cfg(target_os = "macos")]
fn wait_for_first_composite_frame(
    frame_slots: &[Mutex<CompositeFrameSlot>],
) -> Result<(), CaptureError> {
    for _ in 0..COMPOSITE_STARTUP_POLL_ATTEMPTS {
        if all_display_slots_ready(frame_slots) {
            return Ok(());
        }
        thread::sleep(COMPOSITE_STARTUP_POLL_INTERVAL);
    }

    Err(CaptureError::SpawnFailed(
        "macOS desktop-composite capture did not receive an initial frame for every attached display in time."
            .to_string(),
    ))
}

#[cfg(target_os = "macos")]
fn stop_streams(streams: &mut Vec<SCStream>) -> Result<(), CaptureError> {
    let mut errors = Vec::new();
    for stream in streams.drain(..) {
        if let Err(error) = stream.stop_capture() {
            errors.push(error.to_string());
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(CaptureError::StopFailed(format!(
            "failed to stop macOS desktop-composite streams cleanly: {}",
            errors.join("; ")
        )))
    }
}

#[cfg(target_os = "macos")]
fn run_composite_writer(
    output_path: PathBuf,
    display_plan: Vec<CompositeDisplayPlan>,
    frame_slots: Arc<Vec<Mutex<CompositeFrameSlot>>>,
    stop_flag: Arc<AtomicBool>,
    canvas_width: u32,
    canvas_height: u32,
    fps: u32,
    codec_name: &str,
    audio_writer_plan: Option<CompositeAudioWriterPlan>,
) -> Result<RecordingArtifact, CaptureError> {
    if output_path.is_file() {
        fs::remove_file(&output_path).map_err(|error| {
            CaptureError::SpawnFailed(format!(
                "failed to clear existing composite output `{}`: {error}",
                output_path.display()
            ))
        })?;
    }

    let started_at = SystemTime::now();
    let (writer, writer_input, adaptor, audio_input) = build_writer(
        &output_path,
        canvas_width,
        canvas_height,
        codec_name,
        audio_writer_plan.as_ref(),
    )?;
    let frame_interval = Duration::from_nanos(1_000_000_000u64 / u64::from(fps.max(1)));
    let mut frame_index: i64 = 0;
    let mut next_frame_at = Instant::now();
    let mut audio_drain_stats = CompositeAudioDrainStats::default();

    loop {
        let composite_frame =
            compose_desktop_frame(&display_plan, &frame_slots, canvas_width, canvas_height)?;
        if !wait_until_ready_for_more_media_data(&writer_input, &stop_flag)? {
            break;
        }
        let presentation_time = unsafe { CMTime::new(frame_index, fps.max(1) as i32) };
        let composite_frame_ref = composite_frame_ref(&composite_frame)?;
        let append_ok = unsafe {
            adaptor.appendPixelBuffer_withPresentationTime(composite_frame_ref, presentation_time)
        };
        if !append_ok {
            return Err(CaptureError::StopFailed(writer_error(
                &writer,
                "failed to append a composed desktop frame to AVAssetWriter",
            )));
        }
        drain_audio_samples(
            audio_input.as_deref(),
            audio_writer_plan.as_ref(),
            &writer,
            false,
            &mut audio_drain_stats,
        )?;

        frame_index = frame_index.saturating_add(1);
        if stop_flag.load(Ordering::Relaxed) {
            break;
        }

        next_frame_at += frame_interval;
        let sleep_for = next_frame_at.saturating_duration_since(Instant::now());
        if !sleep_for.is_zero() {
            thread::sleep(sleep_for);
        } else {
            next_frame_at = Instant::now();
        }
    }

    drain_audio_samples(
        audio_input.as_deref(),
        audio_writer_plan.as_ref(),
        &writer,
        true,
        &mut audio_drain_stats,
    )?;
    log_composite_audio_drain_stats(audio_writer_plan.as_ref(), &audio_drain_stats);
    unsafe {
        writer_input.markAsFinished();
    }
    if let Some(audio_input) = audio_input.as_ref() {
        unsafe {
            audio_input.markAsFinished();
        }
    }
    #[allow(deprecated)]
    if !unsafe { writer.finishWriting() } {
        return Err(CaptureError::StopFailed(writer_error(
            &writer,
            "failed to finish AVAssetWriter for macOS desktop-composite recording",
        )));
    }

    let finished_at = SystemTime::now();
    let metadata = fs::metadata(&output_path).map_err(|error| {
        CaptureError::OutputInspectionFailed(super::describe_output_path_error(
            &output_path,
            &error,
        ))
    })?;

    Ok(RecordingArtifact {
        output_path,
        started_at,
        finished_at,
        duration: duration_for_frame_count(frame_index.max(0) as u64, fps.max(1)),
        bytes_written: metadata.len(),
    })
}

#[cfg(target_os = "macos")]
fn build_writer(
    output_path: &Path,
    canvas_width: u32,
    canvas_height: u32,
    codec_name: &str,
    audio_writer_plan: Option<&CompositeAudioWriterPlan>,
) -> Result<
    (
        objc2::rc::Retained<AVAssetWriter>,
        objc2::rc::Retained<AVAssetWriterInput>,
        objc2::rc::Retained<AVAssetWriterInputPixelBufferAdaptor>,
        Option<objc2::rc::Retained<AVAssetWriterInput>>,
    ),
    CaptureError,
> {
    let output_url = NSURL::from_file_path(output_path).ok_or_else(|| {
        CaptureError::SpawnFailed(format!(
            "failed to convert `{}` into a native file URL for AVAssetWriter.",
            output_path.display()
        ))
    })?;
    let file_type = writer_file_type(output_path)?;
    let writer =
        unsafe { AVAssetWriter::assetWriterWithURL_fileType_error(&output_url, file_type) }
            .map_err(|error| {
                CaptureError::SpawnFailed(format!(
                    "failed to create AVAssetWriter for `{}`: {error}",
                    output_path.display()
                ))
            })?;

    let codec_key = unsafe { AVVideoCodecKey }.ok_or_else(|| {
        CaptureError::SpawnFailed("AVFoundation did not expose AVVideoCodecKey.".to_string())
    })?;
    let width_key = unsafe { AVVideoWidthKey }.ok_or_else(|| {
        CaptureError::SpawnFailed("AVFoundation did not expose AVVideoWidthKey.".to_string())
    })?;
    let height_key = unsafe { AVVideoHeightKey }.ok_or_else(|| {
        CaptureError::SpawnFailed("AVFoundation did not expose AVVideoHeightKey.".to_string())
    })?;
    let codec_value = writer_codec_type(codec_name)?;
    let width_value = NSNumber::new_u32(canvas_width);
    let height_value = NSNumber::new_u32(canvas_height);
    let codec_obj: &AnyObject = codec_value;
    let width_obj: &AnyObject = &width_value;
    let height_obj: &AnyObject = &height_value;
    let video_settings = NSDictionary::from_slices(
        &[codec_key, width_key, height_key],
        &[codec_obj, width_obj, height_obj],
    );

    let media_type = unsafe { AVMediaTypeVideo }.ok_or_else(|| {
        CaptureError::SpawnFailed("AVFoundation did not expose AVMediaTypeVideo.".to_string())
    })?;
    let writer_input = unsafe {
        AVAssetWriterInput::assetWriterInputWithMediaType_outputSettings(
            media_type,
            Some(&video_settings),
        )
    };
    unsafe {
        writer_input.setExpectsMediaDataInRealTime(true);
    }
    if !unsafe { writer.canAddInput(&writer_input) } {
        return Err(CaptureError::SpawnFailed(
            "AVAssetWriter refused the desktop-composite video input.".to_string(),
        ));
    }
    unsafe {
        writer.addInput(&writer_input);
    }

    let audio_input = if let Some(audio_writer_plan) = audio_writer_plan {
        Some(build_audio_writer_input(&writer, audio_writer_plan)?)
    } else {
        None
    };

    let adaptor = unsafe {
        AVAssetWriterInputPixelBufferAdaptor::assetWriterInputPixelBufferAdaptorWithAssetWriterInput_sourcePixelBufferAttributes(
            &writer_input,
            None,
        )
    };
    if !unsafe { writer.startWriting() } {
        return Err(CaptureError::SpawnFailed(writer_error(
            &writer,
            "failed to start AVAssetWriter for macOS desktop-composite recording",
        )));
    }
    unsafe {
        writer.startSessionAtSourceTime(kCMTimeZero);
    }

    Ok((writer, writer_input, adaptor, audio_input))
}

#[cfg(target_os = "macos")]
fn writer_codec_type(codec_name: &str) -> Result<&'static NSString, CaptureError> {
    let wants_hevc = codec_name.to_ascii_lowercase().contains("hevc");
    if wants_hevc {
        if let Some(codec) = unsafe { AVVideoCodecTypeHEVC } {
            return Ok(codec);
        }
    }

    unsafe { AVVideoCodecTypeH264 }
        .map(|codec| codec as &NSString)
        .ok_or_else(|| {
            CaptureError::SpawnFailed(
                "AVFoundation did not expose a supported H.264 video codec constant.".to_string(),
            )
        })
}

#[cfg(target_os = "macos")]
fn writer_file_type(output_path: &Path) -> Result<&'static AVFileType, CaptureError> {
    match output_path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("mov") => unsafe { AVFileTypeQuickTimeMovie }
            .or(unsafe { AVFileTypeMPEG4 })
            .ok_or_else(|| {
                CaptureError::SpawnFailed(
                    "AVFoundation did not expose a file type constant for QuickTime output."
                        .to_string(),
                )
            }),
        _ => unsafe { AVFileTypeMPEG4 }
            .or(unsafe { AVFileTypeQuickTimeMovie })
            .ok_or_else(|| {
                CaptureError::SpawnFailed(
                    "AVFoundation did not expose a file type constant for MPEG-4 output."
                        .to_string(),
                )
            }),
    }
}

#[cfg(target_os = "macos")]
fn build_audio_writer_input(
    writer: &AVAssetWriter,
    audio_writer_plan: &CompositeAudioWriterPlan,
) -> Result<objc2::rc::Retained<AVAssetWriterInput>, CaptureError> {
    let media_type = unsafe { AVMediaTypeAudio }.ok_or_else(|| {
        CaptureError::SpawnFailed("AVFoundation did not expose AVMediaTypeAudio.".to_string())
    })?;
    let format_id_key = unsafe { AVFormatIDKey }.ok_or_else(|| {
        CaptureError::SpawnFailed("AVFoundation did not expose AVFormatIDKey.".to_string())
    })?;
    let sample_rate_key = unsafe { AVSampleRateKey }.ok_or_else(|| {
        CaptureError::SpawnFailed("AVFoundation did not expose AVSampleRateKey.".to_string())
    })?;
    let channel_count_key = unsafe { AVNumberOfChannelsKey }.ok_or_else(|| {
        CaptureError::SpawnFailed("AVFoundation did not expose AVNumberOfChannelsKey.".to_string())
    })?;
    let bit_rate_key = unsafe { AVEncoderBitRateKey }.ok_or_else(|| {
        CaptureError::SpawnFailed("AVFoundation did not expose AVEncoderBitRateKey.".to_string())
    })?;

    let format_id_value = NSNumber::new_u32(kAudioFormatMPEG4AAC);
    let sample_rate_value = NSNumber::new_f64(audio_writer_plan.sample_rate);
    let channel_count_value = NSNumber::new_u32(audio_writer_plan.channel_count.max(1));
    let bit_rate_value = NSNumber::new_u32(128_000);
    let format_id_obj: &AnyObject = &format_id_value;
    let sample_rate_obj: &AnyObject = &sample_rate_value;
    let channel_count_obj: &AnyObject = &channel_count_value;
    let bit_rate_obj: &AnyObject = &bit_rate_value;
    let audio_settings = NSDictionary::from_slices(
        &[
            format_id_key,
            sample_rate_key,
            channel_count_key,
            bit_rate_key,
        ],
        &[
            format_id_obj,
            sample_rate_obj,
            channel_count_obj,
            bit_rate_obj,
        ],
    );

    let source_format_hint = current_audio_format_hint(audio_writer_plan)?;
    let audio_input = unsafe {
        AVAssetWriterInput::assetWriterInputWithMediaType_outputSettings_sourceFormatHint(
            media_type,
            Some(&audio_settings),
            source_format_hint.as_deref(),
        )
    };
    unsafe {
        audio_input.setExpectsMediaDataInRealTime(true);
    }
    if !unsafe { writer.canAddInput(&audio_input) } {
        return Err(CaptureError::SpawnFailed(
            "AVAssetWriter refused the desktop-composite audio input.".to_string(),
        ));
    }
    unsafe {
        writer.addInput(&audio_input);
    }

    Ok(audio_input)
}

#[cfg(target_os = "macos")]
fn current_audio_format_hint(
    audio_writer_plan: &CompositeAudioWriterPlan,
) -> Result<Option<objc2_core_foundation::CFRetained<CMFormatDescription>>, CaptureError> {
    if audio_writer_plan.kind == CompositeAudioKind::Mixed {
        return Ok(Some(build_mixed_audio_format_description(
            audio_writer_plan,
        )?));
    }

    let queue_state = audio_writer_plan
        .primary_queue_state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(sample) = queue_state.pending_samples.front() else {
        return Ok(None);
    };
    let sample_ref = sample.as_objc()?;
    Ok(unsafe { sample_ref.format_description() })
}

#[cfg(target_os = "macos")]
fn build_mixed_audio_format_description(
    audio_writer_plan: &CompositeAudioWriterPlan,
) -> Result<objc2_core_foundation::CFRetained<CMFormatDescription>, CaptureError> {
    let mut asbd = AudioStreamBasicDescription {
        mSampleRate: audio_writer_plan.sample_rate,
        mFormatID: kAudioFormatLinearPCM,
        mFormatFlags: audio_writer_plan.format_flags,
        mBytesPerPacket: audio_writer_plan._bytes_per_frame,
        mFramesPerPacket: 1,
        mBytesPerFrame: audio_writer_plan._bytes_per_frame,
        mChannelsPerFrame: audio_writer_plan.channel_count,
        mBitsPerChannel: audio_writer_plan.bits_per_channel,
        mReserved: 0,
    };
    let mut format_description_ptr: *const CMFormatDescription = std::ptr::null();
    let status = unsafe {
        CMAudioFormatDescriptionCreate(
            None,
            NonNull::from(&mut asbd),
            0,
            std::ptr::null(),
            0,
            std::ptr::null(),
            None,
            NonNull::new(&mut format_description_ptr).expect("audio format description output"),
        )
    };
    if status != 0 {
        return Err(CaptureError::SpawnFailed(format!(
            "macOS desktop-composite mixed audio could not create a Core Media format description (status={status})."
        )));
    }

    unsafe {
        Ok(CFRetained::from_raw(NonNull::new(format_description_ptr.cast_mut()).ok_or_else(
            || {
                CaptureError::SpawnFailed(
                    "macOS desktop-composite mixed audio format description creation returned a null pointer."
                        .to_string(),
                )
            },
        )?))
    }
}

#[cfg(target_os = "macos")]
fn wait_until_ready_for_more_media_data(
    writer_input: &AVAssetWriterInput,
    stop_flag: &AtomicBool,
) -> Result<bool, CaptureError> {
    for _ in 0..200 {
        if unsafe { writer_input.isReadyForMoreMediaData() } {
            return Ok(true);
        }
        if stop_flag.load(Ordering::Relaxed) {
            return Ok(false);
        }
        thread::sleep(Duration::from_millis(5));
    }

    if stop_flag.load(Ordering::Relaxed) {
        return Ok(false);
    }

    Err(CaptureError::StopFailed(
        "AVAssetWriter input never became ready for another composed desktop frame.".to_string(),
    ))
}

#[cfg(target_os = "macos")]
fn drain_audio_samples(
    audio_input: Option<&AVAssetWriterInput>,
    audio_writer_plan: Option<&CompositeAudioWriterPlan>,
    writer: &AVAssetWriter,
    final_drain: bool,
    audio_drain_stats: &mut CompositeAudioDrainStats,
) -> Result<(), CaptureError> {
    let Some(audio_input) = audio_input else {
        return Ok(());
    };
    let Some(audio_writer_plan) = audio_writer_plan else {
        return Ok(());
    };

    loop {
        let next_primary_sample = {
            let mut queue_state = audio_writer_plan
                .primary_queue_state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            queue_state.pending_samples.pop_front()
        };
        let next_audio_sample = if audio_writer_plan.kind == CompositeAudioKind::Mixed {
            let secondary_queue_state = audio_writer_plan
                .secondary_queue_state
                .as_ref()
                .ok_or_else(|| {
                    CaptureError::StopFailed(
                    "macOS desktop-composite mixed-audio writer lost its secondary audio queue."
                        .to_string(),
                )
                })?;
            let secondary_sample = {
                let mut queue_state = secondary_queue_state
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                queue_state.pending_samples.pop_front()
            };
            match (next_primary_sample, secondary_sample) {
                (Some(primary), secondary) => {
                    audio_drain_stats.primary_dequeued_samples =
                        audio_drain_stats.primary_dequeued_samples.saturating_add(1);
                    if secondary.is_some() {
                        audio_drain_stats.secondary_dequeued_samples = audio_drain_stats
                            .secondary_dequeued_samples
                            .saturating_add(1);
                    }
                    MixedAudioSample::Mixed { primary, secondary }
                }
                (None, Some(secondary)) => {
                    audio_drain_stats.secondary_dequeued_samples = audio_drain_stats
                        .secondary_dequeued_samples
                        .saturating_add(1);
                    MixedAudioSample::Mixed {
                        primary: secondary,
                        secondary: None,
                    }
                }
                (None, None) => return Ok(()),
            }
        } else {
            let Some(next_sample) = next_primary_sample else {
                return Ok(());
            };
            audio_drain_stats.primary_dequeued_samples =
                audio_drain_stats.primary_dequeued_samples.saturating_add(1);
            MixedAudioSample::Retained(next_sample)
        };

        if !wait_until_audio_input_ready(audio_input, final_drain)? {
            next_audio_sample.requeue(audio_writer_plan);
            return Ok(());
        }

        let normalized_sample = match next_audio_sample {
            MixedAudioSample::Retained(next_sample) => {
                create_retimed_audio_sample(&next_sample, audio_writer_plan)?
            }
            MixedAudioSample::Mixed { primary, secondary } => {
                if secondary.is_none() {
                    audio_drain_stats.silent_secondary_mixes =
                        audio_drain_stats.silent_secondary_mixes.saturating_add(1);
                }
                create_mixed_audio_sample(&primary, secondary.as_ref(), audio_writer_plan)?
            }
        };
        let sample_ref = normalized_sample.as_ref();
        let append_ok = unsafe { audio_input.appendSampleBuffer(sample_ref) };
        if !append_ok {
            return Err(CaptureError::StopFailed(writer_error(
                writer,
                "failed to append an audio sample to the macOS desktop-composite AVAssetWriter",
            )));
        }
        audio_drain_stats.appended_samples = audio_drain_stats.appended_samples.saturating_add(1);
        audio_drain_stats.appended_frames = audio_drain_stats
            .appended_frames
            .saturating_add(unsafe { sample_ref.num_samples() }.max(0) as u64);
    }
}

#[cfg(target_os = "macos")]
fn create_mixed_audio_sample(
    primary_sample: &RetainedSampleBuffer,
    secondary_sample: Option<&RetainedSampleBuffer>,
    audio_writer_plan: &CompositeAudioWriterPlan,
) -> Result<CFRetained<ObjcCMSampleBuffer>, CaptureError> {
    let primary_screen_capture_sample = primary_sample.as_sample()?;
    let primary_decoded = decode_audio_sample_to_f32(&primary_screen_capture_sample)?;
    let target_frame_count = canonical_frame_count(&primary_decoded);
    if target_frame_count == 0 {
        return Err(CaptureError::StopFailed(
            "macOS desktop-composite mixed audio could not derive a non-empty canonical frame count."
                .to_string(),
        ));
    }

    let primary_channels = convert_decoded_audio_sample(&primary_decoded, target_frame_count)?;
    let secondary_channels = if let Some(secondary_sample) = secondary_sample {
        let secondary_screen_capture_sample = secondary_sample.as_sample()?;
        let secondary_decoded = decode_audio_sample_to_f32(&secondary_screen_capture_sample)?;
        convert_decoded_audio_sample(&secondary_decoded, target_frame_count)?
    } else {
        silent_mix_channels(target_frame_count)
    };
    let mixed_payload = mix_audio_buffer_bytes(
        &encode_canonical_audio_bytes(&primary_channels, target_frame_count)?,
        &encode_canonical_audio_bytes(&secondary_channels, target_frame_count)?,
    )?
    .into_boxed_slice();

    let presentation_frame = {
        let mut cursor = audio_writer_plan
            .output_frame_cursor
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let current = *cursor;
        *cursor = cursor.saturating_add(target_frame_count as u64);
        current
    };
    let timing = screencapturekit::cm::CMSampleTimingInfo::with_times(
        mixed_audio_frame_duration(audio_writer_plan.sample_rate),
        screencapturekit::cm::CMTime::new(
            presentation_frame as i64,
            audio_writer_plan.sample_rate.round().max(1.0) as i32,
        ),
        screencapturekit::cm::CMTime::indefinite(),
    );
    let timing_info = ObjcCMSampleTimingInfo {
        duration: objc_cm_time(timing.duration),
        presentationTimeStamp: objc_cm_time(timing.presentation_time_stamp),
        decodeTimeStamp: objc_cm_time(timing.decode_time_stamp),
    };
    let format_description = build_mixed_audio_format_description(audio_writer_plan)?;

    let mut sample_buffer_ptr = std::ptr::null_mut();
    let sample_size = audio_writer_plan._bytes_per_frame as usize;
    let status = unsafe {
        ObjcCMSampleBuffer::create_ready(
            None,
            None,
            Some(format_description.as_ref()),
            target_frame_count.try_into().unwrap_or(isize::MAX),
            1,
            &timing_info,
            1,
            &sample_size,
            NonNull::new(&mut sample_buffer_ptr).expect("sample buffer output pointer"),
        )
    };
    if status != 0 {
        return Err(CaptureError::StopFailed(format!(
            "macOS desktop-composite mixed audio could not create a Core Media sample buffer (status={status})."
        )));
    }

    let sample_buffer = unsafe {
        CFRetained::from_raw(NonNull::new(sample_buffer_ptr).ok_or_else(|| {
            CaptureError::StopFailed(
                "macOS desktop-composite mixed audio sample creation returned a null buffer."
                    .to_string(),
            )
        })?)
    };
    let mixed_buffer_list = OwnedAudioBufferList::new(
        &[audio_writer_plan.channel_count.max(1)],
        vec![mixed_payload],
    )?;
    let status = unsafe {
        sample_buffer.set_data_buffer_from_audio_buffer_list(
            None,
            None,
            kCMSampleBufferFlag_AudioBufferList_Assure16ByteAlignment,
            mixed_buffer_list.as_nonnull(),
        )
    };
    if status != 0 {
        return Err(CaptureError::StopFailed(format!(
            "macOS desktop-composite mixed audio could not attach PCM data to the new sample buffer (status={status})."
        )));
    }

    Ok(sample_buffer)
}

#[cfg(target_os = "macos")]
fn log_composite_audio_drain_stats(
    audio_writer_plan: Option<&CompositeAudioWriterPlan>,
    audio_drain_stats: &CompositeAudioDrainStats,
) {
    if std::env::var_os("RECORD_SCREEN_MAC_COMPOSITE_AUDIO_TRACE").is_none() {
        return;
    }
    let Some(audio_writer_plan) = audio_writer_plan else {
        return;
    };

    let primary_queue_state = audio_writer_plan
        .primary_queue_state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let (secondary_observed, secondary_sample_rate) = audio_writer_plan
        .secondary_queue_state
        .as_ref()
        .map(|queue| {
            let queue = queue
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            (queue.observed_samples, queue.sample_rate)
        })
        .unwrap_or((0, None));
    let rendered_audio_secs =
        audio_drain_stats.appended_frames as f64 / audio_writer_plan.sample_rate.max(1.0);

    eprintln!(
        "[macos-composite-audio] kind={:?} output_rate={} primary_rate={:?} secondary_rate={:?} primary_observed={} secondary_observed={} dequeued_primary={} dequeued_secondary={} appended_samples={} appended_frames={} rendered_audio_secs={:.3} silent_secondary_mixes={}",
        audio_writer_plan.kind,
        audio_writer_plan.sample_rate,
        primary_queue_state.sample_rate,
        secondary_sample_rate,
        primary_queue_state.observed_samples,
        secondary_observed,
        audio_drain_stats.primary_dequeued_samples,
        audio_drain_stats.secondary_dequeued_samples,
        audio_drain_stats.appended_samples,
        audio_drain_stats.appended_frames,
        rendered_audio_secs,
        audio_drain_stats.silent_secondary_mixes,
    );
}

#[cfg(target_os = "macos")]
fn silent_mix_channels(target_frame_count: usize) -> Vec<Vec<f32>> {
    vec![vec![0.0; target_frame_count], vec![0.0; target_frame_count]]
}

#[cfg(target_os = "macos")]
fn decode_audio_sample_to_f32(sample: &CMSampleBuffer) -> Result<DecodedAudioSample, CaptureError> {
    let format_description = sample.format_description().ok_or_else(|| {
        CaptureError::StopFailed(
            "macOS desktop-composite mixed audio could not inspect the source audio format."
                .to_string(),
        )
    })?;
    let metadata = CompositeAudioFormatMetadata {
        sample_rate: format_description
            .audio_sample_rate()
            .unwrap_or(MIXED_AUDIO_TARGET_SAMPLE_RATE),
        channel_count: format_description.audio_channel_count().unwrap_or(1).max(1),
        bits_per_channel: format_description
            .audio_bits_per_channel()
            .unwrap_or(32)
            .max(1),
        bytes_per_frame: format_description
            .audio_bytes_per_frame()
            .unwrap_or(4)
            .max(1),
        format_flags: format_description
            .audio_format_flags()
            .unwrap_or(kAudioFormatFlagIsFloat),
        buffer_count: sample
            .audio_buffer_list()
            .map(|list| list.num_buffers())
            .unwrap_or(1),
    };
    ensure_audio_format_supported(metadata)?;
    let timing = sample.sample_timing_info(0).map_err(|status| {
        CaptureError::StopFailed(format!(
            "macOS desktop-composite mixed audio could not read source timing info (status={status})."
        ))
    })?;
    let buffer_list = sample.audio_buffer_list().ok_or_else(|| {
        CaptureError::StopFailed(
            "macOS desktop-composite mixed audio expected a PCM audio buffer list.".to_string(),
        )
    })?;
    let frame_count = derive_pcm_frame_count(&buffer_list, metadata)?;
    let channels = decode_pcm_buffer_list_to_channels(&buffer_list, metadata, frame_count)?;

    Ok(DecodedAudioSample {
        timing,
        sample_rate: metadata.sample_rate.max(1.0),
        channels,
    })
}

#[cfg(target_os = "macos")]
fn decode_pcm_buffer_list_to_channels(
    buffer_list: &screencapturekit::cm::AudioBufferList,
    metadata: CompositeAudioFormatMetadata,
    frame_count: usize,
) -> Result<Vec<Vec<f32>>, CaptureError> {
    let mut channels = Vec::new();
    for buffer in buffer_list.iter() {
        let local_channels = buffer.number_channels.max(1) as usize;
        channels.extend(decode_pcm_buffer_bytes(
            buffer.data(),
            local_channels,
            frame_count,
            metadata.bits_per_channel,
            metadata.format_flags,
        )?);
    }

    if channels.is_empty() {
        return Err(CaptureError::StopFailed(
            "macOS desktop-composite mixed audio could not decode any PCM channels from the source sample."
                .to_string(),
        ));
    }

    Ok(channels)
}

#[cfg(target_os = "macos")]
fn derive_pcm_frame_count(
    buffer_list: &screencapturekit::cm::AudioBufferList,
    metadata: CompositeAudioFormatMetadata,
) -> Result<usize, CaptureError> {
    let mut frame_count = None;
    for buffer in buffer_list.iter() {
        let buffer_channels = buffer.number_channels.max(1) as usize;
        let bytes_per_frame = pcm_bytes_per_frame_for_buffer(metadata, buffer_channels)?;
        let data_len = buffer.data().len();
        if data_len % bytes_per_frame != 0 {
            return Err(CaptureError::StopFailed(
                "macOS desktop-composite mixed audio PCM payload length was not aligned to its frame width."
                    .to_string(),
            ));
        }
        let local_frame_count = data_len / bytes_per_frame;
        if local_frame_count == 0 {
            return Err(CaptureError::StopFailed(
                "macOS desktop-composite mixed audio could not derive any frames from its PCM payload."
                    .to_string(),
            ));
        }
        if let Some(existing_frame_count) = frame_count {
            if existing_frame_count != local_frame_count {
                return Err(CaptureError::StopFailed(
                    "macOS desktop-composite mixed audio buffers did not agree on their frame count."
                        .to_string(),
                ));
            }
        } else {
            frame_count = Some(local_frame_count);
        }
    }

    frame_count.ok_or_else(|| {
        CaptureError::StopFailed(
            "macOS desktop-composite mixed audio could not derive a frame count from an empty PCM buffer list."
                .to_string(),
        )
    })
}

#[cfg(target_os = "macos")]
fn pcm_bytes_per_frame_for_buffer(
    metadata: CompositeAudioFormatMetadata,
    buffer_channels: usize,
) -> Result<usize, CaptureError> {
    let bytes_per_sample = (metadata.bits_per_channel / 8) as usize;
    if bytes_per_sample == 0 {
        return Err(CaptureError::StopFailed(
            "macOS desktop-composite mixed audio could not derive a PCM sample width.".to_string(),
        ));
    }

    if metadata.format_flags & kAudioFormatFlagIsNonInterleaved != 0 {
        Ok(bytes_per_sample.saturating_mul(buffer_channels.max(1)))
    } else {
        Ok(metadata.bytes_per_frame.max(bytes_per_sample as u32) as usize)
    }
}

#[cfg(target_os = "macos")]
fn decode_pcm_buffer_bytes(
    bytes: &[u8],
    channel_count: usize,
    frame_count: usize,
    bits_per_channel: u32,
    format_flags: u32,
) -> Result<Vec<Vec<f32>>, CaptureError> {
    let bytes_per_sample = (bits_per_channel / 8) as usize;
    if bytes_per_sample == 0 || channel_count == 0 {
        return Err(CaptureError::StopFailed(
            "macOS desktop-composite mixed audio could not derive a valid PCM sample width."
                .to_string(),
        ));
    }
    let expected_bytes = frame_count
        .saturating_mul(channel_count)
        .saturating_mul(bytes_per_sample);
    if bytes.len() < expected_bytes {
        return Err(CaptureError::StopFailed(
            "macOS desktop-composite mixed audio PCM payload was smaller than expected for its frame count."
                .to_string(),
        ));
    }

    let non_interleaved = format_flags & kAudioFormatFlagIsNonInterleaved != 0;
    let is_float = format_flags & kAudioFormatFlagIsFloat != 0;
    let is_signed_integer = format_flags & kAudioFormatFlagIsSignedInteger != 0;
    let mut channels = vec![Vec::with_capacity(frame_count); channel_count];

    for frame_index in 0..frame_count {
        for channel_index in 0..channel_count {
            let sample_index = if non_interleaved {
                channel_index
                    .saturating_mul(frame_count)
                    .saturating_add(frame_index)
            } else {
                frame_index
                    .saturating_mul(channel_count)
                    .saturating_add(channel_index)
            };
            let offset = sample_index.saturating_mul(bytes_per_sample);
            let value = if is_float && bits_per_channel == 32 {
                let chunk: [u8; 4] = bytes[offset..offset + 4].try_into().map_err(|_| {
                    CaptureError::StopFailed(
                        "macOS desktop-composite mixed audio could not decode a float PCM sample."
                            .to_string(),
                    )
                })?;
                f32::from_le_bytes(chunk)
            } else if is_signed_integer && bits_per_channel == 16 {
                let chunk: [u8; 2] = bytes[offset..offset + 2].try_into().map_err(|_| {
                    CaptureError::StopFailed(
                        "macOS desktop-composite mixed audio could not decode a signed PCM sample."
                            .to_string(),
                    )
                })?;
                f32::from(i16::from_le_bytes(chunk)) / f32::from(i16::MAX)
            } else {
                return Err(CaptureError::BackendUnavailable(
                    "Full desktop across multiple macOS displays can only combine microphone and system audio for 32-bit float PCM or 16-bit signed PCM sources right now."
                        .to_string(),
                ));
            };
            channels[channel_index].push(value.clamp(-1.0, 1.0));
        }
    }

    Ok(channels)
}

#[cfg(target_os = "macos")]
fn convert_decoded_audio_sample(
    sample: &DecodedAudioSample,
    target_frame_count: usize,
) -> Result<Vec<Vec<f32>>, CaptureError> {
    let resampled_channels = sample
        .channels
        .iter()
        .map(|channel| resample_channel_linear(channel, target_frame_count))
        .collect::<Vec<_>>();
    normalize_channels_for_mix(&resampled_channels, target_frame_count)
}

#[cfg(target_os = "macos")]
fn normalize_channels_for_mix(
    channels: &[Vec<f32>],
    target_frame_count: usize,
) -> Result<Vec<Vec<f32>>, CaptureError> {
    if channels.is_empty() {
        return Err(CaptureError::StopFailed(
            "macOS desktop-composite mixed audio could not normalize an empty channel layout."
                .to_string(),
        ));
    }

    if channels.len() == 1 {
        let mono = channels[0].clone();
        return Ok(vec![mono.clone(), mono]);
    }

    let mut left = channels[0].clone();
    let mut right = channels[1].clone();
    for frame_index in 0..target_frame_count {
        if channels.len() > 2 {
            let mut extra_sum = 0.0f32;
            let mut extra_count = 0usize;
            for channel in channels.iter().skip(2) {
                if let Some(sample) = channel.get(frame_index) {
                    extra_sum += *sample;
                    extra_count = extra_count.saturating_add(1);
                }
            }
            if extra_count > 0 {
                let extra = extra_sum / extra_count as f32;
                left[frame_index] = ((left[frame_index] + extra) * 0.5).clamp(-1.0, 1.0);
                right[frame_index] = ((right[frame_index] + extra) * 0.5).clamp(-1.0, 1.0);
            }
        }
    }

    Ok(vec![left, right])
}

#[cfg(target_os = "macos")]
fn resample_channel_linear(channel: &[f32], target_frame_count: usize) -> Vec<f32> {
    if target_frame_count == 0 {
        return Vec::new();
    }
    if channel.is_empty() {
        return vec![0.0; target_frame_count];
    }
    if channel.len() == 1 {
        return vec![channel[0]; target_frame_count];
    }
    if channel.len() == target_frame_count {
        return channel.to_vec();
    }
    if target_frame_count == 1 {
        return vec![channel[0]];
    }

    let source_span = channel.len().saturating_sub(1) as f64;
    let target_span = target_frame_count.saturating_sub(1) as f64;
    (0..target_frame_count)
        .map(|target_index| {
            let position = target_index as f64 * source_span / target_span;
            let lower = position.floor() as usize;
            let upper = position.ceil() as usize;
            if lower == upper {
                channel[lower]
            } else {
                let alpha = (position - lower as f64) as f32;
                channel[lower] * (1.0 - alpha) + channel[upper] * alpha
            }
        })
        .collect()
}

#[cfg(target_os = "macos")]
fn canonical_frame_count(sample: &DecodedAudioSample) -> usize {
    let source_frame_count = sample.channels.first().map_or(0, Vec::len);
    if source_frame_count == 0 {
        if let Some(duration_seconds) = sample.timing.duration.as_seconds() {
            let from_timing = (duration_seconds * MIXED_AUDIO_TARGET_SAMPLE_RATE).round() as usize;
            if from_timing > 0 {
                return from_timing;
            }
        }
        return 0;
    }

    ((source_frame_count as f64 / sample.sample_rate.max(1.0)) * MIXED_AUDIO_TARGET_SAMPLE_RATE)
        .round()
        .max(1.0) as usize
}

#[cfg(target_os = "macos")]
fn mixed_audio_frame_duration(sample_rate: f64) -> screencapturekit::cm::CMTime {
    screencapturekit::cm::CMTime::new(1, sample_rate.round().max(1.0) as i32)
}

#[cfg(target_os = "macos")]
fn encode_canonical_audio_bytes(
    channels: &[Vec<f32>],
    target_frame_count: usize,
) -> Result<Vec<u8>, CaptureError> {
    if channels.len() < MIXED_AUDIO_TARGET_CHANNEL_COUNT as usize {
        return Err(CaptureError::StopFailed(
            "macOS desktop-composite mixed audio could not encode fewer than two canonical channels."
                .to_string(),
        ));
    }

    let mut encoded = Vec::with_capacity(
        target_frame_count.saturating_mul(MIXED_AUDIO_TARGET_BYTES_PER_FRAME as usize),
    );
    for frame_index in 0..target_frame_count {
        for channel in channels
            .iter()
            .take(MIXED_AUDIO_TARGET_CHANNEL_COUNT as usize)
        {
            let value = channel.get(frame_index).copied().unwrap_or_default();
            encoded.extend_from_slice(&value.clamp(-1.0, 1.0).to_le_bytes());
        }
    }
    Ok(encoded)
}

#[cfg(target_os = "macos")]
fn create_retimed_audio_sample(
    sample: &RetainedSampleBuffer,
    audio_writer_plan: &CompositeAudioWriterPlan,
) -> Result<CFRetained<ObjcCMSampleBuffer>, CaptureError> {
    let screen_capture_sample = sample.as_sample()?;
    let timing_infos = screen_capture_sample
        .sample_timing_info_array()
        .map_err(|status| {
            CaptureError::StopFailed(format!(
                "macOS desktop-composite audio could not read sample timing info (status={status})."
            ))
        })?
        .into_iter()
        .map(|timing| normalized_audio_timing(timing, &audio_writer_plan.timestamp_origin))
        .collect::<Vec<_>>();
    let retimed_sample = screen_capture_sample
        .create_copy_with_new_timing(&timing_infos)
        .map_err(|status| {
            CaptureError::StopFailed(format!(
                "macOS desktop-composite audio could not create a copy with normalized timing (status={status})."
            ))
        })?;
    let retimed_ptr = retimed_sample.as_ptr().cast::<ObjcCMSampleBuffer>();
    std::mem::forget(retimed_sample);

    unsafe {
        Ok(CFRetained::from_raw(NonNull::new(retimed_ptr).ok_or_else(
            || {
                CaptureError::StopFailed(
                    "macOS desktop-composite retimed audio sample creation returned a null buffer."
                        .to_string(),
                )
            },
        )?))
    }
}

#[cfg(target_os = "macos")]
fn normalized_audio_timing(
    timing: screencapturekit::cm::CMSampleTimingInfo,
    timestamp_origin: &Arc<Mutex<Option<screencapturekit::cm::CMTime>>>,
) -> screencapturekit::cm::CMSampleTimingInfo {
    let origin = {
        let mut origin_slot = timestamp_origin
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if origin_slot.is_none() && timing.presentation_time_stamp.is_valid() {
            *origin_slot = Some(timing.presentation_time_stamp);
        }
        *origin_slot
    };

    let normalized_presentation_time = origin
        .map(|origin| normalize_audio_time(timing.presentation_time_stamp, origin))
        .unwrap_or(timing.presentation_time_stamp);
    let normalized_decode_time = if timing.decode_time_stamp.is_valid() {
        origin
            .map(|origin| normalize_audio_time(timing.decode_time_stamp, origin))
            .unwrap_or(timing.decode_time_stamp)
    } else {
        timing.decode_time_stamp
    };

    screencapturekit::cm::CMSampleTimingInfo::with_times(
        timing.duration,
        normalized_presentation_time,
        normalized_decode_time,
    )
}

#[cfg(target_os = "macos")]
fn normalize_audio_time(
    time: screencapturekit::cm::CMTime,
    origin: screencapturekit::cm::CMTime,
) -> screencapturekit::cm::CMTime {
    if !time.is_valid() || !origin.is_valid() {
        return time;
    }

    let Some(time_seconds) = time.as_seconds() else {
        return time;
    };
    let Some(origin_seconds) = origin.as_seconds() else {
        return time;
    };
    let normalized_seconds = (time_seconds - origin_seconds).max(0.0);
    let normalized_value = (normalized_seconds * 1_000_000.0).round() as i64;

    screencapturekit::cm::CMTime::new(normalized_value, 1_000_000)
}

#[cfg(target_os = "macos")]
fn mix_audio_buffer_bytes(
    primary_bytes: &[u8],
    secondary_bytes: &[u8],
) -> Result<Vec<u8>, CaptureError> {
    if primary_bytes.len() != secondary_bytes.len() {
        return Err(CaptureError::StopFailed(
            "macOS desktop-composite mixed audio received PCM buffers with different byte sizes."
                .to_string(),
        ));
    }
    let mut mixed = Vec::with_capacity(primary_bytes.len());
    for (primary_chunk, secondary_chunk) in primary_bytes
        .chunks_exact(4)
        .zip(secondary_bytes.chunks_exact(4))
    {
        let primary_value = f32::from_le_bytes(primary_chunk.try_into().unwrap_or([0; 4]));
        let secondary_value = f32::from_le_bytes(secondary_chunk.try_into().unwrap_or([0; 4]));
        mixed.extend_from_slice(
            &(0.5 * (primary_value + secondary_value))
                .clamp(-1.0, 1.0)
                .to_le_bytes(),
        );
    }
    Ok(mixed)
}

#[cfg(target_os = "macos")]
fn objc_cm_time(time: screencapturekit::cm::CMTime) -> CMTime {
    CMTime {
        value: time.value,
        timescale: time.timescale,
        flags: CMTimeFlags(time.flags),
        epoch: time.epoch,
    }
}

#[cfg(target_os = "macos")]
fn wait_until_audio_input_ready(
    audio_input: &AVAssetWriterInput,
    final_drain: bool,
) -> Result<bool, CaptureError> {
    let attempts = if final_drain { 200 } else { 10 };
    for _ in 0..attempts {
        if unsafe { audio_input.isReadyForMoreMediaData() } {
            return Ok(true);
        }
        thread::sleep(Duration::from_millis(5));
    }

    if final_drain {
        return Err(CaptureError::StopFailed(
            "AVAssetWriter audio input never became ready while finalizing a desktop-composite recording."
                .to_string(),
        ));
    }

    Ok(false)
}

#[cfg(target_os = "macos")]
fn all_display_slots_ready(frame_slots: &[Mutex<CompositeFrameSlot>]) -> bool {
    !frame_slots.is_empty()
        && frame_slots.iter().all(|slot| {
            slot.lock()
                .map(|slot| slot.observed_frames > 0)
                .unwrap_or(false)
        })
}

#[cfg(test)]
fn composite_dual_audio_support_note(display_count: usize) -> String {
    format!(
        "Full desktop across {} macOS displays can combine microphone and system audio on the native composite lane. Common PCM differences such as sample rate, mono-versus-stereo layout, and 16-bit-versus-float samples are converted automatically. Unsupported PCM encodings still require choosing one audio source or switching to a specific display.",
        display_count.max(2)
    )
}

#[cfg(target_os = "macos")]
fn compose_desktop_frame(
    display_plan: &[CompositeDisplayPlan],
    frame_slots: &[Mutex<CompositeFrameSlot>],
    canvas_width: u32,
    canvas_height: u32,
) -> Result<CVPixelBuffer, CaptureError> {
    let composite_frame = CVPixelBuffer::create(
        canvas_width as usize,
        canvas_height as usize,
        COMPOSITE_PIXEL_FORMAT_BGRA,
    )
    .map_err(|status| {
        CaptureError::SpawnFailed(format!(
            "failed to allocate composite desktop pixel buffer (status={status})."
        ))
    })?;
    let mut composite_guard =
        composite_frame
            .lock(CVPixelBufferLockFlags::NONE)
            .map_err(|status| {
                CaptureError::SpawnFailed(format!(
                    "failed to lock composite desktop pixel buffer (status={status})."
                ))
            })?;
    let composite_stride = composite_guard.bytes_per_row();
    let composite_bytes = composite_guard.as_slice_mut().ok_or_else(|| {
        CaptureError::SpawnFailed(
            "failed to access mutable bytes for composite desktop pixel buffer.".to_string(),
        )
    })?;
    composite_bytes.fill(0);

    for (display, slot) in display_plan.iter().zip(frame_slots.iter()) {
        let latest_buffer = slot
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .latest_buffer
            .clone();
        let Some(latest_buffer) = latest_buffer else {
            continue;
        };
        let source_guard = latest_buffer
            .lock(CVPixelBufferLockFlags::READ_ONLY)
            .map_err(|status| {
                CaptureError::StopFailed(format!(
                    "failed to lock source display buffer for desktop composition (status={status})."
                ))
            })?;
        let source_width = source_guard.width().min(display.width as usize);
        let source_height = source_guard.height().min(display.height as usize);
        let source_stride = source_guard.bytes_per_row();
        let source_bytes = source_guard.as_slice();

        for row in 0..source_height {
            let source_offset = row.saturating_mul(source_stride);
            let destination_offset = display
                .origin_y
                .saturating_add(row)
                .saturating_mul(composite_stride)
                .saturating_add(display.origin_x.saturating_mul(4));
            let bytes_to_copy = source_width.saturating_mul(4);
            let source_end = source_offset.saturating_add(bytes_to_copy);
            let destination_end = destination_offset.saturating_add(bytes_to_copy);
            if source_end > source_bytes.len() || destination_end > composite_bytes.len() {
                break;
            }

            composite_bytes[destination_offset..destination_end]
                .copy_from_slice(&source_bytes[source_offset..source_end]);
        }
    }

    drop(composite_guard);
    Ok(composite_frame)
}

#[cfg(target_os = "macos")]
fn composite_frame_ref(
    composite_frame: &CVPixelBuffer,
) -> Result<&ObjcCVPixelBuffer, CaptureError> {
    let ptr = composite_frame.as_ptr().cast::<ObjcCVPixelBuffer>();
    unsafe { ptr.as_ref() }.ok_or_else(|| {
        CaptureError::SpawnFailed(
            "failed to bridge the composed desktop pixel buffer into AVFoundation.".to_string(),
        )
    })
}

#[cfg(target_os = "macos")]
fn duration_for_frame_count(frame_count: u64, fps: u32) -> Duration {
    if frame_count == 0 || fps == 0 {
        return Duration::default();
    }

    Duration::from_nanos(
        frame_count
            .saturating_mul(1_000_000_000)
            .checked_div(u64::from(fps))
            .unwrap_or(0),
    )
}

#[cfg(target_os = "macos")]
fn writer_error(writer: &AVAssetWriter, context: &str) -> String {
    let error = unsafe { writer.error() }
        .map(|error| error.to_string())
        .unwrap_or_else(|| "unknown AVFoundation error".to_string());
    format!("{context}: {error}")
}

#[cfg(test)]
mod tests {
    use super::{
        CompositeAudioFormatMetadata, CompositeFrameSlot, DecodedAudioSample,
        all_display_slots_ready, canonical_frame_count, composite_dual_audio_support_note,
        convert_decoded_audio_sample, decode_pcm_buffer_bytes, duration_for_frame_count,
        ensure_dual_audio_formats_supported, mix_audio_buffer_bytes, normalize_audio_time,
        resample_channel_linear,
    };
    use objc2_core_audio_types::{
        kAudioFormatFlagIsFloat, kAudioFormatFlagIsPacked, kAudioFormatFlagIsSignedInteger,
    };
    use screencapturekit::cm::CMTime;
    use std::sync::Mutex;
    use std::time::Duration;

    fn format_metadata(
        sample_rate: f64,
        channel_count: u32,
        bits_per_channel: u32,
        bytes_per_frame: u32,
        format_flags: u32,
        buffer_count: usize,
    ) -> CompositeAudioFormatMetadata {
        CompositeAudioFormatMetadata {
            sample_rate,
            channel_count,
            bits_per_channel,
            bytes_per_frame,
            format_flags,
            buffer_count,
        }
    }

    #[test]
    fn calculates_duration_from_frame_count() {
        assert_eq!(duration_for_frame_count(60, 30), Duration::from_secs(2));
        assert_eq!(duration_for_frame_count(0, 30), Duration::default());
    }

    #[test]
    fn normalizes_audio_time_against_origin() {
        let normalized =
            normalize_audio_time(CMTime::new(48_480, 48_000), CMTime::new(48_000, 48_000));
        assert_eq!(normalized.timescale, 1_000_000);
        assert_eq!(normalized.value, 10_000);
    }

    #[test]
    fn requires_every_display_slot_before_startup_completes() {
        let frame_slots = vec![
            Mutex::new(CompositeFrameSlot {
                latest_buffer: None,
                observed_frames: 1,
            }),
            Mutex::new(CompositeFrameSlot {
                latest_buffer: None,
                observed_frames: 0,
            }),
        ];

        assert!(!all_display_slots_ready(&frame_slots));

        frame_slots[1]
            .lock()
            .expect("slot should lock")
            .observed_frames = 1;

        assert!(all_display_slots_ready(&frame_slots));
    }

    #[test]
    fn builds_audio_support_note_for_multi_display_lane() {
        let note = composite_dual_audio_support_note(3);
        assert!(note.contains("3 macOS displays"));
        assert!(note.contains("converted automatically"));
    }

    #[test]
    fn accepts_supported_dual_audio_formats_even_when_they_differ() {
        let primary = format_metadata(
            48_000.0,
            2,
            32,
            4,
            kAudioFormatFlagIsFloat | kAudioFormatFlagIsPacked,
            1,
        );
        let secondary = format_metadata(
            44_100.0,
            1,
            16,
            2,
            kAudioFormatFlagIsSignedInteger | kAudioFormatFlagIsPacked,
            1,
        );

        assert!(ensure_dual_audio_formats_supported(primary, secondary).is_ok());
    }

    #[test]
    fn rejects_unsupported_dual_audio_formats() {
        let primary = format_metadata(
            48_000.0,
            2,
            32,
            4,
            kAudioFormatFlagIsFloat | kAudioFormatFlagIsPacked,
            1,
        );
        let secondary = format_metadata(
            44_100.0,
            2,
            24,
            6,
            kAudioFormatFlagIsSignedInteger | kAudioFormatFlagIsPacked,
            1,
        );

        let error = ensure_dual_audio_formats_supported(primary, secondary).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("32-bit float PCM or 16-bit signed PCM")
        );
    }

    #[test]
    fn decodes_signed_integer_audio_buffers_to_f32() {
        let decoded = decode_pcm_buffer_bytes(
            &[0, 0, 0xFF, 0x7F],
            1,
            2,
            16,
            kAudioFormatFlagIsSignedInteger | kAudioFormatFlagIsPacked,
        )
        .unwrap();
        assert_eq!(decoded.len(), 1);
        assert!((decoded[0][0] - 0.0).abs() < 0.0001);
        assert!((decoded[0][1] - 1.0).abs() < 0.0001);
    }

    #[test]
    fn resamples_linear_audio_channel() {
        let resampled = resample_channel_linear(&[0.0, 1.0], 4);
        assert_eq!(resampled.len(), 4);
        assert!((resampled[1] - 0.3333).abs() < 0.01);
        assert!((resampled[2] - 0.6666).abs() < 0.01);
    }

    #[test]
    fn normalizes_mono_audio_to_stereo() {
        let decoded = DecodedAudioSample {
            timing: screencapturekit::cm::CMSampleTimingInfo::with_times(
                CMTime::new(480, 48_000),
                CMTime::new(0, 48_000),
                CMTime::indefinite(),
            ),
            sample_rate: 48_000.0,
            channels: vec![vec![0.25, -0.25]],
        };
        let converted = convert_decoded_audio_sample(&decoded, 2).unwrap();
        assert_eq!(converted.len(), 2);
        assert_eq!(converted[0], converted[1]);
    }

    #[test]
    fn derives_canonical_frame_count_from_timing() {
        let decoded = DecodedAudioSample {
            timing: screencapturekit::cm::CMSampleTimingInfo::with_times(
                CMTime::new(480, 48_000),
                CMTime::new(0, 48_000),
                CMTime::indefinite(),
            ),
            sample_rate: 44_100.0,
            channels: vec![vec![0.0; 441]],
        };
        assert_eq!(canonical_frame_count(&decoded), 480);
    }

    #[test]
    fn canonical_frame_count_prefers_pcm_payload_over_single_frame_timing() {
        let decoded = DecodedAudioSample {
            timing: screencapturekit::cm::CMSampleTimingInfo::with_times(
                CMTime::new(1, 48_000),
                CMTime::new(0, 48_000),
                CMTime::indefinite(),
            ),
            sample_rate: 48_000.0,
            channels: vec![vec![0.0; 480], vec![0.0; 480]],
        };
        assert_eq!(canonical_frame_count(&decoded), 480);
    }

    #[test]
    fn mixes_canonical_float_audio_buffers() {
        let primary = [0.8f32, -0.2f32]
            .into_iter()
            .flat_map(|value| value.to_le_bytes())
            .collect::<Vec<_>>();
        let secondary = [0.4f32, 0.2f32]
            .into_iter()
            .flat_map(|value| value.to_le_bytes())
            .collect::<Vec<_>>();

        let mixed = mix_audio_buffer_bytes(&primary, &secondary).unwrap();
        let values = mixed
            .chunks_exact(4)
            .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap()))
            .collect::<Vec<_>>();

        assert_eq!(values.len(), 2);
        assert!((values[0] - 0.6).abs() < 0.0001);
        assert!((values[1] - 0.0).abs() < 0.0001);
    }
}
