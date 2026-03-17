# Native Backend Plan

## Purpose

This roadmap tracks the migration from the current MVP `ffmpeg`-centric runtime to the kind of native recording stack used by mature desktop screen-recording products.

The rule for this document is simple:

- Every phase has a status.
- Every phase has per-platform status.
- A phase moves to `complete` only when its exit criteria are met on the codebase, not when design or scaffolding exists.
- A phase is not considered done if the old temporary path still leaks through the architecture in ways that increase maintenance cost.

## Status Legend

- `complete`: shipped in the repo and usable
- `in_progress`: code exists, but the phase is not complete
- `partial`: meaningful work exists on one or more platforms, but the phase is not yet usable as a product capability
- `not_started`: no meaningful implementation yet
- `blocked`: waiting on a prerequisite or unresolved design/runtime constraint

## Current Baseline

Today the app is a real cross-platform MVP, but some platforms still depend heavily on `ffmpeg`.

- `macOS`: legacy `AVFoundation + ffmpeg`, with macOS 15+ now moving onto direct `ScreenCaptureKit / SCRecordingOutput`
- `Windows`: `gdigrab + dshow + ffmpeg`
- `Linux X11/XWayland`: native `GStreamer ximagesrc + pulsesrc`
- `Linux Wayland-only`: ScreenCast portal / PipeWire lifecycle and experimental runtime path exist, but the path is not yet production-ready

This means the shell, UX, shortcuts, permissions, region selection, target preview, and basic recording flows are already in place, while the next major step is replacing runtime capture/encode dependencies with native per-OS stacks.

## Code Hygiene Policy During Migration

This migration must not turn the repository into a permanent hybrid of native paths, temporary shims, and MVP-era fallback code.

The policy for every implementation phase is:

- new backend work must land behind explicit traits or module boundaries
- temporary fallback paths must be clearly labeled and isolated
- duplicated logic between `ffmpeg` and native implementations must be reduced, not multiplied
- settings, diagnostics, and capability reporting must describe the real runtime path
- once a native path is production-ready for a platform, obsolete MVP-only logic for that platform should be removed or downgraded to a narrow fallback layer

This means "implemented" is not enough. The code must also be cleaner after the phase than before it.

## Phase Summary

| Phase | Goal | Overall Status |
| --- | --- | --- |
| Phase 0 | Stable MVP baseline and migration guardrails | `complete` |
| Phase 1 | Native capture foundations per OS | `partial` |
| Phase 2 | Native audio input and system-audio foundations | `partial` |
| Phase 3 | Native encode/output pipeline | `partial` |
| Phase 4 | Runtime hardening and fallback policy | `partial` |
| Phase 5 | Packaging, distribution, and supportability | `in_progress` |
| Phase 6 | Architecture cleanup and legacy retirement | `partial` |

## Phase 0: Stable MVP Baseline and Guardrails

**Status:** `complete`

### Goal

Lock down the current recorder shell so the native migration does not destabilize the product.

### Deliverables

- recorder state owned by Rust
- launcher, HUD, tray, shortcuts, permissions, and recent sessions integrated
- target preview and drag-select region flow
- runtime diagnostics and phase-aware docs
- per-platform capture crates already separated

### Exit Criteria

- the app can still record with the current MVP backend on supported sessions
- migration work can happen behind the existing `capture-*` crate boundaries

### Platform Status

| Platform | Status | Notes |
| --- | --- | --- |
| macOS | `complete` | MVP backend shipped first via `AVFoundation + ffmpeg`; macOS 15+ mainline recording now targets direct `ScreenCaptureKit / SCRecordingOutput` |
| Windows | `complete` | MVP backend works via `gdigrab + dshow + ffmpeg` |
| Linux | `complete` | MVP backend works on `X11/XWayland`; Wayland-only remains experimental |

## Phase 1: Native Capture Foundations

**Status:** `partial`

### Goal

Replace screen-source acquisition with native capture APIs that match modern desktop recorder architecture.

### Target Stack

- `macOS`: `ScreenCaptureKit`
- `Windows`: `Windows.Graphics.Capture`
- `Linux`: `XDG ScreenCast Portal + PipeWire`

### Deliverables

