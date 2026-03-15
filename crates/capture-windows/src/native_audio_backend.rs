use capture::{
    AudioBackendAvailability, AudioBackendDescriptor, AudioBackendFactory, AudioBackendFamily,
    AudioBackendRuntimeReport, AudioInputOption, DEFAULT_AUDIO_INPUT_ID,
    preferred_system_audio_input, resolve_microphone_input_id,
};
use std::process::Command;

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

pub fn backend() -> &'static dyn AudioBackendFactory {
    &WASAPI_WINDOWS_AUDIO_BACKEND
}

impl AudioBackendFactory for WasapiWindowsAudioBackend {
    fn descriptor(&self) -> AudioBackendDescriptor {
        AudioBackendDescriptor {
            id: "windows-wasapi",
            label: "Windows WASAPI",
            family: AudioBackendFamily::Native,
        }
    }

    fn availability(&self) -> AudioBackendAvailability {
        match runtime_plan() {
            Some(plan) => {
                let default_input = plan
                    .default_input_name
                    .clone()
                    .unwrap_or_else(|| "not detected".to_string());
                AudioBackendAvailability::Unavailable {
                    reason: format!(
                        "Windows reports default input `{default_input}`, {} capture endpoint{}, and {} render endpoint{}. The recorder still uses the DirectShow / ffmpeg audio path instead of WASAPI.",
                        plan.capture_endpoint_count,
                        if plan.capture_endpoint_count == 1 { "" } else { "s" },
                        plan.render_endpoint_count,
                        if plan.render_endpoint_count == 1 { "" } else { "s" },
                    ),
                }
            }
            None => AudioBackendAvailability::Unavailable {
                reason: "A WASAPI-based default-input and loopback runtime is planned for Phase 2, but the app could not inspect Windows audio endpoints from this session.".to_string(),
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

pub fn runtime_plan() -> Option<WindowsAudioRuntimePlan> {
    let report = endpoint_report()?;
    Some(build_runtime_plan(&report))
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
            "{} DirectShow microphone route: {}. DirectShow system-audio route: {}.",
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
                description: "DirectShow input: USB Microphone".to_string(),
                kind: AudioInputKind::Microphone,
            },
            AudioInputOption {
                id: "Stereo Mix".to_string(),
                label: "System audio · Stereo Mix".to_string(),
                description: "DirectShow input: Stereo Mix".to_string(),
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
        assert!(start_plan.summary.contains("DirectShow microphone route"));
    }
}
