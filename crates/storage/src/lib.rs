use serde::{Deserialize, Serialize};
use std::{
    env, fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub output_directory: String,
    pub quality_preset: String,
    pub mic_enabled: bool,
    pub audio_input_id: String,
    pub launch_on_login: bool,
    pub show_hud_during_recording: bool,
    pub capture_target_id: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            output_directory: "~/Movies/Record Screen".to_string(),
            quality_preset: "1080p / 30 fps".to_string(),
            mic_enabled: true,
            audio_input_id: capture::DEFAULT_AUDIO_INPUT_ID.to_string(),
            launch_on_login: false,
            show_hud_during_recording: true,
            capture_target_id: "full-desktop".to_string(),
        }
    }
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("failed to create output directory: {0}")]
    CreateDirectory(#[from] std::io::Error),
    #[error("failed to create app config directory: {0}")]
    CreateConfigDirectory(std::io::Error),
    #[error("failed to read app settings: {0}")]
    ReadSettings(std::io::Error),
    #[error("failed to write app settings: {0}")]
    WriteSettings(std::io::Error),
    #[error("failed to parse app settings: {0}")]
    ParseSettings(serde_json::Error),
    #[error("failed to serialize app settings: {0}")]
    SerializeSettings(serde_json::Error),
}

pub fn expand_home_path(input: &str) -> PathBuf {
    if let Some(stripped) = input.strip_prefix("~/") {
        if let Ok(home) = env::var("HOME") {
            return Path::new(&home).join(stripped);
        }
    }

    PathBuf::from(input)
}

pub fn ensure_output_directory(input: &str) -> Result<PathBuf, StorageError> {
    let directory = expand_home_path(input);
    fs::create_dir_all(&directory)?;
    Ok(directory)
}

pub fn app_config_directory() -> PathBuf {
    if let Ok(config_home) = env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(config_home).join("record-screen");
    }

    if let Ok(home) = env::var("HOME") {
        return Path::new(&home).join(".config/record-screen");
    }

    PathBuf::from(".record-screen")
}

pub fn app_settings_path() -> PathBuf {
    app_config_directory().join("settings.json")
}

pub fn load_app_settings() -> Result<AppSettings, StorageError> {
    let path = app_settings_path();
    if !path.exists() {
        return Ok(AppSettings::default());
    }

    let contents = fs::read_to_string(&path).map_err(StorageError::ReadSettings)?;
    serde_json::from_str(&contents).map_err(StorageError::ParseSettings)
}

pub fn save_app_settings(settings: &AppSettings) -> Result<(), StorageError> {
    let directory = app_config_directory();
    fs::create_dir_all(&directory).map_err(StorageError::CreateConfigDirectory)?;
    let payload =
        serde_json::to_string_pretty(settings).map_err(StorageError::SerializeSettings)?;
    fs::write(app_settings_path(), payload).map_err(StorageError::WriteSettings)
}

pub fn next_recording_path(input: &str) -> Result<PathBuf, StorageError> {
    let directory = ensure_output_directory(input)?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    Ok(directory.join(format!("recording-{timestamp}.mp4")))
}

#[cfg(test)]
mod tests {
    use super::{AppSettings, app_settings_path};

    #[test]
    fn default_launch_on_login_is_disabled() {
        assert!(!AppSettings::default().launch_on_login);
    }

    #[test]
    fn settings_path_ends_with_settings_json() {
        assert!(app_settings_path().ends_with("settings.json"));
    }
}
