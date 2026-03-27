use capture::{
    ActiveRecording, CUSTOM_REGION_TARGET_ID, CaptureBackendAvailability, CaptureBackendDescriptor,
    CaptureBackendFactory, CaptureBackendRuntimeReport, CaptureController, CaptureError,
    CaptureTargetOption, DEFAULT_AUDIO_INPUT_ID, FULL_DESKTOP_TARGET_ID, RecordingArtifact,
    RecordingOptions, full_desktop_target,
};
use serde::Deserialize;
#[cfg(target_os = "windows")]
use std::{
    collections::VecDeque,
    fs,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread,
    thread::JoinHandle,
    time::{Duration, SystemTime},
};
#[cfg(target_os = "windows")]
use windows::{
    Foundation::TypedEventHandler,
    Graphics::{
        Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession},
        DirectX::{
            Direct3D11::{IDirect3DDevice, IDirect3DSurface},
            DirectXPixelFormat,
        },
    },
    Win32::{
        Foundation::{HMODULE, HWND, LPARAM, POINT, RECT},
        Graphics::{
            Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL_11_0},
            Direct3D11::{
                D3D11_BOX, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION,
                D3D11_TEXTURE2D_DESC, D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext,
                ID3D11Resource, ID3D11Texture2D,
            },
            Dxgi::{IDXGIDevice, IDXGISurface},
            Gdi::{
                EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITOR_DEFAULTTONEAREST,
                MONITORINFOEXW, MonitorFromPoint,
            },
        },
        System::WinRT::{
            Direct3D11::{CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess},
            Graphics::Capture::IGraphicsCaptureItemInterop,
        },
    },
    core::{BOOL, IInspectable, Interface, factory},
};

const MONITOR_TARGET_PREFIX: &str = "monitor:";
const WINDOW_TARGET_PREFIX: &str = "window:";

pub struct WindowsGraphicsCaptureBackend;

