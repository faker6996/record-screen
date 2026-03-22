use serde::{Deserialize, Serialize};
use shortcuts::{ShortcutAction, ShortcutBinding, default_shortcuts};
use std::{
    env, fs,
    fs::OpenOptions,
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
    pub system_audio_enabled: bool,
    pub audio_input_id: String,
    pub wayland_restore_token: Option<String>,
    pub launch_on_login: bool,
    pub show_hud_during_recording: bool,
    pub capture_target_id: String,
    pub region_x: u32,
    pub region_y: u32,
    pub region_width: u32,
    pub region_height: u32,
    pub region_source_capture_target_id: String,
    pub region_source_origin_x: i32,
    pub region_source_origin_y: i32,
    pub region_source_scale_factor_milli: u32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            output_directory: "~/Movies/Record Screen".to_string(),
            quality_preset: "1080p / 30 fps".to_string(),
            mic_enabled: true,
            system_audio_enabled: false,
            audio_input_id: capture::DEFAULT_AUDIO_INPUT_ID.to_string(),
            wayland_restore_token: None,
            launch_on_login: false,
            show_hud_during_recording: true,
            capture_target_id: "full-desktop".to_string(),
            region_x: 160,
            region_y: 120,
            region_width: 1280,
            region_height: 720,
            region_source_capture_target_id: capture::FULL_DESKTOP_TARGET_ID.to_string(),
            region_source_origin_x: 0,
            region_source_origin_y: 0,
            region_source_scale_factor_milli: 1000,
        }
    }
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("failed to create output directory: {0}")]
    CreateDirectory(#[from] std::io::Error),
    #[error("failed to validate recording output path: {0}")]
    ValidateOutputPath(std::io::Error),
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
    #[error("failed to read shortcuts: {0}")]
    ReadShortcuts(std::io::Error),
    #[error("failed to write shortcuts: {0}")]
    WriteShortcuts(std::io::Error),
    #[error("failed to parse shortcuts: {0}")]
    ParseShortcuts(serde_json::Error),
    #[error("failed to serialize shortcuts: {0}")]
    SerializeShortcuts(serde_json::Error),
}

