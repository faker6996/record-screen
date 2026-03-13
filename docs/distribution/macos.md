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

## Homebrew status

Homebrew distribution is possible through a cask, but this repository does not publish one yet.
The realistic future install command would look like:

```bash
brew install --cask <tap>/record-screen
```

That requires maintaining a Homebrew tap or submitting a cask upstream.

## Uninstall

- Delete `Record Screen.app` from `Applications`
- Remove saved output files from your chosen recording folder if needed
