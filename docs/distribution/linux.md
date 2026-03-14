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
- X11 desktop session, or a Wayland session with XWayland compatibility
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
- on X11: make sure `DISPLAY` is set
- on Wayland: the app can diagnose ScreenCast portal readiness and the code can negotiate a ScreenCast session, but pure Wayland recording is still waiting on PipeWire stream ingestion

## Linux Runtime Status

- `X11`: supported for real recording
- `Wayland + XWayland`: supported through the X11 compatibility path
- `Wayland-only`: not finished for real recording yet

The current repo already includes:

- ScreenCast portal capability probing
- ffmpeg PipeWire support probing
- native DBus lifecycle execution for:
  - `CreateSession`
  - `SelectSources`
  - `Start`
  - `OpenPipeWireRemote`
- parser and command-plan scaffolding for:
  - `CreateSession`
  - `SelectSources`
  - `Start`
  - `OpenPipeWireRemote`

Reference:

- [`docs/architecture/linux-wayland.md`](/Users/tran_van_bach/Desktop/project/record-screen/docs/architecture/linux-wayland.md)
  Includes the current implementation status plus the next-step checklist for continuing pure Wayland support on a Linux machine.

## Uninstall

```bash
sudo apt remove record-screen
```

## Notes on `apt`

The package is installable with `apt install ./file.deb` today.
The release workflow also includes a signed APT repository publish path; see:

- [`docs/distribution/apt-repo.md`](/Users/tran_van_bach/Desktop/project/record-screen/docs/distribution/apt-repo.md)
