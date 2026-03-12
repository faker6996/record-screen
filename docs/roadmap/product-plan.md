# Record Screen Plan

## Product Goals

- Launch a recorder from anywhere with a global shortcut.
- Start, pause, and stop recording without forcing the user back into the app.
- Keep the UI compact, readable, and keyboard-first.
- Share as much logic as possible in Rust while isolating OS-specific capture code.

## Architecture Principles

- Rust is the source of truth for recorder state and lifecycle.
- React is the control surface, not the recording engine.
- OS-specific capture code lives in separate crates to avoid `cfg` sprawl.
- Tauri owns windows, global shortcuts, app lifecycle, and secure command boundaries.
- Every major decision that affects maintainability gets an ADR in `docs/decisions/`.

## Project Structure

```text
apps/desktop
src-tauri
crates/app-core
crates/capture
crates/capture-macos
crates/capture-windows
crates/capture-linux
crates/audio
crates/encoder
crates/shortcuts
crates/permissions
crates/storage
crates/export
crates/telemetry
docs/*
```

## Phases

### Phase 0: Product and Technical Spikes

- Lock MVP scope, output formats, and supported OS versions.
- Validate screen capture APIs per platform.
- Define recorder state machine and failure model.
- Decide signing, permissions copy, and local storage format.

### Phase 1: Foundation

- Bootstrap Tauri v2 shell, React UI, Rust workspace, and CI checks.
- Add launcher window, app state, command handlers, and event wiring.
- Add docs for architecture, roadmap, and coding boundaries.

### Phase 2: Shortcuts and Permissions

- Register global shortcuts and make them user-configurable.
- Add onboarding flow for screen, microphone, and accessibility permissions.
- Support launcher recall from anywhere while the app runs in background.

### Phase 3: Recording MVP

- Record one display at a time.
- Capture microphone audio.
- Save finalized files to a chosen output directory.
- Show elapsed time, recording state, and mic status in launcher and HUD.

### Phase 4: Capture Expansion

- Add monitor, window, and region selection.
- Add pause/resume with resilient file finalization.
- Add system audio where the platform allows it.
- Support quality presets and output format options.

### Phase 5: Product Polish

- Add quick review, recent sessions, and open-folder actions.
- Improve shortcut editor, error copy, and empty states.
- Add tray/HUD refinements and background lifecycle polish.

### Phase 6: Stabilization

- Stress test long recordings, sleep/wake, display changes, and device loss.
- Tune memory, CPU, and encoder backpressure.
- Add packaging, signing, and update flow.

### Phase 7: Advanced Features

- Camera overlay and scene composition.
- Noise suppression and audio cleanup.
- Click highlights, cursor emphasis, and lightweight annotations.
- Share/export presets and cloud handoff.

## UX Direction

- Primary surface is a compact launcher, not a bulky dashboard.
- The first-run experience focuses on permission readiness and shortcut confidence.
- The UI should stay expressive and calm: bold typography, dense but legible cards, strong contrast, and clear command hierarchy.
- Keyboard actions must always have visible labels and shortcut hints.

## Default Shortcuts

- `CmdOrCtrl+Shift+R`: start or stop recording
- `CmdOrCtrl+Shift+P`: pause or resume
- `CmdOrCtrl+Shift+L`: show launcher
- `CmdOrCtrl+Shift+M`: mute or unmute microphone

## Exit Criteria For MVP

- Launcher can be recalled at any time via global shortcut.
- Recorder state survives normal window hiding and focus changes.
- Files finalize cleanly when recording stops.
- Permission status is understandable without opening system docs.
- UI remains usable on laptop and desktop displays.
