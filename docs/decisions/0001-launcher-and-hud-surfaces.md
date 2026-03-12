# ADR 0001: Launcher And HUD Surfaces

## Status

Accepted

## Context

The recorder needs two very different desktop surfaces:

- a richer launcher window for setup, permissions, and recent sessions
- a minimal HUD that can stay visible while the user is working elsewhere

The app also needs a tray entry point so closing the launcher does not feel like quitting the recorder.

## Decision

- Keep `main` as the launcher window.
- Create a separate `hud` webview window from the Tauri shell.
- Render different React surfaces based on the current Tauri window label.
- Intercept close requests for `main` and `hud` and hide them instead of destroying them.
- Expose tray actions for launcher recall, recorder controls, and HUD visibility.

## Consequences

- The UI stays modular without needing two frontend bundles.
- Rust remains responsible for window lifecycle and recorder-driven HUD visibility.
- Future work can add drag, pinning, and compact HUD theming without touching launcher concerns.
- Native capture can later toggle HUD visibility based on recorder state without involving React routing.
