<div align="center">
  <h1>🎥 Record Screen</h1>
  <p>A fast, lightweight, and easy-to-use cross-platform screen recorder.</p>

  <p>
    <img src="https://img.shields.io/badge/Platform-macOS%20%7C%20Windows%20%7C%20Linux-lightgrey" alt="Cross-Platform" />
    <img src="https://img.shields.io/badge/Tauri-v2-24C8DB?logo=tauri&logoColor=white" alt="Tauri v2" />
    <img src="https://img.shields.io/badge/React-19-61DAFB?logo=react&logoColor=black" alt="React 19" />
    <img src="https://img.shields.io/badge/Rust-1.70+-000000?logo=rust&logoColor=white" alt="Rust" />
  </p>
</div>

---

Welcome to **Record Screen**! Whether you want to record a quick tutorial, save an important meeting, or capture a bug, this app provides a seamless and high-performance recording experience directly from your desktop.

## ✨ Key Features

- 🎯 **Capture Anything:** Record your full desktop, a specific window, or drag to select a custom capture region.
- 🔊 **Clear Audio:** Capture your microphone narrations, your system's internal audio, or mix both together effortlessly.
- ⌨️ **Keyboard-First:** Control your entire recording session using global keyboard shortcuts without needing to click around.
- ⚡ **High Performance:** Built with modern technologies (Rust) to ensure smooth recording without draining your system resources.
- 🌍 **Cross-Platform:** Works beautifully everywhere you do—macOS, Windows, and Linux.

---

## 🚀 Quick Start (How to Use)

1. **Launch the App:** Open "Record Screen". A sleek, minimal launcher will appear on your screen.
2. **Select your Target:** Choose whether you want to record the **Full desktop**, a specific **Window**, or a **Custom region**.
3. **Configure Audio:** Toggle your **Microphone** or **System Audio** on or off based on your needs.
4. **Start Recording:** Press `Ctrl/Cmd + Shift + R` or click the Start recording button on the launcher.
5. **Stop Recording:** Press `Ctrl/Cmd + Shift + R` again. Your video will be saved automatically!

---

## ⌨️ Global Shortcuts

You can control the app from anywhere on your computer using these default shortcuts. You can also customize them directly in the app settings!

| Action | Shortcut |
| :--- | :--- |
| 🔴 **Start / Stop Recording** | <kbd>Cmd/Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>R</kbd> |
| ⏸️ **Pause / Resume** | <kbd>Cmd/Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>P</kbd> |
| 🚀 **Show/Hide Launcher** | <kbd>Cmd/Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>L</kbd> |
| 🎙️ **Mute / Unmute Mic** | <kbd>Cmd/Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>M</kbd> |

---

## 🔐 Important Permissions

Depending on your operating system, the app needs permission to capture your screen and audio.

- **macOS:** When you first run the app, you will be prompted to allow **Screen Recording** and **Microphone** access in System Settings. Just follow the on-screen instructions.
- **Windows / Linux:** Generally works out of the box! On some modern Linux setups (like Wayland), your system will ask you to confirm screen sharing when you start a recording.

---

## 📥 Installation

*You can download the latest version of Record Screen from the Releases page.*

- 🍏 **macOS:** Download the `.dmg` file, open it, and drag the app into your Applications folder.
- 🪟 **Windows:** Download the `.exe` setup file and run the installer.
- 🐧 **Linux:** Download the `.deb` package and install it via your package manager.

---

## 🛠️ For Developers

Are you a developer looking to contribute or understand how Record Screen works under the hood? All technical documentation, architecture overviews, and build instructions have been moved to the documentation folder.

- 🏗 **Architecture Overview:** [docs/architecture/overview.md](docs/architecture/overview.md)
- 🚀 **Build locally:** 
  ```bash
  npm install
  npm run dev
  ```
- 📚 **More Docs:** Check the [`docs/`](docs/) directory for detailed backend implementations, native backend plans, and Linux/Wayland compatibility tracking.
