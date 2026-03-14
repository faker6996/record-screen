use app_core::RuntimeDiagnostics;

pub fn runtime_diagnostics() -> RuntimeDiagnostics {
    #[cfg(target_os = "linux")]
    {
        return linux_runtime_diagnostics();
    }

    #[cfg(target_os = "macos")]
    {
        let capabilities = crate::capture_capabilities::current_capture_capabilities();
        return RuntimeDiagnostics {
            summary: "macOS native capture path".to_string(),
            backend_path: "AVFoundation + ffmpeg".to_string(),
            readiness: "Screen recording and microphone permissions are checked separately in the launcher.".to_string(),
            supports_custom_region: capabilities.supports_custom_region,
            custom_region_note: capabilities.custom_region_note,
            supports_system_audio: capabilities.supports_system_audio,
            system_audio_note: capabilities.system_audio_note,
        };
    }

    #[cfg(target_os = "windows")]
    {
        let capabilities = crate::capture_capabilities::current_capture_capabilities();
        let audio_inputs = crate::audio_inputs::available_audio_inputs();
        let enumerated_microphones = audio_inputs
            .iter()
            .filter(|input| input.kind == capture::AudioInputKind::Microphone)
            .count();
        let fallback_default = audio_inputs.iter().find(|input| input.id == capture::DEFAULT_AUDIO_INPUT_ID);
        let audio_readiness = if enumerated_microphones > 0 {
            format!(
                "Microphone discovery is ready. Found {} direct input{}.",
                enumerated_microphones,
                if enumerated_microphones == 1 { "" } else { "s" }
            )
        } else if let Some(default_input) = fallback_default {
            if default_input
                .description
                .to_ascii_lowercase()
                .contains("default recording device")
            {
                format!(
                    "DirectShow microphone discovery is degraded. The app will fall back to the Windows default recording device when `Default input` is selected. {}",
                    default_input.description
                )
            } else {
                default_input.description.clone()
            }
        } else {
            "Windows microphone readiness could not be determined.".to_string()
        };
        return RuntimeDiagnostics {
            summary: "Windows desktop capture path".to_string(),
            backend_path: "gdigrab + dshow + ffmpeg".to_string(),
            readiness: format!(
                "Desktop capture depends on ffmpeg availability, PowerShell window discovery, and DirectShow microphone readiness. {audio_readiness}"
            ),
            supports_custom_region: capabilities.supports_custom_region,
            custom_region_note: capabilities.custom_region_note,
            supports_system_audio: capabilities.supports_system_audio,
            system_audio_note: capabilities.system_audio_note,
        };
    }

    #[allow(unreachable_code)]
    RuntimeDiagnostics {
        summary: "Unsupported platform".to_string(),
        backend_path: "No native backend".to_string(),
        readiness: "This target does not have a recording backend yet.".to_string(),
        supports_custom_region: false,
        custom_region_note: "This platform does not have a recording backend.".to_string(),
        supports_system_audio: false,
        system_audio_note: "This platform does not have a recording backend.".to_string(),
    }
}