- a native capture implementation in each `capture-*` crate
- backend selection policy: native first, MVP fallback second
- monitor / window / full-screen target mapping through native APIs
- no runtime dependency on `ffmpeg` just to begin capture session negotiation
- native and fallback capture paths separated behind explicit internal boundaries

### Exit Criteria

- native capture session can start and stream frames on all supported platforms
- target selection maps to real native capture sources
- fallback rules are explicit and diagnosable
- no platform adds new cross-cutting `cfg` sprawl to shared crates just to support the migration

### Platform Status

| Platform | Status | Notes |
| --- | --- | --- |
| macOS | `partial` | Uses native permission and AVFoundation source concepts; backend registry, selection policy, shared capture runtime reports, and a dedicated `ScreenCaptureKit` module now exist; the candidate no longer relies only on `sw_vers`, now runs a real `ScreenCaptureKit` shareable-content probe for displays/windows/applications, and now also feeds the capture-target list so the app no longer depends on `ffmpeg -list_devices` to populate monitor choices; it also exposes a recorder-facing start plan for full-desktop / monitor / custom-region targeting that can resolve a native display candidate label from probe results, carries stream configuration intent (`width/height/fps`), and is used by the current recording start path for resolved source-target selection plus stream sizing; in addition, the macOS lane can now build a real `SCContentFilter + SCStreamConfiguration` execution plan, construct an `SCStream` foundation object from it, prepare a real screen output handler on that stream, and expose a smoke `start_capture()/stop_capture()` lifecycle that can be enabled explicitly for native-lane validation and now records lightweight Rust-side screen/audio sample bridge stats; the recorder now targets a macOS 15+ `SCRecordingOutput` file-output path for display, monitor, and custom-region capture with system-audio plus native microphone-device-id routing, startup handshake checks, delegate-driven finish/error tracking, explicit `Finalizing` state between stop and idle, output-path preflight, startup/finalize timing logs, clearer output-inspection errors, and explicit runtime gating so older macOS versions fail cleanly instead of attempting unsupported native audio/file-output combinations; microphone selection also now comes from native device discovery, the app path now selects only native capture/audio/encoder backends instead of silently dropping back to ffmpeg, and the legacy ffmpeg capture/output compatibility code has been retired from the macOS crate; pause/resume is still unavailable on the direct `SCRecordingOutput` lane, so the app surfaces that capability explicitly in HUD, tray, and shortcut handling instead of pretending it works |
| Windows | `partial` | Backend registry, selection policy, and shared capture runtime reports now exist, but no `Windows.Graphics.Capture` runtime is implemented yet |
| Linux | `partial` | Backend registry, selection policy, and shared capture runtime reports exist; ScreenCast portal lifecycle and PipeWire groundwork exist, but Wayland-only capture is not production-ready |

## Phase 2: Native Audio Foundations

**Status:** `partial`

### Goal

Move microphone and system-audio capture to native OS audio stacks instead of relying on `ffmpeg` device enumeration and `dshow/pulse` runtime glue.

### Target Stack

- `macOS`: native microphone and system-audio path tied to the capture pipeline
- `Windows`: `WASAPI` for default input and loopback
- `Linux`: PipeWire-native audio path

### Deliverables

- default microphone resolution from native APIs
- system-audio capability based on native endpoint support
- stable device switching and unplug/replug handling
- microphone testing backed by native audio availability
- audio selection logic no longer depends on device-name heuristics for primary paths

### Exit Criteria

- `Default input` means the real system default input on all supported OSes
- system-audio support does not depend on ad hoc loopback device naming
- diagnostics can explain exactly which native audio path is active
- microphone and system-audio state do not require MVP-era fallback rules in shared UI logic

### Platform Status

