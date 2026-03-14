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
- Shared recorder snapshot model
- Real capture backends per OS:
  - macOS: `AVFoundation + ffmpeg`
  - Windows: `gdigrab + dshow + ffmpeg`
  - Linux X11/XWayland: `x11grab + pulse + ffmpeg`
- Runtime diagnostics for active backend path and readiness
- Linux Wayland ScreenCast portal / PipeWire readiness and lifecycle client

## Next Integration Targets

- Linux pure Wayland PipeWire stream ingestion through the existing ScreenCast portal lifecycle
- Persistent settings storage
- Richer diagnostics and benchmark telemetry
- Deeper Windows permission and readiness hardening
