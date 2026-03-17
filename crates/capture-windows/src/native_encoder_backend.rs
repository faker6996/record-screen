#[cfg(target_os = "windows")]
use crate::native_audio_backend::{WindowsWasapiAudioPacket, WindowsWasapiClientFoundation};
use capture::{
    EncoderBackendAvailability, EncoderBackendDescriptor, EncoderBackendFactory,
    EncoderBackendRuntimeReport, RecordingOptions,
};
#[cfg(target_os = "windows")]
use std::{env, fs, os::windows::ffi::OsStrExt, path::Path};
#[cfg(target_os = "windows")]
use windows::{
    Graphics::DirectX::Direct3D11::IDirect3DSurface,
    Win32::Graphics::{Direct3D11::ID3D11Texture2D, Dxgi::IDXGISurface},
    Win32::Media::MediaFoundation::{
        IMFAttributes, IMFByteStream, IMFMediaBuffer, IMFMediaType, IMFSample, IMFSinkWriter,
        MF_MT_ALL_SAMPLES_INDEPENDENT, MF_MT_AUDIO_AVG_BYTES_PER_SECOND,
        MF_MT_AUDIO_BITS_PER_SAMPLE, MF_MT_AUDIO_BLOCK_ALIGNMENT, MF_MT_AUDIO_NUM_CHANNELS,
        MF_MT_AUDIO_SAMPLES_PER_SECOND, MF_MT_AVG_BITRATE, MF_MT_FRAME_RATE, MF_MT_FRAME_SIZE,
        MF_MT_INTERLACE_MODE, MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE, MF_VERSION, MFAudioFormat_AAC,
        MFAudioFormat_PCM, MFCreateDXGISurfaceBuffer, MFCreateMediaType, MFCreateMemoryBuffer,
        MFCreateSample, MFCreateSinkWriterFromURL, MFCreateVideoSampleFromSurface,
        MFMediaType_Audio, MFMediaType_Video, MFSTARTUP_NOSOCKET, MFShutdown, MFStartup,
        MFVideoFormat_ARGB32, MFVideoFormat_H264, MFVideoInterlace_Progressive,
    },
    Win32::System::WinRT::Direct3D11::IDirect3DDxgiInterfaceAccess,
    core::{IUnknown, Interface, PCWSTR},
};

pub struct MediaFoundationWindowsEncoderBackend;

