use capture::{
    AudioBackendAvailability, AudioBackendDescriptor, AudioBackendFactory,
    AudioBackendRuntimeReport, AudioInputKind, AudioInputOption, DEFAULT_AUDIO_INPUT_ID,
    preferred_system_audio_input, resolve_microphone_input_id,
};
use std::process::Command;
#[cfg(target_os = "windows")]
use std::{
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, Sender},
    },
    thread::{self, JoinHandle},
    time::Duration,
};
#[cfg(target_os = "windows")]
use windows::{
    Win32::{
        Media::Audio::{
            AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK,
            EDataFlow, IAudioCaptureClient, IAudioClient, IMMDevice, IMMDeviceEnumerator,
            MMDeviceEnumerator, eCapture, eConsole, eRender,
        },
        System::Com::{
            CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
            CoUninitialize,
        },
    },
    core::{HSTRING, PWSTR},
};

pub struct WasapiWindowsAudioBackend;

static WASAPI_WINDOWS_AUDIO_BACKEND: WasapiWindowsAudioBackend = WasapiWindowsAudioBackend;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsAudioEndpointReport {
    pub default_input_name: Option<String>,
    pub capture_endpoints: Vec<WindowsAudioEndpoint>,
    pub render_endpoints: Vec<WindowsAudioEndpoint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsAudioEndpoint {
    pub instance_id: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsAudioRuntimePlan {
    pub default_input_name: Option<String>,
    pub preferred_capture_endpoint: Option<WindowsAudioEndpoint>,
    pub preferred_render_endpoint: Option<WindowsAudioEndpoint>,
    pub capture_endpoint_count: usize,
    pub render_endpoint_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsAudioRoutePlan {
    pub default_input_label: Option<String>,
    pub default_input_note: String,
    pub preferred_loopback_label: Option<String>,
    pub loopback_note: String,
    pub has_loopback_candidate: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsAudioRuntimeIntent {
    pub microphone_enabled: bool,
    pub system_audio_enabled: bool,
    pub microphone_label: Option<String>,
    pub loopback_label: Option<String>,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsAudioStartPlan {
    pub microphone_device_name: Option<String>,
    pub system_audio_device_name: Option<String>,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsWasapiClientFoundation {
    pub endpoint_role: String,
    pub endpoint_id: String,
    pub loopback: bool,
    pub channels: u16,
    pub sample_rate_hz: u32,
    pub bits_per_sample: u16,
    pub buffer_frames: u32,
    pub default_device_period_100ns: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsWasapiRuntimeFoundation {
    pub microphone: Option<WindowsWasapiClientFoundation>,
    pub loopback: Option<WindowsWasapiClientFoundation>,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsWasapiSmokeLifecycle {
    pub microphone_started: bool,
    pub microphone_next_packet_frames: Option<u32>,
    pub microphone_packets_observed: Option<u32>,
    pub microphone_frames_observed: Option<u32>,
    pub microphone_silent_packets: Option<u32>,
    pub loopback_started: bool,
    pub loopback_next_packet_frames: Option<u32>,
    pub loopback_packets_observed: Option<u32>,
    pub loopback_frames_observed: Option<u32>,
    pub loopback_silent_packets: Option<u32>,
    pub summary: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WindowsWasapiPacketStats {
    pub next_packet_frames: u32,
    pub packets_observed: u32,
    pub frames_observed: u32,
    pub silent_packets: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsWasapiAudioPacket {
    pub sample_time_100ns: i64,
    pub duration_100ns: i64,
    pub frames: u32,
    pub bytes: Vec<u8>,
}

pub fn backend() -> &'static dyn AudioBackendFactory {
    &WASAPI_WINDOWS_AUDIO_BACKEND
}

impl AudioBackendFactory for WasapiWindowsAudioBackend {
    fn descriptor(&self) -> AudioBackendDescriptor {
        AudioBackendDescriptor {
            id: "windows-wasapi",
            label: "Windows WASAPI",
        }
    }

    fn availability(&self) -> AudioBackendAvailability {
        match runtime_plan() {
            Some(_) => AudioBackendAvailability::Available,
            None => AudioBackendAvailability::Unavailable {
                reason: "Windows could not inspect WASAPI audio endpoints from this session."
                    .to_string(),
            },
        }
    }

    fn runtime_report(&self) -> AudioBackendRuntimeReport {
        AudioBackendRuntimeReport {
            summary: runtime_summary(),
            preferred_input_id: preferred_capture_endpoint_id(),
            preferred_input_label: preferred_capture_endpoint_name(),
            preferred_system_id: preferred_render_endpoint_id(),
            preferred_system_label: preferred_render_endpoint_name(),
        }
    }
}

pub fn default_input_device_name() -> Option<String> {
    runtime_plan().and_then(|plan| plan.default_input_name)
}

pub fn preferred_capture_endpoint_name() -> Option<String> {
    runtime_plan().and_then(|plan| {
        plan.preferred_capture_endpoint
            .map(|endpoint| endpoint.label.clone())
    })
}

pub fn preferred_render_endpoint_name() -> Option<String> {
    runtime_plan().and_then(|plan| {
        plan.preferred_render_endpoint
            .map(|endpoint| endpoint.label.clone())
    })
}

pub fn preferred_capture_endpoint_id() -> Option<String> {
    runtime_plan().and_then(|plan| {
        plan.preferred_capture_endpoint
            .map(|endpoint| endpoint.instance_id.clone())
    })
}

pub fn preferred_render_endpoint_id() -> Option<String> {
    runtime_plan().and_then(|plan| {
        plan.preferred_render_endpoint
            .map(|endpoint| endpoint.instance_id.clone())
    })
}

pub fn runtime_summary() -> Option<String> {
    let plan = runtime_plan()?;
    let default_input = plan.default_input_name.clone();
    let preferred_capture = plan.preferred_capture_endpoint.clone();
    let preferred_render = plan.preferred_render_endpoint.clone();

    let mut parts = Vec::new();
    if let Some(name) = default_input {
        parts.push(format!("default input `{name}`"));
    }
    if let Some(endpoint) = preferred_capture {
        parts.push(format!("preferred capture endpoint `{}`", endpoint.label));
    }
    if let Some(endpoint) = preferred_render {
        parts.push(format!("preferred render endpoint `{}`", endpoint.label));
    }

    if parts.is_empty() {
        None
    } else {
        Some(format!(
            "Windows audio probing resolved {}.",
            parts.join(", ")
        ))
    }
}

pub fn runtime_foundation_summary(
    microphone_enabled: bool,
    system_audio_enabled: bool,
) -> Option<String> {
    wasapi_runtime_foundation(microphone_enabled, system_audio_enabled)
        .ok()
        .map(|foundation| foundation.summary)
}

pub fn smoke_lifecycle_summary(
    microphone_enabled: bool,
    system_audio_enabled: bool,
) -> Option<String> {
    wasapi_smoke_lifecycle(microphone_enabled, system_audio_enabled)
        .ok()
        .map(|smoke| smoke.summary)
}

#[cfg(target_os = "windows")]
pub fn start_default_microphone_worker() -> Result<WindowsWasapiCaptureWorker, String> {
    WindowsWasapiCaptureWorker::start(
        eCapture,
        false,
        "microphone".to_string(),
        preferred_capture_endpoint_id(),
    )
}

#[cfg(not(target_os = "windows"))]
pub fn start_default_microphone_worker() -> Result<(), String> {
    Err("WASAPI microphone worker is only available on Windows.".to_string())
}

#[cfg(target_os = "windows")]
pub fn start_default_loopback_worker() -> Result<WindowsWasapiCaptureWorker, String> {
    WindowsWasapiCaptureWorker::start(
        eRender,
        true,
        "loopback".to_string(),
        preferred_render_endpoint_id(),
    )
}

#[cfg(not(target_os = "windows"))]
pub fn start_default_loopback_worker() -> Result<(), String> {
    Err("WASAPI loopback worker is only available on Windows.".to_string())
}

pub fn runtime_plan() -> Option<WindowsAudioRuntimePlan> {
    let report = endpoint_report()?;
    Some(build_runtime_plan(&report))
}

pub fn selectable_audio_inputs() -> Vec<AudioInputOption> {
    let mut inputs = Vec::new();
    if let Some(plan) = runtime_plan() {
        if let Some(endpoint) = plan.preferred_capture_endpoint.clone() {
            inputs.push(AudioInputOption {
                id: endpoint.instance_id.clone(),
                label: endpoint.label.clone(),
                description: "Preferred Windows capture endpoint".to_string(),
                kind: AudioInputKind::Microphone,
            });
        }
        if let Some(endpoint) = plan.preferred_render_endpoint.clone() {
            inputs.push(AudioInputOption {
                id: endpoint.instance_id.clone(),
                label: endpoint.label.clone(),
                description: "Preferred Windows render endpoint".to_string(),
                kind: AudioInputKind::System,
            });
        }
    }
    inputs
}

pub fn route_plan() -> Option<WindowsAudioRoutePlan> {
    let plan = runtime_plan()?;
    Some(build_route_plan(&plan))
}

pub fn runtime_intent(
    microphone_enabled: bool,
    system_audio_enabled: bool,
) -> Option<WindowsAudioRuntimeIntent> {
    let route_plan = route_plan()?;
    Some(build_runtime_intent(
        &route_plan,
        microphone_enabled,
        system_audio_enabled,
    ))
}

pub fn start_plan(
    selected_audio_input_id: &str,
    microphone_enabled: bool,
    system_audio_enabled: bool,
    discovered_inputs: &[AudioInputOption],
) -> WindowsAudioStartPlan {
    let route_plan = route_plan();
    let runtime_intent = route_plan.as_ref().map(|route_plan| {
        build_runtime_intent(route_plan, microphone_enabled, system_audio_enabled)
    });

    let microphone_device_name = if microphone_enabled {
        if selected_audio_input_id == DEFAULT_AUDIO_INPUT_ID {
            resolve_microphone_input_id(selected_audio_input_id, discovered_inputs).or_else(|| {
                route_plan
                    .as_ref()
                    .and_then(|route_plan| route_plan.default_input_label.clone())
            })
        } else {
            resolve_microphone_input_id(selected_audio_input_id, discovered_inputs)
        }
    } else {
        None
    };

    let system_audio_device_name = if system_audio_enabled {
        preferred_system_audio_input(discovered_inputs).map(|input| input.id.clone())
    } else {
        None
    };

    let summary = match runtime_intent {
        Some(intent) => format!(
            "{} Windows microphone route: {}. Windows system-audio route: {}.",
            intent.summary,
            microphone_device_name
                .clone()
                .unwrap_or_else(|| "none".to_string()),
            system_audio_device_name
                .clone()
                .unwrap_or_else(|| "none".to_string())
        ),
        None => format!(
            "Windows audio start plan resolved microphone route `{}` and system-audio route `{}`.",
            microphone_device_name
                .clone()
                .unwrap_or_else(|| "none".to_string()),
            system_audio_device_name
                .clone()
                .unwrap_or_else(|| "none".to_string())
        ),
    };

    WindowsAudioStartPlan {
        microphone_device_name,
        system_audio_device_name,
        summary,
    }
}

pub fn endpoint_report() -> Option<WindowsAudioEndpointReport> {
    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", endpoint_probe_script()])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    parse_endpoint_report(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(target_os = "windows")]
struct ComScope;

#[cfg(target_os = "windows")]
impl ComScope {
    fn init() -> Result<Self, String> {
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED)
                .ok()
                .map_err(|error| error.message().to_string())?;
        }
        Ok(Self)
    }
}

#[cfg(target_os = "windows")]
impl Drop for ComScope {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}

#[cfg(target_os = "windows")]
impl WindowsWasapiCaptureWorker {
    fn start(
        dataflow: EDataFlow,
        loopback: bool,
        endpoint_role: String,
        endpoint_id_override: Option<String>,
    ) -> Result<Self, String> {
        let (stop_tx, stop_rx) = mpsc::channel();
        let (finished_tx, finished_rx) = mpsc::channel();
        let (packet_tx, packet_rx) = mpsc::channel();
        let latest_stats = Arc::new(Mutex::new(WindowsWasapiPacketStats::default()));
        let latest_stats_for_thread = Arc::clone(&latest_stats);
        let foundation = build_wasapi_client_foundation(
            dataflow,
            loopback,
            endpoint_role.clone(),
            endpoint_id_override.clone(),
        )?;
        let foundation_for_thread = foundation.clone();

        let worker_handle = thread::spawn(move || {
            let result = run_wasapi_worker_thread(
                dataflow,
                loopback,
                endpoint_role,
                endpoint_id_override,
                stop_rx,
                packet_tx,
                latest_stats_for_thread,
                foundation_for_thread,
            );
            let _ = finished_tx.send(result);
        });

        Ok(Self {
            stop_tx: Some(stop_tx),
            finished_rx,
            packet_rx,
            worker_handle: Some(worker_handle),
            latest_stats,
            foundation,
        })
    }

    pub fn snapshot(&self) -> WindowsWasapiPacketStats {
        self.latest_stats
            .lock()
            .map(|stats| stats.clone())
            .unwrap_or_default()
    }

    pub fn foundation(&self) -> &WindowsWasapiClientFoundation {
        &self.foundation
    }

    pub fn try_recv_packet(&mut self) -> Option<WindowsWasapiAudioPacket> {
        self.packet_rx.try_recv().ok()
    }

    pub fn stop(mut self) -> Result<WindowsWasapiPacketStats, String> {
        if let Some(stop_tx) = self.stop_tx.take() {
            let _ = stop_tx.send(());
        }
        let result = self.finished_rx.recv().map_err(|error| error.to_string())?;
        if let Some(worker_handle) = self.worker_handle.take() {
            let _ = worker_handle.join();
        }
        result
    }
}

#[cfg(target_os = "windows")]
impl Drop for WindowsWasapiCaptureWorker {
    fn drop(&mut self) {
        if let Some(stop_tx) = self.stop_tx.take() {
            let _ = stop_tx.send(());
        }
        if let Some(worker_handle) = self.worker_handle.take() {
            let _ = worker_handle.join();
        }
    }
}

#[cfg(target_os = "windows")]
struct WasapiClientObjects {
    #[allow(dead_code)]
    com_scope: ComScope,
    audio_client: IAudioClient,
    capture_client: IAudioCaptureClient,
    foundation: WindowsWasapiClientFoundation,
}

#[cfg(target_os = "windows")]
pub struct WindowsWasapiCaptureWorker {
    stop_tx: Option<Sender<()>>,
    finished_rx: Receiver<Result<WindowsWasapiPacketStats, String>>,
    packet_rx: Receiver<WindowsWasapiAudioPacket>,
    worker_handle: Option<JoinHandle<()>>,
    latest_stats: Arc<Mutex<WindowsWasapiPacketStats>>,
    foundation: WindowsWasapiClientFoundation,
}

#[cfg(target_os = "windows")]
fn wasapi_runtime_foundation(
    microphone_enabled: bool,
    system_audio_enabled: bool,
) -> Result<WindowsWasapiRuntimeFoundation, String> {
    let microphone = if microphone_enabled {
        Some(build_wasapi_client_foundation(
            eCapture,
            false,
            "microphone".to_string(),
            None,
        )?)
    } else {
        None
    };

    let loopback = if system_audio_enabled {
        Some(build_wasapi_client_foundation(
            eRender,
            true,
            "loopback".to_string(),
            None,
        )?)
    } else {
        None
    };

    let mut parts = Vec::new();
    if let Some(microphone) = microphone.as_ref() {
        parts.push(format!(
            "microphone endpoint `{}` at {} Hz / {} channel(s) / {} bits",
            microphone.endpoint_id,
            microphone.sample_rate_hz,
            microphone.channels,
            microphone.bits_per_sample
        ));
    }
    if let Some(loopback) = loopback.as_ref() {
        parts.push(format!(
            "loopback endpoint `{}` at {} Hz / {} channel(s) / {} bits",
            loopback.endpoint_id,
            loopback.sample_rate_hz,
            loopback.channels,
            loopback.bits_per_sample
        ));
    }

    let summary = if parts.is_empty() {
        "Windows WASAPI runtime foundation did not request microphone or loopback.".to_string()
    } else {
        format!(
            "Windows WASAPI runtime foundation initialized {}.",
            parts.join(", ")
        )
    };

    Ok(WindowsWasapiRuntimeFoundation {
        microphone,
        loopback,
        summary,
    })
}

#[cfg(not(target_os = "windows"))]
fn wasapi_runtime_foundation(
    _microphone_enabled: bool,
    _system_audio_enabled: bool,
) -> Result<WindowsWasapiRuntimeFoundation, String> {
    Err("WASAPI runtime foundation is only available on Windows.".to_string())
}

#[cfg(target_os = "windows")]
fn wasapi_smoke_lifecycle(
    microphone_enabled: bool,
    system_audio_enabled: bool,
) -> Result<WindowsWasapiSmokeLifecycle, String> {
    let microphone = if microphone_enabled {
        Some(run_wasapi_smoke(eCapture, false, "microphone".to_string())?)
    } else {
        None
    };
    let loopback = if system_audio_enabled {
        Some(run_wasapi_smoke(eRender, true, "loopback".to_string())?)
    } else {
        None
    };

    let summary = format!(
        "Windows WASAPI smoke lifecycle microphone_started={} microphone_next_packet_frames={} microphone_packets={} microphone_frames={} microphone_silent_packets={} loopback_started={} loopback_next_packet_frames={} loopback_packets={} loopback_frames={} loopback_silent_packets={}.",
        microphone.is_some(),
        microphone
            .as_ref()
            .map(|(_, stats)| stats.next_packet_frames.to_string())
            .unwrap_or_else(|| "n/a".to_string()),
        microphone
            .as_ref()
            .map(|(_, stats)| stats.packets_observed.to_string())
            .unwrap_or_else(|| "n/a".to_string()),
        microphone
            .as_ref()
            .map(|(_, stats)| stats.frames_observed.to_string())
            .unwrap_or_else(|| "n/a".to_string()),
        microphone
            .as_ref()
            .map(|(_, stats)| stats.silent_packets.to_string())
            .unwrap_or_else(|| "n/a".to_string()),
        loopback.is_some(),
        loopback
            .as_ref()
            .map(|(_, stats)| stats.next_packet_frames.to_string())
            .unwrap_or_else(|| "n/a".to_string()),
        loopback
            .as_ref()
            .map(|(_, stats)| stats.packets_observed.to_string())
            .unwrap_or_else(|| "n/a".to_string()),
        loopback
            .as_ref()
            .map(|(_, stats)| stats.frames_observed.to_string())
            .unwrap_or_else(|| "n/a".to_string()),
        loopback
            .as_ref()
            .map(|(_, stats)| stats.silent_packets.to_string())
            .unwrap_or_else(|| "n/a".to_string()),
    );

    Ok(WindowsWasapiSmokeLifecycle {
        microphone_started: microphone.is_some(),
        microphone_next_packet_frames: microphone
            .as_ref()
            .map(|(_, stats)| stats.next_packet_frames),
        microphone_packets_observed: microphone.as_ref().map(|(_, stats)| stats.packets_observed),
        microphone_frames_observed: microphone.as_ref().map(|(_, stats)| stats.frames_observed),
        microphone_silent_packets: microphone.as_ref().map(|(_, stats)| stats.silent_packets),
        loopback_started: loopback.is_some(),
        loopback_next_packet_frames: loopback.as_ref().map(|(_, stats)| stats.next_packet_frames),
        loopback_packets_observed: loopback.as_ref().map(|(_, stats)| stats.packets_observed),
        loopback_frames_observed: loopback.as_ref().map(|(_, stats)| stats.frames_observed),
        loopback_silent_packets: loopback.as_ref().map(|(_, stats)| stats.silent_packets),
        summary,
    })
}

#[cfg(not(target_os = "windows"))]
fn wasapi_smoke_lifecycle(
    _microphone_enabled: bool,
    _system_audio_enabled: bool,
) -> Result<WindowsWasapiSmokeLifecycle, String> {
    Err("WASAPI smoke lifecycle is only available on Windows.".to_string())
}

#[cfg(target_os = "windows")]
fn run_wasapi_smoke(
    dataflow: EDataFlow,
    loopback: bool,
    endpoint_role: String,
) -> Result<(WindowsWasapiClientFoundation, WindowsWasapiPacketStats), String> {
    let foundation =
        build_wasapi_client_foundation(dataflow, loopback, endpoint_role.clone(), None)?;
    let worker = WindowsWasapiCaptureWorker::start(dataflow, loopback, endpoint_role, None)?;
    thread::sleep(Duration::from_millis(180));
    let mut packet_stats = worker.snapshot();
    let final_stats = worker.stop()?;
    packet_stats.next_packet_frames = final_stats.next_packet_frames;
    packet_stats.packets_observed = final_stats.packets_observed;
    packet_stats.frames_observed = final_stats.frames_observed;
    packet_stats.silent_packets = final_stats.silent_packets;
    Ok((foundation, packet_stats))
}

#[cfg(target_os = "windows")]
fn run_wasapi_worker_thread(
    dataflow: EDataFlow,
    loopback: bool,
    endpoint_role: String,
    endpoint_id_override: Option<String>,
    stop_rx: Receiver<()>,
    packet_tx: Sender<WindowsWasapiAudioPacket>,
    latest_stats: Arc<Mutex<WindowsWasapiPacketStats>>,
    foundation: WindowsWasapiClientFoundation,
) -> Result<WindowsWasapiPacketStats, String> {
    let client =
        build_wasapi_client_objects(dataflow, loopback, endpoint_role, endpoint_id_override)?;
    unsafe {
        client
            .audio_client
            .Start()
            .map_err(|error| error.message().to_string())?;
    }

    let mut stats = WindowsWasapiPacketStats::default();
    let mut result = Ok(());
    let bytes_per_frame = u32::from(foundation.channels)
        .saturating_mul(u32::from(foundation.bits_per_sample.max(8)) / 8)
        .max(1);
    let mut elapsed_frames = 0u64;

    loop {
        match stop_rx.try_recv() {
            Ok(_) | Err(mpsc::TryRecvError::Disconnected) => break,
            Err(mpsc::TryRecvError::Empty) => {}
        }

        let packet_frames = match unsafe { client.capture_client.GetNextPacketSize() } {
            Ok(packet_frames) => packet_frames,
            Err(error) => {
                result = Err(error.message().to_string());
                break;
            }
        };
        stats.next_packet_frames = packet_frames;

        if packet_frames == 0 {
            if let Ok(mut latest) = latest_stats.lock() {
                *latest = stats.clone();
            }
            thread::sleep(Duration::from_millis(20));
            continue;
        }

        let mut data = std::ptr::null_mut();
        let mut frames_to_read = 0;
        let mut flags = 0u32;
        if let Err(error) = unsafe {
            client
                .capture_client
                .GetBuffer(&mut data, &mut frames_to_read, &mut flags, None, None)
        } {
            result = Err(error.message().to_string());
            break;
        }

        stats.packets_observed += 1;
        stats.frames_observed = stats.frames_observed.saturating_add(frames_to_read);
        if flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 {
            stats.silent_packets += 1;
        }

        let packet_duration_100ns =
            ((frames_to_read as u64) * 10_000_000 / foundation.sample_rate_hz.max(1) as u64) as i64;
        let packet_sample_time_100ns =
            (elapsed_frames * 10_000_000 / foundation.sample_rate_hz.max(1) as u64) as i64;
        let byte_len = frames_to_read.saturating_mul(bytes_per_frame) as usize;
        let packet_bytes = if data.is_null()
            || byte_len == 0
            || flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0
        {
            vec![0u8; byte_len]
        } else {
            unsafe { std::slice::from_raw_parts(data as *const u8, byte_len) }.to_vec()
        };
        elapsed_frames = elapsed_frames.saturating_add(frames_to_read as u64);

        if let Err(error) = unsafe { client.capture_client.ReleaseBuffer(frames_to_read) } {
            result = Err(error.message().to_string());
            break;
        }

        let _ = packet_tx.send(WindowsWasapiAudioPacket {
            sample_time_100ns: packet_sample_time_100ns,
            duration_100ns: packet_duration_100ns,
            frames: frames_to_read,
            bytes: packet_bytes,
        });

        if let Ok(mut latest) = latest_stats.lock() {
            *latest = stats.clone();
        }
    }

    unsafe {
        let _ = client.audio_client.Stop();
        let _ = client.audio_client.Reset();
    }

    result.map(|_| stats)
}

#[cfg(target_os = "windows")]
fn build_wasapi_client_foundation(
    dataflow: EDataFlow,
    loopback: bool,
    endpoint_role: String,
    endpoint_id_override: Option<String>,
) -> Result<WindowsWasapiClientFoundation, String> {
    let client =
        build_wasapi_client_objects(dataflow, loopback, endpoint_role, endpoint_id_override)?;
    Ok(client.foundation)
}

#[cfg(target_os = "windows")]
fn build_wasapi_client_objects(
    dataflow: EDataFlow,
    loopback: bool,
    endpoint_role: String,
    endpoint_id_override: Option<String>,
) -> Result<WasapiClientObjects, String> {
    let com_scope = ComScope::init()?;
    let enumerator: IMMDeviceEnumerator =
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
            .map_err(|error| error.message().to_string())?;
    let device = if let Some(endpoint_id) = endpoint_id_override {
        unsafe { enumerator.GetDevice(&HSTRING::from(endpoint_id)) }
            .map_err(|error| error.message().to_string())?
    } else {
        unsafe { enumerator.GetDefaultAudioEndpoint(dataflow, eConsole) }
            .map_err(|error| error.message().to_string())?
    };
    let endpoint_id = device_id_string(&device)?;
    let audio_client: IAudioClient = unsafe { device.Activate(CLSCTX_ALL, None) }
        .map_err(|error| error.message().to_string())?;
    let mix_format =
        unsafe { audio_client.GetMixFormat() }.map_err(|error| error.message().to_string())?;
    let mix = unsafe { *mix_format };
    let mut default_device_period_100ns = 0;
    let mut minimum_device_period_100ns = 0;
    unsafe {
        audio_client
            .GetDevicePeriod(
                Some(&mut default_device_period_100ns),
                Some(&mut minimum_device_period_100ns),
            )
            .map_err(|error| error.message().to_string())?;
    }
    let _ = minimum_device_period_100ns;
    let stream_flags = if loopback {
        AUDCLNT_STREAMFLAGS_LOOPBACK
    } else {
        0
    };
    unsafe {
        audio_client
            .Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                stream_flags,
                default_device_period_100ns,
                0,
                mix_format,
                None,
            )
            .map_err(|error| error.message().to_string())?;
    }
    let buffer_frames =
        unsafe { audio_client.GetBufferSize() }.map_err(|error| error.message().to_string())?;
    let capture_client: IAudioCaptureClient =
        unsafe { audio_client.GetService() }.map_err(|error| error.message().to_string())?;
    unsafe {
        CoTaskMemFree(Some(mix_format.cast()));
    }

    Ok(WasapiClientObjects {
        com_scope,
        audio_client,
        capture_client,
        foundation: WindowsWasapiClientFoundation {
            endpoint_role,
            endpoint_id,
            loopback,
            channels: mix.nChannels,
            sample_rate_hz: mix.nSamplesPerSec,
            bits_per_sample: mix.wBitsPerSample,
            buffer_frames,
            default_device_period_100ns,
        },
    })
}

#[cfg(target_os = "windows")]
fn device_id_string(device: &IMMDevice) -> Result<String, String> {
    let raw_id: PWSTR = unsafe { device.GetId() }.map_err(|error| error.message().to_string())?;
    let result = unsafe { raw_id.to_string() }.map_err(|error| error.to_string());
    unsafe {
        CoTaskMemFree(Some(raw_id.0.cast()));
    }
    result
}

fn endpoint_probe_script() -> &'static str {
    r#"$mapper = Get-ItemProperty -Path 'HKCU:\Software\Microsoft\Multimedia\Sound Mapper' -Name 'Record' -ErrorAction SilentlyContinue;
if ($mapper -and $mapper.Record) {
  Write-Output ('DEFAULT::' + $mapper.Record)
}

$endpoints = Get-PnpDevice -Class AudioEndpoint -Status OK -ErrorAction SilentlyContinue
foreach ($endpoint in $endpoints) {
  $name = $endpoint.FriendlyName
  if (-not $name) { continue }
  $id = $endpoint.InstanceId
  if (-not $id) { continue }

  if ($name -match 'Microphone|Mic|Headset|Line In|Input') {
    Write-Output ('CAPTURE' + \"`t\" + $id + \"`t\" + $name)
  } else {
    Write-Output ('RENDER' + \"`t\" + $id + \"`t\" + $name)
  }
}"#
}

fn parse_endpoint_report(stdout: &str) -> Option<WindowsAudioEndpointReport> {
    let mut report = WindowsAudioEndpointReport {
        default_input_name: None,
        capture_endpoints: Vec::new(),
        render_endpoints: Vec::new(),
    };

    for line in stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        if let Some(value) = line.strip_prefix("DEFAULT::") {
            report.default_input_name = Some(value.trim().to_string());
            continue;
        }

        if let Some(endpoint) = parse_endpoint_row(line, "CAPTURE") {
            report.capture_endpoints.push(endpoint);
            continue;
        }

        if let Some(endpoint) = parse_endpoint_row(line, "RENDER") {
            report.render_endpoints.push(endpoint);
        }
    }

    if report.default_input_name.is_none()
        && report.capture_endpoints.is_empty()
        && report.render_endpoints.is_empty()
    {
        None
    } else {
        Some(report)
    }
}

fn parse_endpoint_row(line: &str, kind: &str) -> Option<WindowsAudioEndpoint> {
    let mut columns = line.splitn(3, '\t');
    let row_kind = columns.next()?;
    if row_kind != kind {
        return None;
    }

    let instance_id = columns.next()?.trim();
    let label = columns.next()?.trim();
    if instance_id.is_empty() || label.is_empty() {
        return None;
    }

    Some(WindowsAudioEndpoint {
        instance_id: instance_id.to_string(),
        label: label.to_string(),
    })
}

fn build_runtime_plan(report: &WindowsAudioEndpointReport) -> WindowsAudioRuntimePlan {
    WindowsAudioRuntimePlan {
        default_input_name: report.default_input_name.clone(),
        preferred_capture_endpoint: select_capture_endpoint(report).cloned(),
        preferred_render_endpoint: select_render_endpoint(report).cloned(),
        capture_endpoint_count: report.capture_endpoints.len(),
        render_endpoint_count: report.render_endpoints.len(),
    }
}

fn build_route_plan(plan: &WindowsAudioRuntimePlan) -> WindowsAudioRoutePlan {
    let default_input_label = plan
        .preferred_capture_endpoint
        .as_ref()
        .map(|endpoint| endpoint.label.clone())
        .or_else(|| plan.default_input_name.clone());
    let preferred_loopback_label = plan
        .preferred_render_endpoint
        .as_ref()
        .map(|endpoint| endpoint.label.clone());

    let default_input_note = match default_input_label.as_ref() {
        Some(label) => format!(
            "The app can route `Default input` through the preferred Windows capture candidate `{label}`."
        ),
        None => {
            "Windows audio probing could not resolve a preferred capture candidate yet.".to_string()
        }
    };
    let loopback_note = match preferred_loopback_label.as_ref() {
        Some(label) => format!(
            "The app can target the preferred Windows render candidate `{label}` for a future WASAPI loopback path."
        ),
        None => {
            "Windows audio probing could not resolve a preferred render candidate yet.".to_string()
        }
    };

    WindowsAudioRoutePlan {
        default_input_label,
        default_input_note,
        preferred_loopback_label: preferred_loopback_label.clone(),
        loopback_note,
        has_loopback_candidate: preferred_loopback_label.is_some()
            || plan.render_endpoint_count > 0,
    }
}

fn build_runtime_intent(
    route_plan: &WindowsAudioRoutePlan,
    microphone_enabled: bool,
    system_audio_enabled: bool,
) -> WindowsAudioRuntimeIntent {
    let microphone_label = microphone_enabled
        .then(|| route_plan.default_input_label.clone())
        .flatten();
    let loopback_label = system_audio_enabled
        .then(|| route_plan.preferred_loopback_label.clone())
        .flatten();

    let mut parts = Vec::new();
    if let Some(label) = microphone_label.as_ref() {
        parts.push(format!("microphone route `{label}`"));
    } else if microphone_enabled {
        parts.push("microphone route unresolved".to_string());
    }

    if let Some(label) = loopback_label.as_ref() {
        parts.push(format!("loopback route `{label}`"));
    } else if system_audio_enabled {
        parts.push("loopback route unresolved".to_string());
    }

    let summary = if parts.is_empty() {
        "Windows audio runtime intent does not request microphone or system audio.".to_string()
    } else {
        format!(
            "Windows audio runtime intent will use {}.",
            parts.join(", ")
        )
    };

    WindowsAudioRuntimeIntent {
        microphone_enabled,
        system_audio_enabled,
        microphone_label,
        loopback_label,
        summary,
    }
}

fn select_capture_endpoint<'a>(
    report: &'a WindowsAudioEndpointReport,
) -> Option<&'a WindowsAudioEndpoint> {
    if let Some(default_name) = report.default_input_name.as_ref() {
        if let Some(endpoint) = report
            .capture_endpoints
            .iter()
            .find(|endpoint| endpoint.label.eq_ignore_ascii_case(default_name))
        {
            return Some(endpoint);
        }
    }

    report
        .capture_endpoints
        .iter()
        .max_by_key(|endpoint| capture_endpoint_score(&endpoint.label))
}

fn select_render_endpoint<'a>(
    report: &'a WindowsAudioEndpointReport,
) -> Option<&'a WindowsAudioEndpoint> {
    report
        .render_endpoints
        .iter()
        .max_by_key(|endpoint| render_endpoint_score(&endpoint.label))
}

fn capture_endpoint_score(endpoint: &str) -> i32 {
    let lowered = endpoint.to_ascii_lowercase();
    let mut score = 0;

    if lowered.contains("microphone") || lowered.contains("mic") {
        score += 120;
    }
    if lowered.contains("headset") {
        score += 70;
    }
    if lowered.contains("usb") {
        score += 20;
    }
    if lowered.contains("line in") {
        score += 10;
    }

    score
}

fn render_endpoint_score(endpoint: &str) -> i32 {
    let lowered = endpoint.to_ascii_lowercase();
    let mut score = 0;

    if lowered.contains("speaker") {
        score += 120;
    }
    if lowered.contains("headphone") {
        score += 100;
    }
    if lowered.contains("hdmi") || lowered.contains("display") {
        score += 40;
    }
    if lowered.contains("usb") {
        score += 20;
    }

    score
}

#[cfg(test)]
mod tests {
    use super::{
        build_route_plan, build_runtime_intent, build_runtime_plan, parse_endpoint_report,
        select_capture_endpoint, select_render_endpoint,
    };
    use capture::{AudioInputKind, AudioInputOption, DEFAULT_AUDIO_INPUT_ID};

    #[test]
    fn parses_windows_audio_endpoint_report() {
        let stdout = r#"
DEFAULT::USB Microphone
CAPTURE	USB\\VID_1234&PID_5678	Microphone Array
CAPTURE	LINEIN-DEVICE	Line In
RENDER	REALTEK-SPK	Speakers (Realtek Audio)
RENDER	USB-DAC	Headphones (USB DAC)
"#;

        let report = parse_endpoint_report(stdout).expect("report should parse");
        assert_eq!(report.default_input_name.as_deref(), Some("USB Microphone"));
        assert_eq!(report.capture_endpoints.len(), 2);
        assert_eq!(report.render_endpoints.len(), 2);
        assert_eq!(
            report.capture_endpoints[0].instance_id,
            "USB\\\\VID_1234&PID_5678"
        );
        assert_eq!(report.capture_endpoints[0].label, "Microphone Array");
    }

    #[test]
    fn ignores_empty_endpoint_report() {
        assert!(parse_endpoint_report("").is_none());
        assert!(parse_endpoint_report("noise").is_none());
    }

    #[test]
    fn prefers_default_capture_endpoint_when_present() {
        let stdout = r#"
DEFAULT::USB Microphone
CAPTURE	LINEIN-DEVICE	Line In
CAPTURE	USBMIC-DEVICE	USB Microphone
RENDER	REALTEK-SPK	Speakers (Realtek Audio)
"#;

        let report = parse_endpoint_report(stdout).expect("report should parse");
        assert_eq!(
            select_capture_endpoint(&report).map(|endpoint| endpoint.label.as_str()),
            Some("USB Microphone")
        );
    }

    #[test]
    fn prefers_speakers_for_render_loopback_candidate() {
        let stdout = r#"
RENDER	USB-DAC	Headphones (USB DAC)
RENDER	REALTEK-SPK	Speakers (Realtek Audio)
"#;

        let report = parse_endpoint_report(stdout).expect("report should parse");
        assert_eq!(
            select_render_endpoint(&report).map(|endpoint| endpoint.label.as_str()),
            Some("Speakers (Realtek Audio)")
        );
    }

    #[test]
    fn builds_runtime_plan_from_endpoint_report() {
        let stdout = r#"
DEFAULT::USB Microphone
CAPTURE	USBMIC-DEVICE	USB Microphone
RENDER	REALTEK-SPK	Speakers (Realtek Audio)
"#;

        let report = parse_endpoint_report(stdout).expect("report should parse");
        let plan = build_runtime_plan(&report);
        assert_eq!(plan.default_input_name.as_deref(), Some("USB Microphone"));
        assert_eq!(plan.capture_endpoint_count, 1);
        assert_eq!(plan.render_endpoint_count, 1);
        assert_eq!(
            plan.preferred_capture_endpoint
                .as_ref()
                .map(|endpoint| endpoint.label.as_str()),
            Some("USB Microphone")
        );
        assert_eq!(
            plan.preferred_render_endpoint
                .as_ref()
                .map(|endpoint| endpoint.label.as_str()),
            Some("Speakers (Realtek Audio)")
        );
    }

    #[test]
    fn builds_route_plan_from_runtime_plan() {
        let stdout = r#"
DEFAULT::USB Microphone
CAPTURE	USBMIC-DEVICE	USB Microphone
RENDER	REALTEK-SPK	Speakers (Realtek Audio)
"#;

        let report = parse_endpoint_report(stdout).expect("report should parse");
        let runtime_plan = build_runtime_plan(&report);
        let route_plan = build_route_plan(&runtime_plan);
        assert_eq!(
            route_plan.default_input_label.as_deref(),
            Some("USB Microphone")
        );
        assert!(
            route_plan
                .default_input_note
                .contains("preferred Windows capture candidate")
        );
        assert_eq!(
            route_plan.preferred_loopback_label.as_deref(),
            Some("Speakers (Realtek Audio)")
        );
        assert!(route_plan.has_loopback_candidate);
    }

    #[test]
    fn builds_runtime_intent_from_route_plan() {
        let stdout = r#"
DEFAULT::USB Microphone
CAPTURE	USBMIC-DEVICE	USB Microphone
RENDER	REALTEK-SPK	Speakers (Realtek Audio)
"#;

        let report = parse_endpoint_report(stdout).expect("report should parse");
        let runtime_plan = build_runtime_plan(&report);
        let route_plan = build_route_plan(&runtime_plan);
        let intent = build_runtime_intent(&route_plan, true, true);

        assert_eq!(intent.microphone_label.as_deref(), Some("USB Microphone"));
        assert_eq!(
            intent.loopback_label.as_deref(),
            Some("Speakers (Realtek Audio)")
        );
        assert!(intent.summary.contains("microphone route"));
        assert!(intent.summary.contains("loopback route"));
    }

    #[test]
    fn builds_start_plan_from_discovered_inputs() {
        let stdout = r#"
DEFAULT::USB Microphone
CAPTURE	USBMIC-DEVICE	USB Microphone
RENDER	REALTEK-SPK	Speakers (Realtek Audio)
"#;

        let report = parse_endpoint_report(stdout).expect("report should parse");
        let runtime_plan = build_runtime_plan(&report);
        let route_plan = build_route_plan(&runtime_plan);
        let discovered = vec![
            AudioInputOption {
                id: DEFAULT_AUDIO_INPUT_ID.to_string(),
                label: "Default input".to_string(),
                description: "Use the default input.".to_string(),
                kind: AudioInputKind::Default,
            },
            AudioInputOption {
                id: "USB Microphone".to_string(),
                label: "USB Microphone".to_string(),
                description: "Windows capture input: USB Microphone".to_string(),
                kind: AudioInputKind::Microphone,
            },
            AudioInputOption {
                id: "Stereo Mix".to_string(),
                label: "System audio · Stereo Mix".to_string(),
                description: "Windows system-audio input: Stereo Mix".to_string(),
                kind: AudioInputKind::System,
            },
        ];

        let start_plan = super::start_plan(DEFAULT_AUDIO_INPUT_ID, true, true, &discovered);
        assert_eq!(
            start_plan.microphone_device_name.as_deref(),
            Some("USB Microphone")
        );
        assert_eq!(
            start_plan.system_audio_device_name.as_deref(),
            Some("Stereo Mix")
        );
        assert!(
            route_plan
                .default_input_note
                .contains("preferred Windows capture candidate")
        );
        assert!(start_plan.summary.contains("microphone route"));
    }
}
#[cfg(target_os = "windows")]
pub fn start_microphone_worker_for_input(
    selected_audio_input_id: &str,
    discovered_inputs: &[AudioInputOption],
) -> Result<WindowsWasapiCaptureWorker, String> {
    let endpoint_id = resolve_microphone_input_id(selected_audio_input_id, discovered_inputs)
        .ok_or_else(|| "Windows could not resolve a usable microphone endpoint.".to_string())?;
    WindowsWasapiCaptureWorker::start(eCapture, false, "microphone".to_string(), Some(endpoint_id))
}

#[cfg(not(target_os = "windows"))]
pub fn start_microphone_worker_for_input(
    _selected_audio_input_id: &str,
    _discovered_inputs: &[AudioInputOption],
) -> Result<(), String> {
    Err("WASAPI microphone worker is only available on Windows.".to_string())
}
