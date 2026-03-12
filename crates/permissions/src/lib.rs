use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionStatus {
    Granted,
    Pending,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionCheck {
    pub name: String,
    pub status: PermissionStatus,
    pub guidance: String,
}

pub fn default_permissions(platform: &str) -> Vec<PermissionCheck> {
    let mut items = vec![PermissionCheck {
        name: "Launcher readiness".to_string(),
        status: PermissionStatus::Granted,
        guidance: "The app shell is ready to react to shortcuts and UI commands.".to_string(),
    }];

    match platform {
        "macos" => {
            items.push(PermissionCheck {
                name: "Screen recording".to_string(),
                status: PermissionStatus::Pending,
                guidance:
                    "Request Screen Recording access in System Settings before the first capture."
                        .to_string(),
            });
            items.push(PermissionCheck {
                name: "Microphone".to_string(),
                status: PermissionStatus::Pending,
                guidance: "Ask for microphone access when the user enables narration.".to_string(),
            });
        }
        "windows" => {
            items.push(PermissionCheck {
                name: "Graphics capture".to_string(),
                status: PermissionStatus::Pending,
                guidance:
                    "Validate Windows Graphics Capture and microphone device access on first run."
                        .to_string(),
            });
            items.push(PermissionCheck {
                name: "Microphone".to_string(),
                status: PermissionStatus::Pending,
                guidance: "Prompt for microphone permission when audio recording is enabled."
                    .to_string(),
            });
        }
        "linux" => {
            items.push(PermissionCheck {
                name: "ScreenCast portal".to_string(),
                status: PermissionStatus::Pending,
                guidance:
                    "Use the XDG ScreenCast portal for Wayland-friendly screen sharing access."
                        .to_string(),
            });
            items.push(PermissionCheck {
                name: "Microphone".to_string(),
                status: PermissionStatus::Pending,
                guidance: "Validate PipeWire and portal access across target desktop environments."
                    .to_string(),
            });
        }
        _ => items.push(PermissionCheck {
            name: "Capture support".to_string(),
            status: PermissionStatus::Unsupported,
            guidance:
                "Platform-specific permission probing has not been wired for this target yet."
                    .to_string(),
        }),
    }

    items
}