static MEDIA_FOUNDATION_WINDOWS_ENCODER_BACKEND: MediaFoundationWindowsEncoderBackend =
    MediaFoundationWindowsEncoderBackend;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaFoundationOutputPlan {
    pub output_path: String,
    pub container_label: String,
    pub encoder_label: String,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub bitrate: u32,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaFoundationRuntimeFoundation {
    pub output_path: String,
    pub stream_index: u32,
    pub output_encoder_label: String,
    pub input_format_label: String,
    pub writing_started: bool,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaFoundationSampleBridgePlan {
    pub expected_surface_kind: String,
    pub sample_factory: String,
    pub duration_100ns: i64,
    pub summary: String,
}

#[cfg(target_os = "windows")]
pub(crate) struct NativeSinkWriterFoundation {
    sink_writer: IMFSinkWriter,
    video_stream_index: u32,
    audio_stream_index: Option<u32>,
    video_sample_duration_100ns: i64,
}

pub fn backend() -> &'static dyn EncoderBackendFactory {
    &MEDIA_FOUNDATION_WINDOWS_ENCODER_BACKEND
}

impl EncoderBackendFactory for MediaFoundationWindowsEncoderBackend {
    fn descriptor(&self) -> EncoderBackendDescriptor {
        EncoderBackendDescriptor {
            id: "windows-media-foundation",
            label: "Windows Media Foundation",
        }
    }

    fn availability(&self) -> EncoderBackendAvailability {
        if runtime_summary().is_some() {
            EncoderBackendAvailability::Available
        } else {
            EncoderBackendAvailability::Unavailable {
                reason: "Windows Media Foundation output is not available in the current session."
                    .to_string(),
            }
        }
    }

    fn runtime_report(&self) -> EncoderBackendRuntimeReport {
        EncoderBackendRuntimeReport {
            summary: runtime_summary(),
            preferred_encoder_label: preferred_encoder_label(),
        }
    }
}

pub fn preferred_encoder_label() -> Option<String> {
    Some("Windows Media Foundation H.264".to_string())
}

pub fn output_plan(options: &RecordingOptions) -> MediaFoundationOutputPlan {
    let (width, height, fps, bitrate) = quality_settings(&options.quality_preset);
    MediaFoundationOutputPlan {
        output_path: options.output_path.display().to_string(),
        container_label: "MP4".to_string(),
        encoder_label: "Media Foundation H.264".to_string(),
        width,
        height,
        fps,
        bitrate,
        summary: format!(
            "Windows Media Foundation output plan would write `{}` as MP4 using H.264 at {}x{} / {} fps (~{} kbps) with ARGB32 input frames.",
            options.output_path.display(),
            width,
            height,
            fps,
            bitrate / 1000,
        ),
    }
}

pub fn output_plan_summary(options: &RecordingOptions) -> Option<String> {
    Some(output_plan(options).summary)
}

pub fn runtime_foundation_summary(options: &RecordingOptions) -> Option<String> {
    smoke_runtime_foundation(options)
        .ok()
        .map(|foundation| foundation.summary)
}

pub fn sample_bridge_summary(options: &RecordingOptions) -> Option<String> {
    Some(sample_bridge_plan(options, Some("d3d11-texture2d")).summary)
}

#[cfg(target_os = "windows")]
pub(crate) fn write_surface_sample_smoke(
    options: &RecordingOptions,
    surface: &IDirect3DSurface,
    sample_time_100ns: i64,
) -> Result<String, capture::CaptureError> {
    let smoke_output_path = temp_smoke_output_path();
    let smoke_options = RecordingOptions {
        output_path: smoke_output_path.clone(),
        ..options.clone()
    };
    let plan = output_plan(&smoke_options);
    let surface_kind = detect_surface_kind(surface);
    unsafe {
        MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET).map_err(map_windows_error)?;
    }

    let build_result = build_sink_writer_foundation(&smoke_options, &plan, None);
    let summary = match build_result {
        Ok(foundation) => {
            let encoder_summary = write_surface_sample(&foundation, surface, sample_time_100ns)?;
            finalize_sink_writer_recording(foundation)?;
            format!(
                "Windows WGC -> Media Foundation smoke wrote one `{surface_kind}` sample. {encoder_summary}"
            )
        }
        Err(error) => {
            unsafe {
                let _ = MFShutdown();
            }
            return Err(error);
        }
    };

    let _ = fs::remove_file(smoke_output_path);
    Ok(summary)
}

pub fn runtime_summary() -> Option<String> {
    Some(
        "Windows encoder runtime now targets Media Foundation sink-writer output with H.264 video, optional WASAPI-backed audio streams, and native sample bridges for D3D11/DXGI capture surfaces."
            .to_string(),
    )
}

pub fn sample_bridge_plan(
    options: &RecordingOptions,
    surface_kind: Option<&str>,
) -> MediaFoundationSampleBridgePlan {
    let (_, _, fps, _) = quality_settings(&options.quality_preset);
    let expected_surface_kind = surface_kind.unwrap_or("d3d11-texture2d").to_string();
    let sample_factory = match expected_surface_kind.as_str() {
        "d3d11-texture2d" => "MFCreateVideoSampleFromSurface(ID3D11Texture2D)",
        "dxgi-surface" => "MFCreateDXGISurfaceBuffer(IDXGISurface) + MFCreateSample/AddBuffer",
        _ => "MFCreateVideoSampleFromSurface(IUnknown)",
    }
    .to_string();
    let duration_100ns = frame_duration_100ns(fps);
    MediaFoundationSampleBridgePlan {
        expected_surface_kind: expected_surface_kind.clone(),
        sample_factory: sample_factory.clone(),
        duration_100ns,
        summary: format!(
            "Windows Media Foundation sample bridge expects `{expected_surface_kind}` and would use `{sample_factory}` with sample_duration_100ns={duration_100ns}."
        ),
    }
}

