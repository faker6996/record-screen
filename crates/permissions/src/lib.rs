use serde::{Deserialize, Serialize};
use thiserror::Error;

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

#[derive(Debug, Error)]
pub enum PermissionError {
    #[error("permission flow is not implemented for this platform yet")]
    UnsupportedPlatform,
    #[error("unknown permission target: {0}")]
    UnknownTarget(String),
    #[error("failed to request permission: {0}")]
    RequestFailed(String),
    #[error("failed to open system settings: {0}")]
    OpenSettingsFailed(String),
}

pub fn probe_permissions(platform: &str) -> Vec<PermissionCheck> {
    match platform {
        "macos" => macos::probe_permissions(),
        _ => default_permissions(platform),
    }
}

pub fn request_permission(
    platform: &str,
    permission_name: &str,
) -> Result<Vec<PermissionCheck>, PermissionError> {
    match platform {
        "macos" => {
            macos::request_permission(permission_name)?;
            Ok(macos::probe_permissions())
        }
        _ => Err(PermissionError::UnsupportedPlatform),
    }
}

pub fn open_permission_settings(
    platform: &str,
    permission_name: &str,
) -> Result<(), PermissionError> {
    match platform {
        "macos" => macos::open_permission_settings(permission_name),
        _ => Err(PermissionError::UnsupportedPlatform),
    }
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

#[cfg(target_os = "macos")]
mod macos {
    use std::{process::Command, sync::mpsc, time::Duration};

    use block2::RcBlock;
    use core_graphics::access::ScreenCaptureAccess;
    use objc2::runtime::Bool;
    use objc2_av_foundation::{AVAuthorizationStatus, AVCaptureDevice, AVMediaTypeAudio};

    use crate::{PermissionCheck, PermissionError, PermissionStatus};

    const SCREEN_RECORDING: &str = "Screen recording";
    const MICROPHONE: &str = "Microphone";

    pub fn probe_permissions() -> Vec<PermissionCheck> {
        vec![
            PermissionCheck {
                name: "Launcher readiness".to_string(),
                status: PermissionStatus::Granted,
                guidance: "The app shell is ready to react to shortcuts and UI commands."
                    .to_string(),
            },
            PermissionCheck {
                name: SCREEN_RECORDING.to_string(),
                status: screen_recording_status(),
                guidance: screen_recording_guidance(),
            },
            PermissionCheck {
                name: MICROPHONE.to_string(),
                status: microphone_permission_status(),
                guidance: microphone_guidance(),
            },
        ]
    }

    pub fn request_permission(permission_name: &str) -> Result<(), PermissionError> {
        match permission_name {
            SCREEN_RECORDING => {
                let _ = ScreenCaptureAccess.request();
                Ok(())
            }
            MICROPHONE => request_microphone_permission(),
            other => Err(PermissionError::UnknownTarget(other.to_string())),
        }
    }

    pub fn open_permission_settings(permission_name: &str) -> Result<(), PermissionError> {
        let url = match permission_name {
            SCREEN_RECORDING => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture"
            }
            MICROPHONE => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone"
            }
            other => return Err(PermissionError::UnknownTarget(other.to_string())),
        };

        let status = Command::new("open")
            .arg(url)
            .status()
            .map_err(|error| PermissionError::OpenSettingsFailed(error.to_string()))?;

        if status.success() {
            Ok(())
        } else {
            Err(PermissionError::OpenSettingsFailed(format!(
                "open exited with status {status}"
            )))
        }
    }

    fn screen_recording_status() -> PermissionStatus {
        if ScreenCaptureAccess.preflight() {
            PermissionStatus::Granted
        } else {
            PermissionStatus::Pending
        }
    }

    fn screen_recording_guidance() -> String {
        if ScreenCaptureAccess.preflight() {
            "macOS reports that screen capture access is already granted.".to_string()
        } else {
            "Click Request access to trigger the macOS prompt. If you already denied it, use Open settings and enable Screen Recording for this app or Terminal.".to_string()
        }
    }

    fn microphone_permission_status() -> PermissionStatus {
        match microphone_authorization_status() {
            AVAuthorizationStatus::Authorized => PermissionStatus::Granted,
            AVAuthorizationStatus::Denied | AVAuthorizationStatus::Restricted => {
                PermissionStatus::Pending
            }
            _ => PermissionStatus::Pending,
        }
    }

    fn microphone_guidance() -> String {
        match microphone_authorization_status() {
            AVAuthorizationStatus::Authorized => {
                "macOS reports that microphone access is already granted.".to_string()
            }
            AVAuthorizationStatus::NotDetermined => {
                "Click Request access to show the microphone permission prompt the first time."
                    .to_string()
            }
            AVAuthorizationStatus::Denied | AVAuthorizationStatus::Restricted => {
                "Microphone access is blocked. Open settings and allow microphone access before recording narration.".to_string()
            }
            _ => "Microphone permission state is still unresolved.".to_string(),
        }
    }

    fn microphone_authorization_status() -> AVAuthorizationStatus {
        let media_type = unsafe { AVMediaTypeAudio }
            .unwrap_or_else(|| panic!("AVMediaTypeAudio is unavailable on macOS"));

        unsafe { AVCaptureDevice::authorizationStatusForMediaType(media_type) }
    }

    fn request_microphone_permission() -> Result<(), PermissionError> {
        if matches!(
            microphone_authorization_status(),
            AVAuthorizationStatus::Authorized
                | AVAuthorizationStatus::Denied
                | AVAuthorizationStatus::Restricted
        ) {
            return Ok(());
        }

        let media_type = unsafe { AVMediaTypeAudio }.ok_or_else(|| {
            PermissionError::RequestFailed("AVMediaTypeAudio missing".to_string())
        })?;
        let (sender, receiver) = mpsc::channel();
        let completion = RcBlock::new(move |granted: Bool| {
            let _ = sender.send(granted.as_bool());
        });

        unsafe {
            AVCaptureDevice::requestAccessForMediaType_completionHandler(media_type, &completion);
        }

        receiver
            .recv_timeout(Duration::from_secs(120))
            .map(|_| ())
            .map_err(|error| PermissionError::RequestFailed(error.to_string()))
    }
}
