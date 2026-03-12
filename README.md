# 🎥 Record Screen

A cross-platform desktop screen recorder crafted for a compact, keyboard-first experience. 
Built using **Tauri v2**, **React 19**, and **Rust**.

---

## ✨ Features & Product Goals

- ⚡️ **Keyboard-First:** Launch the recorder from anywhere with global shortcuts.
- 🎯 **Minimalist UI:** Quick, compact launcher that stays out of your way. Start, pause, or stop without switching contexts.
- 🦀 **Rust Core:** Robust and performant system interaction, keeping OS-specific capture logic contained and modular.
- 🖼 **React Surface:** Clean, dense, bold typography for extreme legibility and high contrast.

## 🛠 Project Structure (Workspace)

The project leverages a monorepo structure separating view surfaces from hardware logic:

- 💻 **`apps/desktop`**: The React-based launcher and settings user interface. 
- 🐚 **`src-tauri`**: The desktop shell. Manages window lifecycles, global shortcuts, and acts as the secure command bridge.
- ⚙️ **`crates/*`**: The Rust backend divided by domain:
  - `app-core`: The source of truth for recorder state.
  - `capture-*`: Cleanly separated OS-specific backends (Windows, macOS, Linux).
  - `audio`, `encoder`, `export`: Hardware access and media processing lines.
  - `permissions`, `shortcuts`, `storage`, `telemetry`: Cross-cutting app utilities.
- 📚 **`docs/`**: Essential documentation containing Architecture Decisions (ADR), Roadmaps, and UX blueprints.

## ⌨️ Default Global Shortcuts

| Action | Shortcut |
| :--- | :--- |
| **Start / Stop** | `CmdOrCtrl` + `Shift` + `R` |
| **Pause / Resume** | `CmdOrCtrl` + `Shift` + `P` |
| **Show Launcher** | `CmdOrCtrl` + `Shift` + `L` |
| **Mute / Unmute Mic** | `CmdOrCtrl` + `Shift` + `M` |

## 🚀 Quick Start

### Prerequisites

Ensure you have [Node.js](https://nodejs.org/) and [Rust](https://www.rust-lang.org/) installed for your operating system.

### Commands

Install dependencies:
```bash
npm install
```

Start the application in development mode (runs both Tauri shell and Vite dev server):
```bash
npm run dev
```

Run code quality checks:
```bash
# Lint frontend
npm run lint

# Typecheck Rust backend
cargo check
```

## 🗺 Architecture & Roadmap

Refer to the [`docs/roadmap/product-plan.md`](docs/roadmap/product-plan.md) and [`docs/architecture/overview.md`](docs/architecture/overview.md) files for deeper implementation phases, moving from current capture scoping to advanced features like scene composition and noise suppression.
