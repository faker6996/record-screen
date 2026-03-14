use std::{
    path::PathBuf,
    time::{Duration, SystemTime},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const FULL_DESKTOP_TARGET_ID: &str = "full-desktop";
pub const CUSTOM_REGION_TARGET_ID: &str = "region:custom";
pub const DEFAULT_AUDIO_INPUT_ID: &str = "default";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CaptureTargetOption {
    pub id: String,
    pub label: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AudioInputKind {
    Default,
    Microphone,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AudioInputOption {
    pub id: String,
    pub label: String,
    pub description: String,
    pub kind: AudioInputKind,
}

#[derive(Debug, Clone)]
pub struct RecordingOptions {
    pub output_path: PathBuf,
    pub quality_preset: String,
    pub mic_enabled: bool,
    pub system_audio_enabled: bool,
    pub capture_target_id: String,
    pub audio_input_id: String,
    pub region_x: u32,
    pub region_y: u32,
    pub region_width: u32,
    pub region_height: u32,
    pub region_source_capture_target_id: String,
    pub region_source_origin_x: i32,
    pub region_source_origin_y: i32,
}

#[derive(Debug, Clone)]
pub struct ActiveRecording {
    pub backend_name: String,
    pub encoder_label: String,
    pub output_path: PathBuf,
    pub started_at: SystemTime,
    pub target_label: String,
}

#[derive(Debug, Clone)]
pub struct RecordingArtifact {
    pub output_path: PathBuf,
    pub started_at: SystemTime,
    pub finished_at: SystemTime,
    pub duration: Duration,
    pub bytes_written: u64,
}

pub trait CaptureController: Send {
    fn active_recording(&self) -> &ActiveRecording;
    fn pause(&mut self) -> Result<(), CaptureError>;
    fn resume(&mut self) -> Result<(), CaptureError>;
    fn stop(&mut self) -> Result<RecordingArtifact, CaptureError>;
    fn poll_finished(&mut self) -> Result<Option<RecordingArtifact>, CaptureError> {
        Ok(None)
    }
}

#[derive(Debug, Error)]
pub enum CaptureError {
    #[error("screen recording is not implemented for this platform yet")]
    UnsupportedPlatform,
    #[error("recording backend is unavailable: {0}")]
    BackendUnavailable(String),
    #[error("failed to start recording process: {0}")]
    SpawnFailed(String),
    #[error("failed to stop recording process: {0}")]
    StopFailed(String),
    #[error("failed to pause or resume recording process: {0}")]
    SignalFailed(String),
    #[error("failed to inspect recording output: {0}")]
    OutputInspectionFailed(String),
}

pub fn capability_summary() -> &'static str {
    "Cross-platform capture abstraction. Platform crates implement backend-specific recording."
}

pub fn full_desktop_target() -> CaptureTargetOption {
    CaptureTargetOption {
        id: FULL_DESKTOP_TARGET_ID.to_string(),
        label: "Full desktop".to_string(),
        description: "Record the entire active desktop layout across all connected displays."
            .to_string(),
    }
}

pub fn custom_region_target(x: u32, y: u32, width: u32, height: u32) -> CaptureTargetOption {
    CaptureTargetOption {
        id: CUSTOM_REGION_TARGET_ID.to_string(),
        label: "Custom region".to_string(),
        description: format!("Capture a custom area at {x}, {y} with size {width} x {height}."),
    }
}

pub fn default_audio_input() -> AudioInputOption {
    AudioInputOption {
        id: DEFAULT_AUDIO_INPUT_ID.to_string(),
        label: "Default input".to_string(),
        description: "Use the system default microphone or audio input device.".to_string(),
        kind: AudioInputKind::Default,
    }
}

pub fn resolve_audio_input_id(
    selected_audio_input_id: &str,
    audio_inputs: &[AudioInputOption],
) -> Option<String> {
    if audio_inputs.is_empty() {
        return None;
    }

    if selected_audio_input_id != DEFAULT_AUDIO_INPUT_ID {
        return audio_inputs
            .iter()
            .find(|input| input.id == selected_audio_input_id)
            .map(|input| input.id.clone());
    }

    preferred_audio_input(audio_inputs)
        .or_else(|| {
            audio_inputs
                .iter()
                .find(|input| input.id == DEFAULT_AUDIO_INPUT_ID)
        })
        .map(|input| input.id.clone())
}

pub fn resolve_microphone_input_id(
    selected_audio_input_id: &str,
    audio_inputs: &[AudioInputOption],
) -> Option<String> {
    if audio_inputs.is_empty() {
        return None;
    }

    if selected_audio_input_id != DEFAULT_AUDIO_INPUT_ID {
        return audio_inputs
            .iter()
            .find(|input| {
                input.id == selected_audio_input_id && input.kind != AudioInputKind::System
            })
            .map(|input| input.id.clone())
            .or_else(|| {
                preferred_audio_input(audio_inputs)
                    .map(|input| input.id.clone())
                    .or_else(|| {
                        audio_inputs
                            .iter()
                            .find(|input| input.id == DEFAULT_AUDIO_INPUT_ID)
                            .map(|input| input.id.clone())
                    })
            });
    }

    preferred_audio_input(audio_inputs)
        .or_else(|| {
            audio_inputs
                .iter()
                .find(|input| input.id == DEFAULT_AUDIO_INPUT_ID)
        })
        .map(|input| input.id.clone())
}

pub fn preferred_audio_input(audio_inputs: &[AudioInputOption]) -> Option<&AudioInputOption> {
    audio_inputs
        .iter()
        .filter(|input| input.id != DEFAULT_AUDIO_INPUT_ID)
        .filter(|input| input.kind != AudioInputKind::System)
        .max_by_key(|input| audio_input_score(input))
}

pub fn preferred_system_audio_input(
    audio_inputs: &[AudioInputOption],
) -> Option<&AudioInputOption> {
    audio_inputs
        .iter()
        .filter(|input| input.kind == AudioInputKind::System)
        .max_by_key(|input| audio_input_score(input))
}

fn audio_input_score(input: &AudioInputOption) -> i32 {
    let haystack = format!(
        "{} {}",
        input.label.to_ascii_lowercase(),
        input.description.to_ascii_lowercase()
    );

    let mut score = 0;

    if input.kind == AudioInputKind::System {
        score += 180;
    }

    if haystack.contains("microphone") || haystack.contains("mic") {
        score += 120;
    }
    if haystack.contains("built-in") || haystack.contains("internal") {
        score += 20;
    }
    if haystack.contains("usb") || haystack.contains("interface") {
        score += 18;
    }
    if haystack.contains("headset") || haystack.contains("airpods") {
        score += 14;
    }
    if haystack.contains("array") {
        score += 10;
    }
    if haystack.contains("monitor")
        || haystack.contains("stereo mix")
        || haystack.contains("what u hear")
        || haystack.contains("loopback")
        || haystack.contains("speaker")
        || haystack.contains("output")
    {
        score -= 140;
    }

    score
}

#[cfg(test)]
mod tests {
    use super::{
        AudioInputKind, AudioInputOption, DEFAULT_AUDIO_INPUT_ID, preferred_system_audio_input,
        resolve_audio_input_id, resolve_microphone_input_id,
    };

    #[test]
    fn resolves_default_to_best_microphone_candidate() {
        let audio_inputs = vec![
            AudioInputOption {
                id: DEFAULT_AUDIO_INPUT_ID.to_string(),
                label: "Default input".to_string(),
                description: "System default microphone".to_string(),
                kind: AudioInputKind::Default,
            },
            AudioInputOption {
                id: "monitor.loopback".to_string(),
                label: "Monitor of Built-in Audio".to_string(),
                description: "Loopback output".to_string(),
                kind: AudioInputKind::System,
            },
            AudioInputOption {
                id: "usb-mic".to_string(),
                label: "USB Microphone".to_string(),
                description: "External USB microphone".to_string(),
                kind: AudioInputKind::Microphone,
            },
        ];

        assert_eq!(
            resolve_audio_input_id(DEFAULT_AUDIO_INPUT_ID, &audio_inputs).as_deref(),
            Some("usb-mic")
        );
    }

    #[test]
    fn preserves_explicit_audio_input_when_available() {
        let audio_inputs = vec![AudioInputOption {
            id: "built-in".to_string(),
            label: "Built-in Microphone".to_string(),
            description: "Internal microphone".to_string(),
            kind: AudioInputKind::Microphone,
        }];

        assert_eq!(
            resolve_audio_input_id("built-in", &audio_inputs).as_deref(),
            Some("built-in")
        );
    }

    #[test]
    fn prefers_system_audio_when_requested() {
        let audio_inputs = vec![
            AudioInputOption {
                id: "usb-mic".to_string(),
                label: "USB Microphone".to_string(),
                description: "External USB microphone".to_string(),
                kind: AudioInputKind::Microphone,
            },
            AudioInputOption {
                id: "alsa_output.monitor".to_string(),
                label: "System audio".to_string(),
                description: "Loopback monitor".to_string(),
                kind: AudioInputKind::System,
            },
        ];

        assert_eq!(
            preferred_system_audio_input(&audio_inputs).map(|input| input.id.as_str()),
            Some("alsa_output.monitor")
        );
    }

    #[test]
    fn resolves_microphone_input_away_from_system_sources() {
        let audio_inputs = vec![
            AudioInputOption {
                id: DEFAULT_AUDIO_INPUT_ID.to_string(),
                label: "Default input".to_string(),
                description: "System default".to_string(),
                kind: AudioInputKind::Default,
            },
            AudioInputOption {
                id: "loopback".to_string(),
                label: "Stereo Mix".to_string(),
                description: "System audio".to_string(),
                kind: AudioInputKind::System,
            },
            AudioInputOption {
                id: "usb-mic".to_string(),
                label: "USB Microphone".to_string(),
                description: "External microphone".to_string(),
                kind: AudioInputKind::Microphone,
            },
        ];

        assert_eq!(
            resolve_microphone_input_id("loopback", &audio_inputs).as_deref(),
            Some("usb-mic")
        );
    }
}