static WINDOWS_GRAPHICS_CAPTURE_BACKEND: WindowsGraphicsCaptureBackend =
    WindowsGraphicsCaptureBackend;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsCaptureStartPlan {
    pub target_id: String,
    pub target_label: String,
    pub source_kind: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsGraphicsCaptureExecutionPlan {
    pub target_id: String,
    pub target_label: String,
    pub item_kind: String,
    pub width: u32,
    pub height: u32,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsGraphicsCaptureRuntimeFoundation {
    pub target_label: String,
    pub item_kind: String,
    pub size: (u32, u32),
    pub capture_supported: bool,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsGraphicsCapturePreparedRuntime {
    pub target_label: String,
    pub item_kind: String,
    pub size: (u32, u32),
    pub frame_handler_registered: bool,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsGraphicsCaptureSmokeLifecycle {
    pub target_label: String,
    pub item_kind: String,
    pub size: (u32, u32),
    pub started: bool,
    pub saw_frame: bool,
    pub frames_observed: u64,
    pub summary: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WindowsGraphicsCaptureFrameMetadata {
    pub frames_observed: u64,
    pub latest_width: Option<u32>,
    pub latest_height: Option<u32>,
    pub latest_relative_time_100ns: Option<i64>,
    pub latest_surface_kind: Option<String>,
}

#[cfg(target_os = "windows")]
struct NativeRuntimeFoundationObjects {
    #[allow(dead_code)]
    capture_item: GraphicsCaptureItem,
    #[allow(dead_code)]
    d3d11_device: ID3D11Device,
    #[allow(dead_code)]
    direct3d_device: IDirect3DDevice,
    #[allow(dead_code)]
    frame_pool: Direct3D11CaptureFramePool,
    #[allow(dead_code)]
    session: GraphicsCaptureSession,
}

#[cfg(target_os = "windows")]
struct NativePreparedRuntimeObjects {
    foundation: NativeRuntimeFoundationObjects,
    frame_arrived_token: i64,
    saw_frame: Arc<AtomicBool>,
    frames_observed: Arc<AtomicU64>,
    latest_frame_metadata: Arc<Mutex<WindowsGraphicsCaptureFrameMetadata>>,
}

#[cfg(target_os = "windows")]
pub struct WindowsGraphicsCaptureController {
    active_recording: ActiveRecording,
    stop_tx: Option<Sender<()>>,
    finished_rx: Receiver<Result<RecordingArtifact, CaptureError>>,
    finished_artifact: Option<RecordingArtifact>,
    worker_handle: Option<JoinHandle<()>>,
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
    primary: bool,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CropRect {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

#[cfg(target_os = "windows")]
struct DesktopCaptureSourceObjects {
    #[allow(dead_code)]
    capture_item: GraphicsCaptureItem,
    frame_pool: Direct3D11CaptureFramePool,
    session: GraphicsCaptureSession,
    offset_x: u32,
    offset_y: u32,
    width: u32,
    height: u32,
    cached_texture: Option<ID3D11Texture2D>,
}

#[cfg(target_os = "windows")]
struct DesktopRuntimeFoundationObjects {
    #[allow(dead_code)]
    d3d11_device: ID3D11Device,
    #[allow(dead_code)]
    direct3d_device: IDirect3DDevice,
    composite_width: u32,
    composite_height: u32,
    sources: Vec<DesktopCaptureSourceObjects>,
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedTarget {
    pub label: String,
    pub source: String,
    pub offset_x: Option<i32>,
    pub offset_y: Option<i32>,
    pub video_size: Option<(u32, u32)>,
}

#[cfg(target_os = "windows")]
#[derive(Debug, Clone)]
enum CaptureItemTarget {
    Monitor { label: String, monitor: HMONITOR },
    Window { label: String, window: HWND },
}

pub fn backend() -> &'static dyn CaptureBackendFactory {
    &WINDOWS_GRAPHICS_CAPTURE_BACKEND
}

pub fn start_plan(options: &RecordingOptions) -> Result<WindowsCaptureStartPlan, CaptureError> {
    let resolved = resolve_target(options)?;
    let (width, height) = resolved.video_size.unwrap_or((0, 0));
    Ok(WindowsCaptureStartPlan {
        target_id: options.capture_target_id.clone(),
        target_label: resolved.label.clone(),
        source_kind: resolved.source.clone(),
        width: (width > 0).then_some(width),
        height: (height > 0).then_some(height),
        summary: match resolved.video_size {
            Some((width, height)) => format!(
                "Windows capture start plan would target `{}` through {} at {}x{}.",
                resolved.label, resolved.source, width, height
            ),
            None => format!(
                "Windows capture start plan would target `{}` through {}.",
                resolved.label, resolved.source
            ),
        },
    })
}

pub fn execution_plan(
    options: &RecordingOptions,
) -> Result<WindowsGraphicsCaptureExecutionPlan, CaptureError> {
    if let Some(monitors) = desktop_composite_monitors(options)? {
        let resolved = resolve_full_desktop_target(&monitors);
        let (width, height) = resolved.video_size.unwrap_or((0, 0));
        return Ok(WindowsGraphicsCaptureExecutionPlan {
            target_id: options.capture_target_id.clone(),
            target_label: resolved.label,
            item_kind: "desktop".to_string(),
            width,
            height,
            summary: format!(
                "Windows.Graphics.Capture execution plan would compose {} monitor session(s) into a full-desktop texture at {}x{}.",
                monitors.len(),
                width.max(1),
                height.max(1),
            ),
        });
    }

    let capture_target = capture_item_target(options)?;
    let resolved = resolve_target(options)?;
    let (width, height) = resolved.video_size.unwrap_or((0, 0));
    let item_kind = match capture_target {
        #[cfg(target_os = "windows")]
        CaptureItemTarget::Monitor { .. } => "monitor",
        #[cfg(target_os = "windows")]
        CaptureItemTarget::Window { .. } => "window",
        #[cfg(not(target_os = "windows"))]
        _ => "unknown",
    }
    .to_string();
    Ok(WindowsGraphicsCaptureExecutionPlan {
        target_id: options.capture_target_id.clone(),
        target_label: resolved.label.clone(),
        item_kind: item_kind.clone(),
        width,
        height,
        summary: format!(
            "Windows.Graphics.Capture execution plan would create a {item_kind} capture item for `{}` at {}x{}.",
            resolved.label,
            width.max(1),
            height.max(1),
        ),
    })
}

pub fn runtime_foundation_summary(options: &RecordingOptions) -> Option<String> {
    runtime_foundation(options)
        .ok()
        .map(|foundation| foundation.summary)
}

pub fn prepared_runtime_summary(options: &RecordingOptions) -> Option<String> {
    prepared_runtime(options)
        .ok()
        .map(|prepared| prepared.summary)
}

pub fn smoke_lifecycle_summary(options: &RecordingOptions) -> Option<String> {
    smoke_lifecycle(options).ok().map(|smoke| smoke.summary)
}

pub fn encoder_bridge_smoke_summary(options: &RecordingOptions) -> Option<String> {
    encoder_bridge_smoke(options).ok()
}

#[cfg(target_os = "windows")]
fn native_recording_runtime_supported() -> bool {
    GraphicsCaptureSession::IsSupported().unwrap_or(false)
}

#[cfg(not(target_os = "windows"))]
fn native_recording_runtime_supported() -> bool {
    false
}

fn native_recording_unsupported_reason(options: &RecordingOptions) -> Option<String> {
    let target_id = if options.capture_target_id == CUSTOM_REGION_TARGET_ID {
        options.region_source_capture_target_id.as_str()
    } else {
        options.capture_target_id.as_str()
    };

    if target_id == FULL_DESKTOP_TARGET_ID {
        let monitor_count = query_monitors().map(|monitors| monitors.len()).unwrap_or(0);
        if monitor_count == 0 {
            return Some(
                "Windows native capture controller could not resolve any monitors for the full-desktop target."
                    .to_string(),
            );
        }
    }

    None
}

impl CaptureBackendFactory for WindowsGraphicsCaptureBackend {
    fn descriptor(&self) -> CaptureBackendDescriptor {
        CaptureBackendDescriptor {
            id: "windows-graphics-capture",
            label: "Windows Graphics Capture",
        }
    }

    fn availability(&self) -> CaptureBackendAvailability {
        if !native_recording_runtime_supported() {
            CaptureBackendAvailability::Unavailable {
                reason: "Windows.Graphics.Capture is not supported in the current Windows session."
                    .to_string(),
            }
        } else {
            CaptureBackendAvailability::Available
        }
    }

    fn runtime_report(&self) -> CaptureBackendRuntimeReport {
        CaptureBackendRuntimeReport {
            summary: Some(
                "Windows native capture candidate targets Windows.Graphics.Capture. The runtime foundation can now build capture items, D3D11 devices, frame pools, capture sessions, and a native controller path backed by Media Foundation sink-writer output with WASAPI microphone and loopback bridging."
                    .to_string(),
            ),
            preferred_target_label: Some("Full desktop".to_string()),
        }
    }

    fn start(&self, options: RecordingOptions) -> Result<Box<dyn CaptureController>, CaptureError> {
        if let Some(reason) = native_recording_unsupported_reason(&options) {
            return Err(CaptureError::BackendUnavailable(reason));
        }

        WindowsGraphicsCaptureController::start(options.clone())
            .map(|controller| Box::new(controller) as Box<dyn CaptureController>)
    }
}

#[cfg(target_os = "windows")]
impl WindowsGraphicsCaptureController {
    fn start(options: RecordingOptions) -> Result<Self, CaptureError> {
        if let Some(reason) = native_recording_unsupported_reason(&options) {
            return Err(CaptureError::BackendUnavailable(reason));
        }

        let target = resolve_target(&options)?;
        let active_recording = ActiveRecording {
            backend_name: "Windows Graphics Capture".to_string(),
            encoder_label: super::native_encoder_backend::preferred_encoder_label()
                .unwrap_or_else(|| "Windows Media Foundation H.264".to_string()),
            output_path: options.output_path.clone(),
            started_at: SystemTime::now(),
            target_label: target.label,
        };
        let started_at = active_recording.started_at;
        let output_path = options.output_path.clone();
        let (stop_tx, stop_rx) = mpsc::channel();
        let (finished_tx, finished_rx) = mpsc::channel();
        let thread_options = options.clone();
        let worker_handle = thread::spawn(move || {
            let result =
                run_native_recording_thread(thread_options, started_at, output_path, stop_rx);
            let _ = finished_tx.send(result);
        });

        Ok(Self {
            active_recording,
            stop_tx: Some(stop_tx),
            finished_rx,
            finished_artifact: None,
            worker_handle: Some(worker_handle),
        })
    }
}

#[cfg(target_os = "windows")]
impl CaptureController for WindowsGraphicsCaptureController {
    fn active_recording(&self) -> &ActiveRecording {
        &self.active_recording
    }

    fn pause(&mut self) -> Result<(), CaptureError> {
        Err(CaptureError::SignalFailed(
            "Windows native capture controller does not support pause/resume yet.".to_string(),
        ))
    }

    fn resume(&mut self) -> Result<(), CaptureError> {
        Err(CaptureError::SignalFailed(
            "Windows native capture controller does not support pause/resume yet.".to_string(),
        ))
    }

    fn stop(&mut self) -> Result<RecordingArtifact, CaptureError> {
        if let Some(artifact) = self.finished_artifact.clone() {
            return Ok(artifact);
        }

        if let Some(stop_tx) = self.stop_tx.take() {
            let _ = stop_tx.send(());
        }

        let result = self
            .finished_rx
            .recv()
            .map_err(|error| CaptureError::StopFailed(error.to_string()))?;
        if let Some(worker_handle) = self.worker_handle.take() {
            let _ = worker_handle.join();
        }
        let artifact = result?;
        self.finished_artifact = Some(artifact.clone());
        Ok(artifact)
    }

    fn supports_pause_resume(&self) -> bool {
        false
    }

    fn pause_resume_note(&self) -> Option<String> {
        Some("Windows native capture does not support pause/resume yet.".to_string())
    }

    fn poll_finished(&mut self) -> Result<Option<RecordingArtifact>, CaptureError> {
        if let Some(artifact) = self.finished_artifact.clone() {
            return Ok(Some(artifact));
        }

        match self.finished_rx.try_recv() {
            Ok(result) => {
                if let Some(worker_handle) = self.worker_handle.take() {
                    let _ = worker_handle.join();
                }
                let artifact = result?;
                self.finished_artifact = Some(artifact.clone());
                Ok(Some(artifact))
            }
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => Err(CaptureError::StopFailed(
                "Windows native capture thread disconnected before finishing.".to_string(),
            )),
        }
    }
}

#[cfg(target_os = "windows")]
impl Drop for WindowsGraphicsCaptureController {
    fn drop(&mut self) {
        if let Some(stop_tx) = self.stop_tx.take() {
            let _ = stop_tx.send(());
        }
        if let Some(worker_handle) = self.worker_handle.take() {
            let _ = worker_handle.join();
        }
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
    Ok((
        target.offset_x.unwrap_or(0),
        target.offset_y.unwrap_or(0),
        width,
        height,
    ))
}

pub(crate) fn resolve_target(options: &RecordingOptions) -> Result<ResolvedTarget, CaptureError> {
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

pub fn supports_native_runtime_foundation(options: &RecordingOptions) -> bool {
    runtime_foundation(options).is_ok()
}

#[cfg(target_os = "windows")]
fn runtime_foundation(
    options: &RecordingOptions,
) -> Result<WindowsGraphicsCaptureRuntimeFoundation, CaptureError> {
    if !GraphicsCaptureSession::IsSupported().map_err(map_windows_error)? {
        return Err(CaptureError::BackendUnavailable(
            "Windows.Graphics.Capture is not supported in the current Windows session.".to_string(),
        ));
    }

    if let Some(monitors) = desktop_composite_monitors(options)? {
        let foundation = build_desktop_runtime_foundation_objects(&monitors)?;
        let size = (foundation.composite_width, foundation.composite_height);
        let monitor_count = foundation.sources.len();
        shutdown_desktop_runtime_foundation(foundation);
        return Ok(WindowsGraphicsCaptureRuntimeFoundation {
            target_label: "Full desktop".to_string(),
            item_kind: "desktop".to_string(),
            size,
            capture_supported: true,
            summary: format!(
                "Windows.Graphics.Capture runtime foundation created {} monitor session(s) for full-desktop composition at {}x{}.",
                monitor_count,
                size.0.max(1),
                size.1.max(1),
            ),
        });
    }

    let capture_target = capture_item_target(options)?;
    let foundation = build_runtime_foundation_objects(&capture_target)?;
    let item_size = foundation.capture_item.Size().map_err(map_windows_error)?;
    let target_label = match &capture_target {
        CaptureItemTarget::Monitor { label, .. } | CaptureItemTarget::Window { label, .. } => {
            label.clone()
        }
    };
    let item_kind = match capture_target {
        CaptureItemTarget::Monitor { .. } => "monitor",
        CaptureItemTarget::Window { .. } => "window",
    }
    .to_string();
    Ok(WindowsGraphicsCaptureRuntimeFoundation {
        target_label: target_label.clone(),
        item_kind: item_kind.clone(),
        size: (
            item_size.Width.max(1) as u32,
            item_size.Height.max(1) as u32,
        ),
        capture_supported: true,
        summary: format!(
            "Windows.Graphics.Capture runtime foundation created a {item_kind} capture item for `{target_label}` with a D3D11 device, frame pool, and capture session at {}x{}.",
            item_size.Width.max(1),
            item_size.Height.max(1),
        ),
    })
}

#[cfg(not(target_os = "windows"))]
fn runtime_foundation(
    _options: &RecordingOptions,
) -> Result<WindowsGraphicsCaptureRuntimeFoundation, CaptureError> {
    Err(CaptureError::BackendUnavailable(
        "Windows.Graphics.Capture runtime foundation is only available on Windows.".to_string(),
    ))
}

#[cfg(target_os = "windows")]
fn prepared_runtime(
    options: &RecordingOptions,
) -> Result<WindowsGraphicsCapturePreparedRuntime, CaptureError> {
    if let Some(monitors) = desktop_composite_monitors(options)? {
        let foundation = build_desktop_runtime_foundation_objects(&monitors)?;
        let size = (foundation.composite_width, foundation.composite_height);
        let monitor_count = foundation.sources.len();
        shutdown_desktop_runtime_foundation(foundation);

        return Ok(WindowsGraphicsCapturePreparedRuntime {
            target_label: "Full desktop".to_string(),
            item_kind: "desktop".to_string(),
            size,
            frame_handler_registered: false,
            summary: format!(
                "Windows.Graphics.Capture prepared runtime staged {} monitor session(s) for a full-desktop composite texture at {}x{}.",
                monitor_count,
                size.0.max(1),
                size.1.max(1),
            ),
        });
    }

    let capture_target = capture_item_target(options)?;
    let prepared = build_prepared_runtime_objects(&capture_target)?;
    let item_size = prepared
        .foundation
        .capture_item
        .Size()
        .map_err(map_windows_error)?;
    let target_label = match &capture_target {
        CaptureItemTarget::Monitor { label, .. } | CaptureItemTarget::Window { label, .. } => {
            label.clone()
        }
    };
    let item_kind = match capture_target {
        CaptureItemTarget::Monitor { .. } => "monitor",
        CaptureItemTarget::Window { .. } => "window",
    }
    .to_string();
    shutdown_prepared_runtime(prepared);

    Ok(WindowsGraphicsCapturePreparedRuntime {
        target_label: target_label.clone(),
        item_kind: item_kind.clone(),
        size: (
            item_size.Width.max(1) as u32,
            item_size.Height.max(1) as u32,
        ),
        frame_handler_registered: true,
        summary: format!(
            "Windows.Graphics.Capture prepared runtime registered a frame-arrived handler for `{target_label}` on a {item_kind} capture session at {}x{}.",
            item_size.Width.max(1),
            item_size.Height.max(1),
        ),
    })
}

#[cfg(not(target_os = "windows"))]
fn prepared_runtime(
    _options: &RecordingOptions,
) -> Result<WindowsGraphicsCapturePreparedRuntime, CaptureError> {
    Err(CaptureError::BackendUnavailable(
        "Windows.Graphics.Capture prepared runtime is only available on Windows.".to_string(),
    ))
}

#[cfg(target_os = "windows")]
#[allow(dead_code)]
fn smoke_lifecycle(
    options: &RecordingOptions,
) -> Result<WindowsGraphicsCaptureSmokeLifecycle, CaptureError> {
    let capture_target = capture_item_target(options)?;
    let prepared = build_prepared_runtime_objects(&capture_target)?;
    let item_size = prepared
        .foundation
        .capture_item
        .Size()
        .map_err(map_windows_error)?;
    let target_label = match &capture_target {
        CaptureItemTarget::Monitor { label, .. } | CaptureItemTarget::Window { label, .. } => {
            label.clone()
        }
    };
    let item_kind = match capture_target {
        CaptureItemTarget::Monitor { .. } => "monitor",
        CaptureItemTarget::Window { .. } => "window",
    }
    .to_string();

    prepared
        .foundation
        .session
        .StartCapture()
        .map_err(map_windows_error)?;
    thread::sleep(Duration::from_millis(250));

    let frames_observed = prepared.frames_observed.load(Ordering::Relaxed);
    let saw_frame = prepared.saw_frame.load(Ordering::Relaxed);
    let latest_frame_metadata = prepared
        .latest_frame_metadata
        .lock()
        .map(|metadata| metadata.clone())
        .unwrap_or_default();
    shutdown_prepared_runtime(prepared);

    Ok(WindowsGraphicsCaptureSmokeLifecycle {
        target_label: target_label.clone(),
        item_kind: item_kind.clone(),
        size: (
            item_size.Width.max(1) as u32,
            item_size.Height.max(1) as u32,
        ),
        started: true,
        saw_frame,
        frames_observed,
        summary: format!(
            "Windows.Graphics.Capture smoke lifecycle started and stopped a {item_kind} session for `{target_label}` at {}x{}, saw_frame={}, observed {frames_observed} frame event(s), latest_frame={}x{}, latest_time_100ns={}, latest_surface={}.",
            item_size.Width.max(1),
            item_size.Height.max(1),
            saw_frame,
            latest_frame_metadata.latest_width.unwrap_or(0),
            latest_frame_metadata.latest_height.unwrap_or(0),
            latest_frame_metadata
                .latest_relative_time_100ns
                .map(|value| value.to_string())
                .unwrap_or_else(|| "n/a".to_string()),
            latest_frame_metadata
                .latest_surface_kind
                .clone()
                .unwrap_or_else(|| "n/a".to_string()),
        ),
    })
}

#[cfg(not(target_os = "windows"))]
fn smoke_lifecycle(
    _options: &RecordingOptions,
) -> Result<WindowsGraphicsCaptureSmokeLifecycle, CaptureError> {
    Err(CaptureError::BackendUnavailable(
        "Windows.Graphics.Capture smoke lifecycle is only available on Windows.".to_string(),
    ))
}

#[cfg(target_os = "windows")]
fn encoder_bridge_smoke(options: &RecordingOptions) -> Result<String, CaptureError> {
    let capture_target = capture_item_target(options)?;
    let foundation = build_runtime_foundation_objects(&capture_target)?;
    foundation
        .session
        .StartCapture()
        .map_err(map_windows_error)?;

    let result = capture_first_frame_and_write_sample(options, &foundation);
    shutdown_runtime_foundation(foundation);
    result
}

#[cfg(not(target_os = "windows"))]
fn encoder_bridge_smoke(_options: &RecordingOptions) -> Result<String, CaptureError> {
    Err(CaptureError::BackendUnavailable(
        "Windows WGC -> Media Foundation smoke is only available on Windows.".to_string(),
    ))
}

#[cfg(target_os = "windows")]
fn build_runtime_foundation_objects(
    capture_target: &CaptureItemTarget,
) -> Result<NativeRuntimeFoundationObjects, CaptureError> {
    let capture_item = build_capture_item_for_target(capture_target)?;
    let d3d11_device = build_d3d11_device()?;
    let direct3d_device = build_direct3d_device(&d3d11_device)?;
    let item_size = capture_item.Size().map_err(map_windows_error)?;
    let frame_pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
        &direct3d_device,
        DirectXPixelFormat::B8G8R8A8UIntNormalized,
        2,
        item_size,
    )
    .map_err(map_windows_error)?;
    let session = frame_pool
        .CreateCaptureSession(&capture_item)
        .map_err(map_windows_error)?;
    let _ = session.SetIsCursorCaptureEnabled(true);
    let _ = session.SetIsBorderRequired(false);

    Ok(NativeRuntimeFoundationObjects {
        capture_item,
        d3d11_device,
        direct3d_device,
        frame_pool,
        session,
    })
}

#[cfg(target_os = "windows")]
fn run_native_recording_thread(
    options: RecordingOptions,
    started_at: SystemTime,
    output_path: std::path::PathBuf,
    stop_rx: Receiver<()>,
) -> Result<RecordingArtifact, CaptureError> {
    if let Some(monitors) = desktop_composite_monitors(&options)? {
        return run_multi_monitor_native_recording_thread(
            options,
            started_at,
            output_path,
            stop_rx,
            monitors,
        );
    }

    let capture_target = capture_item_target(&options)?;
    let foundation = build_runtime_foundation_objects(&capture_target)?;
    let item_size = foundation.capture_item.Size().map_err(map_windows_error)?;
    let encoder_video_size = encoder_video_size_from_source(
        &options,
        item_size.Width.max(1) as u32,
        item_size.Height.max(1) as u32,
    );
    let available_audio_inputs = crate::list_audio_inputs();
    let mut microphone_worker = if options.mic_enabled {
        match super::native_audio_backend::start_microphone_worker_for_input(
            &options.audio_input_id,
            &available_audio_inputs,
        ) {
            Ok(worker) => Some(worker),
            Err(error) => {
                shutdown_runtime_foundation(foundation);
                return Err(CaptureError::BackendUnavailable(format!(
                    "Windows WASAPI microphone worker could not start: {error}"
                )));
            }
        }
    } else {
        None
    };
    let mut loopback_worker = if options.system_audio_enabled {
        match super::native_audio_backend::start_default_loopback_worker() {
            Ok(worker) => Some(worker),
            Err(error) => {
                shutdown_runtime_foundation(foundation);
                return Err(CaptureError::BackendUnavailable(format!(
                    "Windows WASAPI loopback worker could not start: {error}"
                )));
            }
        }
    } else {
        None
    };

    let selected_audio_foundation = match (
        microphone_worker.as_ref().map(|worker| worker.foundation()),
        loopback_worker.as_ref().map(|worker| worker.foundation()),
    ) {
        (Some(microphone), Some(loopback)) => {
            ensure_mix_compatible_foundations(microphone, loopback)?;
            Some(microphone)
        }
        (Some(microphone), None) => Some(microphone),
        (None, Some(loopback)) => Some(loopback),
        (None, None) => None,
    };
    let selected_audio_foundation = selected_audio_foundation.cloned();

    let writer_foundation = match super::native_encoder_backend::start_sink_writer_recording(
        &options,
        selected_audio_foundation.as_ref(),
        encoder_video_size,
    ) {
        Ok(foundation_writer) => foundation_writer,
        Err(error) => {
            drop(microphone_worker.take());
            drop(loopback_worker.take());
            shutdown_runtime_foundation(foundation);
            return Err(error);
        }
    };
    let mut first_sample_time_100ns = None;
    let mut microphone_queue = VecDeque::new();
    let mut loopback_queue = VecDeque::new();
    let mut audio_samples_written = false;
    let mut video_samples_written = false;

    let recording_result = foundation
        .session
        .StartCapture()
        .map_err(map_windows_error)
        .and_then(|_| {
            loop {
                match stop_rx.try_recv() {
                    Ok(_) | Err(mpsc::TryRecvError::Disconnected) => break,
                    Err(mpsc::TryRecvError::Empty) => {}
                }

                if let Ok(frame) = foundation.frame_pool.TryGetNextFrame() {
                    let frame_result = process_recording_frame(
                        &frame,
                        &options,
                        &foundation.d3d11_device,
                        &writer_foundation,
                        &mut first_sample_time_100ns,
                    );
                    let _ = frame.Close();
                    frame_result?;
                    video_samples_written = true;
                }

                flush_pending_audio_packets(
                    &options,
                    &writer_foundation,
                    microphone_worker.as_mut(),
                    loopback_worker.as_mut(),
                    &mut microphone_queue,
                    &mut loopback_queue,
                    &mut audio_samples_written,
                    &mut video_samples_written,
                )?;

                thread::sleep(Duration::from_millis(16));
            }

            Ok(())
        });

    flush_pending_audio_packets(
        &options,
        &writer_foundation,
        microphone_worker.as_mut(),
        loopback_worker.as_mut(),
        &mut microphone_queue,
        &mut loopback_queue,
        &mut audio_samples_written,
        &mut video_samples_written,
    )?;
    write_fallback_video_sample_if_needed(&writer_foundation, video_samples_written)?;

    let microphone_stop_result = match microphone_worker.take() {
        Some(worker) => worker
            .stop()
            .map_err(|error| CaptureError::StopFailed(error.to_string())),
        None => Ok(super::native_audio_backend::WindowsWasapiPacketStats::default()),
    };
    let loopback_stop_result = match loopback_worker.take() {
        Some(worker) => worker
            .stop()
            .map_err(|error| CaptureError::StopFailed(error.to_string())),
        None => Ok(super::native_audio_backend::WindowsWasapiPacketStats::default()),
    };

    write_silent_audio_sample_if_needed(
        &writer_foundation,
        selected_audio_foundation.as_ref(),
        audio_samples_written,
    )?;
    shutdown_runtime_foundation(foundation);
    let finalize_result =
        super::native_encoder_backend::finalize_sink_writer_recording(writer_foundation);

    if let Err(error) = recording_result {
        let _ = microphone_stop_result;
        let _ = loopback_stop_result;
        let _ = finalize_result;
        return Err(error);
    }

    microphone_stop_result?;
    loopback_stop_result?;

    finalize_result?;
    build_recording_artifact(&output_path, started_at, SystemTime::now())
}

#[cfg(target_os = "windows")]
fn run_multi_monitor_native_recording_thread(
    options: RecordingOptions,
    started_at: SystemTime,
    output_path: std::path::PathBuf,
    stop_rx: Receiver<()>,
    monitors: Vec<MonitorDescriptor>,
) -> Result<RecordingArtifact, CaptureError> {
    let mut foundation = build_desktop_runtime_foundation_objects(&monitors)?;
    let encoder_video_size = encoder_video_size_from_source(
        &options,
        foundation.composite_width,
        foundation.composite_height,
    );
    let available_audio_inputs = crate::list_audio_inputs();
    let mut microphone_worker = if options.mic_enabled {
        match super::native_audio_backend::start_microphone_worker_for_input(
            &options.audio_input_id,
            &available_audio_inputs,
        ) {
            Ok(worker) => Some(worker),
            Err(error) => {
                shutdown_desktop_runtime_foundation(foundation);
                return Err(CaptureError::BackendUnavailable(format!(
                    "Windows WASAPI microphone worker could not start: {error}"
                )));
            }
        }
    } else {
        None
    };
    let mut loopback_worker = if options.system_audio_enabled {
        match super::native_audio_backend::start_default_loopback_worker() {
            Ok(worker) => Some(worker),
            Err(error) => {
                shutdown_desktop_runtime_foundation(foundation);
                return Err(CaptureError::BackendUnavailable(format!(
                    "Windows WASAPI loopback worker could not start: {error}"
                )));
            }
        }
    } else {
        None
    };

    let selected_audio_foundation = match (
        microphone_worker.as_ref().map(|worker| worker.foundation()),
        loopback_worker.as_ref().map(|worker| worker.foundation()),
    ) {
        (Some(microphone), Some(loopback)) => {
            ensure_mix_compatible_foundations(microphone, loopback)?;
            Some(microphone)
        }
        (Some(microphone), None) => Some(microphone),
        (None, Some(loopback)) => Some(loopback),
        (None, None) => None,
    };
    let selected_audio_foundation = selected_audio_foundation.cloned();

    let writer_foundation = match super::native_encoder_backend::start_sink_writer_recording(
        &options,
        selected_audio_foundation.as_ref(),
        encoder_video_size,
    ) {
        Ok(foundation_writer) => foundation_writer,
        Err(error) => {
            drop(microphone_worker.take());
            drop(loopback_worker.take());
            shutdown_desktop_runtime_foundation(foundation);
            return Err(error);
        }
    };
    let mut first_sample_time_100ns = None;
    let mut microphone_queue = VecDeque::new();
    let mut loopback_queue = VecDeque::new();
    let mut audio_samples_written = false;
    let mut video_samples_written = false;

    let recording_result = start_desktop_sessions(&foundation).and_then(|_| {
        loop {
            match stop_rx.try_recv() {
                Ok(_) | Err(mpsc::TryRecvError::Disconnected) => break,
                Err(mpsc::TryRecvError::Empty) => {}
            }

            if let Some(relative_time) = poll_and_write_desktop_composite_frame(
                &options,
                &mut foundation,
                &writer_foundation,
                &mut first_sample_time_100ns,
            )? {
                let _ = relative_time;
                video_samples_written = true;
            }

            flush_pending_audio_packets(
                &options,
                &writer_foundation,
                microphone_worker.as_mut(),
                loopback_worker.as_mut(),
                &mut microphone_queue,
                &mut loopback_queue,
                &mut audio_samples_written,
                &mut video_samples_written,
            )?;

            thread::sleep(Duration::from_millis(16));
        }

        Ok(())
    });

    flush_pending_audio_packets(
        &options,
        &writer_foundation,
        microphone_worker.as_mut(),
        loopback_worker.as_mut(),
        &mut microphone_queue,
        &mut loopback_queue,
        &mut audio_samples_written,
        &mut video_samples_written,
    )?;
    write_fallback_video_sample_if_needed(&writer_foundation, video_samples_written)?;

    let microphone_stop_result = match microphone_worker.take() {
        Some(worker) => worker
            .stop()
            .map_err(|error| CaptureError::StopFailed(error.to_string())),
        None => Ok(super::native_audio_backend::WindowsWasapiPacketStats::default()),
    };
    let loopback_stop_result = match loopback_worker.take() {
        Some(worker) => worker
            .stop()
            .map_err(|error| CaptureError::StopFailed(error.to_string())),
        None => Ok(super::native_audio_backend::WindowsWasapiPacketStats::default()),
    };

    write_silent_audio_sample_if_needed(
        &writer_foundation,
        selected_audio_foundation.as_ref(),
        audio_samples_written,
    )?;
    shutdown_desktop_runtime_foundation(foundation);
    let finalize_result =
        super::native_encoder_backend::finalize_sink_writer_recording(writer_foundation);

    if let Err(error) = recording_result {
        let _ = microphone_stop_result;
        let _ = loopback_stop_result;
        let _ = finalize_result;
        return Err(error);
    }

    microphone_stop_result?;
    loopback_stop_result?;
    finalize_result?;
    build_recording_artifact(&output_path, started_at, SystemTime::now())
}

#[cfg(target_os = "windows")]
fn process_recording_frame(
    frame: &windows::Graphics::Capture::Direct3D11CaptureFrame,
    options: &RecordingOptions,
    d3d11_device: &ID3D11Device,
    writer_foundation: &super::native_encoder_backend::NativeSinkWriterFoundation,
    first_sample_time_100ns: &mut Option<i64>,
) -> Result<(), CaptureError> {
    let surface = frame.Surface().map_err(map_windows_error)?;
    let content_size = frame.ContentSize().map_err(map_windows_error)?;
    let relative_time = frame
        .SystemRelativeTime()
        .map(|time| time.Duration)
        .unwrap_or(0);
    let first_time = first_sample_time_100ns.get_or_insert(relative_time);
    let normalized_sample_time_100ns = relative_time.saturating_sub(*first_time);

    if let Some(crop_rect) = encoder_crop_rect_from_source(
        options,
        content_size.Width.max(1) as u32,
        content_size.Height.max(1) as u32,
    ) {
        let cropped_texture = crop_surface_to_texture(&surface, d3d11_device, crop_rect)?;
        super::native_encoder_backend::write_texture_sample(
            writer_foundation,
            &cropped_texture,
            normalized_sample_time_100ns,
        )?;
    } else {
        super::native_encoder_backend::write_surface_sample(
            writer_foundation,
            &surface,
            normalized_sample_time_100ns,
        )?;
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn desktop_composite_monitors(
    options: &RecordingOptions,
) -> Result<Option<Vec<MonitorDescriptor>>, CaptureError> {
    let target_id = if options.capture_target_id == CUSTOM_REGION_TARGET_ID {
        options.region_source_capture_target_id.as_str()
    } else {
        options.capture_target_id.as_str()
    };

    if target_id != FULL_DESKTOP_TARGET_ID {
        return Ok(None);
    }

    let monitors = query_monitors()?;
    if monitors.len() > 1 {
        Ok(Some(monitors))
    } else {
        Ok(None)
    }
}

#[cfg(target_os = "windows")]
fn build_desktop_runtime_foundation_objects(
    monitors: &[MonitorDescriptor],
) -> Result<DesktopRuntimeFoundationObjects, CaptureError> {
    let d3d11_device = build_d3d11_device()?;
    let direct3d_device = build_direct3d_device(&d3d11_device)?;
    let min_x = monitors.iter().map(|monitor| monitor.x).min().unwrap_or(0);
    let min_y = monitors.iter().map(|monitor| monitor.y).min().unwrap_or(0);
    let max_x = monitors
        .iter()
        .map(|monitor| monitor.x + monitor.width as i32)
        .max()
        .unwrap_or(min_x);
    let max_y = monitors
        .iter()
        .map(|monitor| monitor.y + monitor.height as i32)
        .max()
        .unwrap_or(min_y);
    let composite_width = (max_x - min_x).max(1) as u32;
    let composite_height = (max_y - min_y).max(1) as u32;

    let mut sources = Vec::with_capacity(monitors.len());
    for monitor in monitors {
        let capture_target = monitor_capture_target(monitor)?;
        let capture_item = build_capture_item_for_target(&capture_target)?;
        let item_size = capture_item.Size().map_err(map_windows_error)?;
        let frame_pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &direct3d_device,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            2,
            item_size,
        )
        .map_err(map_windows_error)?;
        let session = frame_pool
            .CreateCaptureSession(&capture_item)
            .map_err(map_windows_error)?;
        let _ = session.SetIsCursorCaptureEnabled(true);
        let _ = session.SetIsBorderRequired(false);
        sources.push(DesktopCaptureSourceObjects {
            capture_item,
            frame_pool,
            session,
            offset_x: (monitor.x - min_x).max(0) as u32,
            offset_y: (monitor.y - min_y).max(0) as u32,
            width: item_size.Width.max(1) as u32,
            height: item_size.Height.max(1) as u32,
            cached_texture: None,
        });
    }

    Ok(DesktopRuntimeFoundationObjects {
        d3d11_device,
        direct3d_device,
        composite_width,
        composite_height,
        sources,
    })
}

#[cfg(target_os = "windows")]
fn start_desktop_sessions(
    foundation: &DesktopRuntimeFoundationObjects,
) -> Result<(), CaptureError> {
    for source in &foundation.sources {
        source.session.StartCapture().map_err(map_windows_error)?;
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn poll_and_write_desktop_composite_frame(
    options: &RecordingOptions,
    foundation: &mut DesktopRuntimeFoundationObjects,
    writer_foundation: &super::native_encoder_backend::NativeSinkWriterFoundation,
    first_sample_time_100ns: &mut Option<i64>,
) -> Result<Option<i64>, CaptureError> {
    let mut latest_relative_time: Option<i64> = None;
    let mut saw_new_frame = false;

    for source in &mut foundation.sources {
        while let Ok(frame) = source.frame_pool.TryGetNextFrame() {
            let surface = frame.Surface().map_err(map_windows_error)?;
            let relative_time = frame
                .SystemRelativeTime()
                .map(|time| time.Duration)
                .unwrap_or(0);
            let cached_texture = copy_surface_to_cached_texture(
                &surface,
                &foundation.d3d11_device,
                source.cached_texture.take(),
            )?;
            source.cached_texture = Some(cached_texture);
            latest_relative_time = Some(
                latest_relative_time
                    .map(|current| current.max(relative_time))
                    .unwrap_or(relative_time),
            );
            saw_new_frame = true;
            let _ = frame.Close();
        }
    }

    if !saw_new_frame {
        return Ok(None);
    }

    let relative_time = latest_relative_time.unwrap_or(0);
    let first_time = first_sample_time_100ns.get_or_insert(relative_time);
    let normalized_sample_time_100ns = relative_time.saturating_sub(*first_time);
    let crop_rect = encoder_crop_rect_from_source(
        options,
        foundation.composite_width,
        foundation.composite_height,
    );
    let composed_texture = compose_desktop_texture(foundation, crop_rect)?;
    super::native_encoder_backend::write_texture_sample(
        writer_foundation,
        &composed_texture,
        normalized_sample_time_100ns,
    )?;
    Ok(Some(relative_time))
}

#[cfg(target_os = "windows")]
fn copy_surface_to_cached_texture(
    surface: &IDirect3DSurface,
    d3d11_device: &ID3D11Device,
    existing_texture: Option<ID3D11Texture2D>,
) -> Result<ID3D11Texture2D, CaptureError> {
    let interface_access: IDirect3DDxgiInterfaceAccess =
        surface.cast().map_err(map_windows_error)?;
    let source_texture = unsafe {
        interface_access
            .GetInterface::<ID3D11Texture2D>()
            .map_err(map_windows_error)?
    };

    let cached_texture = match existing_texture {
        Some(texture) => texture,
        None => clone_texture_shape(d3d11_device, &source_texture, None, None)?,
    };

    let mut source_desc = D3D11_TEXTURE2D_DESC::default();
    unsafe {
        source_texture.GetDesc(&mut source_desc as *mut _);
    }
    let device_context: ID3D11DeviceContext =
        unsafe { d3d11_device.GetImmediateContext() }.map_err(map_windows_error)?;
    let source_resource: ID3D11Resource = source_texture.cast().map_err(map_windows_error)?;
    let cached_resource: ID3D11Resource = cached_texture.cast().map_err(map_windows_error)?;
    let copy_box = D3D11_BOX {
        left: 0,
        top: 0,
        front: 0,
        right: source_desc.Width,
        bottom: source_desc.Height,
        back: 1,
    };
    unsafe {
        device_context.CopySubresourceRegion(
            &cached_resource,
            0,
            0,
            0,
            0,
            &source_resource,
            0,
            Some(&copy_box),
        );
    }

    Ok(cached_texture)
}

#[cfg(target_os = "windows")]
fn compose_desktop_texture(
    foundation: &DesktopRuntimeFoundationObjects,
    crop_rect: Option<CropRect>,
) -> Result<ID3D11Texture2D, CaptureError> {
    let reference_texture = foundation
        .sources
        .iter()
        .find_map(|source| source.cached_texture.as_ref())
        .ok_or_else(|| {
            CaptureError::BackendUnavailable(
                "Windows native desktop composite did not receive any monitor frames yet."
                    .to_string(),
            )
        })?;
    let composite_width = crop_rect
        .map(|rect| rect.width)
        .unwrap_or(foundation.composite_width);
    let composite_height = crop_rect
        .map(|rect| rect.height)
        .unwrap_or(foundation.composite_height);
    let composite_texture = clone_texture_shape(
        &foundation.d3d11_device,
        reference_texture,
        Some(composite_width),
        Some(composite_height),
    )?;
    let composite_resource: ID3D11Resource = composite_texture.cast().map_err(map_windows_error)?;
    let device_context: ID3D11DeviceContext =
        unsafe { foundation.d3d11_device.GetImmediateContext() }.map_err(map_windows_error)?;

    for source in &foundation.sources {
        let Some(texture) = source.cached_texture.as_ref() else {
            continue;
        };
        let source_resource: ID3D11Resource = texture.cast().map_err(map_windows_error)?;
        let source_x = source.offset_x;
        let source_y = source.offset_y;
        let mut dest_x = source_x;
        let mut dest_y = source_y;
        let mut src_left = 0;
        let mut src_top = 0;
        let mut copy_width = source.width;
        let mut copy_height = source.height;

        if let Some(rect) = crop_rect {
            let right = source_x.saturating_add(source.width);
            let bottom = source_y.saturating_add(source.height);
            let crop_right = rect.x.saturating_add(rect.width);
            let crop_bottom = rect.y.saturating_add(rect.height);
            if right <= rect.x
                || bottom <= rect.y
                || source_x >= crop_right
                || source_y >= crop_bottom
            {
                continue;
            }

            let intersect_left = source_x.max(rect.x);
            let intersect_top = source_y.max(rect.y);
            let intersect_right = right.min(crop_right);
            let intersect_bottom = bottom.min(crop_bottom);
            src_left = intersect_left.saturating_sub(source_x);
            src_top = intersect_top.saturating_sub(source_y);
            copy_width = intersect_right.saturating_sub(intersect_left);
            copy_height = intersect_bottom.saturating_sub(intersect_top);
            dest_x = intersect_left.saturating_sub(rect.x);
            dest_y = intersect_top.saturating_sub(rect.y);
        }

        if copy_width == 0 || copy_height == 0 {
            continue;
        }

        let copy_box = D3D11_BOX {
            left: src_left,
            top: src_top,
            front: 0,
            right: src_left + copy_width,
            bottom: src_top + copy_height,
            back: 1,
        };
        unsafe {
            device_context.CopySubresourceRegion(
                &composite_resource,
                0,
                dest_x,
                dest_y,
                0,
                &source_resource,
                0,
                Some(&copy_box),
            );
        }
    }

    Ok(composite_texture)
}

#[cfg(target_os = "windows")]
fn clone_texture_shape(
    d3d11_device: &ID3D11Device,
    reference_texture: &ID3D11Texture2D,
    width_override: Option<u32>,
    height_override: Option<u32>,
) -> Result<ID3D11Texture2D, CaptureError> {
    let mut desc = D3D11_TEXTURE2D_DESC::default();
    unsafe {
        reference_texture.GetDesc(&mut desc as *mut _);
    }
    desc.Width = width_override.unwrap_or(desc.Width).max(1);
    desc.Height = height_override.unwrap_or(desc.Height).max(1);
    desc.MipLevels = 1;
    desc.ArraySize = 1;

    let mut texture = None;
    unsafe {
        d3d11_device
            .CreateTexture2D(&desc as *const _, None, Some(&mut texture))
            .map_err(map_windows_error)?;
    }
    texture.ok_or_else(|| {
        CaptureError::BackendUnavailable(
            "Windows native capture controller could not allocate a D3D11 texture.".to_string(),
        )
    })
}

fn encoder_video_size_from_source(
    options: &RecordingOptions,
    source_width: u32,
    source_height: u32,
) -> (u32, u32) {
    match encoder_crop_rect_from_source(options, source_width, source_height) {
        Some(crop_rect) => (crop_rect.width, crop_rect.height),
        None => (
            normalize_encoder_dimension(source_width, source_width),
            normalize_encoder_dimension(source_height, source_height),
        ),
    }
}

fn encoder_crop_rect_from_source(
    options: &RecordingOptions,
    source_width: u32,
    source_height: u32,
) -> Option<CropRect> {
    let base_crop = build_native_crop_rect_from_dimensions(options, source_width, source_height);
    let normalized_crop = normalize_encoder_crop_rect(
        base_crop.unwrap_or(CropRect {
            x: 0,
            y: 0,
            width: source_width.max(1),
            height: source_height.max(1),
        }),
        source_width,
        source_height,
    );

    let needs_texture_copy = base_crop.is_some()
        || normalized_crop.x != 0
        || normalized_crop.y != 0
        || normalized_crop.width != source_width.max(1)
        || normalized_crop.height != source_height.max(1);

    needs_texture_copy.then_some(normalized_crop)
}

fn normalize_encoder_crop_rect(
    crop_rect: CropRect,
    source_width: u32,
    source_height: u32,
) -> CropRect {
    let max_width = source_width.saturating_sub(crop_rect.x).max(1);
    let max_height = source_height.saturating_sub(crop_rect.y).max(1);

    CropRect {
        x: crop_rect.x.min(source_width.saturating_sub(1)),
        y: crop_rect.y.min(source_height.saturating_sub(1)),
        width: normalize_encoder_dimension(crop_rect.width.min(max_width), max_width),
        height: normalize_encoder_dimension(crop_rect.height.min(max_height), max_height),
    }
}

fn normalize_encoder_dimension(value: u32, max_value: u32) -> u32 {
    let capped = value.min(max_value).max(1);
    if capped > 2 && capped % 2 != 0 {
        capped - 1
    } else {
        capped
    }
}

fn build_native_crop_rect_from_dimensions(
    options: &RecordingOptions,
    source_width: u32,
    source_height: u32,
) -> Option<CropRect> {
    if options.capture_target_id != CUSTOM_REGION_TARGET_ID {
        return None;
    }

    let source_scale = (options.region_source_scale_factor_milli.max(1) as f64) / 1000.0;
    if source_scale <= 0.0 || source_width == 0 || source_height == 0 {
        return None;
    }

    let x =
        ((options.region_x as i32 - options.region_source_origin_x).max(0) as f64) / source_scale;
    let y =
        ((options.region_y as i32 - options.region_source_origin_y).max(0) as f64) / source_scale;
    if x >= source_width as f64 || y >= source_height as f64 {
        return None;
    }

    let width = ((options.region_width.max(1) as f64) / source_scale)
        .max(1.0)
        .min(source_width as f64 - x);
    let height = ((options.region_height.max(1) as f64) / source_scale)
        .max(1.0)
        .min(source_height as f64 - y);

    Some(CropRect {
        x: x.floor() as u32,
        y: y.floor() as u32,
        width: width.floor().max(1.0) as u32,
        height: height.floor().max(1.0) as u32,
    })
}

#[cfg(target_os = "windows")]
fn crop_surface_to_texture(
    surface: &IDirect3DSurface,
    d3d11_device: &ID3D11Device,
    crop_rect: CropRect,
) -> Result<ID3D11Texture2D, CaptureError> {
    let interface_access: IDirect3DDxgiInterfaceAccess =
        surface.cast().map_err(map_windows_error)?;
    let source_texture = unsafe {
        interface_access
            .GetInterface::<ID3D11Texture2D>()
            .map_err(map_windows_error)?
    };

    let mut source_desc = D3D11_TEXTURE2D_DESC::default();
    unsafe {
        source_texture.GetDesc(&mut source_desc as *mut _);
    }

    let crop_width = crop_rect
        .width
        .min(source_desc.Width.saturating_sub(crop_rect.x));
    let crop_height = crop_rect
        .height
        .min(source_desc.Height.saturating_sub(crop_rect.y));
    if crop_width == 0 || crop_height == 0 {
        return Err(CaptureError::BackendUnavailable(
            "Windows native custom region crop resolved to an empty D3D11 texture.".to_string(),
        ));
    }

    source_desc.Width = crop_width;
    source_desc.Height = crop_height;
    source_desc.MipLevels = 1;
    source_desc.ArraySize = 1;

    let mut cropped_texture = None;
    unsafe {
        d3d11_device
            .CreateTexture2D(&source_desc as *const _, None, Some(&mut cropped_texture))
            .map_err(map_windows_error)?;
    }
    let cropped_texture = cropped_texture.ok_or_else(|| {
        CaptureError::BackendUnavailable(
            "Windows native capture controller could not create a cropped D3D11 texture for custom-region capture."
                .to_string(),
        )
    })?;
    let device_context: ID3D11DeviceContext =
        unsafe { d3d11_device.GetImmediateContext() }.map_err(map_windows_error)?;
    let source_resource: ID3D11Resource = source_texture.cast().map_err(map_windows_error)?;
    let cropped_resource: ID3D11Resource = cropped_texture.cast().map_err(map_windows_error)?;
    let crop_box = D3D11_BOX {
        left: crop_rect.x,
        top: crop_rect.y,
        front: 0,
        right: crop_rect.x + crop_width,
        bottom: crop_rect.y + crop_height,
        back: 1,
    };
    unsafe {
        device_context.CopySubresourceRegion(
            &cropped_resource,
            0,
            0,
            0,
            0,
            &source_resource,
            0,
            Some(&crop_box),
        );
    }

    Ok(cropped_texture)
}

#[cfg(target_os = "windows")]
fn build_recording_artifact(
    output_path: &std::path::Path,
    started_at: SystemTime,
    finished_at: SystemTime,
) -> Result<RecordingArtifact, CaptureError> {
    let metadata = fs::metadata(output_path)
        .map_err(|error| CaptureError::OutputInspectionFailed(error.to_string()))?;
    Ok(RecordingArtifact {
        output_path: output_path.to_path_buf(),
        started_at,
        finished_at,
        duration: finished_at.duration_since(started_at).unwrap_or_default(),
        bytes_written: metadata.len(),
    })
}

#[cfg(target_os = "windows")]
fn flush_pending_audio_packets(
    options: &RecordingOptions,
    writer_foundation: &super::native_encoder_backend::NativeSinkWriterFoundation,
    microphone_worker: Option<&mut super::native_audio_backend::WindowsWasapiCaptureWorker>,
    loopback_worker: Option<&mut super::native_audio_backend::WindowsWasapiCaptureWorker>,
    microphone_queue: &mut VecDeque<super::native_audio_backend::WindowsWasapiAudioPacket>,
    loopback_queue: &mut VecDeque<super::native_audio_backend::WindowsWasapiAudioPacket>,
    audio_samples_written: &mut bool,
    video_samples_written: &mut bool,
) -> Result<(), CaptureError> {
    if let Some(worker) = microphone_worker {
        while let Some(packet) = worker.try_recv_packet() {
            microphone_queue.push_back(packet);
        }
    }

    if let Some(worker) = loopback_worker {
        while let Some(packet) = worker.try_recv_packet() {
            loopback_queue.push_back(packet);
        }
    }

    match (options.mic_enabled, options.system_audio_enabled) {
        (true, true) => {
            while let Some(packet) = try_mix_audio_packets(microphone_queue, loopback_queue)? {
                ensure_video_stream_seeded_for_audio(
                    writer_foundation,
                    video_samples_written,
                )?;
                super::native_encoder_backend::write_audio_sample(writer_foundation, &packet)?;
                *audio_samples_written = true;
            }
        }
        (true, false) => {
            while let Some(packet) = microphone_queue.pop_front() {
                ensure_video_stream_seeded_for_audio(
                    writer_foundation,
                    video_samples_written,
                )?;
                super::native_encoder_backend::write_audio_sample(writer_foundation, &packet)?;
                *audio_samples_written = true;
            }
        }
        (false, true) => {
            while let Some(packet) = loopback_queue.pop_front() {
                ensure_video_stream_seeded_for_audio(
                    writer_foundation,
                    video_samples_written,
                )?;
                super::native_encoder_backend::write_audio_sample(writer_foundation, &packet)?;
                *audio_samples_written = true;
            }
        }
        (false, false) => {}
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn ensure_video_stream_seeded_for_audio(
    writer_foundation: &super::native_encoder_backend::NativeSinkWriterFoundation,
    video_samples_written: &mut bool,
) -> Result<(), CaptureError> {
    if *video_samples_written {
        return Ok(());
    }

    super::native_encoder_backend::write_black_video_sample(writer_foundation, 0)?;
    *video_samples_written = true;
    Ok(())
}

#[cfg(target_os = "windows")]
fn write_silent_audio_sample_if_needed(
    writer_foundation: &super::native_encoder_backend::NativeSinkWriterFoundation,
    audio_foundation: Option<&super::native_audio_backend::WindowsWasapiClientFoundation>,
    audio_samples_written: bool,
) -> Result<(), CaptureError> {
    let Some(audio_foundation) = audio_foundation else {
        return Ok(());
    };
    if audio_samples_written {
        return Ok(());
    }

    let frames = audio_foundation
        .buffer_frames
        .max(audio_foundation.sample_rate_hz / 20)
        .max(1);
    let bytes_per_frame = u32::from(audio_foundation.channels)
        .saturating_mul(u32::from(audio_foundation.bits_per_sample.max(8)) / 8)
        .max(1);
    let packet = super::native_audio_backend::WindowsWasapiAudioPacket {
        sample_time_100ns: 0,
        duration_100ns: ((frames as u64) * 10_000_000
            / audio_foundation.sample_rate_hz.max(1) as u64) as i64,
        frames,
        bytes: vec![0u8; frames.saturating_mul(bytes_per_frame) as usize],
    };
    super::native_encoder_backend::write_audio_sample(writer_foundation, &packet)?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn write_fallback_video_sample_if_needed(
    writer_foundation: &super::native_encoder_backend::NativeSinkWriterFoundation,
    video_samples_written: bool,
) -> Result<(), CaptureError> {
    if video_samples_written {
        return Ok(());
    }

    super::native_encoder_backend::write_black_video_sample(writer_foundation, 0)
}

#[cfg(target_os = "windows")]
fn ensure_mix_compatible_foundations(
    microphone: &super::native_audio_backend::WindowsWasapiClientFoundation,
    loopback: &super::native_audio_backend::WindowsWasapiClientFoundation,
) -> Result<(), CaptureError> {
    if microphone.sample_rate_hz != loopback.sample_rate_hz
        || microphone.channels != loopback.channels
        || microphone.bits_per_sample != loopback.bits_per_sample
        || microphone.bits_per_sample != 16
    {
        return Err(CaptureError::BackendUnavailable(
            "Windows native mic+loopback mixing currently requires matching 16-bit PCM WASAPI formats."
                .to_string(),
        ));
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn try_mix_audio_packets(
    microphone_queue: &mut VecDeque<super::native_audio_backend::WindowsWasapiAudioPacket>,
    loopback_queue: &mut VecDeque<super::native_audio_backend::WindowsWasapiAudioPacket>,
) -> Result<Option<super::native_audio_backend::WindowsWasapiAudioPacket>, CaptureError> {
    let Some(microphone_packet) = microphone_queue.front() else {
        return Ok(None);
    };
    let Some(loopback_packet) = loopback_queue.front() else {
        return Ok(None);
    };

    if microphone_packet.sample_time_100ns < loopback_packet.sample_time_100ns {
        let _ = microphone_queue.pop_front();
        return Ok(None);
    }

    if loopback_packet.sample_time_100ns < microphone_packet.sample_time_100ns {
        let _ = loopback_queue.pop_front();
        return Ok(None);
    }

    let microphone_packet = microphone_queue.pop_front().expect("front checked");
    let loopback_packet = loopback_queue.pop_front().expect("front checked");

    if microphone_packet.duration_100ns != loopback_packet.duration_100ns
        || microphone_packet.frames != loopback_packet.frames
        || microphone_packet.bytes.len() != loopback_packet.bytes.len()
        || microphone_packet.bytes.len() % 2 != 0
    {
        return Err(CaptureError::BackendUnavailable(
            "Windows native mic+loopback mixing hit incompatible packet timing or PCM frame sizes."
                .to_string(),
        ));
    }

    let mut mixed_bytes = Vec::with_capacity(microphone_packet.bytes.len());
    for (microphone_chunk, loopback_chunk) in microphone_packet
        .bytes
        .chunks_exact(2)
        .zip(loopback_packet.bytes.chunks_exact(2))
    {
        let microphone_sample = i16::from_le_bytes([microphone_chunk[0], microphone_chunk[1]]);
        let loopback_sample = i16::from_le_bytes([loopback_chunk[0], loopback_chunk[1]]);
        let mixed_sample = ((i32::from(microphone_sample) + i32::from(loopback_sample)) / 2)
            .clamp(i16::MIN as i32, i16::MAX as i32) as i16;
        mixed_bytes.extend_from_slice(&mixed_sample.to_le_bytes());
    }

    Ok(Some(
        super::native_audio_backend::WindowsWasapiAudioPacket {
            sample_time_100ns: microphone_packet.sample_time_100ns,
            duration_100ns: microphone_packet.duration_100ns,
            frames: microphone_packet.frames,
            bytes: mixed_bytes,
        },
    ))
}

#[cfg(target_os = "windows")]
fn capture_first_frame_and_write_sample(
    options: &RecordingOptions,
    foundation: &NativeRuntimeFoundationObjects,
) -> Result<String, CaptureError> {
    for _ in 0..10 {
        thread::sleep(Duration::from_millis(50));
        if let Ok(frame) = foundation.frame_pool.TryGetNextFrame() {
            let surface = frame.Surface().map_err(map_windows_error)?;
            let content_size = frame.ContentSize().map_err(map_windows_error)?;
            let relative_time = frame
                .SystemRelativeTime()
                .map(|time| time.Duration)
                .unwrap_or(0);
            let surface_kind =
                describe_surface_kind(&frame).unwrap_or_else(|| "direct3d-surface".to_string());
            let encoder_summary = super::native_encoder_backend::write_surface_sample_smoke(
                options,
                &surface,
                relative_time,
            )?;
            let _ = frame.Close();
            return Ok(format!(
                "Windows WGC -> Media Foundation integrated smoke captured first frame at {}x{} as `{surface_kind}`. {encoder_summary}",
                content_size.Width.max(1),
                content_size.Height.max(1),
            ));
        }
    }

    Err(CaptureError::BackendUnavailable(
        "Windows.Graphics.Capture smoke did not receive a frame in time for Media Foundation sample writing."
            .to_string(),
    ))
}

#[cfg(target_os = "windows")]
fn build_prepared_runtime_objects(
    capture_target: &CaptureItemTarget,
) -> Result<NativePreparedRuntimeObjects, CaptureError> {
    let foundation = build_runtime_foundation_objects(capture_target)?;
    let saw_frame = Arc::new(AtomicBool::new(false));
    let frames_observed = Arc::new(AtomicU64::new(0));
    let latest_frame_metadata =
        Arc::new(Mutex::new(WindowsGraphicsCaptureFrameMetadata::default()));
    let saw_frame_for_handler = Arc::clone(&saw_frame);
    let frames_observed_for_handler = Arc::clone(&frames_observed);
    let latest_frame_metadata_for_handler = Arc::clone(&latest_frame_metadata);
    let token = foundation
        .frame_pool
        .FrameArrived(
            &TypedEventHandler::<Direct3D11CaptureFramePool, IInspectable>::new(
                move |sender, _args| {
                    if let Some(pool) = sender.as_ref() {
                        if let Ok(frame) = pool.TryGetNextFrame() {
                            saw_frame_for_handler.store(true, Ordering::Relaxed);
                            let frames_observed =
                                frames_observed_for_handler.fetch_add(1, Ordering::Relaxed) + 1;
                            let content_size = frame.ContentSize().ok();
                            let relative_time = frame.SystemRelativeTime().ok();
                            let surface_kind = describe_surface_kind(&frame);
                            if let Ok(mut metadata) = latest_frame_metadata_for_handler.lock() {
                                metadata.frames_observed = frames_observed;
                                metadata.latest_width =
                                    content_size.map(|size| size.Width.max(1) as u32);
                                metadata.latest_height =
                                    content_size.map(|size| size.Height.max(1) as u32);
                                metadata.latest_relative_time_100ns =
                                    relative_time.map(|time| time.Duration);
                                metadata.latest_surface_kind = surface_kind;
                            }
                            let _ = frame.Close();
                            return Ok(());
                        }
                    }
                    Ok(())
                },
            ),
        )
        .map_err(map_windows_error)?;

    Ok(NativePreparedRuntimeObjects {
        foundation,
        frame_arrived_token: token,
        saw_frame,
        frames_observed,
        latest_frame_metadata,
    })
}

#[cfg(target_os = "windows")]
fn describe_surface_kind(
    frame: &windows::Graphics::Capture::Direct3D11CaptureFrame,
) -> Option<String> {
    let surface: IDirect3DSurface = frame.Surface().ok()?;
    let interface_access: IDirect3DDxgiInterfaceAccess = surface.cast().ok()?;
    unsafe {
        if interface_access.GetInterface::<ID3D11Texture2D>().is_ok() {
            return Some("d3d11-texture2d".to_string());
        }
        if interface_access.GetInterface::<IDXGISurface>().is_ok() {
            return Some("dxgi-surface".to_string());
        }
    }
    Some("direct3d-surface".to_string())
}

#[cfg(target_os = "windows")]
fn build_capture_item_for_target(
    capture_target: &CaptureItemTarget,
) -> Result<GraphicsCaptureItem, CaptureError> {
    let interop =
        factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>().map_err(map_windows_error)?;

    match capture_target {
        CaptureItemTarget::Monitor { monitor, .. } => unsafe {
            interop
                .CreateForMonitor::<GraphicsCaptureItem>(*monitor)
                .map_err(map_windows_error)
        },
        CaptureItemTarget::Window { window, .. } => unsafe {
            interop
                .CreateForWindow::<GraphicsCaptureItem>(*window)
                .map_err(map_windows_error)
        },
    }
}

#[cfg(target_os = "windows")]
fn build_d3d11_device() -> Result<ID3D11Device, CaptureError> {
    let mut device = None;
    let mut feature_level = D3D_FEATURE_LEVEL_11_0;
    let feature_levels = [D3D_FEATURE_LEVEL_11_0];
    unsafe {
        D3D11CreateDevice(
            None::<&windows::Win32::Graphics::Dxgi::IDXGIAdapter>,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE(std::ptr::null_mut()),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            Some(&feature_levels),
            D3D11_SDK_VERSION,
            Some(&mut device),
            Some(&mut feature_level),
            None,
        )
        .map_err(map_windows_error)?;
    }

    device.ok_or_else(|| {
        CaptureError::BackendUnavailable(
            "Windows.Graphics.Capture could not create a D3D11 device.".to_string(),
        )
    })
}

#[cfg(target_os = "windows")]
fn build_direct3d_device(d3d11_device: &ID3D11Device) -> Result<IDirect3DDevice, CaptureError> {
    let dxgi_device: IDXGIDevice = d3d11_device.cast().map_err(map_windows_error)?;
    let inspectable =
        unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi_device) }.map_err(map_windows_error)?;
    inspectable.cast().map_err(map_windows_error)
}

#[cfg(target_os = "windows")]
fn capture_item_target(options: &RecordingOptions) -> Result<CaptureItemTarget, CaptureError> {
    let target_id = if options.capture_target_id == CUSTOM_REGION_TARGET_ID {
        options.region_source_capture_target_id.as_str()
    } else {
        options.capture_target_id.as_str()
    };

    if target_id == FULL_DESKTOP_TARGET_ID {
        let monitors = query_monitors()?;
        if monitors.len() == 1 {
            return monitor_capture_target(&monitors[0]);
        }

        return Err(CaptureError::BackendUnavailable(
            "Windows.Graphics.Capture native foundation needs a specific monitor or window target. Full-desktop capture across multiple monitors is not wired yet."
                .to_string(),
        ));
    }

    if let Some(device_name) = target_id.strip_prefix(MONITOR_TARGET_PREFIX) {
        let monitor = query_monitors()?
            .into_iter()
            .find(|item| item.device_name == device_name)
            .ok_or_else(|| {
                CaptureError::BackendUnavailable(format!(
                    "the selected monitor `{device_name}` is no longer available"
                ))
            })?;
        return monitor_capture_target(&monitor);
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
        return Ok(CaptureItemTarget::Window {
            label: format!("Window · {}", window.title),
            window: HWND(window.id as usize as *mut _),
        });
    }

    Err(CaptureError::BackendUnavailable(format!(
        "Windows.Graphics.Capture could not build a native capture item for target `{target_id}`."
    )))
}

#[cfg(not(target_os = "windows"))]
fn capture_item_target(_options: &RecordingOptions) -> Result<(), CaptureError> {
    Err(CaptureError::BackendUnavailable(
        "Windows.Graphics.Capture native targets are only available on Windows.".to_string(),
    ))
}

#[cfg(target_os = "windows")]
fn monitor_capture_target(monitor: &MonitorDescriptor) -> Result<CaptureItemTarget, CaptureError> {
    let center = POINT {
        x: monitor.x + (monitor.width as i32 / 2),
        y: monitor.y + (monitor.height as i32 / 2),
    };
    let handle = unsafe { MonitorFromPoint(center, MONITOR_DEFAULTTONEAREST) };
    if handle.0.is_null() {
        return Err(CaptureError::BackendUnavailable(format!(
            "Windows.Graphics.Capture could not resolve a native monitor handle for `{}`.",
            monitor.label
        )));
    }

    Ok(CaptureItemTarget::Monitor {
        label: monitor.label.clone(),
        monitor: handle,
    })
}

#[cfg(target_os = "windows")]
fn map_windows_error(error: windows::core::Error) -> CaptureError {
    CaptureError::BackendUnavailable(error.message().to_string())
}

#[cfg(target_os = "windows")]
fn shutdown_prepared_runtime(prepared: NativePreparedRuntimeObjects) {
    let _ = prepared
        .foundation
        .frame_pool
        .RemoveFrameArrived(prepared.frame_arrived_token);
    let _ = prepared.foundation.session.Close();
    let _ = prepared.foundation.frame_pool.Close();
}

#[cfg(target_os = "windows")]
fn shutdown_runtime_foundation(foundation: NativeRuntimeFoundationObjects) {
    let _ = foundation.session.Close();
    let _ = foundation.frame_pool.Close();
}

#[cfg(target_os = "windows")]
fn shutdown_desktop_runtime_foundation(foundation: DesktopRuntimeFoundationObjects) {
    for source in foundation.sources {
        let _ = source.session.Close();
        let _ = source.frame_pool.Close();
    }
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

#[cfg(target_os = "windows")]
fn query_monitors() -> Result<Vec<MonitorDescriptor>, CaptureError> {
    const PRIMARY_MONITOR_FLAG: u32 = 1;

    unsafe extern "system" fn enum_monitor_proc(
        monitor: HMONITOR,
        _hdc: HDC,
        _rect: *mut RECT,
        lparam: LPARAM,
    ) -> BOOL {
        let monitors = unsafe { &mut *(lparam.0 as *mut Vec<MonitorDescriptor>) };
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;

        if !unsafe { GetMonitorInfoW(monitor, &mut info as *mut _ as *mut _) }.as_bool() {
            return true.into();
        }

        let rect = info.monitorInfo.rcMonitor;
        let device_name = utf16_device_name(&info.szDevice);
        let primary = info.monitorInfo.dwFlags & PRIMARY_MONITOR_FLAG != 0;

        monitors.push(MonitorDescriptor {
            device_name,
            label: String::new(),
            width: (rect.right - rect.left).max(0) as u32,
            height: (rect.bottom - rect.top).max(0) as u32,
            x: rect.left,
            y: rect.top,
            primary,
        });

        true.into()
    }

    let mut monitors: Vec<MonitorDescriptor> = Vec::new();
    let ok = unsafe {
        EnumDisplayMonitors(
            None,
            None,
            Some(enum_monitor_proc),
            LPARAM((&mut monitors as *mut Vec<MonitorDescriptor>) as isize),
        )
    };

    if !ok.as_bool() {
        return Err(CaptureError::BackendUnavailable(
            "Windows native capture controller could not enumerate any monitors.".to_string(),
        ));
    }

    monitors.sort_by_key(|monitor| {
        (
            !monitor.primary,
            display_ordinal(&monitor.device_name).unwrap_or(usize::MAX),
            monitor.y,
            monitor.x,
        )
    });

    for (index, monitor) in monitors.iter_mut().enumerate() {
        monitor.label = if monitor.primary {
            format!("Display {index} (Primary)")
        } else {
            format!("Display {index}")
        };
    }

    Ok(monitors)
}

#[cfg(not(target_os = "windows"))]
fn query_monitors() -> Result<Vec<MonitorDescriptor>, CaptureError> {
    Err(CaptureError::BackendUnavailable(
        "Windows monitor enumeration is only available on Windows.".to_string(),
    ))
}

#[cfg(target_os = "windows")]
fn utf16_device_name(raw: &[u16]) -> String {
    let len = raw
        .iter()
        .position(|value| *value == 0)
        .unwrap_or(raw.len());
    String::from_utf16_lossy(&raw[..len])
}

fn display_ordinal(device_name: &str) -> Option<usize> {
    let digits = device_name
        .chars()
        .rev()
        .take_while(|character| character.is_ascii_digit())
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();

    let number = digits.parse::<usize>().ok()?;
    number.checked_sub(1)
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

fn parse_json_command<T>(script: &str) -> Result<Vec<T>, CaptureError>
where
    T: for<'de> Deserialize<'de>,
{
    let output = std::process::Command::new("powershell")
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

#[cfg(test)]
mod tests {
    use super::{
        MONITOR_TARGET_PREFIX, native_recording_runtime_supported,
        native_recording_unsupported_reason,
    };
    use capture::{CUSTOM_REGION_TARGET_ID, DEFAULT_AUDIO_INPUT_ID, RecordingOptions};
    use std::path::PathBuf;

    fn recording_options(capture_target_id: &str) -> RecordingOptions {
        RecordingOptions {
            output_path: PathBuf::from("windows-native-test.mp4"),
            quality_preset: "1080p / 30 fps".to_string(),
            mic_enabled: false,
            system_audio_enabled: false,
            capture_target_id: capture_target_id.to_string(),
            audio_input_id: DEFAULT_AUDIO_INPUT_ID.to_string(),
            portal_parent_window: None,
            portal_restore_token: None,
            region_x: 0,
            region_y: 0,
            region_width: 640,
            region_height: 360,
            region_source_capture_target_id: format!("{MONITOR_TARGET_PREFIX}DISPLAY1"),
            region_source_origin_x: 0,
            region_source_origin_y: 0,
            region_source_scale_factor_milli: 1000,
        }
    }

    #[test]
    fn mic_plus_system_audio_is_not_blocked_by_native_policy() {
        let mut options = recording_options(&format!("{MONITOR_TARGET_PREFIX}DISPLAY1"));
        options.mic_enabled = true;
        options.system_audio_enabled = true;

        assert!(
            native_recording_unsupported_reason(&options).is_none(),
            "native Windows policy should allow strict-format mic + loopback mixing",
        );
    }

    #[test]
    fn custom_region_is_supported_by_native_policy() {
        let options = recording_options(CUSTOM_REGION_TARGET_ID);
        assert!(
            native_recording_unsupported_reason(&options).is_none(),
            "custom region should now stay on the native Windows path",
        );
        let _ = native_recording_runtime_supported();
    }
}