#[cfg(target_os = "windows")]
fn detect_surface_kind(surface: &IDirect3DSurface) -> String {
    let interface_access: IDirect3DDxgiInterfaceAccess = match surface.cast() {
        Ok(access) => access,
        Err(_) => return "direct3d-surface".to_string(),
    };
    unsafe {
        if interface_access.GetInterface::<ID3D11Texture2D>().is_ok() {
            return "d3d11-texture2d".to_string();
        }
        if interface_access.GetInterface::<IDXGISurface>().is_ok() {
            return "dxgi-surface".to_string();
        }
    }
    "direct3d-surface".to_string()
}

#[cfg(target_os = "windows")]
fn smoke_runtime_foundation(
    options: &RecordingOptions,
) -> Result<MediaFoundationRuntimeFoundation, capture::CaptureError> {
    let smoke_output_path = temp_smoke_output_path();
    let smoke_options = RecordingOptions {
        output_path: smoke_output_path.clone(),
        ..options.clone()
    };
    let plan = output_plan(&smoke_options);

    unsafe {
        MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET).map_err(map_windows_error)?;
    }

    let build_result = build_sink_writer_foundation(&smoke_options, &plan, None);

    let foundation = match build_result {
        Ok(foundation) => {
            let finalize_result = unsafe { foundation.sink_writer.Finalize() };
            unsafe {
                let _ = MFShutdown();
            }
            finalize_result.map_err(map_windows_error)?;

            MediaFoundationRuntimeFoundation {
                output_path: smoke_options.output_path.display().to_string(),
                stream_index: foundation.video_stream_index,
                output_encoder_label: plan.encoder_label.clone(),
                input_format_label: "ARGB32".to_string(),
                writing_started: true,
                summary: format!(
                    "Windows Media Foundation runtime foundation created a sink writer for `{}` with stream_index={} and began/finalized an H.264 MP4 session at {}x{} / {} fps.",
                    smoke_options.output_path.display(),
                    foundation.video_stream_index,
                    plan.width,
                    plan.height,
                    plan.fps,
                ),
            }
        }
        Err(error) => {
            unsafe {
                let _ = MFShutdown();
            }
            return Err(error);
        }
    };

    let _ = fs::remove_file(smoke_output_path);
    Ok(foundation)
}

#[cfg(not(target_os = "windows"))]
fn smoke_runtime_foundation(
    _options: &RecordingOptions,
) -> Result<MediaFoundationRuntimeFoundation, capture::CaptureError> {
    Err(capture::CaptureError::BackendUnavailable(
        "Media Foundation encoder foundation is only available on Windows.".to_string(),
    ))
}

#[cfg(target_os = "windows")]
fn build_sink_writer_foundation(
    options: &RecordingOptions,
    plan: &MediaFoundationOutputPlan,
    audio_foundation: Option<&WindowsWasapiClientFoundation>,
) -> Result<NativeSinkWriterFoundation, capture::CaptureError> {
    let sink_writer = create_sink_writer(&options.output_path)?;
    let output_media_type = build_output_media_type(plan)?;
    let video_stream_index =
        unsafe { sink_writer.AddStream(&output_media_type) }.map_err(map_windows_error)?;
    let input_media_type = build_input_media_type(plan)?;
    unsafe {
        sink_writer
            .SetInputMediaType(
                video_stream_index,
                &input_media_type,
                None::<&IMFAttributes>,
            )
            .map_err(map_windows_error)?;
    }

    let audio_stream_index = if let Some(audio_foundation) = audio_foundation {
        let output_audio_media_type = build_output_audio_media_type(audio_foundation)?;
        let audio_stream_index = unsafe { sink_writer.AddStream(&output_audio_media_type) }
            .map_err(map_windows_error)?;
        let input_audio_media_type = build_input_audio_media_type(audio_foundation)?;
        unsafe {
            sink_writer
                .SetInputMediaType(
                    audio_stream_index,
                    &input_audio_media_type,
                    None::<&IMFAttributes>,
                )
                .map_err(map_windows_error)?;
        }
        Some(audio_stream_index)
    } else {
        None
    };

    unsafe {
        sink_writer.BeginWriting().map_err(map_windows_error)?;
    }

    Ok(NativeSinkWriterFoundation {
        sink_writer,
        video_stream_index,
        audio_stream_index,
        video_sample_duration_100ns: frame_duration_100ns(plan.fps),
    })
}

