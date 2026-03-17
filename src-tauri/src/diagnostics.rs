use std::{
    sync::atomic::{AtomicBool, Ordering},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use app_core::RuntimeDiagnostics;
use capture::{
    AudioBackendRuntimeSnapshot, CaptureBackendRuntimeSnapshot, EncoderBackendRuntimeSnapshot,
};

const DIAGNOSTICS_TTL: Duration = Duration::from_secs(20);

#[derive(Clone)]
struct CachedRuntimeDiagnostics {
    diagnostics: RuntimeDiagnostics,
    refreshed_at: Instant,
}

fn cache() -> &'static Mutex<Option<CachedRuntimeDiagnostics>> {
    static CACHE: OnceLock<Mutex<Option<CachedRuntimeDiagnostics>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

fn refresh_in_flight() -> &'static AtomicBool {
    static REFRESH_IN_FLIGHT: OnceLock<AtomicBool> = OnceLock::new();
    REFRESH_IN_FLIGHT.get_or_init(|| AtomicBool::new(false))
}

pub fn initial_runtime_diagnostics() -> RuntimeDiagnostics {
    {
        let cache = cache()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(entry) = cache.as_ref() {
            return entry.diagnostics.clone();
        }
    }

    schedule_background_refresh();
    fallback_runtime_diagnostics()
}

pub fn refreshed_runtime_diagnostics() -> RuntimeDiagnostics {
    load_runtime_diagnostics(true)
}

fn load_runtime_diagnostics(force_refresh: bool) -> RuntimeDiagnostics {
    let mut cache = cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    if !force_refresh {
        if let Some(entry) = cache.as_ref() {
            if entry.refreshed_at.elapsed() < DIAGNOSTICS_TTL {
                return entry.diagnostics.clone();
            }
        }
    }

    let diagnostics = runtime_diagnostics_now();
    *cache = Some(CachedRuntimeDiagnostics {
        diagnostics: diagnostics.clone(),
        refreshed_at: Instant::now(),
    });
    diagnostics
}

fn schedule_background_refresh() {
    if refresh_in_flight()
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }

    std::thread::spawn(|| {
        let diagnostics = runtime_diagnostics_now();
        let mut cache = cache()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *cache = Some(CachedRuntimeDiagnostics {
            diagnostics,
            refreshed_at: Instant::now(),
        });
        refresh_in_flight().store(false, Ordering::Release);
    });
}

