use permissions::{PermissionCheck, default_permissions};
use serde::{Deserialize, Serialize};
use shortcuts::{ShortcutBinding, default_shortcuts};
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapSnapshot {
    pub app_name: String,
    pub platform: String,
    pub launcher_window_label: String,
    pub recorder: RecorderSnapshot,
    pub settings: AppSettings,
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
    shortcuts: Vec<ShortcutBinding>,
    recent_sessions: Vec<SessionSummary>,
}

impl Default for AppCore {
    fn default() -> Self {
        Self {
            settings: AppSettings::default(),
            status: RecorderStatus::Idle,
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

    pub fn bootstrap(&self, platform: &str) -> BootstrapSnapshot {
        BootstrapSnapshot {
            app_name: "Record Screen".to_string(),
            platform: platform.to_string(),
            launcher_window_label: "main".to_string(),
            recorder: self.current_snapshot(),
            settings: self.settings.clone(),
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

    pub fn toggle_recording(&mut self) -> RecorderSnapshot {
        self.status = match self.status {
            RecorderStatus::Idle => RecorderStatus::Recording,
            RecorderStatus::Recording | RecorderStatus::Paused => RecorderStatus::Idle,
        };
        self.current_snapshot()
    }

    pub fn pause_resume(&mut self) -> Option<RecorderSnapshot> {
        self.status = match self.status {
            RecorderStatus::Recording => RecorderStatus::Paused,
            RecorderStatus::Paused => RecorderStatus::Recording,
            RecorderStatus::Idle => return None,
        };
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

    fn current_snapshot(&self) -> RecorderSnapshot {
        let elapsed_label = match self.status {
            RecorderStatus::Idle => "Ready when you are".to_string(),
            RecorderStatus::Recording => "00:12:41".to_string(),
            RecorderStatus::Paused => "Paused at 00:12:41".to_string(),
        };

        RecorderSnapshot {
            status: self.status,
            elapsed_label,
            active_target: "Entire display".to_string(),
            quality_preset: self.settings.quality_preset.clone(),
            output_directory: self.settings.output_directory.clone(),
            mic_enabled: self.settings.mic_enabled,
        }
    }
}
