# macOS Guide

## Install

The macOS release artifact is a DMG.

Install flow:

1. Open `Record Screen.dmg`.
2. Drag `Record Screen.app` into `Applications`.
3. Launch the app from `Applications`.

Install `ffmpeg` if it is missing:

```bash
brew install ffmpeg
```

## Requirements

- macOS with Screen Recording permission support
- `ffmpeg` on `PATH`
- Screen Recording permission granted
- Microphone permission granted if narration is enabled

## First run

On first launch:

- allow `Screen Recording` access when prompted
- allow `Microphone` access if you plan to record narration
- verify the selected output folder and quality preset in the launcher
- `Custom region` selection is available on macOS and records through the display capture path

## Current backend scope

- full desktop capture
- single-display capture
- custom-region capture on the selected display
- microphone narration

Not available yet:

- system-audio mixing

## Homebrew status

Homebrew distribution is supported through a custom tap publish flow once the repository token is configured.
The intended install command is:

```bash
brew tap faker6996/tap
brew install --cask faker6996/tap/record-screen
```

The direct DMG install path still works even if the Homebrew tap is not configured yet.

## Uninstall

- Delete `Record Screen.app` from `Applications`
- Remove saved output files from your chosen recording folder if needed
