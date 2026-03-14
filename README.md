<div align="center">
  <h1>🎥 Record Screen</h1>
  <p>A high-performance, cross-platform desktop screen recorder built with modern web and systems tech.</p>

  <p>
    <img src="https://img.shields.io/badge/Tauri-v2-24C8DB?logo=tauri&logoColor=white" alt="Tauri v2" />
    <img src="https://img.shields.io/badge/React-19-61DAFB?logo=react&logoColor=black" alt="React 19" />
    <img src="https://img.shields.io/badge/Rust-1.70+-000000?logo=rust&logoColor=white" alt="Rust" />
    <img src="https://img.shields.io/badge/Platform-macOS%20%7C%20Windows%20%7C%20Linux-lightgrey" alt="Cross-Platform" />
  </p>
</div>

---

## ✨ Features

- ⌨️ **Keyboard-First Interface** - Navigate and control entirely via global shortcuts.
- ⌨️ **Configurable Shortcuts** - Remap core recorder shortcuts and re-register them without restarting the app.
- 🎨 **Modern UI** - Sleek launcher, HUD, tray menu, and recent sessions manager.
- 🦀 **Rust App State** - High-performance, memory-safe backend as the source of truth.
- 🧾 **Local Runtime Log** - Persist launch/runtime/panic logs in the app config directory for debugging real machines.
- 🎯 **Flexible Capture Targets** - Record the full desktop, a specific display, or an individual window.
- ✂️ **Custom Region Capture** - Configure a reusable capture rectangle on backends that support launcher-defined cropping.
- 🔊 **Audio Input + System Audio Mixing** - Choose a microphone and optionally mix a supported system-audio loopback source where the backend can do it.
- 🔒 **Native Permissions** - Seamless flow for `Screen recording` and `Microphone` access on macOS.
- 🚀 **Automated Builds** - GitHub Actions CI/CD for macOS DMG, Windows setup EXE, and Linux DEB packages.
- 💅 **Scalable CSS** - Styled with a modular architecture (`foundation`, `shared`, `blocks`).

<details>
<summary><b>Current MVP Backend Implementations</b></summary>
<br>

- **macOS:** `ffmpeg + AVFoundation`
- **Linux:** `ffmpeg + x11grab + pulse` on X11/XWayland today, plus a native ScreenCast portal lifecycle client and experimental GStreamer PipeWire path for pure Wayland
- **Windows:** `ffmpeg + gdigrab + dshow`
</details>

## 🗺️ Roadmap (Not Implemented Yet)
- [ ] Production-grade encoder pipeline beyond MVP.
- [ ] Full video review and export workflow.

---

## ⌨️ Global Shortcuts

| Action | Shortcut |
| :--- | :--- |
| 🔴 **Start / Stop** | <kbd>CmdOrCtrl</kbd> + <kbd>Shift</kbd> + <kbd>R</kbd> |
| ⏸️ **Pause / Resume** | <kbd>CmdOrCtrl</kbd> + <kbd>Shift</kbd> + <kbd>P</kbd> |
| 🚀 **Show Launcher** | <kbd>CmdOrCtrl</kbd> + <kbd>Shift</kbd> + <kbd>L</kbd> |
| 🎙️ **Mute / Unmute Mic** | <kbd>CmdOrCtrl</kbd> + <kbd>Shift</kbd> + <kbd>M</kbd> |

---

## 🛠️ Local Development

### Prerequisites

