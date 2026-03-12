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
    pub launch_on_login: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            output_directory: "~/Movies/Record Screen".to_string(),
            quality_preset: "1080p / 60 fps".to_string(),
            mic_enabled: true,
            launch_on_login: true,
        }
    }
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("failed to create output directory: {0}")]
    CreateDirectory(#[from] std::io::Error),
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

pub fn next_recording_path(input: &str) -> Result<PathBuf, StorageError> {
    let directory = ensure_output_directory(input)?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    Ok(directory.join(format!("recording-{timestamp}.mp4")))
}
