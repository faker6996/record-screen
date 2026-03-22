# Native Backend Plan

## Purpose

This roadmap tracks the migration from the original MVP shell-process runtime to the kind of native recording stack used by mature desktop screen-recording products.

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

Today the app is a real cross-platform recorder with native mainline recorder paths on the three desktop OSes, but parts of the historical roadmap still describe older migration checkpoints.

- `macOS`: legacy `AVFoundation + external-process runtime`, with macOS 15+ now moving onto direct `ScreenCaptureKit / SCRecordingOutput`
- `Windows`: native `Windows.Graphics.Capture + Media Foundation + WASAPI`
- `Linux X11/XWayland`: native `GStreamer ximagesrc + pulsesrc`
- `Linux Wayland-only`: ScreenCast portal / PipeWire lifecycle and native GStreamer runtime path exist, but the path is not yet production-ready

This means the shell, UX, shortcuts, permissions, region selection, target preview, and basic recording flows are already in place, while the next major step is replacing runtime capture/encode dependencies with native per-OS stacks.

## Code Hygiene Policy During Migration

This migration must not turn the repository into a permanent hybrid of native paths, temporary shims, and MVP-era fallback code.

The policy for every implementation phase is:

- new backend work must land behind explicit traits or module boundaries
- temporary fallback paths must be clearly labeled and isolated
- duplicated logic between legacy process runtimes and native implementations must be reduced, not multiplied
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
| macOS | `complete` | MVP backend shipped first via `AVFoundation + an external process runtime`; macOS 15+ mainline recording now targets direct `ScreenCaptureKit / SCRecordingOutput` |
| Windows | `complete` | MVP backend originally shipped via legacy desktop/audio shell tooling; the current mainline recorder path is now native |
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
- no runtime dependency on an external shell recorder just to begin capture session negotiation
- native and fallback capture paths separated behind explicit internal boundaries
- minimal `Windows.Graphics.Capture` runtime lifecycle scaffold (`execution_plan`, `runtime_foundation`, `prepared_runtime`) with bounded object construction, summary output, `FrameArrived` handling, and smoke `StartCapture`/`StopCapture` validation
- Windows capture smoke now records metadata from `Direct3D11CaptureFrame` including latest `d3d11-texture2d` / `dxgi-surface` / `direct3d-surface` surface kind, latest width/height, latest system-relative time (100ns), and frame count/saw-frame state

### Exit Criteria

- native capture session can start and stream frames on all supported platforms
- target selection maps to real native capture sources
- fallback rules are explicit and diagnosable
- no platform adds new cross-cutting `cfg` sprawl to shared crates just to support the migration

### Platform Status