| Platform | Status | Notes |
| --- | --- | --- |
| macOS | `partial` | Audio-backend registry, diagnostics, and a dedicated Core Audio native-audio module with structured device reporting and default input/output candidate summary now exist; those preferred candidates are surfaced through runtime diagnostics, now populate the primary selectable microphone list, and now feed a recorder-facing audio start plan written into runtime support logs and used by the current recording start path for default-microphone resolution; on macOS 15+ the direct `SCRecordingOutput` lane now consumes that native microphone routing, while older runtimes fail cleanly because the app no longer ships a separate ffmpeg-based compatibility path |
| Windows | `partial` | Audio-backend registry, diagnostics, and a dedicated WASAPI native-audio module with default-device plus capture/render-endpoint probing and candidate selection now exist; those preferred candidates are surfaced through runtime diagnostics, carry structured endpoint identity (`instance_id + label`), and now feed shared Windows audio runtime/route plans plus recorder-facing runtime intent and start-plan helpers reused by `Default input` fallback, loopback support summaries, fallback copy, the current recording start path, and runtime support logs, but recording and loopback still depend on DirectShow / ffmpeg rather than WASAPI-native endpoints |
| Linux | `partial` | Audio-backend registry, diagnostics, and a dedicated PipeWire native-audio module with structured source/sink reporting and preferred candidate summary now exist; those preferred candidates are surfaced through runtime diagnostics, reused by the current Linux launcher/runtime selection, and the X11/XWayland mainline recorder path now runs through native GStreamer audio capture instead of an ffmpeg lane; pure Wayland runtime hardening is still incomplete |

The Phase 2 boundary is now explicit in code: `src-tauri` consumes shared audio-backend runtime reports instead of calling OS-specific native-audio helpers directly.
Those shared runtime reports now carry both candidate labels and candidate IDs, so the migration can move toward native endpoint identity without leaking per-platform types into the app layer.

## Phase 3: Native Encode and Output Pipeline

**Status:** `partial`

### Goal

Stop depending on `ffmpeg` as the primary recording runtime by moving file creation and hardware-backed encoding to native OS media stacks.

### Target Stack

- `macOS`: native recording output today, with deeper `AVAssetWriter` separation still planned where applicable
- `Windows`: `Media Foundation`
- `Linux`: GStreamer / PipeWire-friendly encode pipeline, plus hardware acceleration where practical

### Deliverables

- file output written by native encoder/muxer path
- hardware-first codec selection policy
- accurate stop/finalize without shelling out to `ffmpeg`
- the output pipeline is modeled as a first-class backend path instead of ad hoc process orchestration

### Exit Criteria

- at least one production output path per OS does not require `ffmpeg` to record
- stop/finalize path is stable for long recordings
- output metadata and file finalization match current MVP quality bar
- native encode selection is visible and diagnosable without reading backend code

### Platform Status

| Platform | Status | Notes |
| --- | --- | --- |
| macOS | `partial` | Encoder-backend registry, diagnostics, and a dedicated native-output module now exist; the candidate exposes a recorder-facing output plan that is written into runtime support logs and is now consumed by the current output path for resolved codec/preset labeling, and on macOS 15+ file output now targets direct `ScreenCaptureKit / SCRecordingOutput` with no ffmpeg-based mainline path left in the macOS crate |
| Windows | `partial` | Encoder-backend registry, diagnostics, and a dedicated `Media Foundation` native-encoder module now exist, but recording still writes output through ffmpeg |
| Linux | `partial` | Encoder-backend registry, diagnostics, and a dedicated GStreamer native-encoder module now exist, and the X11/XWayland mainline recorder path now writes through the native GStreamer encoder/muxer lane; pure Wayland still needs more runtime hardening before it reaches the same production bar |

## Phase 4: Runtime Hardening and Fallback Policy

**Status:** `partial`

### Goal

Make native backends dependable under the edge cases real users hit every day.

### Deliverables

- device loss / display change handling
- sleep / wake resilience
- fallback matrix from native backend to MVP backend where needed
- diagnostics and logs that identify which path was used and why
- stress coverage for long recordings and rapid start/stop cycles
- a bounded fallback policy so legacy paths do not silently become the default forever

### Exit Criteria

- each OS has a documented fallback policy
- startup, stop, and recovery paths are predictable
- users can understand why a native path was unavailable
- native failure does not leave the architecture in an ambiguous mixed state

### Platform Status

