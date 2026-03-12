# Record Screen

A cross-platform desktop screen recorder built with **Tauri v2**, **React 19**, and **Rust**.

The current repo state is a strong MVP foundation with a working macOS recording path, launcher UI, global shortcuts, tray/HUD shell, and a Rust workspace split by domain.

## Current Status

Implemented now:

- keyboard-first desktop shell with global shortcuts
- launcher, HUD, tray menu, settings, recent sessions
- Rust app state as the source of truth
- macOS recording MVP using `ffmpeg + AVFoundation`
- macOS permission flow for `Screen recording` and `Microphone`
- GitHub Actions build workflow for macOS, Windows, and Linux installers
- frontend CSS structure split into `foundation`, `shared`, and `blocks`

Not implemented yet:

- native Windows capture backend
- native Linux capture backend
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
  capture-windows/  Windows backend scaffold
  capture-linux/    Linux backend scaffold
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
- `ffmpeg` is currently required for the macOS recording MVP

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

The current real recording path is macOS-first.

Requirements:

- `ffmpeg` must be installed and available on `PATH`
- `Screen Recording` permission must be granted
- if microphone narration is enabled, `Microphone` permission must also be granted

The launcher permission panel can:

- refresh permission status
- request access
- open the matching macOS Privacy page

## Documentation

- Roadmap: [`docs/roadmap/product-plan.md`](docs/roadmap/product-plan.md)
- Architecture: [`docs/architecture/overview.md`](docs/architecture/overview.md)
- Decision record: [`docs/decisions/0001-launcher-and-hud-surfaces.md`](docs/decisions/0001-launcher-and-hud-surfaces.md)
- Frontend CSS conventions: [`docs/frontend/styleguide.md`](docs/frontend/styleguide.md)