#[cfg(target_os = "windows")]
pub(crate) fn start_sink_writer_recording(
    options: &RecordingOptions,
    audio_foundation: Option<&WindowsWasapiClientFoundation>,
) -> Result<NativeSinkWriterFoundation, capture::CaptureError> {
    let plan = output_plan(options);
    unsafe {
        MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET).map_err(map_windows_error)?;
    }

    match build_sink_writer_foundation(options, &plan, audio_foundation) {
        Ok(foundation) => Ok(foundation),
        Err(error) => {
            unsafe {
                let _ = MFShutdown();
            }
            Err(error)
        }
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn finalize_sink_writer_recording(
    foundation: NativeSinkWriterFoundation,
) -> Result<(), capture::CaptureError> {
    let finalize_result = unsafe { foundation.sink_writer.Finalize().map_err(map_windows_error) };
    let shutdown_result = unsafe { MFShutdown().map_err(map_windows_error) };

    finalize_result?;
    shutdown_result?;
    Ok(())
}

#[cfg(target_os = "windows")]
pub(crate) fn write_surface_sample(
    foundation: &NativeSinkWriterFoundation,
    surface: &IDirect3DSurface,
    sample_time_100ns: i64,
) -> Result<String, capture::CaptureError> {
    let surface_kind = detect_surface_kind(surface);
    let sample = create_sample_from_surface(
        surface,
        &surface_kind,
        sample_time_100ns,
        foundation.video_sample_duration_100ns,
    )?;
    unsafe {
        foundation
            .sink_writer
            .WriteSample(foundation.video_stream_index, &sample)
            .map_err(map_windows_error)?;
    }
    Ok(format!(
        "surface_kind={surface_kind} stream_index={} sample_time_100ns={} sample_duration_100ns={}",
        foundation.video_stream_index, sample_time_100ns, foundation.video_sample_duration_100ns
    ))
}

#[cfg(target_os = "windows")]
pub(crate) fn write_texture_sample(
    foundation: &NativeSinkWriterFoundation,
    texture: &ID3D11Texture2D,
    sample_time_100ns: i64,
) -> Result<String, capture::CaptureError> {
    let sample = create_sample_from_d3d11_texture(
        texture,
        sample_time_100ns,
        foundation.video_sample_duration_100ns,
    )?;
    unsafe {
        foundation
            .sink_writer
            .WriteSample(foundation.video_stream_index, &sample)
            .map_err(map_windows_error)?;
    }
    Ok(format!(
        "surface_kind=d3d11-texture2d-cropped stream_index={} sample_time_100ns={} sample_duration_100ns={}",
        foundation.video_stream_index, sample_time_100ns, foundation.video_sample_duration_100ns
    ))
}

#[cfg(target_os = "windows")]
pub(crate) fn write_audio_sample(
    foundation: &NativeSinkWriterFoundation,
    packet: &WindowsWasapiAudioPacket,
) -> Result<(), capture::CaptureError> {
    let Some(audio_stream_index) = foundation.audio_stream_index else {
        return Ok(());
    };

    let buffer =
        unsafe { MFCreateMemoryBuffer(packet.bytes.len() as u32) }.map_err(map_windows_error)?;
    let mut raw_buffer = std::ptr::null_mut();
    unsafe {
        buffer
            .Lock(&mut raw_buffer, None, None)
            .map_err(map_windows_error)?;
    }
    if !raw_buffer.is_null() && !packet.bytes.is_empty() {
        unsafe {
            std::ptr::copy_nonoverlapping(packet.bytes.as_ptr(), raw_buffer, packet.bytes.len());
        }
    }
    unsafe {
        buffer.Unlock().map_err(map_windows_error)?;
        buffer
            .SetCurrentLength(packet.bytes.len() as u32)
            .map_err(map_windows_error)?;
    }

    let sample = unsafe { MFCreateSample() }.map_err(map_windows_error)?;
    unsafe {
        sample.AddBuffer(&buffer).map_err(map_windows_error)?;
        sample
            .SetSampleTime(packet.sample_time_100ns)
            .map_err(map_windows_error)?;
        sample
            .SetSampleDuration(packet.duration_100ns)
            .map_err(map_windows_error)?;
        foundation
            .sink_writer
            .WriteSample(audio_stream_index, &sample)
            .map_err(map_windows_error)?;
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn create_sink_writer(output_path: &Path) -> Result<IMFSinkWriter, capture::CaptureError> {
    let wide_path = wide_path(output_path);
    unsafe {
        MFCreateSinkWriterFromURL(
            PCWSTR(wide_path.as_ptr()),
            None::<&IMFByteStream>,
            None::<&IMFAttributes>,
        )
        .map_err(map_windows_error)
    }
}

#[cfg(target_os = "windows")]
fn build_output_media_type(
    plan: &MediaFoundationOutputPlan,
) -> Result<IMFMediaType, capture::CaptureError> {
    let media_type = unsafe { MFCreateMediaType() }.map_err(map_windows_error)?;
    unsafe {
        media_type
            .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
            .map_err(map_windows_error)?;
        media_type
            .SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264)
            .map_err(map_windows_error)?;
        media_type
            .SetUINT32(&MF_MT_AVG_BITRATE, plan.bitrate)
            .map_err(map_windows_error)?;
        media_type
            .SetUINT64(&MF_MT_FRAME_SIZE, pack_u32_pair(plan.width, plan.height))
            .map_err(map_windows_error)?;
        media_type
            .SetUINT64(&MF_MT_FRAME_RATE, pack_u32_pair(plan.fps, 1))
            .map_err(map_windows_error)?;
        media_type
            .SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)
            .map_err(map_windows_error)?;
    }
    Ok(media_type)
}

#[cfg(target_os = "windows")]
fn build_input_media_type(
    plan: &MediaFoundationOutputPlan,
) -> Result<IMFMediaType, capture::CaptureError> {
    let media_type = unsafe { MFCreateMediaType() }.map_err(map_windows_error)?;
    unsafe {
        media_type
            .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
            .map_err(map_windows_error)?;
        media_type
            .SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_ARGB32)
            .map_err(map_windows_error)?;
        media_type
            .SetUINT64(&MF_MT_FRAME_SIZE, pack_u32_pair(plan.width, plan.height))
            .map_err(map_windows_error)?;
        media_type
            .SetUINT64(&MF_MT_FRAME_RATE, pack_u32_pair(plan.fps, 1))
            .map_err(map_windows_error)?;
        media_type
            .SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)
            .map_err(map_windows_error)?;
    }
    Ok(media_type)
}