| Platform | Status | Notes |
| --- | --- | --- |
| macOS | `partial` | Uses native permission and `ScreenCaptureKit` concepts end-to-end; backend registry, selection policy, shared capture runtime reports, and a dedicated `ScreenCaptureKit` module now exist; the candidate now runs a real shareable-content probe for displays/windows/applications and feeds the capture-target list so the app no longer depends on an older device-listing helper to populate monitor choices. Display, monitor, and custom-region capture on macOS 15+ now use direct `SCRecordingOutput` with startup handshake checks, delegate-driven finish/error tracking, explicit `Finalizing` state, output-path preflight, startup/finalize timing logs, clearer output-inspection errors, and explicit runtime gating so older macOS versions fail cleanly. Full-desktop capture on multi-display setups now routes through a dedicated native desktop-composite lane built from multiple `SCStream` inputs plus `AVAssetWriter`, so `Full desktop` no longer silently records only the first display. That composite lane now supports video-only, microphone-only, and system-audio-only runs across multiple displays, and it can attempt `microphone + system audio` together when `ScreenCaptureKit` exposes matching PCM layouts. Audio timestamps are now normalized before append and retained sample buffers use correct Core Media ownership semantics, fixing the previous giant-duration metadata bug and composite-audio crash. Pause/resume is still unavailable on the direct `SCRecordingOutput` lane, so the app surfaces that capability explicitly in HUD, tray, and shortcut handling instead of pretending it works. |
| Windows | `partial` | Backend registry, selection policy, and shared capture runtime reports now exist; the `Windows.Graphics.Capture` candidate and target-resolution logic now also live in a dedicated native-capture module. That module supports `execution_plan`, `runtime_foundation`, `prepared_runtime`, smoke-lifecycle summaries, an integrated bridge smoke, and a native-first controller path. It builds `GraphicsCaptureItem`, `ID3D11Device`, `Direct3D11CaptureFramePool`, and `GraphicsCaptureSession`, registers `FrameArrived` for scaffold/smoke validation, reads `Direct3D11CaptureFrame` metadata without copying texture bytes, captures latest content width/height, latest system-relative time in 100ns units, frame counter/saw-frame state, and latest surface kind (`d3d11-texture2d`, `dxgi-surface`, `direct3d-surface`), and can now poll frames into a `Media Foundation` sink-writer controller with explicit cleanup/finalize handling. That controller also starts `WASAPI` audio workers, drains raw PCM packets, writes them into an audio stream on the same `Media Foundation` sink writer for microphone, default loopback, or strict-format `mic + loopback` mixing, crops `custom region` frames natively before writing them, and composes multi-monitor full-desktop capture natively into one texture before encoding. App-facing capture/audio/encoder selection on Windows now points only at the native stack, and the mainline Windows recorder path no longer depends on the old shell-based recorder flow. |
| Linux | `partial` | Backend registry, selection policy, and shared capture runtime reports exist; `X11/XWayland` mainline capture now runs through the native GStreamer lane and no longer depends on the legacy ffmpeg runtime, while pure Wayland still needs more runtime hardening |

## Phase 2: Native Audio Foundations

**Status:** `partial`

### Goal

Move microphone and system-audio capture to native OS audio stacks instead of relying on shell-driven device enumeration and MVP-era audio glue.

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
| macOS | `partial` | Audio-backend registry, diagnostics, and a dedicated Core Audio native-audio module with structured device reporting and default input/output candidate summary now exist; those preferred candidates are surfaced through runtime diagnostics, populate the primary selectable microphone list, and feed a recorder-facing audio start plan written into runtime support logs and used by the current recording start path for default-microphone resolution. On macOS 15+ the direct `SCRecordingOutput` lane now consumes that native microphone routing for display/monitor/custom-region capture, while the multi-display full-desktop composite lane can now write microphone-only and system-audio-only native audio across all active displays. The remaining audio gap on macOS is the multi-display `microphone + system audio` case when `ScreenCaptureKit` returns incompatible PCM layouts and the app has to fail explicitly instead of resampling. |
| Windows | `partial` | Audio-backend registry, diagnostics, and a dedicated WASAPI native-audio module with default-device plus capture/render-endpoint probing and candidate selection now exist; those preferred candidates are surfaced through runtime diagnostics, carry structured endpoint identity (`instance_id + label`), and now feed shared Windows audio runtime/route plans plus recorder-facing runtime intent and start-plan helpers reused by launcher audio lists and runtime support logs. The native module now also has a real WASAPI runtime foundation and a worker-style smoke lifecycle for default microphone / loopback (`IMMDeviceEnumerator -> IMMDevice -> IAudioClient -> IAudioCaptureClient`, with `Start/Stop`, `GetNextPacketSize`, and `GetBuffer/ReleaseBuffer` packet polling plus silent-packet/frame counters), so app-facing audio discovery/reporting is native-first and the `Windows WASAPI` backend now reports `Available` when runtime probing succeeds. The Windows native controller now routes supported recorder cases through `Windows.Graphics.Capture + Media Foundation + WASAPI`, including strict-format `mic + loopback` mixing, native custom-region crop, and native multi-monitor full-desktop composition before encode. App-facing encoder selection/runtime reports now also point at `Media Foundation` instead of exposing a separate legacy encoder backend. |
| Linux | `partial` | Audio-backend registry, diagnostics, and a dedicated PipeWire native-audio module with structured source/sink reporting and preferred candidate summary now exist; those preferred candidates are surfaced through runtime diagnostics, reused by the current Linux launcher/runtime selection, the X11/XWayland mainline recorder path now runs through native GStreamer audio capture, and Linux mic testing now uses the native GStreamer probe path; pure Wayland runtime hardening is still incomplete |

