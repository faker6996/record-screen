use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum ShortcutAction {
    ToggleRecording,
    PauseRecording,
    OpenLauncher,
    ToggleMicrophone,
}

impl ShortcutAction {
    pub const ALL: [Self; 4] = [
        Self::ToggleRecording,
        Self::PauseRecording,
        Self::OpenLauncher,
        Self::ToggleMicrophone,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::ToggleRecording => "Start / stop recording",
            Self::PauseRecording => "Pause / resume recording",
            Self::OpenLauncher => "Open launcher",
            Self::ToggleMicrophone => "Mute / unmute microphone",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::ToggleRecording => "Instantly begin or finalize the current recording session.",
            Self::PauseRecording => "Freeze capture without losing the current session.",
            Self::OpenLauncher => "Bring the command launcher back into focus from anywhere.",
            Self::ToggleMicrophone => "Flip the microphone state while keeping the session alive.",
        }
    }

    pub fn default_accelerator(self) -> &'static str {
        match self {
            Self::ToggleRecording => "CmdOrCtrl+Shift+R",
            Self::PauseRecording => "CmdOrCtrl+Shift+P",
            Self::OpenLauncher => "CmdOrCtrl+Shift+L",
            Self::ToggleMicrophone => "CmdOrCtrl+Shift+M",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutBinding {
    pub action: ShortcutAction,
    pub label: String,
    pub accelerator: String,
    pub enabled: bool,
    pub description: String,
}

pub fn default_shortcuts() -> Vec<ShortcutBinding> {
    ShortcutAction::ALL
        .into_iter()
        .map(|action| ShortcutBinding {
            action,
            label: action.label().to_string(),
            accelerator: action.default_accelerator().to_string(),
            enabled: true,
            description: action.description().to_string(),
        })
        .collect()
}