#[cfg(target_os = "windows")]
fn build_output_audio_media_type(
    foundation: &WindowsWasapiClientFoundation,
) -> Result<IMFMediaType, capture::CaptureError> {
    let media_type = unsafe { MFCreateMediaType() }.map_err(map_windows_error)?;
    let avg_bytes_per_second = u32::from(foundation.channels)
        .saturating_mul(foundation.sample_rate_hz)
        .saturating_mul(24);
    let block_alignment = u32::from(foundation.channels).saturating_mul(2).max(1);
    unsafe {
        media_type
            .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)
            .map_err(map_windows_error)?;
        media_type
            .SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_AAC)
            .map_err(map_windows_error)?;
        media_type
            .SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, u32::from(foundation.channels))
            .map_err(map_windows_error)?;
        media_type
            .SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, foundation.sample_rate_hz)
            .map_err(map_windows_error)?;
        media_type
            .SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, avg_bytes_per_second)
            .map_err(map_windows_error)?;
        media_type
            .SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, block_alignment)
            .map_err(map_windows_error)?;
        media_type
            .SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16)
            .map_err(map_windows_error)?;
        media_type
            .SetUINT32(&MF_MT_ALL_SAMPLES_INDEPENDENT, 1)
            .map_err(map_windows_error)?;
    }
    Ok(media_type)
}

