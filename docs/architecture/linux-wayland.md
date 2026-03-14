# Linux Wayland Status

## Current State

The Linux recorder has two practical paths today:

- `X11`: real recording through `ffmpeg + x11grab + pulse`
- `Wayland + XWayland`: real recording through the same X11 compatibility path

Pure `Wayland-only` sessions are not fully recordable yet, but the repo now contains a dedicated `ScreenCast portal / PipeWire` module in:

- `crates/capture-linux/src/wayland_portal.rs`

## What Is Implemented

- session classification:
  - `X11`
  - `Wayland + XWayland`
  - `Wayland-only`
  - `Headless`
- ScreenCast portal capability probing
  - `AvailableSourceTypes`
  - `AvailableCursorModes`
- ffmpeg PipeWire device probing
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

## What Is Not Implemented Yet

- ingesting the returned PipeWire remote fd into the recorder
- replacing `x11grab` with a pure Wayland capture path during actual recording

## Practical Meaning

- Linux is stable today on `X11`
- Linux is usable on `Wayland + XWayland`
- Linux `Wayland-only` is now diagnosed correctly and can negotiate a real ScreenCast portal session, but it still cannot ingest the returned PipeWire stream into the recorder

## Next Work On A Linux Machine

If you want to continue implementation on a real Linux Wayland desktop, the remaining work is:

1. verify `CreateSession -> SelectSources -> Start -> OpenPipeWireRemote` on a live Wayland session and log:
   - returned `session_handle`
   - returned `stream node ids`
   - whether the PipeWire remote fd is valid
2. attach a PipeWire client to the returned remote fd
3. enumerate the remote registry and confirm the selected stream node appears there
4. bind a capture stream to that node
5. bridge decoded video frames into the existing recorder pipeline or replace the Linux `ffmpeg + x11grab` path with a dedicated Wayland capture path

## Files To Continue In

- [`crates/capture-linux/src/wayland_portal.rs`](/Users/tran_van_bach/Desktop/project/record-screen/crates/capture-linux/src/wayland_portal.rs)
- [`crates/capture-linux/src/lib.rs`](/Users/tran_van_bach/Desktop/project/record-screen/crates/capture-linux/src/lib.rs)
- [`src-tauri/src/diagnostics.rs`](/Users/tran_van_bach/Desktop/project/record-screen/src-tauri/src/diagnostics.rs)

## Practical Linux Checklist

- run on a real `Wayland-only` session, not `XWayland`
- ensure `xdg-desktop-portal` and the compositor-specific portal backend are installed
- ensure `PipeWire` is running for the user session
- verify `ffmpeg` is available for the current fallback paths
- keep one terminal open to capture portal / PipeWire logs while testing
