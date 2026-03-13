# Record Screen

A cross-platform desktop screen recorder built with **Tauri v2**, **React 19**, and **Rust**.

The current repo state is a strong MVP foundation with a working macOS recording path, launcher UI, global shortcuts, tray/HUD shell, and a Rust workspace split by domain.

## Current Status

Implemented now:

- keyboard-first desktop shell with global shortcuts
- launcher, HUD, tray menu, settings, recent sessions
- Rust app state as the source of truth
- macOS recording MVP using `ffmpeg + AVFoundation`
- Linux recording MVP using `ffmpeg + x11grab + pulse`
- Windows recording MVP using `ffmpeg + gdigrab + dshow`
- capture target selection for full desktop, a single display, or a single window where the platform backend supports it
- macOS permission flow for `Screen recording` and `Microphone`
- GitHub Actions build workflow for macOS, Windows, and Linux installers
- frontend CSS structure split into `foundation`, `shared`, and `blocks`

Not implemented yet:

- production-grade encoder pipeline beyond the current macOS MVP path
- full review/export workflow

## Project Structure

```text
apps/
  desktop/          React launcher and desktop UI
src-tauri/          Tauri shell, commands, tray, windows, runtime orchestration
crates/
  app-core/         Recorder state and session summaries
  capture/          Shared capture abstractions
  capture-macos/    macOS recording backend
  capture-windows/  Windows recording backend
  capture-linux/    Linux recording backend
  permissions/      Permission probing and request flow
  shortcuts/        Shortcut bindings
  storage/          Settings and output-path helpers
docs/               Architecture, roadmap, decisions, frontend conventions
```

## Default Global Shortcuts

| Action | Shortcut |
| :--- | :--- |
| Start / Stop | `CmdOrCtrl+Shift+R` |
| Pause / Resume | `CmdOrCtrl+Shift+P` |
| Show Launcher | `CmdOrCtrl+Shift+L` |
| Mute / Unmute Mic | `CmdOrCtrl+Shift+M` |

## Local Development

Prerequisites:

- [Node.js](https://nodejs.org/)
- [Rust](https://www.rust-lang.org/)
- macOS developers should also have Xcode Command Line Tools
- `ffmpeg` is required for the current macOS, Linux, and Windows recording paths

Install dependencies:

```bash
npm install
```

Run the desktop app in development mode:

```bash
npm run dev
```

Run only the web UI preview:

```bash
npm run dev:web
```

Build the desktop app locally without installer bundling:

```bash
npm run build -- --no-bundle --ci
```

Build a macOS DMG locally:

```bash
npm run build -- --bundles dmg --ci --no-sign
```

## Validation

Useful checks:

```bash
npm run lint
npm run build:web
cargo check
```

## macOS Notes

The current real recording path is implemented for macOS.

Requirements:

- `ffmpeg` must be installed and available on `PATH`
- `Screen Recording` permission must be granted
- if microphone narration is enabled, `Microphone` permission must also be granted

## Linux Notes

The Linux MVP path currently targets X11 sessions.

Requirements:

- `ffmpeg` must be installed and available on `PATH`
- the app must run inside an X11 desktop session with `DISPLAY` set
- microphone narration uses the default PulseAudio / PipeWire source when available
- the launcher can target the full desktop, a single monitor, or an individual window discovered from X11

The launcher permission panel can:

- refresh permission status
- request access
- open the matching macOS Privacy page

## Windows Notes

The Windows MVP path uses `gdigrab` for the desktop video stream and `dshow` for microphone capture.

Requirements:

- `ffmpeg` must be installed and available on `PATH`
- PowerShell must be available so the app can enumerate displays, windows, and control pause / resume
- microphone narration uses the first matching DirectShow microphone device reported by `ffmpeg`

The launcher can target the full desktop, a single monitor, or a single top-level window discovered from the current desktop session.

## Documentation

- Roadmap: [`docs/roadmap/product-plan.md`](docs/roadmap/product-plan.md)
- Architecture: [`docs/architecture/overview.md`](docs/architecture/overview.md)
- Decision record: [`docs/decisions/0001-launcher-and-hud-surfaces.md`](docs/decisions/0001-launcher-and-hud-surfaces.md)
- Frontend CSS conventions: [`docs/frontend/styleguide.md`](docs/frontend/styleguide.md)