| Platform | Status | Notes |
| --- | --- | --- |
| macOS | `partial` | Shared backend-selection explanations now exist for capture/audio/encoder paths and are surfaced through diagnostics/UI; unsupported pause is now surfaced explicitly in HUD, tray, and shortcut handling for the direct `SCRecordingOutput` lane, and a dedicated hardening checklist now exists in `docs/roadmap/macos-native-backend-qa.md`, but runtime hardening still lacks device-loss, sleep/wake, and production-scale scenario signoff |
| Windows | `partial` | Shared backend-selection explanations now exist for capture/audio/encoder paths and are surfaced through diagnostics/UI, but runtime hardening still lacks device-loss, sleep/wake, and native fallback execution |
| Linux | `partial` | Shared backend-selection explanations now exist for capture/audio/encoder paths and are surfaced through diagnostics/UI, but runtime hardening still lacks stable Wayland-native recovery and explicit device-loss handling |

## Phase 5: Packaging, Distribution, and Supportability

**Status:** `in_progress`

### Goal

Make distribution and support match product expectations once native backends arrive.

### Deliverables

- installer and release flows for the three desktop OSes
- release notes and artifact distribution
- runtime requirements clearly documented
- support matrix kept in sync with actual backend state
- optional bundled media runtime where still needed during migration
- runtime logs capture active capture/audio/encoder backend selection plus recording lifecycle events for support handoff

### Exit Criteria

- each release can describe which backend path is shipping on each OS
- support docs no longer drift away from code
- runtime dependencies are explicit per installer, not tribal knowledge

### Platform Status

| Platform | Status | Notes |
| --- | --- | --- |
| macOS | `in_progress` | DMG build and docs exist |
| Windows | `in_progress` | NSIS build exists, runtime dependency story still needs improvement |
| Linux | `in_progress` | DEB/APT flow exists, Wayland support matrix still evolving |

## Phase 6: Architecture Cleanup and Legacy Retirement

**Status:** `partial`

### Goal

Retire MVP-era recording paths and clean up migration scaffolding so the final codebase is maintainable, not just functional.

### Deliverables

- remove dead or duplicated backend logic once native paths are stable
- collapse temporary compatibility shims that only existed to bridge migration
- make `capture-*` crates internally coherent: `native`, `fallback`, and `shared` submodules where needed
- simplify settings and diagnostics so they describe supported runtime paths, not historical implementation leftovers
- document which fallback code remains and why

### Exit Criteria

- obsolete per-platform `ffmpeg` runtime code is removed where no longer needed
- shared crates no longer carry migration-only conditionals
- each platform backend has a clear ownership model for capture, audio, encode, and fallback
- a new contributor can understand the runtime path from crate boundaries without archaeology

### Platform Status

| Platform | Status | Notes |
| --- | --- | --- |
| macOS | `partial` | Shared runtime snapshots now reduce duplication between backend selection, diagnostics, and support logs, and the current macOS recorder path now builds one runtime plan that feeds capture/audio/encoder planning together, but native/fallback modules are not retired yet |
| Windows | `partial` | Shared runtime snapshots now reduce duplication between backend selection, diagnostics, and support logs, but native/fallback modules are not retired yet |
| Linux | `partial` | Shared runtime snapshots now reduce duplication between backend selection, diagnostics, and support logs, but Wayland/X11 fallback cleanup is still pending |

## Recommended Implementation Order

1. `macOS native capture + encode`
Reason: best return on effort because the repo is already closest to a native stack there.

2. `Windows native capture + audio`
Reason: removes the biggest user-facing dependency issue on Windows and gets rid of the `ffmpeg.exe` requirement for basic recording.

3. `Linux Wayland production path`
Reason: this is the final step required for Linux to feel modern instead of X11-first.

4. `Cross-platform fallback policy and packaging cleanup`
Reason: once real native paths exist, the app needs a clean migration and support story.

5. `Legacy retirement and architecture cleanup`
Reason: the final product should not preserve MVP complexity forever.

## Immediate Next Tasks

- build a `ScreenCaptureKit` backend path in `crates/capture-macos`
- define a backend-selection trait so `native` and `ffmpeg` implementations can coexist temporarily
- add a `Windows.Graphics.Capture` design doc and crate boundary plan
- finish the Linux Wayland runtime path from negotiation into stable PipeWire ingestion
- define per-platform cleanup checkpoints so each native milestone deletes old code instead of only adding new code

## How To Update This Document

When a phase changes status:

1. update the phase table at the top
2. update the per-platform row inside the phase
3. add one concise note to `docs/architecture/overview.md` if the integrated scope changed
4. update `README.md` only when the user-visible support matrix changed