pub fn expand_home_path(input: &str) -> PathBuf {
    if let Some(stripped) = input.strip_prefix("~/") {
        if let Some(home) = user_home_directory() {
            return home.join(stripped);
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
    if let Some(config_home) = env_var_path("XDG_CONFIG_HOME") {
        return config_home.join("record-screen");
    }

    if let Some(app_data) = env_var_path("APPDATA") {
        return app_data.join("record-screen");
    }

    if let Some(home) = user_home_directory() {
        return home.join(".config/record-screen");
    }

    PathBuf::from(".record-screen")
}

fn env_var_string(key: &str) -> Option<String> {
    env_var_string_with(&|candidate| env::var(candidate).ok(), key)
}

fn env_var_string_with(get_env: &impl Fn(&str) -> Option<String>, key: &str) -> Option<String> {
    get_env(key)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn env_var_path(key: &str) -> Option<PathBuf> {
    env_var_string(key).map(PathBuf::from)
}

fn user_home_directory() -> Option<PathBuf> {
    user_home_directory_with(&|candidate| env::var(candidate).ok())
}

fn user_home_directory_with(get_env: &impl Fn(&str) -> Option<String>) -> Option<PathBuf> {
    if let Some(home) = env_var_string_with(get_env, "HOME") {
        return Some(PathBuf::from(home));
    }

    if let Some(user_profile) = env_var_string_with(get_env, "USERPROFILE") {
        return Some(PathBuf::from(user_profile));
    }

    let home_drive = env_var_string_with(get_env, "HOMEDRIVE");
    let home_path = env_var_string_with(get_env, "HOMEPATH");

    match (home_drive, home_path) {
        (Some(home_drive), Some(home_path)) => {
            Some(PathBuf::from(format!("{home_drive}{home_path}")))
        }
        _ => None,
    }
}

#[cfg(test)]
fn app_config_directory_with(get_env: &impl Fn(&str) -> Option<String>) -> PathBuf {
    if let Some(config_home) = env_var_string_with(get_env, "XDG_CONFIG_HOME") {
        return PathBuf::from(config_home).join("record-screen");
    }

    if let Some(app_data) = env_var_string_with(get_env, "APPDATA") {
        return PathBuf::from(app_data).join("record-screen");
    }

    if let Some(home) = user_home_directory_with(get_env) {
        return home.join(".config/record-screen");
    }

    PathBuf::from(".record-screen")
}

#[cfg(test)]
fn expand_home_path_with(input: &str, get_env: &impl Fn(&str) -> Option<String>) -> PathBuf {
    if let Some(stripped) = input.strip_prefix("~/") {
        if let Some(home) = user_home_directory_with(get_env) {
            return home.join(stripped);
        }
    }

    PathBuf::from(input)
}

pub fn app_settings_path() -> PathBuf {
    app_config_directory().join("settings.json")
}

pub fn shortcuts_path() -> PathBuf {
    app_config_directory().join("shortcuts.json")
}

pub fn load_app_settings() -> Result<AppSettings, StorageError> {
    let path = app_settings_path();
    if !path.exists() {
        return Ok(AppSettings::default());
    }

    let contents = fs::read_to_string(&path).map_err(StorageError::ReadSettings)?;
    let persisted: serde_json::Value =
        serde_json::from_str(&contents).map_err(StorageError::ParseSettings)?;
    let defaults =
        serde_json::to_value(AppSettings::default()).map_err(StorageError::SerializeSettings)?;
    let merged = merge_json(defaults, persisted);
    serde_json::from_value(merged).map_err(StorageError::ParseSettings)
}

pub fn save_app_settings(settings: &AppSettings) -> Result<(), StorageError> {
    let directory = app_config_directory();
    fs::create_dir_all(&directory).map_err(StorageError::CreateConfigDirectory)?;
    let payload =
        serde_json::to_string_pretty(settings).map_err(StorageError::SerializeSettings)?;
    fs::write(app_settings_path(), payload).map_err(StorageError::WriteSettings)
}

pub fn load_shortcuts() -> Result<Vec<ShortcutBinding>, StorageError> {
    let path = shortcuts_path();
    if !path.exists() {
        return Ok(default_shortcuts());
    }

    let contents = fs::read_to_string(&path).map_err(StorageError::ReadShortcuts)?;
    let shortcuts: Vec<ShortcutBinding> =
        serde_json::from_str(&contents).map_err(StorageError::ParseShortcuts)?;

    Ok(normalize_shortcuts(shortcuts))
}

pub fn save_shortcuts(shortcuts: &[ShortcutBinding]) -> Result<(), StorageError> {
    let directory = app_config_directory();
    fs::create_dir_all(&directory).map_err(StorageError::CreateConfigDirectory)?;
    let payload =
        serde_json::to_string_pretty(shortcuts).map_err(StorageError::SerializeShortcuts)?;
    fs::write(shortcuts_path(), payload).map_err(StorageError::WriteShortcuts)
}

fn normalize_shortcuts(shortcuts: Vec<ShortcutBinding>) -> Vec<ShortcutBinding> {
    let defaults = default_shortcuts();

    ShortcutAction::ALL
        .into_iter()
        .map(|action| {
            shortcuts
                .iter()
                .find(|binding| binding.action == action)
                .cloned()
                .or_else(|| {
                    defaults
                        .iter()
                        .find(|binding| binding.action == action)
                        .cloned()
                })
                .expect("default shortcut bindings are complete")
        })
        .collect()
}

pub fn next_recording_path(input: &str) -> Result<PathBuf, StorageError> {
    let directory = ensure_output_directory(input)?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    Ok(directory.join(format!("recording-{timestamp}.mp4")))
}

pub fn validate_recording_output_path(path: &Path) -> Result<(), StorageError> {
    let directory = path.parent().map(Path::to_path_buf).ok_or_else(|| {
        StorageError::ValidateOutputPath(std::io::Error::other("missing output directory"))
    })?;
    fs::create_dir_all(&directory).map_err(StorageError::CreateDirectory)?;

    let probe_path = directory.join(format!(
        ".record-screen-write-test-{}-{}.tmp",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    ));

    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe_path)
        .map_err(StorageError::ValidateOutputPath)?;
    drop(file);
    fs::remove_file(&probe_path).map_err(StorageError::ValidateOutputPath)?;
    Ok(())
}

fn merge_json(defaults: serde_json::Value, persisted: serde_json::Value) -> serde_json::Value {
    match (defaults, persisted) {
        (serde_json::Value::Object(mut defaults), serde_json::Value::Object(persisted)) => {
            for (key, value) in persisted {
                let merged = match defaults.remove(&key) {
                    Some(default_value) => merge_json(default_value, value),
                    None => value,
                };
                defaults.insert(key, merged);
            }
            serde_json::Value::Object(defaults)
        }
        (_, persisted) => persisted,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AppSettings, app_config_directory_with, app_settings_path, expand_home_path_with,
        shortcuts_path,
    };
    use std::{collections::HashMap, path::PathBuf};

    #[test]
    fn default_launch_on_login_is_disabled() {
        assert!(!AppSettings::default().launch_on_login);
    }

    #[test]
    fn settings_path_ends_with_settings_json() {
        assert!(app_settings_path().ends_with("settings.json"));
    }

    #[test]
    fn shortcuts_path_ends_with_shortcuts_json() {
        assert!(shortcuts_path().ends_with("shortcuts.json"));
    }

    #[test]
    fn expand_home_path_uses_userprofile_when_home_is_missing() {
        let env = HashMap::from([("USERPROFILE", r"C:\Users\Tester".to_string())]);
        let expanded =
            expand_home_path_with("~/Movies/Record Screen", &|key| env.get(key).cloned());

        assert_eq!(
            expanded,
            PathBuf::from(r"C:\Users\Tester").join("Movies/Record Screen")
        );
    }

    #[test]
    fn app_config_directory_uses_appdata_when_available() {
        let env = HashMap::from([("APPDATA", r"C:\Users\Tester\AppData\Roaming".to_string())]);
        let config_directory = app_config_directory_with(&|key| env.get(key).cloned());

        assert_eq!(
            config_directory,
            PathBuf::from(r"C:\Users\Tester\AppData\Roaming").join("record-screen")
        );
    }
}
