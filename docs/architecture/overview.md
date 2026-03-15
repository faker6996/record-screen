# Architecture Overview

## Boundaries

- `apps/desktop` renders the launcher, settings, and session surfaces.
- `src-tauri` translates Tauri runtime events into app-core actions.
- `crates/app-core` owns recorder state, shortcut presets, and bootstrap snapshots.
- `crates/capture-*` will wrap native APIs for each operating system.
- `crates/encoder` will own muxing and final file creation.

## Runtime Flow

1. Tauri starts and registers window and shortcut handlers.
2. `app-core` builds a bootstrap snapshot for the UI.
3. React requests bootstrap data and renders launcher sections.
4. Commands and shortcut events mutate Rust state.
5. Tauri emits recorder updates back to the UI.

## Current Integrated Scope

- Launcher window and state wiring
- Separate HUD surface with lightweight state path
- Tray menu for launcher recall and recorder actions
- Global shortcut registration
- Persisted shortcut remapping with runtime re-registration
- Shared recorder snapshot model
- Audio input classification for microphone vs system-audio loopback sources
- Custom region settings, drag-to-select overlay, and target injection on supported backends
- System-audio mix toggle with per-platform support guards
- Real capture backends per OS:
  - macOS: `AVFoundation + ffmpeg`, including custom-region crop on the display path
  - Windows: `gdigrab + dshow + ffmpeg`
  - Linux X11/XWayland: `x11grab + pulse + ffmpeg`
- Target preview overlay when choosing a display or custom region
- Runtime diagnostics for active backend path and readiness
- Local runtime crash/error logging
- Cross-platform launch-on-login integration
- Linux Wayland ScreenCast portal / PipeWire readiness and lifecycle client
- Experimental Linux Wayland GStreamer PipeWire runtime path

## Next Integration Targets

- Native backend migration now tracks in `docs/roadmap/native-backend-plan.md`
- The native migration now explicitly includes legacy cleanup and architecture-boundary tightening, not only feature parity
- Production-grade Linux pure Wayland capture hardening beyond the current experimental GStreamer PipeWire path
- Windows native capture/audio backend work beyond the current ffmpeg stack
- macOS native encode/system-audio backend work beyond the current ffmpeg runtime
- Richer diagnostics and benchmark telemetry
- Richer export workflow
