# macOS Native Backend QA Checklist

This checklist is the hardening gate for the direct macOS native lane built on `ScreenCaptureKit + SCRecordingOutput`.

The rule is simple:

- A scenario is not considered covered until it has been run on real hardware.
- A green compile or smoke test is not enough to mark the lane as production-hardened.
- The checklist should be updated when a scenario is verified, regresses, or is intentionally unsupported.

## Current Lane Scope

The direct native lane currently targets:

- `display`
- `monitor`
- `custom region`
- `system audio`
- `microphone device id`

Known limitation:

- `pause/resume` is not available on the direct `SCRecordingOutput` lane because the public API does not expose a real pause/resume primitive.
- The app therefore disables pause in HUD, tray, and shortcut handling when that lane is active.

## Runtime Signals To Inspect

Before running any scenario, inspect:

- `runtime.log` in the app config directory
- `capture_backend`, `audio_backend`, and `encoder_backend` selection notes
- `capture_start_plan`, `capture_execution_plan`, `capture_runtime_foundation`, `capture_prepared_runtime`
- `audio_start_plan`
- `encoder_start_plan`
- `can_pause` and `pause_note`

## Scenario Matrix

### Core lifecycle

- [ ] Start recording and stop normally on a single internal display
- [ ] Start and stop within 1 second
- [ ] Start and stop repeatedly 10 times in a row
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
- [ ] Unsupported pause action is disabled in HUD
- [ ] Unsupported pause action is disabled in tray
- [ ] Unsupported pause shortcut returns a clear runtime error instead of a backend crash
- [ ] Unsupported combination falls back to the legacy path with a clear selection note

## Exit Bar For "Production-Hardened"

The macOS direct native lane can be called production-hardened only when:

- all core lifecycle scenarios are green on real hardware
- target-routing scenarios are green on both Retina and non-Retina setups
- audio-routing scenarios are green for default and explicit microphone selection
- permission and filesystem failure cases are recoverable with clear runtime diagnostics
- unsupported pause remains explicit and never degrades into a backend error path
- fallback decisions stay visible in diagnostics and `runtime.log`
