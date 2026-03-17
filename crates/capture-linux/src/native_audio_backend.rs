use capture::{
    AudioBackendAvailability, AudioBackendDescriptor, AudioBackendFactory,
    AudioBackendRuntimeReport, AudioInputKind, AudioInputOption,
};
use std::process::Command;

use super::{LinuxDesktopSession, current_desktop_session};

pub struct PipewireLinuxAudioBackend;

static PIPEWIRE_LINUX_AUDIO_BACKEND: PipewireLinuxAudioBackend = PipewireLinuxAudioBackend;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxAudioRuntimeReport {
    pub server_name: Option<String>,
    pub sources: Vec<String>,
    pub sinks: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WpctlEntry {
    id: u32,
    label: String,
}

pub fn backend() -> &'static dyn AudioBackendFactory {
    &PIPEWIRE_LINUX_AUDIO_BACKEND
}

impl AudioBackendFactory for PipewireLinuxAudioBackend {
    fn descriptor(&self) -> AudioBackendDescriptor {
        AudioBackendDescriptor {
            id: "linux-pipewire-audio",
            label: "Linux PipeWire audio",
        }
    }

    fn availability(&self) -> AudioBackendAvailability {
        availability_for(&current_desktop_session(), runtime_summary().is_some())
    }

    fn runtime_report(&self) -> AudioBackendRuntimeReport {
        AudioBackendRuntimeReport {
            summary: runtime_summary().map(|summary| {
                format!(
                    "{summary} Linux native audio discovery can feed both the Wayland portal path and the X11 GStreamer path, though system-audio mixing still depends on the active runtime."
                )
            }),
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

pub fn discovered_audio_inputs() -> Vec<AudioInputOption> {
    let sources = pactl_short_names("sources");
    let sinks = pactl_short_names("sinks");

    if !sources.is_empty() || !sinks.is_empty() {
        let mut inputs = Vec::new();

        inputs.extend(sources.into_iter().map(|name| {
            let kind = if name.ends_with(".monitor") {
                AudioInputKind::System
            } else {
                AudioInputKind::Microphone
            };
            AudioInputOption {
                id: name.clone(),
                label: if kind == AudioInputKind::System {
                    format!("System audio · {name}")
                } else {
                    name.clone()
                },
                description: if kind == AudioInputKind::System {
                    format!("PulseAudio/PipeWire monitor source: {name}")
                } else {
                    format!("PulseAudio source: {name}")
                },
                kind,
            }
        }));

        return inputs;
    }

    wpctl_audio_inputs()
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
    let server_name = pactl_server_name().or_else(wpctl_server_name);
    let mut sources = pactl_short_names("sources");
    let mut sinks = pactl_short_names("sinks");

    if sources.is_empty() && sinks.is_empty() {
        let wpctl_inputs = wpctl_audio_inputs();
        sources = wpctl_inputs
            .iter()
            .filter(|input| input.kind == AudioInputKind::Microphone)
            .map(|input| input.id.clone())
            .collect();
        sinks = wpctl_inputs
            .iter()
            .filter(|input| input.kind == AudioInputKind::System)
            .map(|input| input.id.trim_end_matches(".monitor").to_string())
            .collect();
    }

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

fn wpctl_server_name() -> Option<String> {
    let output = Command::new("wpctl").args(["status"]).output().ok()?;
    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_wpctl_server_name(&stdout)
}

fn wpctl_audio_inputs() -> Vec<AudioInputOption> {
    let output = match Command::new("wpctl").args(["status"]).output() {
        Ok(output) if output.status.success() => output,
        _ => return Vec::new(),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let sources = parse_wpctl_status_entries(&stdout, "Sources:");
    let sinks = parse_wpctl_status_entries(&stdout, "Sinks:");

    let mut inputs = Vec::new();

    for source in sources {
        let node_name = wpctl_node_name(source.id).unwrap_or_else(|| source.label.clone());
        inputs.push(AudioInputOption {
            id: node_name.clone(),
            label: source.label.clone(),
            description: format!("PipeWire source: {}", source.label),
            kind: AudioInputKind::Microphone,
        });
    }

    for sink in sinks {
        let node_name = wpctl_node_name(sink.id).unwrap_or_else(|| sink.label.clone());
        let monitor_name = format!("{node_name}.monitor");
        inputs.push(AudioInputOption {
            id: monitor_name,
            label: format!("System audio · {}", sink.label),
            description: format!("PipeWire monitor source for sink: {}", sink.label),
            kind: AudioInputKind::System,
        });
    }

    inputs
}

fn wpctl_node_name(node_id: u32) -> Option<String> {
    let output = Command::new("wpctl")
        .args(["inspect", &node_id.to_string()])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_wpctl_node_name(&stdout)
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

fn parse_wpctl_server_name(stdout: &str) -> Option<String> {
    stdout
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("PipeWire "))
        .map(ToOwned::to_owned)
}

fn parse_wpctl_status_entries(stdout: &str, section_header: &str) -> Vec<WpctlEntry> {
    let mut in_audio = false;
    let mut in_section = false;
    let mut entries = Vec::new();

    for line in stdout.lines() {
        let trimmed = line.trim();

        if trimmed == "Audio" {
            in_audio = true;
            in_section = false;
            continue;
        }

        if !in_audio {
            continue;
        }

        if trimmed == "Video" || trimmed == "Settings" {
            break;
        }

        if trimmed.ends_with(section_header) {
            in_section = true;
            continue;
        }

        if trimmed.ends_with(':') {
            in_section = false;
            continue;
        }

        if !in_section {
            continue;
        }

        let candidate = trimmed.trim_start_matches(['│', '*', ' ']).trim_start();

        let Some((id_text, rest)) = candidate.split_once('.') else {
            continue;
        };
        let Ok(id) = id_text.trim().parse::<u32>() else {
            continue;
        };

        let label = rest.split("  [").next().unwrap_or(rest).trim().to_string();

        if !label.is_empty() {
            entries.push(WpctlEntry { id, label });
        }
    }

    entries
}

fn parse_wpctl_node_name(stdout: &str) -> Option<String> {
    stdout.lines().find_map(|line| {
        let trimmed = line.trim();
        trimmed
            .strip_prefix("* node.name = \"")
            .and_then(|value| value.strip_suffix('"'))
            .map(ToOwned::to_owned)
    })
}

pub(crate) fn availability_for(
    session: &LinuxDesktopSession,
    has_runtime: bool,
) -> AudioBackendAvailability {
    match session {
        LinuxDesktopSession::X11 { .. }
        | LinuxDesktopSession::WaylandOnly { .. }
        | LinuxDesktopSession::WaylandWithX11 { .. } => {
            if has_runtime {
                AudioBackendAvailability::Available
            } else {
                AudioBackendAvailability::Unavailable {
                    reason: "Linux native audio could not discover a usable PipeWire/PulseAudio runtime."
                        .to_string(),
                }
            }
        }
        LinuxDesktopSession::Headless => AudioBackendAvailability::Unavailable {
            reason:
                "A desktop session is required before the Linux native audio lane can be probed."
                    .to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{
        availability_for, parse_pactl_server_name, parse_pactl_short_names,
        parse_wpctl_server_name, parse_wpctl_status_entries,
    };
    use crate::LinuxDesktopSession;
    use capture::AudioBackendAvailability;

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

    #[test]
    fn parses_wpctl_server_name() {
        let sample = "PipeWire 'pipewire-0' [1.0.5, user@host, cookie:123]\n └─ Clients:";
        assert_eq!(
            parse_wpctl_server_name(sample).as_deref(),
            Some("PipeWire 'pipewire-0' [1.0.5, user@host, cookie:123]")
        );
    }

    #[test]
    fn parses_wpctl_sources_and_sinks() {
        let sample = r#"
PipeWire 'pipewire-0' [1.0.5, user@host, cookie:123]

Audio
 ├─ Sinks:
 │  *   49. Built-in Audio Analog Stereo        [vol: 0.49]
 │      51. HDA NVidia Digital Stereo (HDMI)    [vol: 0.40]
 │
 ├─ Sources:
 │  *   50. Built-in Audio Analog Stereo        [vol: 0.13]
 │
Video
"#;
        let sinks = parse_wpctl_status_entries(sample, "Sinks:");
        let sources = parse_wpctl_status_entries(sample, "Sources:");

        assert_eq!(sinks.len(), 2);
        assert_eq!(sinks[0].id, 49);
        assert_eq!(sinks[0].label, "Built-in Audio Analog Stereo");
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].id, 50);
        assert_eq!(sources[0].label, "Built-in Audio Analog Stereo");
    }

    #[test]
    fn parses_wpctl_node_name() {
        let sample = r#"
id 50, type PipeWire:Interface:Node
  * media.class = "Audio/Source"
  * node.description = "Built-in Audio Analog Stereo"
  * node.name = "alsa_input.pci-0000_00_1f.3.analog-stereo"
"#;

        assert_eq!(
            super::parse_wpctl_node_name(sample).as_deref(),
            Some("alsa_input.pci-0000_00_1f.3.analog-stereo")
        );
    }

    #[test]
    fn pure_wayland_audio_availability_can_be_available() {
        assert!(matches!(
            availability_for(
                &LinuxDesktopSession::WaylandOnly {
                    wayland_display: "wayland-0".to_string()
                },
                true
            ),
            AudioBackendAvailability::Available
        ));
    }
}
