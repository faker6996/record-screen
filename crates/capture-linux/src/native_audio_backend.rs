use capture::{
    AudioBackendAvailability, AudioBackendDescriptor, AudioBackendFactory, AudioBackendFamily,
    AudioBackendRuntimeReport,
};
use std::process::Command;

pub struct PipewireLinuxAudioBackend;

static PIPEWIRE_LINUX_AUDIO_BACKEND: PipewireLinuxAudioBackend = PipewireLinuxAudioBackend;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxAudioRuntimeReport {
    pub server_name: Option<String>,
    pub sources: Vec<String>,
    pub sinks: Vec<String>,
}

pub fn backend() -> &'static dyn AudioBackendFactory {
    &PIPEWIRE_LINUX_AUDIO_BACKEND
}

impl AudioBackendFactory for PipewireLinuxAudioBackend {
    fn descriptor(&self) -> AudioBackendDescriptor {
        AudioBackendDescriptor {
            id: "linux-pipewire-audio",
            label: "Linux PipeWire audio",
            family: AudioBackendFamily::Native,
        }
    }

    fn availability(&self) -> AudioBackendAvailability {
        let reason = match runtime_summary() {
            Some(summary) => format!(
                "{summary} The recorder still uses the PulseAudio / ffmpeg audio path instead of a production PipeWire-native pipeline."
            ),
            None => "A production PipeWire-native microphone and system-audio runtime is planned for Phase 2, but Linux audio still runs through PulseAudio / ffmpeg and the experimental Wayland path.".to_string(),
        };

        AudioBackendAvailability::Unavailable { reason }
    }

    fn runtime_report(&self) -> AudioBackendRuntimeReport {
        AudioBackendRuntimeReport {
            summary: runtime_summary(),
            preferred_input_id: preferred_input_source_name(),
            preferred_input_label: preferred_input_source_name(),
            preferred_system_id: preferred_monitor_source_name(),
            preferred_system_label: preferred_monitor_source_name(),
        }
    }
}

pub fn preferred_input_source_name() -> Option<String> {
    runtime_report().and_then(|report| {
        report
            .sources
            .iter()
            .find(|source| !source.ends_with(".monitor"))
            .or_else(|| report.sources.first())
            .cloned()
    })
}

pub fn preferred_monitor_source_name() -> Option<String> {
    runtime_report().and_then(|report| {
        report
            .sources
            .iter()
            .find(|source| source.ends_with(".monitor"))
            .cloned()
    })
}

pub fn runtime_summary() -> Option<String> {
    let report = runtime_report()?;
    let mut parts = Vec::new();

    if let Some(server_name) = report.server_name {
        parts.push(format!("Linux audio server reports `{server_name}`"));
    }
    parts.push(format!(
        "{} source{} and {} sink{} detected",
        report.sources.len(),
        if report.sources.len() == 1 { "" } else { "s" },
        report.sinks.len(),
        if report.sinks.len() == 1 { "" } else { "s" }
    ));
    if let Some(input) = preferred_input_source_name() {
        parts.push(format!("preferred input source `{input}`"));
    }
    if let Some(monitor) = preferred_monitor_source_name() {
        parts.push(format!("preferred monitor source `{monitor}`"));
    }

    Some(parts.join(", "))
}

pub fn pipewire_runtime_summary() -> Option<String> {
    runtime_summary()
}

fn pactl_server_name() -> Option<String> {
    let output = Command::new("pactl").args(["info"]).output().ok()?;
    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_pactl_server_name(&stdout)
}

fn wpctl_available() -> bool {
    Command::new("wpctl")
        .args(["status"])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn runtime_report() -> Option<LinuxAudioRuntimeReport> {
    let server_name = pactl_server_name();
    let sources = pactl_short_names("sources");
    let sinks = pactl_short_names("sinks");

    if server_name.is_none() && sources.is_empty() && sinks.is_empty() && !wpctl_available() {
        return None;
    }

    Some(LinuxAudioRuntimeReport {
        server_name,
        sources,
        sinks,
    })
}

fn pactl_short_names(kind: &str) -> Vec<String> {
    let output = Command::new("pactl").args(["list", "short", kind]).output();
    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }

    parse_pactl_short_names(&String::from_utf8_lossy(&output.stdout))
}

fn parse_pactl_server_name(stdout: &str) -> Option<String> {
    stdout
        .lines()
        .find_map(|line| line.strip_prefix("Server Name:").map(str::trim))
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn parse_pactl_short_names(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .filter_map(|line| {
            let mut columns = line.split('\t');
            let _index = columns.next()?;
            let name = columns.next()?.trim();
            if name.is_empty() {
                None
            } else {
                Some(name.to_string())
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{parse_pactl_server_name, parse_pactl_short_names};

    #[test]
    fn parses_server_name_from_pactl_info() {
        let sample = r#"
Server String: /run/user/1000/pulse/native
Library Protocol Version: 35
Server Protocol Version: 35
Is Local: yes
Server Name: PulseAudio (on PipeWire 1.0.3)
"#;

        assert_eq!(
            parse_pactl_server_name(sample).as_deref(),
            Some("PulseAudio (on PipeWire 1.0.3)")
        );
    }

    #[test]
    fn rejects_missing_server_name() {
        assert!(parse_pactl_server_name("Server String: foo").is_none());
    }

    #[test]
    fn parses_short_names_from_pactl_list() {
        let sample = "42\talsa_input.usb-Blue_Yeti\tPipeWire\t...\n43\talsa_output.pci.monitor\tPipeWire\t...";
        let items = parse_pactl_short_names(sample);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0], "alsa_input.usb-Blue_Yeti");
        assert_eq!(items[1], "alsa_output.pci.monitor");
    }
}