Ensure you have the following installed before proceeding:
- [Node.js](https://nodejs.org/)
- [Rust](https://www.rust-lang.org/)
- **macOS only:** Xcode Command Line Tools
- **All platforms:** `ffmpeg` is required for the MVP recording paths.

### Getting Started

1. **Install dependencies:**
   ```bash
   npm install
   ```

2. **Run the desktop app in development mode:**
   ```bash
   npm run dev
   ```

<details>
<summary><b>Other useful commands</b></summary>

- Run only the web UI preview:
  ```bash
  npm run dev:web
  ```
- Build desktop app locally (no installer bundling):
  ```bash
  npm run build -- --no-bundle --ci
  ```
- Build a macOS DMG locally:
  ```bash
  npm run build -- --bundles dmg --ci --no-sign
  ```
- Build a Linux DEB locally:
  ```bash
  npm run build -- --bundles deb --ci --no-sign
  ```
- Linting and type-checking:
  ```bash
  npm run lint
  npm run build:web
  cargo check
  ```
</details>

---

## 🏗️ Project Structure

The repository is structured as a monorepo, separating the UI from the Rust core:

```text
📁 record-screen/
├── 📁 apps/
│   └── 🖥️ desktop/          # React launcher and desktop UI
├── 📁 src-tauri/             # Tauri shell, commands, tray, windows
├── 📁 crates/                # Rust workspace
│   ├── ⚙️ app-core/         # Recorder state and session summaries
│   ├── 🎥 capture/          # Shared capture abstractions
│   ├── 🍏 capture-macos/    # macOS recording backend
│   ├── 🪟 capture-windows/  # Windows recording backend
│   ├── 🐧 capture-linux/    # Linux recording backend
│   ├── 🔐 permissions/      # Permission probing and request flow
│   ├── ⌨️ shortcuts/        # Shortcut bindings
│   └── 💾 storage/          # Settings and output-path helpers
└── 📁 docs/                  # Architecture, decisions, conventions
```

---

## 📝 Platform Notes

### 🍏 macOS
- `ffmpeg` must be installed and available on `PATH`.
- Must grant **Screen Recording** permission.
- If narration is enabled, must grant **Microphone** permission.
- `Custom region` and `system audio mixing` are not wired into the macOS backend yet.

### 🐧 Linux
- Supports real recording on `X11` and `Wayland + XWayland`.
- `Custom region` works on `X11` and `XWayland`, but not on pure `Wayland-only`.
- `System audio mixing` works on the X11/XWayland PulseAudio path when a monitor source exists.
- Pure `Wayland-only` capture is still experimental, but the repo now includes:
  - ScreenCast portal capability probing
  - a native DBus lifecycle client for `CreateSession`, `SelectSources`, `Start`, and `OpenPipeWireRemote`
  - PipeWire readiness probing
  - an experimental `GStreamer + pipewiresrc` runtime path
- `ffmpeg` must be on `PATH`.
- Must run inside an X11 desktop session with `DISPLAY` set, or a Wayland session with XWayland compatibility enabled.
- Microphone narration uses default PulseAudio/PipeWire source.
- Can discover individual windows from X11.
- Release packages are distributed as `.deb` files for `apt install ./record-screen_<version>_amd64.deb`.
- The launcher reports Linux readiness for:
  - `ffmpeg`
  - X11/XWayland access
  - Wayland ScreenCast portal capability
  - PipeWire readiness hints
  - whether the remaining gap is stream ingestion rather than portal negotiation
  - microphone availability

### 🪟 Windows
- Uses `gdigrab` (desktop) and `dshow` (microphone).
- `ffmpeg` must be on `PATH`.
- **PowerShell** is required to enumerate monitors/windows and control pause/resume.
- Auto-selects the best available DirectShow microphone when `Default input` is selected.
- The launcher can target the full desktop, a single monitor, or a single top-level window.
- `Custom region` is available on the desktop path.
- `System audio mixing` depends on Windows exposing a usable DirectShow loopback source such as `Stereo Mix`.
- Release packages are distributed as NSIS setup executables.

---

## 📚 Documentation
Dive deeper into the project's design and conventions:
- 📌 **Roadmap:** [`docs/roadmap/product-plan.md`](docs/roadmap/product-plan.md)
- 📋 **Product audit:** [`docs/reports/product-audit-2026-03-14.md`](docs/reports/product-audit-2026-03-14.md)
- 🏛 **Architecture:** [`docs/architecture/overview.md`](docs/architecture/overview.md)
- 🐧 **Linux Wayland status:** [`docs/architecture/linux-wayland.md`](docs/architecture/linux-wayland.md)
  This doc now also includes the handoff checklist for continuing pure Wayland work on a Linux machine.
- 📝 **Decisions:** [`docs/decisions/0001-launcher-and-hud-surfaces.md`](docs/decisions/0001-launcher-and-hud-surfaces.md)
- 💅 **Frontend:** [`docs/frontend/styleguide.md`](docs/frontend/styleguide.md)
- 📦 **Distribution overview:** [`docs/distribution/overview.md`](docs/distribution/overview.md)
- 🐧 **Linux install guide:** [`docs/distribution/linux.md`](docs/distribution/linux.md)
- 🧰 **APT repository guide:** [`docs/distribution/apt-repo.md`](docs/distribution/apt-repo.md)
- 🍺 **Homebrew cask template:** [`docs/distribution/homebrew.md`](docs/distribution/homebrew.md)
- 🍏 **macOS install guide:** [`docs/distribution/macos.md`](docs/distribution/macos.md)
- 🪟 **Windows install guide:** [`docs/distribution/windows.md`](docs/distribution/windows.md)
- ⚖️ **Licensing:** [`docs/legal/licensing.md`](docs/legal/licensing.md)
