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

## Current Scaffold Scope

- Launcher window and state wiring
- Separate HUD window scaffold driven by Tauri window labels
- Tray menu for launcher recall and recorder actions
- Global shortcut registration scaffold
- Shared recorder snapshot model
- Permissions and recent sessions placeholder data

## Next Integration Targets

- Native capture pipeline selection
- Persistent settings storage
- Real permission probing per OS
- Floating HUD window