fn fallback_runtime_diagnostics() -> RuntimeDiagnostics {
    #[cfg(target_os = "macos")]
    {
        let capabilities = crate::capture_capabilities::current_capture_capabilities();
        return RuntimeDiagnostics {
            summary: "macOS diagnostics are loading in the background.".to_string(),
            backend_path: "macOS capture backend (warming up)".to_string(),
            audio_backend_path: "macOS audio backend (warming up)".to_string(),
            encoder_backend_path: "macOS encoder backend (warming up)".to_string(),
            readiness: "The launcher is using a lightweight diagnostics snapshot while native capture probing finishes.".to_string(),
            capture_selection_note: "Capture diagnostics are still warming up.".to_string(),
            audio_selection_note: "Audio diagnostics are still warming up.".to_string(),
            encoder_selection_note: "Encoder diagnostics are still warming up.".to_string(),
            preferred_audio_input_id: None,
            preferred_audio_input_label: None,
            preferred_system_audio_id: None,
            preferred_system_audio_label: None,
            preferred_encoder_label: None,
            supports_custom_region: capabilities.supports_custom_region,
            custom_region_note: capabilities.custom_region_note,
            supports_system_audio: capabilities.supports_system_audio,
            system_audio_note: capabilities.system_audio_note,
        };
    }

    #[cfg(target_os = "windows")]
    {
        let capabilities = crate::capture_capabilities::current_capture_capabilities();
        return RuntimeDiagnostics {
            summary: "Windows diagnostics are loading in the background.".to_string(),
            backend_path: "Windows capture backend (warming up)".to_string(),
            audio_backend_path: "Windows audio backend (warming up)".to_string(),
            encoder_backend_path: "Windows encoder backend (warming up)".to_string(),
            readiness: "The launcher is using a lightweight diagnostics snapshot while runtime probing finishes.".to_string(),
            capture_selection_note: "Capture diagnostics are still warming up.".to_string(),
            audio_selection_note: "Audio diagnostics are still warming up.".to_string(),
            encoder_selection_note: "Encoder diagnostics are still warming up.".to_string(),
            preferred_audio_input_id: None,
            preferred_audio_input_label: None,
            preferred_system_audio_id: None,
            preferred_system_audio_label: None,
            preferred_encoder_label: None,
            supports_custom_region: capabilities.supports_custom_region,
            custom_region_note: capabilities.custom_region_note,
            supports_system_audio: capabilities.supports_system_audio,
            system_audio_note: capabilities.system_audio_note,
        };
    }

    #[cfg(target_os = "linux")]
    {
        let capabilities = crate::capture_capabilities::current_capture_capabilities();
        return RuntimeDiagnostics {
            summary: "Linux diagnostics are loading in the background.".to_string(),
            backend_path: "Linux capture backend (warming up)".to_string(),
            audio_backend_path: "Linux audio backend (warming up)".to_string(),
            encoder_backend_path: "Linux encoder backend (warming up)".to_string(),
            readiness: "The launcher is using a lightweight diagnostics snapshot while runtime probing finishes.".to_string(),
            capture_selection_note: "Capture diagnostics are still warming up.".to_string(),
            audio_selection_note: "Audio diagnostics are still warming up.".to_string(),
            encoder_selection_note: "Encoder diagnostics are still warming up.".to_string(),
            preferred_audio_input_id: None,
            preferred_audio_input_label: None,
            preferred_system_audio_id: None,
            preferred_system_audio_label: None,
            preferred_encoder_label: None,
            supports_custom_region: capabilities.supports_custom_region,
            custom_region_note: capabilities.custom_region_note,
            supports_system_audio: capabilities.supports_system_audio,
            system_audio_note: capabilities.system_audio_note,
        };
    }

    #[allow(unreachable_code)]
    RuntimeDiagnostics {
        summary: "Diagnostics are loading.".to_string(),
        backend_path: "Unknown capture backend".to_string(),
        audio_backend_path: "Unknown audio backend".to_string(),
        encoder_backend_path: "Unknown encoder backend".to_string(),
        readiness: "This target does not have a recording backend yet.".to_string(),
        capture_selection_note: "No capture backend was selected.".to_string(),
        audio_selection_note: "No audio backend was selected.".to_string(),
        encoder_selection_note: "No encoder backend was selected.".to_string(),
        preferred_audio_input_id: None,
        preferred_audio_input_label: None,
        preferred_system_audio_id: None,
        preferred_system_audio_label: None,
        preferred_encoder_label: None,
        supports_custom_region: false,
        custom_region_note: "This platform does not have a recording backend.".to_string(),
        supports_system_audio: false,
        system_audio_note: "This platform does not have a recording backend.".to_string(),
    }
}

