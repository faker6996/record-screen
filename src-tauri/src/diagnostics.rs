use app_core::RuntimeDiagnostics;

pub fn runtime_diagnostics() -> RuntimeDiagnostics {
    #[cfg(target_os = "linux")]
    {
        return linux_runtime_diagnostics();
    }

    #[cfg(target_os = "macos")]
    {
        return RuntimeDiagnostics {
            summary: "macOS native capture path".to_string(),
            backend_path: "AVFoundation + ffmpeg".to_string(),
            readiness: "Screen recording and microphone permissions are checked separately in the launcher.".to_string(),
        };
    }

    #[cfg(target_os = "windows")]
    {
        return RuntimeDiagnostics {
            summary: "Windows desktop capture path".to_string(),
            backend_path: "gdigrab + dshow + ffmpeg".to_string(),
            readiness: "Desktop capture depends on ffmpeg availability, PowerShell window discovery, and DirectShow microphone readiness.".to_string(),
        };
    }

    #[allow(unreachable_code)]
    RuntimeDiagnostics {
        summary: "Unsupported platform".to_string(),
        backend_path: "No native backend".to_string(),
        readiness: "This target does not have a recording backend yet.".to_string(),
    }
}

#[cfg(target_os = "linux")]
fn linux_runtime_diagnostics() -> RuntimeDiagnostics {
    use capture_linux::wayland_portal::{
        PipeWireFfmpegSupport, ScreenCastPortalProbe, ffmpeg_pipewire_support,
        probe_screen_cast_portal,
    };
    use std::env;

    let display = env::var("DISPLAY")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let wayland = env::var("WAYLAND_DISPLAY")
        .ok()
        .filter(|value| !value.trim().is_empty());

    match (display, wayland) {
        (Some(display), Some(wayland)) => RuntimeDiagnostics {
            summary: format!("Linux session: Wayland + XWayland ({wayland}, {display})"),
            backend_path: "x11grab compatibility path".to_string(),
            readiness: "Recording can use the X11 compatibility path today. The pure Wayland ScreenCast portal lifecycle now exists in code, but PipeWire stream ingestion is still pending.".to_string(),
        },
        (Some(display), None) => RuntimeDiagnostics {
            summary: format!("Linux session: X11 ({display})"),
            backend_path: "x11grab native path".to_string(),
            readiness: "Recording can start directly through X11grab.".to_string(),
        },
        (None, Some(wayland)) => RuntimeDiagnostics {
            summary: format!("Linux session: Wayland only ({wayland})"),
            backend_path: "ScreenCast portal / PipeWire negotiation path".to_string(),
            readiness: match (probe_screen_cast_portal(), ffmpeg_pipewire_support()) {
                (ScreenCastPortalProbe::Available(_), PipeWireFfmpegSupport::Available) => {
                    "ScreenCast portal is reachable and ffmpeg reports PipeWire support. The codebase now has a native DBus lifecycle path for CreateSession, SelectSources, Start, and OpenPipeWireRemote. The remaining gap is ingesting the returned PipeWire remote fd into the recorder.".to_string()
                }
                (ScreenCastPortalProbe::Available(_), PipeWireFfmpegSupport::Missing) => {
                    "ScreenCast portal is reachable and the app can negotiate the portal lifecycle, but ffmpeg does not report PipeWire device support. Pure Wayland recording still needs either a PipeWire-enabled ffmpeg build or a native PipeWire client path for the returned remote fd.".to_string()
                }
                (ScreenCastPortalProbe::Available(_), PipeWireFfmpegSupport::Unknown) => {
                    "ScreenCast portal is reachable and the code can negotiate the portal lifecycle, but PipeWire capture support in ffmpeg could not be determined. Pure Wayland recording still depends on wiring the returned remote fd into a live capture path.".to_string()
                }
                (ScreenCastPortalProbe::MissingPortal, _) => {
                    "Wayland is active, but no ScreenCast portal could be reached. Install xdg-desktop-portal or switch to an X11/XWayland session.".to_string()
                }
                (ScreenCastPortalProbe::MissingDbusTools, _) => {
                    "Wayland is active, but the app could not inspect ScreenCast portal readiness because neither gdbus nor busctl is available.".to_string()
                }
                (ScreenCastPortalProbe::Unreachable, _) => {
                    "Wayland is active and a portal may be installed, but the ScreenCast portal could not be reached on the session bus.".to_string()
                }
            },
        },
        (None, None) => RuntimeDiagnostics {
            summary: "Linux session: no desktop display detected".to_string(),
            backend_path: "No active X11 or Wayland session".to_string(),
            readiness: "Start the app from an active desktop session to record the screen.".to_string(),
        },
    }
}
