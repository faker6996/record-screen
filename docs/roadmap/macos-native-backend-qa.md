# macOS Native Backend QA Checklist

This checklist is the hardening gate for the macOS native recorder lanes built on:

- direct `ScreenCaptureKit + SCRecordingOutput`
- multi-display `ScreenCaptureKit + AVAssetWriter` desktop composite

The rule is simple:

- A scenario is not considered covered until it has been run on real hardware.
- A green compile or smoke test is not enough to mark the lane as production-hardened.
- The checklist should be updated when a scenario is verified, regresses, or is intentionally unsupported.

## Current Lane Scope

The current macOS native lanes target:

- `display`
- `monitor`
- `custom region`
- `full desktop` across multiple displays through the desktop-composite lane
- `system audio`
- `microphone device id`

Known limitations:

- `pause/resume` is not available on the direct `SCRecordingOutput` lane because the public API does not expose a real pause/resume primitive.
- The app therefore disables pause in HUD, tray, and shortcut handling when that lane is active.
- `full desktop + microphone + system audio` on the multi-display composite lane currently depends on `ScreenCaptureKit` exposing matching PCM layouts for both audio sources. When the layouts differ, the app fails explicitly instead of mixing invalid audio.

## Runtime Signals To Inspect

Before running any scenario, inspect:

- `runtime.log` in the app config directory
- `capture_backend`, `audio_backend`, and `encoder_backend` selection notes
- `controller_ready_ms` on `recording started`
- `finalize_ms` on `recording finalized`
- `finalizing` recorder state between stop request and idle completion
- `can_pause` and `pause_note`
- any output-path preflight error before startup
- any finalize/output-inspection error that mentions missing output, permissions, or full disk

## Instrumentation Added In Code

The current macOS lane now includes:

- output-path writability preflight before recording starts
- startup timing in `runtime.log`
- finalize timing in `runtime.log`
- explicit `Finalizing` recorder state while `SCRecordingOutput` completes file output
- clearer output inspection errors for:
  - missing finalized file
  - permission-denied output
  - storage-full output

These help QA identify whether a failure happened before capture, during encoder start, or during output finalization.

## Scenario Matrix

### Core lifecycle

- [ ] Start recording and stop normally on a single internal display
- [ ] Start and stop within 1 second
- [ ] Start and stop repeatedly 10 times in a row
- [ ] Stop transitions through `Finalizing` instead of flashing idle before the file is done
- [ ] Record for 30+ minutes without stop/finalize corruption
- [ ] Quit the launcher window while a recording is running
- [ ] Hide/show HUD while a recording is running

### Target routing

- [ ] Full desktop on a Retina internal display
- [ ] Explicit monitor target on a Retina internal display
- [ ] Custom region on a Retina internal display
- [ ] Full desktop on a non-Retina external display
- [ ] Custom region on a non-Retina external display
- [ ] Custom region that crosses UI-heavy content and fast motion
- [ ] Multiple-monitor setup with target on display A while display B is also active
- [ ] Disconnect external monitor after a completed recording and verify next start still works

### Audio routing

- [ ] Video-only recording with no audio enabled
- [ ] System audio only
- [ ] Default microphone only
- [ ] Explicit non-default microphone selection
- [ ] System audio + default microphone together
- [ ] System audio + explicit microphone together
- [ ] Switch microphone selection between runs and verify the recorded source changes

Current observed note:

- automated smoke currently passes for:
  - `full desktop`
  - `custom region`
  - `full desktop + system audio`
- app-path validation on the current machine now passes for:
  - `full desktop · 3 displays`
  - `full desktop · 3 displays + microphone`
  - `full desktop · 3 displays + system audio`
- the multi-display composite lane previously had two real bugs:
  - giant output durations caused by unre-based audio timestamps
  - a composite-audio `EXC_BAD_ACCESS` caused by incorrect Core Media sample-buffer ownership
- both bugs are now fixed and documented in `docs/reports/macos-app-path-validation-2026-03-22.md`.
- the remaining active gap on the current machine is:
  - `full desktop · 3 displays + microphone + system audio`
  - this now fails explicitly only when `ScreenCaptureKit` reports mismatched PCM layouts for the two sources

### Permissions and session changes

- [ ] First run with screen recording permission already granted
- [ ] First run with microphone permission already granted
- [ ] Revoke screen recording permission between runs
- [ ] Revoke microphone permission between runs
- [ ] Start recording, then put the machine to sleep and wake it back up
- [ ] Lock/unlock the screen between runs

### Filesystem and output failure cases

- [ ] Output directory writable and normal
- [ ] Output directory removed between runs
- [ ] Output directory becomes read-only between runs
- [ ] Disk nearly full before start
- [ ] Disk becomes full during recording
- [ ] Existing file collision path still resolves safely

### Recovery and fallback

- [ ] Native direct lane starts and finishes without fallback
- [ ] Native multi-display composite lane starts and finishes without fallback
- [ ] Unsupported pause action is disabled in HUD
- [ ] Unsupported pause action is disabled in tray
- [ ] Unsupported pause shortcut returns a clear runtime error instead of a backend crash
- [ ] Unsupported combination fails explicitly with a clear native-lane selection note

## Exit Bar For "Production-Hardened"

The macOS direct native lane can be called production-hardened only when:

- all core lifecycle scenarios are green on real hardware
- target-routing scenarios are green on both Retina and non-Retina setups
- audio-routing scenarios are green for default and explicit microphone selection
- permission and filesystem failure cases are recoverable with clear runtime diagnostics
- unsupported pause remains explicit and never degrades into a backend error path
- fallback decisions stay visible in diagnostics and `runtime.log`
- multi-display composite output files have sane finalized duration/metadata instead of timeline drift