fn runtime_diagnostics_now() -> RuntimeDiagnostics {
    #[cfg(target_os = "linux")]
    {
        return linux_runtime_diagnostics();
    }

    #[cfg(target_os = "macos")]
    {
        let capabilities = crate::capture_capabilities::current_capture_capabilities();
        let capture_runtime = capture_macos::capture_runtime_snapshot();
        let audio_runtime = capture_macos::audio_runtime_snapshot();
        let encoder_runtime = capture_macos::encoder_runtime_snapshot();
        return RuntimeDiagnostics {
            summary: capture_runtime
                .summary
                .clone()
                .unwrap_or_else(|| "macOS native capture path".to_string()),
            backend_path: capture_runtime.path.clone(),
            audio_backend_path: audio_runtime.path.clone(),
            encoder_backend_path: encoder_runtime.path.clone(),
            readiness: build_readiness(
                "Screen recording and microphone permissions are checked separately in the launcher.",
                &capture_runtime,
                Some(capture_macos::audio_input_support_summary),
                &audio_runtime,
                &encoder_runtime,
            ),
            capture_selection_note: capture_runtime.selection_note.clone(),
            audio_selection_note: audio_runtime.selection_note.clone(),
            encoder_selection_note: encoder_runtime.selection_note.clone(),
            preferred_audio_input_id: audio_runtime.preferred_input_id.clone(),
            preferred_audio_input_label: audio_runtime.preferred_input_label.clone(),
            preferred_system_audio_id: audio_runtime.preferred_system_id.clone(),
            preferred_system_audio_label: audio_runtime.preferred_system_label.clone(),
            preferred_encoder_label: encoder_runtime.preferred_encoder_label.clone(),
            supports_custom_region: capabilities.supports_custom_region,
            custom_region_note: capabilities.custom_region_note,
            supports_system_audio: capabilities.supports_system_audio,
            system_audio_note: capabilities.system_audio_note,
        };
    }

    #[cfg(target_os = "windows")]
    {
        let capabilities = crate::capture_capabilities::current_capture_capabilities();
        let capture_runtime = capture_windows::capture_runtime_snapshot();
        let audio_runtime = capture_windows::audio_runtime_snapshot();
        let encoder_runtime = capture_windows::encoder_runtime_snapshot();
        return RuntimeDiagnostics {
            summary: capture_runtime
                .summary
                .clone()
                .unwrap_or_else(|| "Windows desktop capture path".to_string()),
            backend_path: capture_runtime.path.clone(),
            audio_backend_path: audio_runtime.path.clone(),
            encoder_backend_path: encoder_runtime.path.clone(),
            readiness: build_readiness(
                "Desktop capture depends on ffmpeg availability, PowerShell window discovery, and DirectShow microphone readiness.",
                &capture_runtime,
                Some(capture_windows::audio_input_support_summary),
                &audio_runtime,
                &encoder_runtime,
            ),
            capture_selection_note: capture_runtime.selection_note.clone(),
            audio_selection_note: audio_runtime.selection_note.clone(),
            encoder_selection_note: encoder_runtime.selection_note.clone(),
            preferred_audio_input_id: audio_runtime.preferred_input_id.clone(),
            preferred_audio_input_label: audio_runtime.preferred_input_label.clone(),
            preferred_system_audio_id: audio_runtime.preferred_system_id.clone(),
            preferred_system_audio_label: audio_runtime.preferred_system_label.clone(),
            preferred_encoder_label: encoder_runtime.preferred_encoder_label.clone(),
            supports_custom_region: capabilities.supports_custom_region,
            custom_region_note: capabilities.custom_region_note,
            supports_system_audio: capabilities.supports_system_audio,
            system_audio_note: capabilities.system_audio_note,
        };
    }

    #[allow(unreachable_code)]
    fallback_runtime_diagnostics()
}

