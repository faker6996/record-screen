use std::{
    env,
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
    pub portal_parent_window: Option<String>,
    pub portal_restore_token: Option<String>,
    pub region_x: u32,
    pub region_y: u32,
    pub region_width: u32,
    pub region_height: u32,
    pub region_source_capture_target_id: String,
    pub region_source_origin_x: i32,
    pub region_source_origin_y: i32,
    pub region_source_scale_factor_milli: u32,
}

#[derive(Debug, Clone)]
pub struct ActiveRecording {
    pub backend_name: String,
    pub encoder_label: String,
    pub output_path: PathBuf,
    pub started_at: SystemTime,
    pub target_label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioBackendDescriptor {
    pub id: &'static str,
    pub label: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioBackendAvailability {
    Available,
    Unavailable { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioBackendStatus {
    pub descriptor: AudioBackendDescriptor,
    pub availability: AudioBackendAvailability,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AudioBackendRuntimeReport {
    pub summary: Option<String>,
    pub preferred_input_id: Option<String>,
    pub preferred_input_label: Option<String>,
    pub preferred_system_id: Option<String>,
    pub preferred_system_label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncoderBackendDescriptor {
    pub id: &'static str,
    pub label: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncoderBackendAvailability {
    Available,
    Unavailable { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncoderBackendStatus {
    pub descriptor: EncoderBackendDescriptor,
    pub availability: EncoderBackendAvailability,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EncoderBackendRuntimeReport {
    pub summary: Option<String>,
    pub preferred_encoder_label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureBackendDescriptor {
    pub id: &'static str,
    pub label: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureBackendAvailability {
    Available,
    Unavailable { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureBackendStatus {
    pub descriptor: CaptureBackendDescriptor,
    pub availability: CaptureBackendAvailability,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CaptureBackendRuntimeReport {
    pub summary: Option<String>,
    pub preferred_target_label: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CaptureBackendRuntimeSnapshot {
    pub path: String,
    pub summary: Option<String>,
    pub preferred_target_label: Option<String>,
    pub selection_note: String,
    pub native_unavailable_note: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AudioBackendRuntimeSnapshot {
    pub path: String,
    pub summary: Option<String>,
    pub preferred_input_id: Option<String>,
    pub preferred_input_label: Option<String>,
    pub preferred_system_id: Option<String>,
    pub preferred_system_label: Option<String>,
    pub selection_note: String,
    pub native_unavailable_note: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EncoderBackendRuntimeSnapshot {
    pub path: String,
    pub summary: Option<String>,
    pub preferred_encoder_label: Option<String>,
    pub selection_note: String,
    pub native_unavailable_note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendSelectionExplanation {
    pub selected_id: &'static str,
    pub selected_label: &'static str,
    pub note: String,
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
    fn supports_pause_resume(&self) -> bool {
        true
    }
    fn pause_resume_note(&self) -> Option<String> {
        None
    }
    fn poll_finished(&mut self) -> Result<Option<RecordingArtifact>, CaptureError> {
        Ok(None)
    }
}

pub trait CaptureBackendFactory: Send + Sync {
    fn descriptor(&self) -> CaptureBackendDescriptor;
    fn availability(&self) -> CaptureBackendAvailability;
    fn runtime_report(&self) -> CaptureBackendRuntimeReport;
    fn start(&self, options: RecordingOptions) -> Result<Box<dyn CaptureController>, CaptureError>;
}

pub trait AudioBackendFactory: Send + Sync {
    fn descriptor(&self) -> AudioBackendDescriptor;
    fn availability(&self) -> AudioBackendAvailability;
    fn runtime_report(&self) -> AudioBackendRuntimeReport;
}

pub trait EncoderBackendFactory: Send + Sync {
    fn descriptor(&self) -> EncoderBackendDescriptor;
    fn availability(&self) -> EncoderBackendAvailability;
    fn runtime_report(&self) -> EncoderBackendRuntimeReport;
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

pub fn backend_statuses(
    candidates: &[&'static dyn CaptureBackendFactory],
) -> Vec<CaptureBackendStatus> {
    candidates
        .iter()
        .map(|backend| CaptureBackendStatus {
            descriptor: backend.descriptor(),
            availability: backend.availability(),
        })
        .collect()
}

pub fn audio_backend_statuses(
    candidates: &[&'static dyn AudioBackendFactory],
) -> Vec<AudioBackendStatus> {
    candidates
        .iter()
        .map(|backend| AudioBackendStatus {
            descriptor: backend.descriptor(),
            availability: backend.availability(),
        })
        .collect()
}

pub fn encoder_backend_statuses(
    candidates: &[&'static dyn EncoderBackendFactory],
) -> Vec<EncoderBackendStatus> {
    candidates
        .iter()
        .map(|backend| EncoderBackendStatus {
            descriptor: backend.descriptor(),
            availability: backend.availability(),
        })
        .collect()
}

pub fn select_backend(
    candidates: &[&'static dyn CaptureBackendFactory],
) -> &'static dyn CaptureBackendFactory {
    if let Some(requested_id) = env::var_os("RECORD_SCREEN_CAPTURE_BACKEND") {
        let requested_id = requested_id.to_string_lossy();
        if let Some(candidate) = candidates
            .iter()
            .copied()
            .find(|backend| backend.descriptor().id == requested_id)
        {
            return candidate;
        }
    }

    candidates
        .iter()
        .copied()
        .find(|backend| {
            matches!(
                backend.availability(),
                CaptureBackendAvailability::Available
            )
        })
        .or_else(|| candidates.first().copied())
        .expect("capture backend registry must contain at least one backend")
}

pub fn select_audio_backend(
    candidates: &[&'static dyn AudioBackendFactory],
) -> &'static dyn AudioBackendFactory {
    if let Some(requested_id) = env::var_os("RECORD_SCREEN_AUDIO_BACKEND") {
        let requested_id = requested_id.to_string_lossy();
        if let Some(candidate) = candidates
            .iter()
            .copied()
            .find(|backend| backend.descriptor().id == requested_id)
        {
            return candidate;
        }
    }

    candidates
        .iter()
        .copied()
        .find(|backend| matches!(backend.availability(), AudioBackendAvailability::Available))
        .or_else(|| candidates.first().copied())
        .expect("audio backend registry must contain at least one backend")
}

pub fn select_encoder_backend(
    candidates: &[&'static dyn EncoderBackendFactory],
) -> &'static dyn EncoderBackendFactory {
    if let Some(requested_id) = env::var_os("RECORD_SCREEN_ENCODER_BACKEND") {
        let requested_id = requested_id.to_string_lossy();
        if let Some(candidate) = candidates
            .iter()
            .copied()
            .find(|backend| backend.descriptor().id == requested_id)
        {
            return candidate;
        }
    }

    candidates
        .iter()
        .copied()
        .find(|backend| {
            matches!(
                backend.availability(),
                EncoderBackendAvailability::Available
            )
        })
        .or_else(|| candidates.first().copied())
        .expect("encoder backend registry must contain at least one backend")
}

pub fn explain_capture_backend_selection(
    candidates: &[&'static dyn CaptureBackendFactory],
) -> BackendSelectionExplanation {
    let selected = select_backend(candidates);
    let descriptor = selected.descriptor();
    let availability = selected.availability();

    if let Some(requested_id) = env::var_os("RECORD_SCREEN_CAPTURE_BACKEND") {
        let requested_id = requested_id.to_string_lossy();
        if requested_id == descriptor.id {
            return BackendSelectionExplanation {
                selected_id: descriptor.id,
                selected_label: descriptor.label,
                note: match availability {
                    CaptureBackendAvailability::Available => format!(
                        "Capture backend forced by RECORD_SCREEN_CAPTURE_BACKEND={requested_id}."
                    ),
                    CaptureBackendAvailability::Unavailable { reason } => format!(
                        "Capture backend forced by RECORD_SCREEN_CAPTURE_BACKEND={requested_id}, but that backend is not available yet: {reason}"
                    ),
                },
            };
        }
    }

    let note = match availability {
        CaptureBackendAvailability::Available => {
            "Capture selected the first available backend in the registry.".to_string()
        }
        CaptureBackendAvailability::Unavailable { reason } => format!(
            "Capture registry had no available backend, so it fell back to `{}`. {reason}",
            descriptor.label
        ),
    };

    BackendSelectionExplanation {
        selected_id: descriptor.id,
        selected_label: descriptor.label,
        note,
    }
}

pub fn capture_backend_runtime_snapshot(
    candidates: &[&'static dyn CaptureBackendFactory],
) -> CaptureBackendRuntimeSnapshot {
    let selected = select_backend(candidates);
    let runtime_report = selected.runtime_report();
    let selection = explain_capture_backend_selection(candidates);
    let native_unavailable_note =
        join_capture_native_unavailable_notes(&backend_statuses(candidates));

    CaptureBackendRuntimeSnapshot {
        path: selected.descriptor().label.to_string(),
        summary: runtime_report.summary,
        preferred_target_label: runtime_report.preferred_target_label,
        selection_note: selection.note,
        native_unavailable_note,
    }
}

pub fn explain_audio_backend_selection(
    candidates: &[&'static dyn AudioBackendFactory],
) -> BackendSelectionExplanation {
    let selected = select_audio_backend(candidates);
    let descriptor = selected.descriptor();
    let availability = selected.availability();

    if let Some(requested_id) = env::var_os("RECORD_SCREEN_AUDIO_BACKEND") {
        let requested_id = requested_id.to_string_lossy();
        if requested_id == descriptor.id {
            return BackendSelectionExplanation {
                selected_id: descriptor.id,
                selected_label: descriptor.label,
                note: match availability {
                    AudioBackendAvailability::Available => format!(
                        "Audio backend forced by RECORD_SCREEN_AUDIO_BACKEND={requested_id}."
                    ),
                    AudioBackendAvailability::Unavailable { reason } => format!(
                        "Audio backend forced by RECORD_SCREEN_AUDIO_BACKEND={requested_id}, but that backend is not available yet: {reason}"
                    ),
                },
            };
        }
    }

    let note = match availability {
        AudioBackendAvailability::Available => {
            "Audio selected the first available backend in the registry.".to_string()
        }
        AudioBackendAvailability::Unavailable { reason } => format!(
            "Audio registry had no available backend, so it fell back to `{}`. {reason}",
            descriptor.label
        ),
    };

    BackendSelectionExplanation {
        selected_id: descriptor.id,
        selected_label: descriptor.label,
        note,
    }
}

pub fn audio_backend_runtime_snapshot(
    candidates: &[&'static dyn AudioBackendFactory],
) -> AudioBackendRuntimeSnapshot {
    let selected = select_audio_backend(candidates);
    let runtime_report = selected.runtime_report();
    let selection = explain_audio_backend_selection(candidates);
    let native_unavailable_note =
        join_audio_native_unavailable_notes(&audio_backend_statuses(candidates));

    AudioBackendRuntimeSnapshot {
        path: selected.descriptor().label.to_string(),
        summary: runtime_report.summary,
        preferred_input_id: runtime_report.preferred_input_id,
        preferred_input_label: runtime_report.preferred_input_label,
        preferred_system_id: runtime_report.preferred_system_id,
        preferred_system_label: runtime_report.preferred_system_label,
        selection_note: selection.note,
        native_unavailable_note,
    }
}

pub fn explain_encoder_backend_selection(
    candidates: &[&'static dyn EncoderBackendFactory],
) -> BackendSelectionExplanation {
    let selected = select_encoder_backend(candidates);
    let descriptor = selected.descriptor();
    let availability = selected.availability();

    if let Some(requested_id) = env::var_os("RECORD_SCREEN_ENCODER_BACKEND") {
        let requested_id = requested_id.to_string_lossy();
        if requested_id == descriptor.id {
            return BackendSelectionExplanation {
                selected_id: descriptor.id,
                selected_label: descriptor.label,
                note: match availability {
                    EncoderBackendAvailability::Available => format!(
                        "Encoder backend forced by RECORD_SCREEN_ENCODER_BACKEND={requested_id}."
                    ),
                    EncoderBackendAvailability::Unavailable { reason } => format!(
                        "Encoder backend forced by RECORD_SCREEN_ENCODER_BACKEND={requested_id}, but that backend is not available yet: {reason}"
                    ),
                },
            };
        }
    }

    let note = match availability {
        EncoderBackendAvailability::Available => {
            "Encoder selected the first available backend in the registry.".to_string()
        }
        EncoderBackendAvailability::Unavailable { reason } => format!(
            "Encoder registry had no available backend, so it fell back to `{}`. {reason}",
            descriptor.label
        ),
    };

    BackendSelectionExplanation {
        selected_id: descriptor.id,
        selected_label: descriptor.label,
        note,
    }
}

pub fn encoder_backend_runtime_snapshot(
    candidates: &[&'static dyn EncoderBackendFactory],
) -> EncoderBackendRuntimeSnapshot {
    let selected = select_encoder_backend(candidates);
    let runtime_report = selected.runtime_report();
    let selection = explain_encoder_backend_selection(candidates);
    let native_unavailable_note =
        join_encoder_native_unavailable_notes(&encoder_backend_statuses(candidates));

    EncoderBackendRuntimeSnapshot {
        path: selected.descriptor().label.to_string(),
        summary: runtime_report.summary,
        preferred_encoder_label: runtime_report.preferred_encoder_label,
        selection_note: selection.note,
        native_unavailable_note,
    }
}

fn join_capture_native_unavailable_notes(statuses: &[CaptureBackendStatus]) -> Option<String> {
    let note = statuses
        .iter()
        .filter_map(|status| match &status.availability {
            CaptureBackendAvailability::Unavailable { reason } => Some(format!(
                "{} is not active yet: {reason}",
                status.descriptor.label
            )),
            CaptureBackendAvailability::Available => None,
        })
        .collect::<Vec<_>>()
        .join(" ");

    (!note.is_empty()).then_some(note)
}

fn join_audio_native_unavailable_notes(statuses: &[AudioBackendStatus]) -> Option<String> {
    let note = statuses
        .iter()
        .filter_map(|status| match &status.availability {
            AudioBackendAvailability::Unavailable { reason } => Some(format!(
                "{} is not active yet: {reason}",
                status.descriptor.label
            )),
            AudioBackendAvailability::Available => None,
        })
        .collect::<Vec<_>>()
        .join(" ");

    (!note.is_empty()).then_some(note)
}

fn join_encoder_native_unavailable_notes(statuses: &[EncoderBackendStatus]) -> Option<String> {
    let note = statuses
        .iter()
        .filter_map(|status| match &status.availability {
            EncoderBackendAvailability::Unavailable { reason } => Some(format!(
                "{} is not active yet: {reason}",
                status.descriptor.label
            )),
            EncoderBackendAvailability::Available => None,
        })
        .collect::<Vec<_>>()
        .join(" ");

    (!note.is_empty()).then_some(note)
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
