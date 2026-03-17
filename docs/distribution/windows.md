# Windows Guide

## Install

The Windows release artifact is a setup executable built with NSIS.

Install flow:

1. Download `Record Screen Setup.exe`.
2. Run the installer.
3. Follow the setup wizard.

## Requirements

- Windows desktop session
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
- microphone narration through WASAPI
- system-audio capture through WASAPI loopback

Notes:

- when `Default input` is selected, the app resolves the current Windows default capture endpoint through native WASAPI probing
- supported recording lanes now use the native `Windows.Graphics.Capture + Media Foundation + WASAPI` path
- full-desktop capture across multiple monitors is now composed natively from multiple monitor sessions

## Uninstall

- remove the app from Windows `Apps & features`
- or run the uninstaller created by the setup package