#[cfg(target_os = "linux")]
fn linux_runtime_diagnostics() -> RuntimeDiagnostics {
    use capture_linux::wayland_portal::{
        PipeWireGstreamerSupport, ScreenCastPortalProbe, gstreamer_pipewire_support,
        probe_screen_cast_portal,
    };
    use std::env;

    let display = env::var("DISPLAY")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let wayland = env::var("WAYLAND_DISPLAY")
        .ok()
        .filter(|value| !value.trim().is_empty());

    let capabilities = crate::capture_capabilities::current_capture_capabilities();
    let capture_runtime = capture_linux::capture_runtime_snapshot();
    let audio_runtime = capture_linux::audio_runtime_snapshot();
    let encoder_runtime = capture_linux::encoder_runtime_snapshot();

    match (display, wayland) {
        (Some(display), Some(wayland)) => RuntimeDiagnostics {
            summary: format!("Linux session: Wayland + XWayland ({wayland}, {display})"),
            backend_path: capture_runtime.path.clone(),
            audio_backend_path: audio_runtime.path.clone(),
            encoder_backend_path: encoder_runtime.path.clone(),
            readiness: build_readiness(
                "Recording can use the X11 compatibility path today. Pure Wayland sessions now route through the ScreenCast portal / PipeWire backend instead of the X11 lane.",
                &capture_runtime,
                Some(capture_linux::audio_input_support_summary),
                &audio_runtime,
                &encoder_runtime,
            ),
            capture_selection_note: capture_runtime.selection_note.clone(),
            audio_selection_note: audio_runtime.selection_note.clone(),
            encoder_selection_note: encoder_runtime.selection_note.clone(),
            preferred_audio_input_id: audio_runtime.preferred_input_id.clone(),
            preferred_audio_input_label: audio_runtime.preferred_input_label.clone(),
            preferred_system_audio_id: audio_runtime.preferred_system_id.clone(),
            preferred_system_audio_label: audio_runtime.preferred_system_label.clone(),
            preferred_encoder_label: encoder_runtime.preferred_encoder_label.clone(),
            supports_custom_region: capabilities.supports_custom_region,
            custom_region_note: capabilities.custom_region_note.clone(),
            supports_system_audio: capabilities.supports_system_audio,
            system_audio_note: capabilities.system_audio_note.clone(),
        },
        (Some(display), None) => RuntimeDiagnostics {
            summary: format!("Linux session: X11 ({display})"),
            backend_path: capture_runtime.path.clone(),
            audio_backend_path: audio_runtime.path.clone(),
            encoder_backend_path: encoder_runtime.path.clone(),
            readiness: build_readiness(
                "Recording can start directly through the Linux native X11 GStreamer path.",
                &capture_runtime,
                Some(capture_linux::audio_input_support_summary),
                &audio_runtime,
                &encoder_runtime,
            ),
            capture_selection_note: capture_runtime.selection_note.clone(),
            audio_selection_note: audio_runtime.selection_note.clone(),
            encoder_selection_note: encoder_runtime.selection_note.clone(),
            preferred_audio_input_id: audio_runtime.preferred_input_id.clone(),
            preferred_audio_input_label: audio_runtime.preferred_input_label.clone(),
            preferred_system_audio_id: audio_runtime.preferred_system_id.clone(),
            preferred_system_audio_label: audio_runtime.preferred_system_label.clone(),
            preferred_encoder_label: encoder_runtime.preferred_encoder_label.clone(),
            supports_custom_region: capabilities.supports_custom_region,
            custom_region_note: capabilities.custom_region_note.clone(),
            supports_system_audio: capabilities.supports_system_audio,
            system_audio_note: capabilities.system_audio_note.clone(),
        },
        (None, Some(wayland)) => RuntimeDiagnostics {
            summary: format!("Linux session: Wayland only ({wayland})"),
            backend_path: capture_runtime.path.clone(),
            audio_backend_path: audio_runtime.path.clone(),
            encoder_backend_path: encoder_runtime.path.clone(),
            readiness: match (probe_screen_cast_portal(), gstreamer_pipewire_support()) {
                (ScreenCastPortalProbe::Available(_), PipeWireGstreamerSupport::Available) => {
                    extend_with_native_notes(
                        "ScreenCast portal is reachable and the required GStreamer PipeWire plugins are installed. The Linux backend can use the native portal + PipeWire + GStreamer recording path in pure Wayland sessions.".to_string(),
                        &capture_runtime,
                        &audio_runtime,
                        &encoder_runtime,
                    )
                }
                (
                    ScreenCastPortalProbe::Available(_),
                    PipeWireGstreamerSupport::Missing | PipeWireGstreamerSupport::Unknown,
                ) => {
                    extend_with_native_notes(
                        "ScreenCast portal is reachable, but a usable GStreamer PipeWire runtime was not detected. Install the required GStreamer PipeWire plugins before pure Wayland recording can start.".to_string(),
                        &capture_runtime,
                        &audio_runtime,
                        &encoder_runtime,
                    )
                }
                (ScreenCastPortalProbe::MissingPortal, _) => {
                    extend_with_native_notes(
                        "Wayland is active, but no ScreenCast portal could be reached. Install xdg-desktop-portal or switch to an X11/XWayland session.".to_string(),
                        &capture_runtime,
                        &audio_runtime,
                        &encoder_runtime,
                    )
                }
                (ScreenCastPortalProbe::MissingDbusTools, _) => {
                    extend_with_native_notes(
                        "Wayland is active, but the app could not inspect ScreenCast portal readiness because neither gdbus nor busctl is available.".to_string(),
                        &capture_runtime,
                        &audio_runtime,
                        &encoder_runtime,
                    )
                }
                (ScreenCastPortalProbe::Unreachable, _) => {
                    extend_with_native_notes(
                        "Wayland is active and a portal may be installed, but the ScreenCast portal could not be reached on the session bus.".to_string(),
                        &capture_runtime,
                        &audio_runtime,
                        &encoder_runtime,
                    )
                }
            },
            capture_selection_note: capture_runtime.selection_note.clone(),
            audio_selection_note: audio_runtime.selection_note.clone(),
            encoder_selection_note: encoder_runtime.selection_note.clone(),
            preferred_audio_input_id: audio_runtime.preferred_input_id,
            preferred_audio_input_label: audio_runtime.preferred_input_label,
            preferred_system_audio_id: audio_runtime.preferred_system_id,
            preferred_system_audio_label: audio_runtime.preferred_system_label,
            preferred_encoder_label: encoder_runtime.preferred_encoder_label,
            supports_custom_region: capabilities.supports_custom_region,
            custom_region_note: capabilities.custom_region_note.clone(),
            supports_system_audio: capabilities.supports_system_audio,
            system_audio_note: capabilities.system_audio_note.clone(),
        },
        (None, None) => RuntimeDiagnostics {
            summary: "Linux session: no desktop display detected".to_string(),
            backend_path: "No active X11 or Wayland session".to_string(),
            audio_backend_path: "No active Linux audio backend".to_string(),
            encoder_backend_path: "No active Linux encoder backend".to_string(),
            readiness: "Start the app from an active desktop session to record the screen."
                .to_string(),
            capture_selection_note: "No Linux capture backend was selected because no active desktop session is available.".to_string(),
            audio_selection_note: "No Linux audio backend was selected because no active desktop session is available.".to_string(),
            encoder_selection_note: "No Linux encoder backend was selected because no active desktop session is available.".to_string(),
            preferred_audio_input_id: None,
            preferred_audio_input_label: None,
            preferred_system_audio_id: None,
            preferred_system_audio_label: None,
            preferred_encoder_label: None,
            supports_custom_region: capabilities.supports_custom_region,
            custom_region_note: capabilities.custom_region_note,
            supports_system_audio: capabilities.supports_system_audio,
            system_audio_note: capabilities.system_audio_note,
        },
    }
}

