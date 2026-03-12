use serde::{Deserialize, Serialize};

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