#[cfg(target_os = "linux")]
fn linux_runtime_diagnostics() -> RuntimeDiagnostics {
    use capture_linux::wayland_portal::{
        PipeWireFfmpegSupport, PipeWireGstreamerSupport, ScreenCastPortalProbe,
        ffmpeg_pipewire_support, gstreamer_pipewire_support, probe_screen_cast_portal,
    };
    use std::env;

    let display = env::var("DISPLAY")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let wayland = env::var("WAYLAND_DISPLAY")
        .ok()
        .filter(|value| !value.trim().is_empty());

    let capabilities = crate::capture_capabilities::current_capture_capabilities();

    match (display, wayland) {
        (Some(display), Some(wayland)) => RuntimeDiagnostics {
            summary: format!("Linux session: Wayland + XWayland ({wayland}, {display})"),
            backend_path: "x11grab compatibility path".to_string(),
            readiness: "Recording can use the X11 compatibility path today. The pure Wayland ScreenCast portal lifecycle now exists in code, but PipeWire stream ingestion is still pending.".to_string(),
            supports_custom_region: capabilities.supports_custom_region,
            custom_region_note: capabilities.custom_region_note.clone(),
            supports_system_audio: capabilities.supports_system_audio,
            system_audio_note: capabilities.system_audio_note.clone(),
        },
        (Some(display), None) => RuntimeDiagnostics {
            summary: format!("Linux session: X11 ({display})"),
            backend_path: "x11grab native path".to_string(),
            readiness: "Recording can start directly through X11grab.".to_string(),
            supports_custom_region: capabilities.supports_custom_region,
            custom_region_note: capabilities.custom_region_note.clone(),
            supports_system_audio: capabilities.supports_system_audio,
            system_audio_note: capabilities.system_audio_note.clone(),
        },
        (None, Some(wayland)) => RuntimeDiagnostics {
            summary: format!("Linux session: Wayland only ({wayland})"),
            backend_path: "ScreenCast portal / PipeWire negotiation path".to_string(),
            readiness: match (
                probe_screen_cast_portal(),
                ffmpeg_pipewire_support(),
                gstreamer_pipewire_support(),
            ) {
                (
                    ScreenCastPortalProbe::Available(_),
                    PipeWireFfmpegSupport::Available,
                    _,
                ) => {
                    "ScreenCast portal is reachable and ffmpeg reports PipeWire support. The app can use the native portal lifecycle today, and ffmpeg remains a viable future capture path if the PipeWire device is wired in directly.".to_string()
                }
                (
                    ScreenCastPortalProbe::Available(_),
                    PipeWireFfmpegSupport::Missing,
                    PipeWireGstreamerSupport::Available,
                ) => {
                    "ScreenCast portal is reachable, ffmpeg does not expose a PipeWire input device, but the required GStreamer PipeWire plugins are installed. The Linux backend now attempts an experimental portal + PipeWire + GStreamer recording path in pure Wayland sessions.".to_string()
                }
                (
                    ScreenCastPortalProbe::Available(_),
                    PipeWireFfmpegSupport::Missing,
                    PipeWireGstreamerSupport::Missing | PipeWireGstreamerSupport::Unknown,
                ) => {
                    "ScreenCast portal is reachable, but this machine is missing both ffmpeg PipeWire support and the required GStreamer PipeWire plugins. Pure Wayland recording will not start until one of those runtime paths is installed.".to_string()
                }
                (
                    ScreenCastPortalProbe::Available(_),
                    PipeWireFfmpegSupport::Unknown,
                    PipeWireGstreamerSupport::Available,
                ) => {
                    "ScreenCast portal is reachable and the app can negotiate the full lifecycle. ffmpeg PipeWire support could not be determined, but the required GStreamer plugins are present, so the backend can attempt the experimental Wayland recording path.".to_string()
                }
                (
                    ScreenCastPortalProbe::Available(_),
                    PipeWireFfmpegSupport::Unknown,
                    PipeWireGstreamerSupport::Missing | PipeWireGstreamerSupport::Unknown,
                ) => {
                    "ScreenCast portal is reachable and the app can negotiate the full lifecycle, but a usable PipeWire capture runtime was not detected. Install the GStreamer PipeWire plugins or a PipeWire-enabled ffmpeg build for pure Wayland recording.".to_string()
                }
                (ScreenCastPortalProbe::MissingPortal, _, _) => {
                    "Wayland is active, but no ScreenCast portal could be reached. Install xdg-desktop-portal or switch to an X11/XWayland session.".to_string()
                }
                (ScreenCastPortalProbe::MissingDbusTools, _, _) => {
                    "Wayland is active, but the app could not inspect ScreenCast portal readiness because neither gdbus nor busctl is available.".to_string()
                }
                (ScreenCastPortalProbe::Unreachable, _, _) => {
                    "Wayland is active and a portal may be installed, but the ScreenCast portal could not be reached on the session bus.".to_string()
                }
            },
            supports_custom_region: capabilities.supports_custom_region,
            custom_region_note: capabilities.custom_region_note.clone(),
            supports_system_audio: capabilities.supports_system_audio,
            system_audio_note: capabilities.system_audio_note.clone(),
        },
        (None, None) => RuntimeDiagnostics {
            summary: "Linux session: no desktop display detected".to_string(),
            backend_path: "No active X11 or Wayland session".to_string(),
            readiness: "Start the app from an active desktop session to record the screen.".to_string(),
            supports_custom_region: capabilities.supports_custom_region,
            custom_region_note: capabilities.custom_region_note,
            supports_system_audio: capabilities.supports_system_audio,
            system_audio_note: capabilities.system_audio_note,
        },
    }
}
