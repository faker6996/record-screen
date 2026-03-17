# Linux Guide

## Install

The Linux release artifact is a Debian package:

```bash
sudo apt install ./record-screen_<version>_amd64.deb
```

Install the native runtime packages first if they are not already available:

```bash
sudo apt update
sudo apt install \
  gstreamer1.0-tools \
  gstreamer1.0-plugins-base \
  gstreamer1.0-plugins-good \
  gstreamer1.0-plugins-ugly \
  gstreamer1.0-pipewire
```

## Requirements

- Debian or Ubuntu based distribution with `apt`
- X11 desktop session, or a Wayland session with XWayland compatibility
- `gst-launch-1.0` and the required GStreamer plugins on `PATH`
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
- on Wayland: the app can negotiate a native ScreenCast portal / PipeWire session, but pure Wayland recording still needs more runtime hardening before it matches the X11 lane

## Linux Runtime Status

- `X11`: supported for real recording through the native X11 GStreamer lane
- `Wayland + XWayland`: supported through the same native X11 GStreamer lane
- `Wayland-only`: native portal / PipeWire / GStreamer lane exists, but is not fully hardened yet

The current repo already includes:

- ScreenCast portal capability probing
- GStreamer PipeWire runtime probing
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

- [`docs/architecture/linux-wayland.md`](../architecture/linux-wayland.md)
  Includes the current implementation status plus the next-step checklist for continuing pure Wayland support on a Linux machine.

## Uninstall

```bash
sudo apt remove record-screen
```

## Notes on `apt`

The package is installable with `apt install ./file.deb` today.
The release workflow also includes a signed APT repository publish path; see:

- [`docs/distribution/apt-repo.md`](./apt-repo.md)