#[cfg(target_os = "windows")]
fn build_input_audio_media_type(
    foundation: &WindowsWasapiClientFoundation,
) -> Result<IMFMediaType, capture::CaptureError> {
    let media_type = unsafe { MFCreateMediaType() }.map_err(map_windows_error)?;
    let bytes_per_sample = u32::from(foundation.bits_per_sample.max(8)) / 8;
    let block_alignment = u32::from(foundation.channels)
        .saturating_mul(bytes_per_sample)
        .max(1);
    let avg_bytes_per_second = foundation.sample_rate_hz.saturating_mul(block_alignment);
    unsafe {
        media_type
            .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)
            .map_err(map_windows_error)?;
        media_type
            .SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_PCM)
            .map_err(map_windows_error)?;
        media_type
            .SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, u32::from(foundation.channels))
            .map_err(map_windows_error)?;
        media_type
            .SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, foundation.sample_rate_hz)
            .map_err(map_windows_error)?;
        media_type
            .SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, block_alignment)
            .map_err(map_windows_error)?;
        media_type
            .SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, avg_bytes_per_second)
            .map_err(map_windows_error)?;
        media_type
            .SetUINT32(
                &MF_MT_AUDIO_BITS_PER_SAMPLE,
                u32::from(foundation.bits_per_sample.max(8)),
            )
            .map_err(map_windows_error)?;
        media_type
            .SetUINT32(&MF_MT_ALL_SAMPLES_INDEPENDENT, 1)
            .map_err(map_windows_error)?;
    }
    Ok(media_type)
}

#[cfg(target_os = "windows")]
#[allow(dead_code)]
fn create_sample_from_d3d11_texture(
    texture: &ID3D11Texture2D,
    sample_time_100ns: i64,
    sample_duration_100ns: i64,
) -> Result<IMFSample, capture::CaptureError> {
    let surface_unknown = texture.cast::<IUnknown>().map_err(map_windows_error)?;
    let sample =
        unsafe { MFCreateVideoSampleFromSurface(&surface_unknown) }.map_err(map_windows_error)?;
    unsafe {
        sample
            .SetSampleTime(sample_time_100ns)
            .map_err(map_windows_error)?;
        sample
            .SetSampleDuration(sample_duration_100ns)
            .map_err(map_windows_error)?;
    }
    Ok(sample)
}

#[cfg(target_os = "windows")]
#[allow(dead_code)]
fn create_sample_from_dxgi_surface(
    surface: &IDXGISurface,
    sample_time_100ns: i64,
    sample_duration_100ns: i64,
) -> Result<IMFSample, capture::CaptureError> {
    let surface_unknown = surface.cast::<IUnknown>().map_err(map_windows_error)?;
    let buffer: IMFMediaBuffer =
        unsafe { MFCreateDXGISurfaceBuffer(&IDXGISurface::IID, &surface_unknown, 0, false) }
            .map_err(map_windows_error)?;
    let sample = unsafe { MFCreateSample() }.map_err(map_windows_error)?;
    unsafe {
        sample.AddBuffer(&buffer).map_err(map_windows_error)?;
        sample
            .SetSampleTime(sample_time_100ns)
            .map_err(map_windows_error)?;
        sample
            .SetSampleDuration(sample_duration_100ns)
            .map_err(map_windows_error)?;
    }
    Ok(sample)
}

#[cfg(target_os = "windows")]
#[allow(dead_code)]
fn create_sample_from_unknown_surface(
    surface: &IUnknown,
    sample_time_100ns: i64,
    sample_duration_100ns: i64,
) -> Result<IMFSample, capture::CaptureError> {
    let sample = unsafe { MFCreateVideoSampleFromSurface(surface) }.map_err(map_windows_error)?;
    unsafe {
        sample
            .SetSampleTime(sample_time_100ns)
            .map_err(map_windows_error)?;
        sample
            .SetSampleDuration(sample_duration_100ns)
            .map_err(map_windows_error)?;
    }
    Ok(sample)
}

#[cfg(target_os = "windows")]
fn create_sample_from_surface(
    surface: &IDirect3DSurface,
    surface_kind: &str,
    sample_time_100ns: i64,
    sample_duration_100ns: i64,
) -> Result<IMFSample, capture::CaptureError> {
    let interface_access: IDirect3DDxgiInterfaceAccess =
        surface.cast().map_err(map_windows_error)?;
    unsafe {
        if surface_kind == "d3d11-texture2d" {
            let texture = interface_access
                .GetInterface::<ID3D11Texture2D>()
                .map_err(map_windows_error)?;
            return create_sample_from_d3d11_texture(
                &texture,
                sample_time_100ns,
                sample_duration_100ns,
            );
        }
        if surface_kind == "dxgi-surface" {
            let dxgi_surface = interface_access
                .GetInterface::<IDXGISurface>()
                .map_err(map_windows_error)?;
            return create_sample_from_dxgi_surface(
                &dxgi_surface,
                sample_time_100ns,
                sample_duration_100ns,
            );
        }
    }
    let surface_unknown = surface.cast::<IUnknown>().map_err(map_windows_error)?;
    create_sample_from_unknown_surface(&surface_unknown, sample_time_100ns, sample_duration_100ns)
}