fn build_readiness(
    prefix: &str,
    capture_runtime: &CaptureBackendRuntimeSnapshot,
    fallback_audio_summary: Option<fn() -> String>,
    audio_runtime: &AudioBackendRuntimeSnapshot,
    encoder_runtime: &EncoderBackendRuntimeSnapshot,
) -> String {
    let audio_summary = audio_runtime
        .summary
        .clone()
        .or_else(|| fallback_audio_summary.map(|summary| summary()));
    extend_with_native_notes(
        join_parts([
            Some(prefix.to_string()),
            capture_runtime.summary.clone(),
            audio_summary,
            encoder_runtime.summary.clone(),
        ]),
        capture_runtime,
        audio_runtime,
        encoder_runtime,
    )
}

fn extend_with_native_notes(
    base: String,
    capture_runtime: &CaptureBackendRuntimeSnapshot,
    audio_runtime: &AudioBackendRuntimeSnapshot,
    encoder_runtime: &EncoderBackendRuntimeSnapshot,
) -> String {
    join_parts([
        Some(base),
        capture_runtime.native_unavailable_note.clone(),
        audio_runtime.native_unavailable_note.clone(),
        encoder_runtime.native_unavailable_note.clone(),
    ])
}

fn join_parts(parts: impl IntoIterator<Item = Option<String>>) -> String {
    parts
        .into_iter()
        .flatten()
        .map(|part| part.trim().to_string())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}
