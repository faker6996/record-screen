# macOS App-Path Validation 2026-03-22

## Scope

This report captures real macOS app-path validation on the development machine after the native multi-display desktop-composite lane was added and hardened.

The focus of this run was:

- `Full desktop` across 3 displays
- `microphone`
- `system audio`
- composite-lane timing, finalize, and output metadata correctness

## Real App-Path Results

Validated through the actual `record-screen-desktop` app path:

- `Full desktop · 3 displays` video-only: pass
- `Full desktop · 3 displays + microphone`: pass
- `Full desktop · 3 displays + system audio`: pass
- `Full desktop · 3 displays + microphone + system audio`: blocked by mismatched PCM layouts from `ScreenCaptureKit` on this machine

## Composite Audio Fixes Validated

Two macOS composite-lane bugs were fixed and re-tested in the real app path:

1. audio timestamps are now rebased to a zero origin before append
2. retained audio sample buffers now use correct Core Media ownership semantics

Before the fix, finalized files could report durations of many hours and the app could crash in the composite-audio path with `EXC_BAD_ACCESS`.

After the fix, the real app-path recording:

- `/Users/tran_van_bach/Movies/Record Screen/recording-1774170927.mp4`

reported:

- duration: `12.864s`
- media types: `Video`, `Sound`
- codecs: `H.264`, `MPEG-4 AAC`

Audio-sample inspection on that file also showed non-zero signal:

- `samples=290752`
- `non_zero=283308`
- `rms=0.003066`

That means the microphone track was not only present, but contained real signal on the validated run.

## Runtime Notes

Observed runtime metrics from `runtime.log` on the validated microphone run:

- `controller_ready_ms=745`
- `finalize_ms=238`

The app now also surfaces blocked microphone permission earlier through app commands and bootstrap sanitation, instead of letting ScreenCaptureKit fail with a generic TCC error after startup begins.

## Remaining Gap

The remaining multi-display macOS gap is:

- `Full desktop + microphone + system audio` together when `ScreenCaptureKit` exposes different PCM layouts for the two audio sources

The current lane uses strict-format mixing. When the layouts do not match, the app now fails explicitly instead of recording incorrect audio.

## Conclusion

As of this validation run, the macOS native stack is confirmed on the current machine for:

- single display
- custom region
- multi-display full desktop
- multi-display full desktop + microphone
- multi-display full desktop + system audio

The next macOS-specific hardening step is format conversion or resampling for the strict-format dual-audio composite case.