The Phase 2 boundary is now explicit in code: `src-tauri` consumes shared audio-backend runtime reports instead of calling OS-specific native-audio helpers directly.
Those shared runtime reports now carry both candidate labels and candidate IDs, so the migration can move toward native endpoint identity without leaking per-platform types into the app layer.

## Phase 3: Native Encode and Output Pipeline

**Status:** `partial`

### Goal

Stop depending on the older shell-process recording runtime by moving file creation and hardware-backed encoding to native OS media stacks.

### Target Stack

- `macOS`: native recording output today, with deeper `AVAssetWriter` separation still planned where applicable
- `Windows`: `Media Foundation`
- `Linux`: GStreamer / PipeWire-friendly encode pipeline, plus hardware acceleration where practical

### Deliverables

- file output written by native encoder/muxer path
- hardware-first codec selection policy
- accurate stop/finalize without shelling out to an external recorder process
- the output pipeline is modeled as a first-class backend path instead of ad hoc process orchestration
- Windows encoder phase now includes native Media Foundation sink-writer foundation scaffolding for smoke validation: `MFStartup`, sink-writer creation, media-type registration, and `BeginWriting`/`Finalize` state transitions captured in a runtime summary

### Exit Criteria

- at least one production output path per OS does not require the older shell-process recorder to record
- stop/finalize path is stable for long recordings
- output metadata and file finalization match current MVP quality bar
- native encode selection is visible and diagnosable without reading backend code

### Platform Status

| Platform | Status | Notes |
| --- | --- | --- |
| macOS | `partial` | Encoder-backend registry, diagnostics, and a dedicated native-output module now exist; the candidate exposes a recorder-facing output plan that is written into runtime support logs and is consumed by the current output path for resolved codec/preset labeling. On macOS 15+ display/monitor/custom-region file output now targets direct `ScreenCaptureKit / SCRecordingOutput`, while multi-display full-desktop capture uses a separate native `AVAssetWriter` desktop-composite path. The composite lane now normalizes audio timing before append and has fixed Core Media sample-buffer ownership, so finalized output metadata no longer drifts into many-hour durations and the real app path no longer crashes in the microphone composite path. |
| Windows | `partial` | Encoder-backend registry, diagnostics, and a dedicated `Media Foundation` native-encoder module now exist beyond placeholder state; output-plan summary and a runtime-foundation smoke path are now available and include `MFStartup`, sink-writer creation, media-type initialization, and `BeginWriting`/`Finalize` checks. The module now also carries an explicit sample-bridge plan for turning `Windows.Graphics.Capture` surfaces into `IMFSample` objects (`MFCreateVideoSampleFromSurface` for `ID3D11Texture2D`, `MFCreateDXGISurfaceBuffer + MFCreateSample` for `IDXGISurface`) and can now also create an optional audio stream that accepts raw PCM packets from `WASAPI`. The active Windows recorder path now points at `Media Foundation` on the native lane instead of exposing a separate legacy encoder backend. |
| Linux | `partial` | Encoder-backend registry, diagnostics, and a dedicated GStreamer native-encoder module now exist, and the X11/XWayland mainline recorder path now writes through the native GStreamer encoder/muxer lane with real long-run X11 soak validation; pure Wayland still needs more runtime hardening before it reaches the same production bar |

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
| macOS | `partial` | Shared backend-selection explanations now exist for capture/audio/encoder paths and are surfaced through diagnostics/UI; unsupported pause is surfaced explicitly in HUD, tray, and shortcut handling for the direct `SCRecordingOutput` lane; blocked microphone permission now fails early with explicit UI/runtime guidance instead of opaque TCC starts; and a dedicated hardening checklist now exists in `docs/roadmap/macos-native-backend-qa.md`. Remaining work is concentrated in device-loss/sleep-wake hardening, production-scale signoff, and format-conversion/resample support for multi-display `microphone + system audio` composite runs. |
| Windows | `partial` | Shared backend-selection explanations now exist for capture/audio/encoder paths and are surfaced through diagnostics/UI, but runtime hardening still lacks device-loss, sleep/wake, and real-hardware validation of the native multi-monitor/audio stack |
| Linux | `partial` | Shared backend-selection explanations now exist for capture/audio/encoder paths and are surfaced through diagnostics/UI; X11 native recording is now benchmarked and soak-tested, but runtime hardening still lacks stable Wayland-native recovery and explicit device-loss handling |

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
| Windows | `in_progress` | NSIS build exists; packaging, support copy, and hardware validation still need to catch up with the native runtime now shipping in code |
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

