# Linux Guide

## Install

The Linux release artifact is a Debian package:

```bash
sudo apt install ./record-screen_<version>_amd64.deb
```

Install `ffmpeg` first if it is not already available:

```bash
sudo apt update
sudo apt install ffmpeg
```

## Requirements

- Debian or Ubuntu based distribution with `apt`
- X11 desktop session
- `ffmpeg` on `PATH`
- PulseAudio or PipeWire for microphone capture

## First run

Launch the app from the application menu or from the shell:

```bash
record-screen
```

Before recording:

- choose `Full desktop`, a specific display, or a single window in the launcher
- confirm the output folder is correct
- make sure `DISPLAY` is set because the current Linux recorder targets X11

## Uninstall

```bash
sudo apt remove record-screen
```

## Notes on `apt`

The package is installable with `apt install ./file.deb` today.
The project does not yet publish a dedicated APT repository, so `apt install record-screen` is not available yet.
