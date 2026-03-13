use capture::CaptureTargetOption;
use permissions::{PermissionCheck, default_permissions};
use serde::{Deserialize, Serialize};
use shortcuts::{ShortcutBinding, default_shortcuts};
use std::time::{Duration, SystemTime};
use storage::AppSettings;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RecorderStatus {
    Idle,
    Recording,
    Paused,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecorderSnapshot {
    pub status: RecorderStatus,
    pub elapsed_label: String,
    pub active_target: String,
    pub active_output_path: Option<String>,
    pub quality_preset: String,
    pub output_directory: String,
    pub mic_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    pub id: String,
    pub title: String,
    pub started_at: String,
    pub duration_label: String,
    pub location: String,
    pub size_label: String,
}

#[derive(Debug, Clone)]
pub struct CompletedRecording {
    pub title: String,
    pub started_at_label: String,
    pub duration: Duration,
    pub location: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapSnapshot {
    pub app_name: String,
    pub platform: String,
    pub launcher_window_label: String,
    pub recorder: RecorderSnapshot,
    pub settings: AppSettings,
    pub capture_targets: Vec<CaptureTargetOption>,
    pub quality_presets: Vec<String>,
    pub shortcuts: Vec<ShortcutBinding>,
    pub permissions: Vec<PermissionCheck>,
    pub recent_sessions: Vec<SessionSummary>,
    pub roadmap: Vec<String>,
}

#[derive(Debug)]
pub struct AppCore {
    settings: AppSettings,
    status: RecorderStatus,
    active_target: String,
    active_output_path: Option<String>,
    started_at: Option<SystemTime>,
    paused_at: Option<SystemTime>,
    accumulated_paused: Duration,
    shortcuts: Vec<ShortcutBinding>,
    recent_sessions: Vec<SessionSummary>,
}

impl Default for AppCore {
    fn default() -> Self {
        Self {
            settings: AppSettings::default(),
            status: RecorderStatus::Idle,
            active_target: "Full desktop".to_string(),
            active_output_path: None,
            started_at: None,
            paused_at: None,
            accumulated_paused: Duration::default(),
            shortcuts: default_shortcuts(),
            recent_sessions: vec![
                SessionSummary {
                    id: "session-001".to_string(),
                    title: "Product walkthrough".to_string(),
                    started_at: "Mar 12, 2026 · 20:30".to_string(),
                    duration_label: "14 min".to_string(),
                    location: "~/Movies/Record Screen/product-walkthrough.mp4".to_string(),
                    size_label: "426 MB".to_string(),
                },
                SessionSummary {
                    id: "session-002".to_string(),
                    title: "Bug repro clip".to_string(),
                    started_at: "Mar 11, 2026 · 17:05".to_string(),
                    duration_label: "5 min".to_string(),
                    location: "~/Movies/Record Screen/bug-repro.mov".to_string(),
                    size_label: "118 MB".to_string(),
                },
            ],
        }
    }
}

impl AppCore {
    pub fn quality_presets() -> Vec<String> {
        vec![
            "720p / 30 fps".to_string(),
            "1080p / 60 fps".to_string(),
            "1440p / 60 fps".to_string(),
            "4K / 60 fps".to_string(),
        ]
    }

    pub fn bootstrap(
        &self,
        platform: &str,
        capture_targets: Vec<CaptureTargetOption>,
    ) -> BootstrapSnapshot {
        BootstrapSnapshot {
            app_name: "Record Screen".to_string(),
            platform: platform.to_string(),
            launcher_window_label: "main".to_string(),
            recorder: self.current_snapshot(),
            settings: self.settings.clone(),
            capture_targets,
            quality_presets: Self::quality_presets(),
            shortcuts: self.shortcuts.clone(),
            permissions: default_permissions(platform),
            recent_sessions: self.recent_sessions.clone(),
            roadmap: vec![
                "Launcher shell and global shortcuts".to_string(),
                "Permission-aware recording bootstrap".to_string(),
                "Cross-platform capture backends".to_string(),
                "Review and export workflow".to_string(),
            ],
        }
    }

    pub fn recorder_status(&self) -> RecorderStatus {
        self.status
    }

    pub fn settings(&self) -> AppSettings {
        self.settings.clone()
    }

    pub fn snapshot(&self) -> RecorderSnapshot {
        self.current_snapshot()
    }

    pub fn start_recording(
        &mut self,
        active_target: String,
        output_path: String,
    ) -> RecorderSnapshot {
        self.status = RecorderStatus::Recording;
        self.active_target = active_target;
        self.active_output_path = Some(output_path);
        self.started_at = Some(SystemTime::now());
        self.paused_at = None;
        self.accumulated_paused = Duration::default();
        self.current_snapshot()
    }

    pub fn stop_recording(&mut self, completed: Option<CompletedRecording>) -> RecorderSnapshot {
        self.status = RecorderStatus::Idle;
        self.active_output_path = None;
        self.started_at = None;
        self.paused_at = None;
        self.accumulated_paused = Duration::default();

        if let Some(recording) = completed {
            self.recent_sessions.insert(
                0,
                SessionSummary {
                    id: format!("session-{}", self.recent_sessions.len() + 1),
                    title: recording.title,
                    started_at: recording.started_at_label,
                    duration_label: format_duration(recording.duration),
                    location: recording.location,
                    size_label: format_size(recording.size_bytes),
                },
            );
            self.recent_sessions.truncate(10);
        }

        self.current_snapshot()
    }

    pub fn pause_recording(&mut self) -> Option<RecorderSnapshot> {
        if self.status != RecorderStatus::Recording {
            return None;
        }

        self.status = RecorderStatus::Paused;
        self.paused_at = Some(SystemTime::now());
        Some(self.current_snapshot())
    }

    pub fn resume_recording(&mut self) -> Option<RecorderSnapshot> {
        if self.status != RecorderStatus::Paused {
            return None;
        }

        if let Some(paused_at) = self.paused_at.take() {
            self.accumulated_paused += SystemTime::now()
                .duration_since(paused_at)
                .unwrap_or_default();
        }
        self.status = RecorderStatus::Recording;
        Some(self.current_snapshot())
    }

    pub fn toggle_microphone(&mut self) -> RecorderSnapshot {
        self.settings.mic_enabled = !self.settings.mic_enabled;
        self.current_snapshot()
    }

    pub fn reset_shortcuts(&mut self) -> Vec<ShortcutBinding> {
        self.shortcuts = default_shortcuts();
        self.shortcuts.clone()
    }

    pub fn update_quality_preset(&mut self, quality_preset: String) -> AppSettings {
        if Self::quality_presets().contains(&quality_preset) {
            self.settings.quality_preset = quality_preset;
        }
        self.settings.clone()
    }

    pub fn update_output_directory(&mut self, output_directory: String) -> AppSettings {
        if !output_directory.trim().is_empty() {
            self.settings.output_directory = output_directory;
        }
        self.settings.clone()
    }

    pub fn update_launch_on_login(&mut self, launch_on_login: bool) -> AppSettings {
        self.settings.launch_on_login = launch_on_login;
        self.settings.clone()
    }

    pub fn update_capture_target(
        &mut self,
        capture_target_id: String,
        capture_target_label: String,
    ) -> AppSettings {
        if !capture_target_id.trim().is_empty() {
            self.settings.capture_target_id = capture_target_id;
            self.active_target = capture_target_label;
        }

        self.settings.clone()
    }

    fn current_snapshot(&self) -> RecorderSnapshot {
        let elapsed = self.elapsed_duration();
        let elapsed_label = match self.status {
            RecorderStatus::Idle => "Ready when you are".to_string(),
            RecorderStatus::Recording => format_duration(elapsed),
            RecorderStatus::Paused => format!("Paused at {}", format_duration(elapsed)),
        };

        RecorderSnapshot {
            status: self.status,
            elapsed_label,
            active_target: self.active_target.clone(),
            active_output_path: self.active_output_path.clone(),
            quality_preset: self.settings.quality_preset.clone(),
            output_directory: self.settings.output_directory.clone(),
            mic_enabled: self.settings.mic_enabled,
        }
    }

    fn elapsed_duration(&self) -> Duration {
        let Some(started_at) = self.started_at else {
            return Duration::default();
        };

        let end = self.paused_at.unwrap_or_else(SystemTime::now);
        end.duration_since(started_at)
            .unwrap_or_default()
            .saturating_sub(self.accumulated_paused)
    }
}

fn format_duration(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    format!("{hours:02}:{minutes:02}:{seconds:02}")
}

fn format_size(bytes: u64) -> String {
    const MB: f64 = 1024.0 * 1024.0;
    const GB: f64 = MB * 1024.0;

    if bytes as f64 >= GB {
        format!("{:.2} GB", bytes as f64 / GB)
    } else {
        format!("{:.0} MB", bytes as f64 / MB)
    }
}
