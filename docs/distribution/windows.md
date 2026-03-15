# Windows Guide

## Install

The Windows release artifact is a setup executable built with NSIS.

Install flow:

1. Download `Record Screen Setup.exe`.
2. Run the installer.
3. Follow the setup wizard.

## Requirements

- Windows desktop session
- `ffmpeg` on `PATH`
- WebView2 runtime available

The installer uses Tauri's WebView2 bootstrapper mode by default, so machines without WebView2 may need an internet connection during setup.

## First run

After launch:

- choose `Full desktop`, a specific display, or a single window in the launcher
- verify the microphone setting
- confirm the output folder before the first recording

## Current backend scope

- full desktop capture
- single-display capture
- top-level window capture
- custom-region capture on the desktop path
- microphone narration through DirectShow

Notes:

- when `Default input` is selected, the app prefers a discovered DirectShow microphone
- if DirectShow discovery fails, the app attempts to fall back to the current Windows default recording device
- system-audio mixing still depends on Windows exposing a usable loopback source such as `Stereo Mix`

## Uninstall

- remove the app from Windows `Apps & features`
- or run the uninstaller created by the setup package
