use std::{
    path::PathBuf,
    time::{Duration, SystemTime},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const FULL_DESKTOP_TARGET_ID: &str = "full-desktop";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CaptureTargetOption {
    pub id: String,
    pub label: String,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct RecordingOptions {
    pub output_path: PathBuf,
    pub quality_preset: String,
    pub mic_enabled: bool,
    pub capture_target_id: String,
}

#[derive(Debug, Clone)]
pub struct ActiveRecording {
    pub backend_name: String,
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
