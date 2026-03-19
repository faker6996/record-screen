# Linux Wayland Status

## Current State

The Linux recorder has two practical paths today:

- `X11`: real recording through `GStreamer + ximagesrc + pulsesrc`
- `Wayland + XWayland`: real recording through the same native X11 GStreamer path

Pure `Wayland-only` sessions are not fully recordable yet, but the repo now contains a dedicated `ScreenCast portal / PipeWire` module in:

- `crates/capture-linux/src/wayland_portal.rs`

## What Is Implemented

- native Linux recorder selection no longer depends on the old ffmpeg runtime
- native X11/XWayland capture, audio, and encode lanes now run through GStreamer
- session classification:
  - `X11`
  - `Wayland + XWayland`
  - `Wayland-only`
  - `Headless`
- ScreenCast portal capability probing
  - `AvailableSourceTypes`
  - `AvailableCursorModes`
- GStreamer PipeWire runtime probing
- native DBus lifecycle execution for:
  - `CreateSession`
  - `SelectSources`
  - `Start`
  - `OpenPipeWireRemote`
- parser coverage and command-plan scaffolding for:
  - `CreateSession`
  - `SelectSources`
  - `Start`
  - `OpenPipeWireRemote`
- launcher diagnostics that explain the active Linux capture path
- Linux X11 smoke tests, GUI verification, and a 10-minute soak run captured in:
  - [`docs/linux-x11-performance-report-2026-03-18.md`](../linux-x11-performance-report-2026-03-18.md)

## What Is Not Implemented Yet

- hardening the returned PipeWire remote fd path into a consistently working production recorder on pure Wayland
- end-to-end runtime validation of the native Wayland lane across more real GNOME / NVIDIA / portal combinations

## Practical Meaning

- Linux is stable today on `X11`
- Linux is usable on `Wayland + XWayland`
- Linux `X11/XWayland` is now on the native mainline recorder path, not the old ffmpeg runtime
- Linux `Wayland-only` is now diagnosed correctly, negotiates a real ScreenCast portal session, and routes into the native GStreamer lane, but it still is not hardened enough to claim production-ready support

## Next Work On A Linux Machine

If you want to continue implementation on a real Linux Wayland desktop, the remaining work is:

1. verify `CreateSession -> SelectSources -> Start -> OpenPipeWireRemote` on a live Wayland session and log:
   - returned `session_handle`
   - returned `stream node ids`
   - whether the PipeWire remote fd is valid
2. attach a PipeWire client to the returned remote fd
3. enumerate the remote registry and confirm the selected stream node appears there
4. bind a capture stream to that node
5. harden the PipeWire -> GStreamer runtime path until it is production-stable across the supported Linux desktop combinations

## Files To Continue In

- [`crates/capture-linux/src/wayland_portal.rs`](../../crates/capture-linux/src/wayland_portal.rs)
- [`crates/capture-linux/src/native_capture_backend.rs`](../../crates/capture-linux/src/native_capture_backend.rs)
- [`crates/capture-linux/src/lib.rs`](../../crates/capture-linux/src/lib.rs)
- [`src-tauri/src/diagnostics.rs`](../../src-tauri/src/diagnostics.rs)

## Practical Linux Checklist

- run on a real `Wayland-only` session, not `XWayland`
- ensure `xdg-desktop-portal` and the compositor-specific portal backend are installed
- ensure `PipeWire` is running for the user session
- verify `gst-launch-1.0`, `pipewiresrc`, `x264enc`, and `mp4mux` are available
- keep one terminal open to capture portal / PipeWire logs while testing
