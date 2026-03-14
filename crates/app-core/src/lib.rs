use capture::{AudioInputOption, CaptureTargetOption};
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
    pub active_encoder_label: Option<String>,
    pub quality_preset: String,
    pub output_directory: String,
    pub mic_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDiagnostics {
    pub summary: String,
    pub backend_path: String,
    pub readiness: String,
    pub supports_custom_region: bool,
    pub custom_region_note: String,
    pub supports_system_audio: bool,
    pub system_audio_note: String,
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
    pub app_version: String,
    pub app_author: String,
    pub app_license: String,
    pub platform: String,
    pub launcher_window_label: String,
    pub recorder: RecorderSnapshot,
    pub settings: AppSettings,
    pub capture_targets: Vec<CaptureTargetOption>,
    pub audio_inputs: Vec<AudioInputOption>,
    pub quality_presets: Vec<String>,
    pub shortcuts: Vec<ShortcutBinding>,
    pub permissions: Vec<PermissionCheck>,
    pub diagnostics: RuntimeDiagnostics,
    pub recent_sessions: Vec<SessionSummary>,
    pub roadmap: Vec<String>,
}

#[derive(Debug)]
pub struct AppCore {
    settings: AppSettings,
    status: RecorderStatus,
    active_target: String,
    active_output_path: Option<String>,
    active_encoder_label: Option<String>,
    started_at: Option<SystemTime>,
    paused_at: Option<SystemTime>,
    accumulated_paused: Duration,
    shortcuts: Vec<ShortcutBinding>,
    recent_sessions: Vec<SessionSummary>,
}

impl Default for AppCore {
    fn default() -> Self {
        Self::new(AppSettings::default(), default_shortcuts())
    }
}

impl AppCore {
    pub fn new(settings: AppSettings, shortcuts: Vec<ShortcutBinding>) -> Self {
        Self {
            settings,
            status: RecorderStatus::Idle,
            active_target: "Full desktop".to_string(),
            active_output_path: None,
            active_encoder_label: None,
            started_at: None,
            paused_at: None,
            accumulated_paused: Duration::default(),
            shortcuts,
            recent_sessions: vec![],
        }
    }

    pub fn quality_presets() -> Vec<String> {
        vec![
            "720p / 30 fps".to_string(),
            "1080p / 30 fps".to_string(),
            "1080p / 60 fps".to_string(),
            "1440p / 60 fps".to_string(),
            "4K / 60 fps".to_string(),
        ]
    }

    pub fn bootstrap(
        &self,
        platform: &str,
        app_version: &str,
        capture_targets: Vec<CaptureTargetOption>,
        audio_inputs: Vec<AudioInputOption>,
        diagnostics: RuntimeDiagnostics,
    ) -> BootstrapSnapshot {
        BootstrapSnapshot {
            app_name: "Record Screen".to_string(),
            app_version: app_version.to_string(),
            app_author: "Tran Van Bach".to_string(),
            app_license: "MIT".to_string(),
            platform: platform.to_string(),
            launcher_window_label: "main".to_string(),
            recorder: self.current_snapshot(),
            settings: self.settings.clone(),
            capture_targets,
            audio_inputs,
            quality_presets: Self::quality_presets(),
            shortcuts: self.shortcuts.clone(),
            permissions: default_permissions(platform),
            diagnostics,
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
        active_encoder_label: String,
        output_path: String,
    ) -> RecorderSnapshot {
        self.status = RecorderStatus::Recording;
        self.active_target = active_target;
        self.active_output_path = Some(output_path);
        self.active_encoder_label = Some(active_encoder_label);
        self.started_at = Some(SystemTime::now());
        self.paused_at = None;
        self.accumulated_paused = Duration::default();
        self.current_snapshot()
    }

    pub fn stop_recording(&mut self, completed: Option<CompletedRecording>) -> RecorderSnapshot {
        self.status = RecorderStatus::Idle;
        self.active_output_path = None;
        self.active_encoder_label = None;
        self.started_at = None;
        self.paused_at = None;
        self.accumulated_paused = Duration::default();

        if let Some(recording) = completed {
            self.push_completed_recording(recording);
        }

        self.current_snapshot()
    }

    pub fn push_completed_recording(&mut self, recording: CompletedRecording) {
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

    pub fn update_system_audio_enabled(&mut self, system_audio_enabled: bool) -> AppSettings {
        self.settings.system_audio_enabled = system_audio_enabled;
        self.settings.clone()
    }

    pub fn reset_shortcuts(&mut self) -> Vec<ShortcutBinding> {
        self.shortcuts = default_shortcuts();
        self.shortcuts.clone()
    }

    pub fn shortcuts(&self) -> Vec<ShortcutBinding> {
        self.shortcuts.clone()
    }

    pub fn update_shortcut(
        &mut self,
        action: shortcuts::ShortcutAction,
        accelerator: String,
    ) -> Vec<ShortcutBinding> {
        if let Some(binding) = self
            .shortcuts
            .iter_mut()
            .find(|binding| binding.action == action)
        {
            binding.accelerator = accelerator;
        }

        self.shortcuts.clone()
    }

    pub fn sync_recent_sessions(&mut self, recent_sessions: Vec<SessionSummary>) {
        self.recent_sessions = recent_sessions;
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

    pub fn update_show_hud_during_recording(
        &mut self,
        show_hud_during_recording: bool,
    ) -> AppSettings {
        self.settings.show_hud_during_recording = show_hud_during_recording;
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

    pub fn update_audio_input(&mut self, audio_input_id: String) -> AppSettings {
        if !audio_input_id.trim().is_empty() {
            self.settings.audio_input_id = audio_input_id;
        }

        self.settings.clone()
    }

    pub fn update_custom_region(
        &mut self,
        region_x: u32,
        region_y: u32,
        region_width: u32,
        region_height: u32,
        region_source_capture_target_id: Option<String>,
        region_source_origin_x: Option<i32>,
        region_source_origin_y: Option<i32>,
    ) -> AppSettings {
        self.settings.region_x = region_x;
        self.settings.region_y = region_y;
        self.settings.region_width = region_width.max(64);
        self.settings.region_height = region_height.max(64);
        if let Some(region_source_capture_target_id) = region_source_capture_target_id {
            if !region_source_capture_target_id.trim().is_empty() {
                self.settings.region_source_capture_target_id = region_source_capture_target_id;
            }
        }
        if let Some(region_source_origin_x) = region_source_origin_x {
            self.settings.region_source_origin_x = region_source_origin_x;
        }
        if let Some(region_source_origin_y) = region_source_origin_y {
            self.settings.region_source_origin_y = region_source_origin_y;
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
            active_encoder_label: self.active_encoder_label.clone(),
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
