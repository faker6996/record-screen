# End-to-End Test Report 2026-03-16

## Summary

This report captures two different automated test layers that now exist for the macOS development flow:

- launcher GUI smoke tests through Playwright
- native recorder smoke test through the real macOS capture backend

The important distinction is that the GUI suite validates launcher behavior and recorder-state UX, while the native smoke test validates that the real backend can start recording, stop, finalize, and materialize an output file on this machine.

## Commands Run

### GUI layer

- `PATH="/opt/homebrew/bin:$PATH" npm run lint --workspace @record-screen/desktop`
- `PATH="/opt/homebrew/bin:$PATH" npm run build:web`
- `PATH="/opt/homebrew/bin:$PATH" npm run test:e2e:web`

### Native macOS backend layer

- `RUSTFLAGS='-D warnings' cargo test -p capture-macos --tests --no-run`
- `DYLD_FALLBACK_LIBRARY_PATH=/Applications/Xcode.app/Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/lib/swift-5.5/macosx cargo test -p capture-macos --test smoke macos_smoke_full_desktop_recording_creates_output_file -- --ignored --nocapture`
- `DYLD_FALLBACK_LIBRARY_PATH=/Applications/Xcode.app/Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/lib/swift-5.5/macosx cargo test -p capture-macos --test smoke macos_smoke_custom_region_recording_creates_output_file -- --ignored --nocapture`
- `DYLD_FALLBACK_LIBRARY_PATH=/Applications/Xcode.app/Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/lib/swift-5.5/macosx cargo test -p capture-macos --test smoke macos_smoke_full_desktop_with_system_audio_creates_output_file -- --ignored --nocapture`
- `DYLD_FALLBACK_LIBRARY_PATH=/Applications/Xcode.app/Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/lib/swift-5.5/macosx cargo test -p capture-macos --test smoke macos_smoke_full_desktop_with_microphone_creates_output_file -- --ignored --nocapture`
- `RUSTFLAGS='-D warnings' cargo check -p record-screen-desktop`

## Results

### GUI smoke

Files:

- [apps/desktop/playwright.config.ts](/Users/tran_van_bach/Desktop/project/record-screen/apps/desktop/playwright.config.ts)
- [apps/desktop/e2e/recorder-smoke.spec.ts](/Users/tran_van_bach/Desktop/project/record-screen/apps/desktop/e2e/recorder-smoke.spec.ts)

Result:

- `2 passed`

Covered:

- countdown starts
- countdown cancels
- recorder reaches `recording`
- stop enters `finalizing`
- recorder returns to `idle`

### Native macOS backend smoke

File:

- [crates/capture-macos/tests/smoke.rs](/Users/tran_van_bach/Desktop/project/record-screen/crates/capture-macos/tests/smoke.rs)

Result:

- `3 passed`
- `1 blocked candidate in the test harness`

Passing scenarios:

- `full desktop`
- `custom region`
- `full desktop + system audio`

Test file:

- [crates/capture-macos/tests/smoke.rs](/Users/tran_van_bach/Desktop/project/record-screen/crates/capture-macos/tests/smoke.rs)

For each passing scenario, the smoke test:

- starts the real selected macOS backend
- records for roughly 3 seconds
- stops recording
- asserts that the finalized artifact exists
- asserts that the output file is non-empty

Observed passing results:

- the backend started successfully
- the recording stopped successfully
- the output file was created
- the output file had non-zero size

Observed failing candidate:

- `full desktop + microphone`
- this scenario now reaches the native `SCRecordingOutput` lane instead of any older microphone-specific fallback
- the smoke asserts that audio-enabled runs must use the native recording-output lane, and that assertion holds
- the current failing start error is:
  - `SpawnFailed("Failed to start capture: Stream error: The user declined TCCs for application, window, display capture")`
- that means the remaining blocker is now macOS permission/TCC handling for the test binary rather than the older legacy microphone stop bug

## Interpretation

The macOS stack now has automated evidence for both:

- launcher/UI state transitions
- real backend start/stop/finalize behavior on the current machine

This is stronger than the previous state, where the project had unit tests and partial smoke helpers but no explicit macOS end-to-end smoke run that created a real recording artifact.

It also surfaces a real gap instead of masking it: the microphone-enabled macOS smoke path now depends on native-lane TCC approval in the test harness.

## Remaining Gaps

These runs do **not** yet prove full production-hardening:

- long recordings
- multi-monitor target accuracy across repeated runs
- sleep/wake
- permission revoke between runs
- disk-full and output-directory failure during an active recording
- system audio and explicit microphone combinations on the direct native lane
- microphone-enabled native smoke still needs a harness/permission pass because the explicit microphone smoke run is now blocked by native-lane TCC start failure

Those remain tracked in:

- [docs/roadmap/macos-native-backend-qa.md](/Users/tran_van_bach/Desktop/project/record-screen/docs/roadmap/macos-native-backend-qa.md)
