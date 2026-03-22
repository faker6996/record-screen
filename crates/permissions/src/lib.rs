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
        "linux" => linux::probe_permissions(),
        "windows" => windows::probe_permissions(),
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
        "windows" => {
            windows::request_permission(permission_name)?;
            Ok(windows::probe_permissions())
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
        "windows" => windows::open_permission_settings(permission_name),
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

pub fn microphone_permission_blocked(platform: &str) -> bool {
    match platform {
        #[cfg(target_os = "macos")]
        "macos" => macos::microphone_permission_blocked(),
        _ => false,
    }
}

pub fn microphone_permission_guidance(platform: &str) -> Option<String> {
    match platform {
        #[cfg(target_os = "macos")]
        "macos" => Some(macos::microphone_guidance()),
        _ => None,
    }
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

    pub(crate) fn microphone_permission_blocked() -> bool {
        matches!(
            microphone_authorization_status(),
            AVAuthorizationStatus::Denied | AVAuthorizationStatus::Restricted
        )
    }

    pub(crate) fn microphone_guidance() -> String {
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

#[cfg(not(target_os = "macos"))]
mod macos {
    use crate::{PermissionCheck, PermissionError, default_permissions};

    pub fn probe_permissions() -> Vec<PermissionCheck> {
        default_permissions("macos")
    }

    pub fn request_permission(_permission_name: &str) -> Result<(), PermissionError> {
        Err(PermissionError::UnsupportedPlatform)
    }

    pub fn open_permission_settings(_permission_name: &str) -> Result<(), PermissionError> {
        Err(PermissionError::UnsupportedPlatform)
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use std::{env, process::Command};

    use crate::{PermissionCheck, PermissionStatus};

    pub fn probe_permissions() -> Vec<PermissionCheck> {
        vec![
            PermissionCheck {
                name: "Launcher readiness".to_string(),
                status: PermissionStatus::Granted,
                guidance: "The app shell is ready to react to shortcuts and UI commands."
                    .to_string(),
            },
            x11_display_check(),
            wayland_portal_check(),
            microphone_check(),
        ]
    }

    fn x11_display_check() -> PermissionCheck {
        let display = env::var("DISPLAY")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let wayland = env::var("WAYLAND_DISPLAY")
            .ok()
            .filter(|value| !value.trim().is_empty());

        match (display, wayland) {
            (Some(display), Some(wayland)) => PermissionCheck {
                name: "X11 display access".to_string(),
                status: PermissionStatus::Granted,
                guidance: format!(
                    "Wayland session {wayland} is running with XWayland display {display}. The app can use the native X11 GStreamer lane while the full Wayland portal path is also available."
                ),
            },
            (Some(display), None) => PermissionCheck {
                name: "X11 display access".to_string(),
                status: PermissionStatus::Granted,
                guidance: format!(
                    "X11 session detected on {display}. Screen recording can start immediately through the native X11 GStreamer lane."
                ),
            },
            (None, Some(wayland)) => PermissionCheck {
                name: "X11 display access".to_string(),
                status: PermissionStatus::Unsupported,
                guidance: format!(
                    "Wayland session {wayland} was detected without XWayland DISPLAY. The app must rely on the native ScreenCast portal / PipeWire path instead of X11."
                ),
            },
            (None, None) => PermissionCheck {
                name: "X11 display access".to_string(),
                status: PermissionStatus::Unsupported,
                guidance:
                    "No DISPLAY environment variable was found. Start the app from the active desktop session to allow Linux screen capture."
                        .to_string(),
            },
        }
    }

    fn wayland_portal_check() -> PermissionCheck {
        let wayland = env::var("WAYLAND_DISPLAY")
            .ok()
            .filter(|value| !value.trim().is_empty());

        if wayland.is_none() {
            return PermissionCheck {
                name: "Wayland portal".to_string(),
                status: PermissionStatus::Unsupported,
                guidance:
                    "No Wayland session is active, so the ScreenCast portal path is not needed."
                        .to_string(),
            };
        }

        match screen_cast_portal_capabilities() {
            Some(capabilities) => PermissionCheck {
                name: "Wayland portal".to_string(),
                status: PermissionStatus::Granted,
                guidance: format!(
                    "ScreenCast portal detected. It reports {} and {}. Pure Wayland recording can use the native ScreenCast / PipeWire / GStreamer lane after the desktop grants screen sharing access.",
                    capabilities.source_summary,
                    capabilities.cursor_summary
                ),
            },
            None if command_succeeds("xdg-desktop-portal", &["--version"]) => PermissionCheck {
                name: "Wayland portal".to_string(),
                status: PermissionStatus::Pending,
                guidance:
                    "XDG Desktop Portal is installed, but the app could not inspect ScreenCast capabilities from the session bus. Pure Wayland capture may still work after the desktop grants screen sharing access."
                        .to_string(),
            },
            None => PermissionCheck {
                name: "Wayland portal".to_string(),
                status: PermissionStatus::Unsupported,
                guidance:
                    "Wayland is active but xdg-desktop-portal was not detected. Install the portal stack or use an X11/XWayland session."
                        .to_string(),
            },
        }
    }

    fn microphone_check() -> PermissionCheck {
        let output = Command::new("gst-launch-1.0")
            .args([
                "-q",
                "pulsesrc",
                "device=default",
                "num-buffers=8",
                "!",
                "fakesink",
            ])
            .output();

        match output {
            Ok(result) if result.status.success() => PermissionCheck {
                name: "Microphone source".to_string(),
                status: PermissionStatus::Granted,
                guidance:
                    "The default PulseAudio/PipeWire microphone source is available for narration."
                        .to_string(),
            },
            Ok(_) => PermissionCheck {
                name: "Microphone source".to_string(),
                status: PermissionStatus::Pending,
                guidance:
                    "The default microphone source is not ready. You can still record the screen with microphone disabled.".to_string(),
            },
            Err(_) => PermissionCheck {
                name: "Microphone source".to_string(),
                status: PermissionStatus::Pending,
                guidance:
                    "Could not probe the default microphone source. If narration fails, disable the mic toggle before starting a recording.".to_string(),
            },
        }
    }

    fn command_succeeds(program: &str, args: &[&str]) -> bool {
        Command::new(program)
            .args(args)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    struct ScreenCastPortalCapabilities {
        source_summary: String,
        cursor_summary: String,
    }

    fn screen_cast_portal_capabilities() -> Option<ScreenCastPortalCapabilities> {
        let source_types = query_portal_u32_property("AvailableSourceTypes")?;
        let cursor_modes = query_portal_u32_property("AvailableCursorModes")?;

        Some(ScreenCastPortalCapabilities {
            source_summary: describe_source_types(source_types),
            cursor_summary: describe_cursor_modes(cursor_modes),
        })
    }

    fn query_portal_u32_property(property: &str) -> Option<u32> {
        query_portal_property_with_gdbus(property)
            .or_else(|| query_portal_property_with_busctl(property))
    }

    fn query_portal_property_with_gdbus(property: &str) -> Option<u32> {
        let output = Command::new("gdbus")
            .args([
                "call",
                "--session",
                "--dest",
                "org.freedesktop.portal.Desktop",
                "--object-path",
                "/org/freedesktop/portal/desktop",
                "--method",
                "org.freedesktop.DBus.Properties.Get",
                "org.freedesktop.portal.ScreenCast",
                property,
            ])
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        parse_first_u32(&String::from_utf8_lossy(&output.stdout))
    }

    fn query_portal_property_with_busctl(property: &str) -> Option<u32> {
        let output = Command::new("busctl")
            .args([
                "--user",
                "get-property",
                "org.freedesktop.portal.Desktop",
                "/org/freedesktop/portal/desktop",
                "org.freedesktop.portal.ScreenCast",
                property,
            ])
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        parse_first_u32(&String::from_utf8_lossy(&output.stdout))
    }

    fn parse_first_u32(output: &str) -> Option<u32> {
        output
            .split(|character: char| !character.is_ascii_digit())
            .find(|token| !token.is_empty())
            .and_then(|token| token.parse::<u32>().ok())
    }

    fn describe_source_types(mask: u32) -> String {
        let mut items = Vec::new();

        if mask & 1 != 0 {
            items.push("monitor sharing");
        }
        if mask & 2 != 0 {
            items.push("window sharing");
        }
        if mask & 4 != 0 {
            items.push("virtual displays");
        }

        if items.is_empty() {
            "no source types".to_string()
        } else {
            join_items(&items)
        }
    }

    fn describe_cursor_modes(mask: u32) -> String {
        let mut items = Vec::new();

        if mask & 1 != 0 {
            items.push("hidden cursor");
        }
        if mask & 2 != 0 {
            items.push("embedded cursor");
        }
        if mask & 4 != 0 {
            items.push("cursor metadata");
        }

        if items.is_empty() {
            "no cursor modes".to_string()
        } else {
            join_items(&items)
        }
    }

    fn join_items(items: &[&str]) -> String {
        match items {
            [] => String::new(),
            [single] => (*single).to_string(),
            [head @ .., tail] => format!("{} and {}", head.join(", "), tail),
        }
    }
}

#[cfg(not(target_os = "linux"))]
mod linux {
    use crate::PermissionCheck;
    use crate::default_permissions;

    pub fn probe_permissions() -> Vec<PermissionCheck> {
        default_permissions("linux")
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use std::process::Command;

    use capture::{AudioBackendAvailability, AudioInputKind, CaptureBackendAvailability};
    use capture_windows::{list_audio_inputs, selected_audio_backend, selected_backend};

    use crate::{PermissionCheck, PermissionError, PermissionStatus};

    const DESKTOP_CAPTURE: &str = "Desktop capture";
    const MICROPHONE: &str = "Microphone";

    pub fn probe_permissions() -> Vec<PermissionCheck> {
        vec![
            PermissionCheck {
                name: "Launcher readiness".to_string(),
                status: PermissionStatus::Granted,
                guidance: "The app shell is ready to react to shortcuts and UI commands."
                    .to_string(),
            },
            desktop_capture_check(),
            microphone_check(),
        ]
    }

    pub fn request_permission(permission_name: &str) -> Result<(), PermissionError> {
        match permission_name {
            MICROPHONE => open_uri("ms-settings:privacy-microphone"),
            DESKTOP_CAPTURE | "Launcher readiness" => Ok(()),
            other => Err(PermissionError::UnknownTarget(other.to_string())),
        }
    }

    pub fn open_permission_settings(permission_name: &str) -> Result<(), PermissionError> {
        match permission_name {
            MICROPHONE => open_uri("ms-settings:privacy-microphone"),
            DESKTOP_CAPTURE => open_uri("ms-settings:privacy"),
            other => Err(PermissionError::UnknownTarget(other.to_string())),
        }
    }

    fn desktop_capture_check() -> PermissionCheck {
        match selected_backend().availability() {
            CaptureBackendAvailability::Available => PermissionCheck {
                name: DESKTOP_CAPTURE.to_string(),
                status: PermissionStatus::Granted,
                guidance:
                    "Windows native capture is available through Windows.Graphics.Capture and Media Foundation."
                        .to_string(),
            },
            CaptureBackendAvailability::Unavailable { reason } => PermissionCheck {
                name: DESKTOP_CAPTURE.to_string(),
                status: PermissionStatus::Pending,
                guidance: format!(
                    "Windows native desktop capture is not ready in this session. {reason}"
                ),
            },
        }
    }

    fn microphone_check() -> PermissionCheck {
        let has_microphone = list_audio_inputs()
            .into_iter()
            .any(|input| matches!(input.kind, AudioInputKind::Microphone));
        let audio_backend_available = matches!(
            selected_audio_backend().availability(),
            AudioBackendAvailability::Available
        );

        if has_microphone && audio_backend_available {
            PermissionCheck {
                name: MICROPHONE.to_string(),
                status: PermissionStatus::Granted,
                guidance:
                    "A native Windows microphone route is available for narration on this machine."
                        .to_string(),
            }
        } else if !has_microphone {
            PermissionCheck {
                name: MICROPHONE.to_string(),
                status: PermissionStatus::Pending,
                guidance:
                    "No Windows microphone input was detected. You can still record the screen with the mic toggle disabled."
                        .to_string(),
            }
        } else {
            PermissionCheck {
                name: MICROPHONE.to_string(),
                status: PermissionStatus::Pending,
                guidance:
                    "Windows audio routing is not ready in this session. Open Windows microphone privacy settings if narration does not work."
                        .to_string(),
            }
        }
    }

    fn open_uri(uri: &str) -> Result<(), PermissionError> {
        let status = Command::new("cmd")
            .args(["/C", "start", "", uri])
            .status()
            .map_err(|error| PermissionError::OpenSettingsFailed(error.to_string()))?;

        if status.success() {
            Ok(())
        } else {
            Err(PermissionError::OpenSettingsFailed(format!(
                "start exited with status {status}"
            )))
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod windows {
    use crate::{PermissionCheck, PermissionError, default_permissions};

    pub fn probe_permissions() -> Vec<PermissionCheck> {
        default_permissions("windows")
    }

    pub fn request_permission(_permission_name: &str) -> Result<(), PermissionError> {
        Err(PermissionError::UnsupportedPlatform)
    }

    pub fn open_permission_settings(_permission_name: &str) -> Result<(), PermissionError> {
        Err(PermissionError::UnsupportedPlatform)
    }
}
