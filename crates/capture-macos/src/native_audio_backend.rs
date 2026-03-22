use capture::{
    AudioBackendAvailability, AudioBackendDescriptor, AudioBackendFactory,
    AudioBackendRuntimeReport, AudioInputKind, AudioInputOption, DEFAULT_AUDIO_INPUT_ID,
    resolve_audio_input_id,
};
#[cfg(target_os = "macos")]
use screencapturekit::audio_devices::AudioInputDevice;
use std::process::Command;

pub struct CoreAudioMacosBackend;

static CORE_AUDIO_MACOS_BACKEND: CoreAudioMacosBackend = CoreAudioMacosBackend;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacosAudioDeviceReport {
    pub devices: Vec<MacosAudioDevice>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacosAudioDevice {
    pub name: String,
    pub default_input: bool,
    pub default_output: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacosAudioRoutePlan {
    pub default_input_label: Option<String>,
    pub default_output_label: Option<String>,
    pub default_input_note: String,
    pub default_output_note: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacosAudioStartPlan {
    pub microphone_device_id: Option<String>,
    pub microphone_device_name: Option<String>,
    pub output_device_name: Option<String>,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MacosNativeAudioInputDevice {
    id: String,
    name: String,
    is_default: bool,
}

pub fn backend() -> &'static dyn AudioBackendFactory {
    &CORE_AUDIO_MACOS_BACKEND
}

impl AudioBackendFactory for CoreAudioMacosBackend {
    fn descriptor(&self) -> AudioBackendDescriptor {
        AudioBackendDescriptor {
            id: "macos-core-audio",
            label: "macOS Core Audio",
        }
    }

    fn availability(&self) -> AudioBackendAvailability {
        if native_recording_output_runtime_is_supported() {
            AudioBackendAvailability::Available
        } else {
            let reason = match runtime_summary() {
                Some(summary) => format!(
                    "{summary} On older macOS runtimes the app still lacks a fully separate Core Audio capture runtime for every path, so native audio routing is only partial there."
                ),
                None => "A dedicated Core Audio microphone and system-audio runtime is planned for older macOS runtimes; today the fully native audio lane is only active together with ScreenCaptureKit recording output on macOS 15+.".to_string(),
            };

            AudioBackendAvailability::Unavailable { reason }
        }
    }

    fn runtime_report(&self) -> AudioBackendRuntimeReport {
        AudioBackendRuntimeReport {
            summary: Some(match runtime_summary() {
                Some(summary) if native_recording_output_runtime_is_supported() => format!(
                    "{summary} Native microphone routing is active through the ScreenCaptureKit recording-output lane on supported macOS runtimes."
                ),
                Some(summary) => format!(
                    "{summary} Older macOS runtimes only expose partial native-audio support, so unsupported paths fail explicitly there."
                ),
                None if native_recording_output_runtime_is_supported() =>
                    "Native microphone routing is active through the ScreenCaptureKit recording-output lane on supported macOS runtimes."
                        .to_string(),
                None => "Native microphone routing is only partially active on this macOS runtime; older runtimes do not expose every native audio path."
                    .to_string(),
            }),
            preferred_input_id: preferred_input_device_name(),
            preferred_input_label: preferred_input_device_name(),
            preferred_system_id: preferred_output_device_name(),
            preferred_system_label: preferred_output_device_name(),
        }
    }
}

pub fn preferred_input_device_name() -> Option<String> {
    device_report().and_then(|report| {
        report
            .devices
            .iter()
            .find(|device| device.default_input)
            .or_else(|| report.devices.first())
            .map(|device| device.name.clone())
    })
}

pub fn preferred_output_device_name() -> Option<String> {
    device_report().and_then(|report| {
        report
            .devices
            .iter()
            .find(|device| device.default_output)
            .or_else(|| report.devices.first())
            .map(|device| device.name.clone())
    })
}

pub fn selectable_audio_inputs() -> Vec<AudioInputOption> {
    let native_inputs = native_input_devices();
    if !native_inputs.is_empty() {
        return native_inputs
            .into_iter()
            .map(|device| AudioInputOption {
                id: device.id,
                label: device.name.clone(),
                description: format!("Native macOS input: {}", device.name),
                kind: AudioInputKind::Microphone,
            })
            .collect();
    }

    device_report()
        .map(|report| {
            report
                .devices
                .into_iter()
                .filter(|device| device.default_input)
                .map(|device| AudioInputOption {
                    id: device.name.clone(),
                    label: device.name.clone(),
                    description: format!("Native macOS input: {}", device.name),
                    kind: AudioInputKind::Microphone,
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn runtime_summary() -> Option<String> {
    let report = device_report()?;
    let mut parts = vec![format!(
        "macOS reports {} audio device{}",
        report.devices.len(),
        if report.devices.len() == 1 { "" } else { "s" }
    )];

    if let Some(input) = preferred_input_device_name() {
        parts.push(format!("default input `{input}`"));
    }
    if let Some(output) = preferred_output_device_name() {
        parts.push(format!("default output `{output}`"));
    }

    Some(parts.join(" with "))
}

pub fn route_plan() -> Option<MacosAudioRoutePlan> {
    let input = preferred_input_device_name();
    let output = preferred_output_device_name();
    if input.is_none() && output.is_none() {
        return None;
    }

    Some(MacosAudioRoutePlan {
        default_input_label: input.clone(),
        default_output_label: output.clone(),
        default_input_note: match input {
            Some(input) => {
                format!(
                    "The app can route `Default input` through the macOS default input `{input}`."
                )
            }
            None => "macOS audio probing could not resolve a default input device yet.".to_string(),
        },
        default_output_note: match output {
            Some(output) => {
                format!(
                    "The app can target macOS default output `{output}` for a future native system-audio path."
                )
            }
            None => {
                "macOS audio probing could not resolve a default output device yet.".to_string()
            }
        },
    })
}

pub fn start_plan(
    selected_audio_input_id: &str,
    mic_enabled: bool,
    system_audio_enabled: bool,
    discovered_inputs: &[AudioInputOption],
) -> MacosAudioStartPlan {
    let route_plan = route_plan();
    let native_inputs = native_input_devices();
    let (microphone_device_id, microphone_device_name) = resolve_native_microphone_device(
        selected_audio_input_id,
        mic_enabled,
        discovered_inputs,
        &native_inputs,
    );
    let output_device_name = if system_audio_enabled {
        route_plan
            .as_ref()
            .and_then(|route_plan| route_plan.default_output_label.clone())
    } else {
        None
    };

    let summary = format!(
        "macOS audio start plan would use microphone route `{}` (device id `{}`) and output route `{}`.",
        microphone_device_name
            .clone()
            .unwrap_or_else(|| "none".to_string()),
        microphone_device_id
            .clone()
            .unwrap_or_else(|| "default".to_string()),
        output_device_name
            .clone()
            .unwrap_or_else(|| "none".to_string())
    );

    MacosAudioStartPlan {
        microphone_device_id,
        microphone_device_name,
        output_device_name,
        summary,
    }
}

fn resolve_native_microphone_device(
    selected_audio_input_id: &str,
    mic_enabled: bool,
    discovered_inputs: &[AudioInputOption],
    native_inputs: &[MacosNativeAudioInputDevice],
) -> (Option<String>, Option<String>) {
    if !mic_enabled {
        return (None, None);
    }

    if selected_audio_input_id == DEFAULT_AUDIO_INPUT_ID {
        return (
            None,
            native_inputs
                .iter()
                .find(|device| device.is_default)
                .map(|device| device.name.clone())
                .or_else(preferred_input_device_name),
        );
    }

    let selected_label = discovered_inputs
        .iter()
        .find(|input| input.id == selected_audio_input_id)
        .map(|input| input.label.clone())
        .or_else(|| resolve_audio_input_id(selected_audio_input_id, discovered_inputs));

    if let Some(selected_label) = selected_label.as_ref() {
        if let Some(device) = native_inputs
            .iter()
            .find(|device| device.name.eq_ignore_ascii_case(&selected_label))
        {
            return (Some(device.id.clone()), Some(device.name.clone()));
        }
    }

    if selected_audio_input_id == DEFAULT_AUDIO_INPUT_ID {
        return (
            None,
            preferred_input_device_name().or_else(|| {
                discovered_inputs
                    .iter()
                    .find(|input| input.id == DEFAULT_AUDIO_INPUT_ID)
                    .map(|input| input.label.clone())
            }),
        );
    }

    (None, selected_label)
}

#[cfg(target_os = "macos")]
fn native_input_devices() -> Vec<MacosNativeAudioInputDevice> {
    AudioInputDevice::list()
        .into_iter()
        .map(|device| MacosNativeAudioInputDevice {
            id: device.id,
            name: device.name,
            is_default: device.is_default,
        })
        .collect()
}

#[cfg(not(target_os = "macos"))]
fn native_input_devices() -> Vec<MacosNativeAudioInputDevice> {
    Vec::new()
}

fn device_report() -> Option<MacosAudioDeviceReport> {
    let output = Command::new("system_profiler")
        .args(["SPAudioDataType"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    parse_system_profiler_audio_report(&String::from_utf8_lossy(&output.stdout))
}

fn native_recording_output_runtime_is_supported() -> bool {
    matches!(macos_version(), Some((major, _, _)) if major >= 15)
}

fn macos_version() -> Option<(u64, u64, u64)> {
    super::current_macos_version()
}

#[cfg(test)]
fn parse_macos_version(value: &str) -> Option<(u64, u64, u64)> {
    let mut parts = value.trim().split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().unwrap_or("0").parse().ok()?;
    let patch = parts.next().unwrap_or("0").parse().ok()?;
    Some((major, minor, patch))
}

fn parse_system_profiler_audio_report(stdout: &str) -> Option<MacosAudioDeviceReport> {
    let mut devices = Vec::new();
    let mut current: Option<MacosAudioDevice> = None;

    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed == "Audio:" || trimmed == "Devices:" {
            continue;
        }

        if trimmed.ends_with(':') {
            if let Some(device) = current.take() {
                devices.push(device);
            }

            current = Some(MacosAudioDevice {
                name: trimmed.trim_end_matches(':').to_string(),
                default_input: false,
                default_output: false,
            });
            continue;
        }

        if let Some(device) = current.as_mut() {
            if trimmed.eq("Default Input Device: Yes") {
                device.default_input = true;
            }
            if trimmed.eq("Default Output Device: Yes") {
                device.default_output = true;
            }
        }
    }

    if let Some(device) = current.take() {
        devices.push(device);
    }

    if devices.is_empty() {
        None
    } else {
        Some(MacosAudioDeviceReport { devices })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MacosNativeAudioInputDevice, parse_system_profiler_audio_report,
        resolve_native_microphone_device, route_plan, start_plan,
    };
    use capture::{AudioInputKind, AudioInputOption, DEFAULT_AUDIO_INPUT_ID};

    #[test]
    fn parses_audio_devices_from_system_profiler_output() {
        let sample = r#"
Audio:

    Devices:

        MacBook Pro Speakers:

            Default Output Device: Yes

        MacBook Pro Microphone:

            Default Input Device: Yes
"#;

        let report = parse_system_profiler_audio_report(sample).expect("report should parse");
        assert_eq!(report.devices.len(), 2);
        assert_eq!(report.devices[0].name, "MacBook Pro Speakers");
        assert!(report.devices[0].default_output);
        assert_eq!(report.devices[1].name, "MacBook Pro Microphone");
        assert!(report.devices[1].default_input);
    }

    #[test]
    fn builds_audio_start_plan_from_default_input() {
        let discovered = vec![
            AudioInputOption {
                id: DEFAULT_AUDIO_INPUT_ID.to_string(),
                label: "Default input".to_string(),
                description: "Use the default input.".to_string(),
                kind: AudioInputKind::Default,
            },
            AudioInputOption {
                id: "MacBook Pro Microphone".to_string(),
                label: "MacBook Pro Microphone".to_string(),
                description: "AVFoundation input: MacBook Pro Microphone".to_string(),
                kind: AudioInputKind::Microphone,
            },
        ];

        let plan = start_plan(DEFAULT_AUDIO_INPUT_ID, true, false, &discovered);
        assert_eq!(plan.microphone_device_id, None);
        assert_eq!(
            plan.microphone_device_name.as_deref(),
            Some("MacBook Pro Microphone")
        );
        assert!(plan.summary.contains("microphone route"));
    }

    #[test]
    fn route_plan_is_constructible_when_devices_exist() {
        let _ = route_plan();
    }

    #[test]
    fn resolves_specific_native_microphone_device_from_selected_label() {
        let discovered = vec![AudioInputOption {
            id: "mic-1".to_string(),
            label: "USB Microphone".to_string(),
            description: "AVFoundation input: USB Microphone".to_string(),
            kind: AudioInputKind::Microphone,
        }];
        let native_devices = vec![MacosNativeAudioInputDevice {
            id: "native-usb-1".to_string(),
            name: "USB Microphone".to_string(),
            is_default: false,
        }];

        let (device_id, device_name) =
            resolve_native_microphone_device("mic-1", true, &discovered, &native_devices);
        assert_eq!(device_id.as_deref(), Some("native-usb-1"));
        assert_eq!(device_name.as_deref(), Some("USB Microphone"));
    }
}