- obsolete per-platform shell-process runtime code is removed where no longer needed
- shared crates no longer carry migration-only conditionals
- each platform backend has a clear ownership model for capture, audio, encode, and fallback
- a new contributor can understand the runtime path from crate boundaries without archaeology

### Platform Status

| Platform | Status | Notes |
| --- | --- | --- |
| macOS | `partial` | Shared runtime snapshots now reduce duplication between backend selection, diagnostics, and support logs; the current macOS recorder path now builds one runtime plan that feeds capture/audio/encoder planning together; the legacy compatibility recorder path has been retired from the macOS crate; and the remaining cleanup is concentrated in simplifying the newer direct-vs-composite native split and removing migration-only wording/docs. |
| Windows | `partial` | Shared runtime snapshots now reduce duplication between backend selection, diagnostics, and support logs, and the Windows capture crate plus permissions path no longer carry the old shell-based runtime/controller code; broader repo-level cleanup and real-hardware validation are still pending |
| Linux | `partial` | Shared runtime snapshots now reduce duplication between backend selection, diagnostics, and support logs, and the legacy ffmpeg runtime has been retired from the Linux mainline recorder path; the remaining cleanup is concentrated in the unfinished pure-Wayland hardening path |

## Recommended Implementation Order

1. `macOS native capture + encode`
Reason: best return on effort because the repo is already closest to a native stack there.

2. `Windows native capture + audio`
Reason: consolidates Windows onto one native recorder stack instead of maintaining a separate MVP-era runtime.

3. `Linux Wayland production path`
Reason: this is the final step required for Linux to feel modern instead of X11-first.

4. `Cross-platform fallback policy and packaging cleanup`
Reason: once real native paths exist, the app needs a clean migration and support story.

5. `Legacy retirement and architecture cleanup`
Reason: the final product should not preserve MVP complexity forever.

## Immediate Next Tasks

- validate the macOS multi-display native stack on real hardware for the remaining dual-audio case: `full desktop + microphone + system audio` across differing ScreenCaptureKit PCM layouts
- add format-conversion / resample support to the macOS multi-display composite lane so dual-audio runs are not blocked by strict-format mixing
- validate the Windows native stack on real hardware: monitor/window/custom-region/full-desktop, microphone, loopback, and `mic + loopback` across single-monitor and multi-monitor sessions
- harden the Windows native lifecycle for device-loss, sleep/wake, and start/stop stress cases now that the old shell-process runtime is retired from the active Windows path
- finish the Linux Wayland runtime path from negotiation into stable PipeWire ingestion
- document the current Linux X11 performance baseline and keep the support matrix aligned with it
- define per-platform cleanup checkpoints so each native milestone deletes old code instead of only adding new code

## How To Update This Document

When a phase changes status:

1. update the phase table at the top
2. update the per-platform row inside the phase
3. add one concise note to `docs/architecture/overview.md` if the integrated scope changed
4. update `README.md` only when the user-visible support matrix changed
