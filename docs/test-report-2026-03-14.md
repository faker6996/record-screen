# Test Report - 2026-03-14

## Scope

This report covers a full Linux verification pass for the current `record-screen` workspace on the local development machine, including:

- workspace automated tests
- frontend lint/build
- Tauri no-bundle release build
- Linux smoke tests for real recording
- Linux library command verification for `Recent / Save As / Trash`
- runtime GUI verification for launcher, HUD, shortcut flow, pause/resume, and recording artifact output

## Environment

- Date: `2026-03-14`
- OS: `Ubuntu 24.04`-based Linux
- Kernel: `6.17.0-14-generic`
- Session: `X11`
- `DISPLAY`: `:1`
- `WAYLAND_DISPLAY`: unset
- Runtime tools present:
  - `ffmpeg`
  - `ffprobe`
  - `xdotool`
  - `wmctrl`
  - `gst-launch-1.0`
  - `gst-inspect-1.0`
- Runtime notes:
  - `pactl` missing on this machine
  - `ffmpeg` PipeWire device missing
  - GStreamer PipeWire path available

## Automated Checks

### 1. Frontend lint

Command:

```bash
npm run lint -- --quiet
```

Result:

- Pass

### 2. Frontend production build

Command:

```bash
npm run build:web
```

Result:

- Pass

### 3. Workspace tests

Command:

```bash
cargo test --workspace
```

Result:

- Pass

Highlights:

- `capture` tests: `2/2` pass
- `capture-linux` tests: `20/20` pass
- `storage` tests: `2/2` pass
- ignored smoke tests remained ignored in the workspace-wide run, as expected

### 4. Linux smoke report script

Command:

```bash
bash scripts/linux-test-report.sh
```

Result:

- Pass

Verified by script:

- `cargo check`
- `capture-linux` unit tests
- Linux smoke test without microphone
- Linux smoke test with microphone
- `ffmpeg -f pulse -i default` probe

### 4b. Linux library command tests

Command:

```bash
cargo test -p record-screen-desktop library::tests -- --nocapture
```

Result:

- Pass

Verified by these tests:

- recent recordings scan filters only supported video extensions
- recent recordings list is sorted by newest modified time first
- Linux `Save As` command copies the selected recording to the chosen destination
- Linux `Trash` command uses the configured trash handler and removes the original file from the recordings directory

Implementation coverage:

- [library.rs](/home/bachtv/Desktop/project/record-screen/src-tauri/src/commands/library.rs)

### 5. Tauri no-bundle release build

Command:

```bash
npm run build -- --no-bundle --ci
```

Result:

- Pass
- built binary:
  - `target/release/record-screen-desktop`

## Runtime GUI Verification

### Scenario: launcher + HUD + shortcut-driven recording

Runtime steps executed:

1. launch `target/release/record-screen-desktop`
2. verify launcher window exists
3. start recording via global shortcut `Ctrl+Shift+R`
4. verify HUD window appears
5. pause via `Ctrl+Shift+P`
6. wait
7. resume via `Ctrl+Shift+P`
8. wait
9. stop via `Ctrl+Shift+R`
10. verify HUD window disappears
11. inspect generated recording with `ffprobe`

Observed runtime results:

- launcher window detected: yes
- HUD window after start: yes
- HUD window after stop: hidden
- recording artifact created: yes
- pause/resume effect visible in output duration: yes

Generated artifact:

- file: `/home/bachtv/Movies/Record Screen/recording-1773489702.mp4`
- duration: `6.5s`
- size: `954540 bytes`
- codec: `h264`
- resolution: `1920x1080`
- frame rate: `30 fps`

Timing summary from the runtime check:

- start shortcut sent at `1773489702`
- pause shortcut sent at `1773489704`
- resume shortcut sent at `1773489706`
- stop shortcut sent at `1773489709`

Interpretation:

- total wall time between start and stop was about `7s`
- recorded clip duration was about `6.5s`
- this is consistent with pause/resume affecting the capture timeline rather than merely changing UI state

## Coverage Summary

### Verified on this machine

- Linux `X11` recording path
- launcher window boot and runtime
- HUD visibility on start/stop
- shortcut-driven start, pause, resume, stop
- real clip creation and metadata
- Linux smoke coverage with and without microphone
- settings/build/test integration at workspace level

### Not fully verified in this pass

- Pure `Wayland-only` runtime end-to-end capture
- Manual click-by-click validation of every launcher button
- Full click-scripted GUI validation of native `Save As` dialog interaction during runtime
- Permissions flows on macOS/Windows
- Real packaging/install test from the newly built `.deb` in this pass

## Findings

### Medium

- `Wayland-only` runtime is still not fully verified end-to-end on this machine because the current session is `X11`, not Wayland.

### Low

- No blocking local issues remain after the latest warning cleanup; remaining gaps are mainly platform coverage rather than failing checks.

## Overall Assessment

For the current local Linux `X11` environment, the project is in good shape:

- automated checks pass
- release no-bundle build passes
- real recording works
- Recent/Save As/Trash command path works on Linux
- HUD lifecycle works
- pause/resume works in runtime, not only in UI state

The primary remaining platform-level gap is full `Wayland-only` runtime verification.