#[cfg(target_os = "windows")]
fn quality_settings(preset: &str) -> (u32, u32, u32, u32) {
    match preset {
        "720p / 30 fps" => (1280, 720, 30, 5_000_000),
        "1080p / 30 fps" => (1920, 1080, 30, 8_000_000),
        "1440p / 60 fps" => (2560, 1440, 60, 16_000_000),
        "4K / 60 fps" => (3840, 2160, 60, 32_000_000),
        _ => (1920, 1080, 30, 8_000_000),
    }
}

#[cfg(not(target_os = "windows"))]
fn quality_settings(_preset: &str) -> (u32, u32, u32, u32) {
    (1920, 1080, 30, 8_000_000)
}

#[cfg(target_os = "windows")]
fn wide_path(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}

#[cfg(target_os = "windows")]
fn temp_smoke_output_path() -> std::path::PathBuf {
    env::temp_dir().join("record-screen-windows-mf-smoke.mp4")
}

#[cfg(target_os = "windows")]
fn pack_u32_pair(left: u32, right: u32) -> u64 {
    ((left as u64) << 32) | right as u64
}

fn frame_duration_100ns(fps: u32) -> i64 {
    (10_000_000u64 / fps.max(1) as u64) as i64
}

#[cfg(target_os = "windows")]
fn map_windows_error(error: windows::core::Error) -> capture::CaptureError {
    capture::CaptureError::BackendUnavailable(format!("Windows Media Foundation error: {error}"))
}

#[cfg(test)]
mod tests {
    use super::output_plan;
    use capture::RecordingOptions;

    #[test]
    fn output_plan_uses_expected_profile_for_1080p30() {
        let options = RecordingOptions {
            output_path: std::env::temp_dir().join("record-screen-test.mp4"),
            quality_preset: "1080p / 30 fps".to_string(),
            mic_enabled: false,
            system_audio_enabled: false,
            capture_target_id: "full-desktop".to_string(),
            audio_input_id: "default".to_string(),
            portal_parent_window: None,
            portal_restore_token: None,
            region_x: 0,
            region_y: 0,
            region_width: 0,
            region_height: 0,
            region_source_capture_target_id: "full-desktop".to_string(),
            region_source_origin_x: 0,
            region_source_origin_y: 0,
            region_source_scale_factor_milli: 1000,
        };

        let plan = output_plan(&options);
        assert_eq!(plan.container_label, "MP4");
        assert_eq!(plan.encoder_label, "Media Foundation H.264");
        assert_eq!(plan.width, 1920);
        assert_eq!(plan.height, 1080);
        assert_eq!(plan.fps, 30);
        assert_eq!(plan.bitrate, 8_000_000);
        assert!(plan.summary.contains("MP4"));
        assert!(plan.summary.contains("H.264"));
    }

    #[test]
    fn sample_bridge_plan_uses_expected_duration_for_30fps() {
        let options = RecordingOptions {
            output_path: std::env::temp_dir().join("record-screen-test.mp4"),
            quality_preset: "1080p / 30 fps".to_string(),
            mic_enabled: false,
            system_audio_enabled: false,
            capture_target_id: "full-desktop".to_string(),
            audio_input_id: "default".to_string(),
            portal_parent_window: None,
            portal_restore_token: None,
            region_x: 0,
            region_y: 0,
            region_width: 0,
            region_height: 0,
            region_source_capture_target_id: "full-desktop".to_string(),
            region_source_origin_x: 0,
            region_source_origin_y: 0,
            region_source_scale_factor_milli: 1000,
        };

        let plan = super::sample_bridge_plan(&options, Some("d3d11-texture2d"));
        assert_eq!(plan.expected_surface_kind, "d3d11-texture2d");
        assert_eq!(
            plan.sample_factory,
            "MFCreateVideoSampleFromSurface(ID3D11Texture2D)"
        );
        assert_eq!(plan.duration_100ns, 333_333);
    }
}
