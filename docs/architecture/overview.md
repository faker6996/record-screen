# Architecture Overview

## Boundaries

- `apps/desktop` renders the launcher, settings, and session surfaces.
- `src-tauri` translates Tauri runtime events into app-core actions.
- `crates/app-core` owns recorder state, shortcut presets, and bootstrap snapshots.
- `crates/capture-*` will wrap native APIs for each operating system.
- `crates/encoder` will own muxing and final file creation.

## Runtime Flow

1. Tauri starts and registers window and shortcut handlers.
2. `app-core` builds a bootstrap snapshot for the UI.
3. React requests bootstrap data and renders launcher sections.
4. Commands and shortcut events mutate Rust state.
5. Tauri emits recorder updates back to the UI.

## Current Integrated Scope

- Launcher window and state wiring
- Separate HUD surface with lightweight state path
- Tray menu for launcher recall and recorder actions
- Global shortcut registration
- Persisted shortcut remapping with runtime re-registration
- Shared recorder snapshot model
- Shared backend-selection boundary and backend-registry policy for per-platform recorder runtimes
- Shared audio-backend selection boundary and backend-registry policy for per-platform recorder audio runtimes
- Diagnostics now surface active backend choice and native-candidate availability through that registry
- Diagnostics now surface both the active capture backend and active audio backend path
- macOS native-backend work now has a dedicated `ScreenCaptureKit` module instead of an inline placeholder
- The macOS `ScreenCaptureKit` candidate now also runs a real shareable-content probe for displays, windows, and applications instead of relying only on `sw_vers` version checks
- The macOS `ScreenCaptureKit` start plan can now also resolve a native display candidate label from those probe results, so current planning/logging uses real native target metadata instead of only string target IDs
- The current macOS recording start path now also reuses the `ScreenCaptureKit` stream plan for width/height/fps, so fallback AVFoundation sizing is aligned with the native-capture planning model too
- The macOS lane can now also build a real `SCContentFilter + SCStreamConfiguration` execution plan from the native candidate and write that summary into runtime logs, even though frame streaming still has not been switched over end-to-end
- The macOS lane can now also construct an `SCStream` foundation object from that native filter/config path and record the result in runtime logs, so the migration has touched real `ScreenCaptureKit` runtime objects without switching the recorder over completely yet
- The macOS lane can now also register a real screen output handler on that `SCStream` foundation object and write that prepared-runtime summary into runtime logs, so native runtime setup has progressed past object creation into handler wiring
- The macOS lane now also has a gated smoke lifecycle for `SCStream::start_capture()/stop_capture()`, enabled only when `RECORD_SCREEN_MACOS_SCSTREAM_SMOKE` is set, so native capture lifecycle can be validated explicitly without running on every recording by default
- That macOS smoke lifecycle now also records lightweight Rust-side bridge stats for observed screen/audio samples plus first sample PTS values, so ScreenCaptureKit sample-buffer flow can be inspected without switching the recorder over end-to-end yet
- On macOS 15+ display, monitor, custom-region, microphone, and system-audio capture now all target the direct `SCRecordingOutput` lane instead of the older hybrid or legacy microphone fallback
- Older macOS runtimes now also enumerate capture targets from `ScreenCaptureKit` instead of `ffmpeg -list_devices`, and unsupported native runtime combinations fail explicitly instead of falling back to a legacy ffmpeg path
- macOS microphone selection now also prefers native device discovery, so the launcher no longer depends on `ffmpeg` just to populate the primary microphone combobox
- The macOS app path now also selects only native capture/audio/encoder backends, so unsupported runtimes fail explicitly instead of silently dropping back to an ffmpeg-based runtime
- On macOS 15.0+, the recorder now also has a direct `SCRecordingOutput` lane for display, monitor, and custom-region capture and can request system-audio plus microphone-device-id capture there, so the native migration has started touching real file-output APIs instead of only feeding native frames into the older encoder path
- That direct `SCRecordingOutput` lane now also tracks delegate start/fail/finish state and exposes real `poll_finished()` behavior, so stop/finalize logic no longer relies only on “wait for file to appear”
- The macOS direct lane now also preflights the output path before startup, logs controller-start and finalize timings in `runtime.log`, and reports missing-output / permission / storage-full failures more explicitly for QA and support
- The shared recorder state now also exposes a real `Finalizing` phase, so stop/finalize no longer flashes back to idle while the macOS direct lane is still writing the output file
- That direct `SCRecordingOutput` lane is also surfaced as non-pausing in the recorder state, so HUD/UI/tray/shortcut handling now disables pause/resume there instead of letting users hit a backend error path
- The macOS runtime now also gates that direct `SCRecordingOutput` lane behind real OS-version support and falls back cleanly on older macOS versions, so system-audio/native-output testing no longer depends on accidentally running only on macOS 15+
- The macOS `ScreenCaptureKit` candidate now also exposes a recorder-facing start plan for target and custom-region routing, and that plan is written into runtime support logs when recording starts
- The current macOS recording start path now also consumes that ScreenCaptureKit start plan for resolved source-target selection, so native runtime selection stays aligned with the capture migration model
- The macOS Core Audio candidate now also exposes a recorder-facing audio start plan, and that plan is written into runtime support logs when recording starts
- The current macOS recording start path now also consumes that Core Audio start plan for default-microphone resolution, so the direct native mic lane stays aligned with the native-audio migration model
- Native-audio candidates now also live in dedicated OS modules instead of staying embedded as inline placeholders in the main capture crates
- Those native-audio modules now do lightweight runtime probing so diagnostics can describe real OS readiness instead of only static roadmap intent
- Windows native-audio probing now inspects default, capture, and render endpoints to prepare the WASAPI migration path
- Windows native-audio probing now also resolves preferred capture and render candidates, and the current `Default input` fallback path reuses those candidates when DirectShow enumeration fails
- macOS and Linux native-audio probing now also produce structured device/source reports and preferred candidates, and the current fallback audio summaries/default-input copy reuse those candidates so Phase 2 diagnostics stay consistent across the three OS backends
- Preferred native-audio candidates are now surfaced through runtime diagnostics and shown in the launcher, so Phase 2 progress is visible without reading backend code
- Shared audio runtime diagnostics now carry candidate identifiers as well as labels, so native-audio migrations can move away from pure device-name heuristics without pushing OS-specific types into `src-tauri`
- Windows native-audio probing now stores structured endpoint records (`instance_id + label`) instead of raw name lists, which is the first clean step toward a WASAPI runtime that does not key everything off friendly names
- Windows native-audio probing now also builds a shared runtime plan for preferred capture/render endpoints, and the current DirectShow fallback path reuses that plan instead of re-deriving Windows audio decisions in multiple places
- Windows native-audio probing now also builds a shared route plan for default-input and loopback copy, so launcher messaging and fallback behavior use the same WASAPI candidate model instead of separate Windows-specific heuristics
- Windows native-audio probing now also exposes a recorder-facing runtime intent, so the future WASAPI runtime and the current DirectShow fallback can converge on one model for microphone and loopback routing
- Windows recorder start-up now also consumes a shared audio start plan, so microphone and loopback routing in the active recording path no longer diverge from the WASAPI migration model used by diagnostics/support copy
- Windows runtime logging now also records the shared audio start-plan summary when a recording begins, so support logs can show the intended microphone/loopback route instead of only static backend selection
- Phase 3 is now started behind the same style of boundary: a shared encoder-backend registry exists, each OS has a dedicated native-encoder module plus an ffmpeg fallback encoder module, and diagnostics surface the active encoder path separately from capture/audio paths
- The macOS native-output candidate now also exposes a recorder-facing output plan, and that plan is written into runtime support logs when recording starts
- The current macOS output path now also consumes that native-output plan for resolved codec/preset labeling, so the direct `SCRecordingOutput` lane stays aligned with the native-encoder migration model
- Phase 1 capture diagnostics now also go through shared capture runtime reports instead of hardcoded app-layer copy, so `src-tauri` sees capture/audio/encoder backends through the same style of abstraction
- Phase 4 has now started as well: backend selection for capture/audio/encoder is explained through shared selection notes, so diagnostics/UI can tell the user why a fallback path was chosen instead of only naming the active backend
- Runtime supportability now also records those backend selections and recording lifecycle events into `runtime.log`, so the same explanation visible in the launcher can be inspected after the fact during debugging
- Phase 6 has now started as well: shared runtime snapshots carry backend path, preferred candidates, selection notes, and native-unavailable reasons for capture/audio/encoder, so `src-tauri` diagnostics no longer reassemble that state separately per OS
- The macOS recorder path now also builds one unified runtime plan for capture/audio/encoder, so current fallback execution and native migration planning no longer assemble those three lanes independently
- `src-tauri` diagnostics now consume shared audio-backend runtime reports instead of calling per-platform helper functions directly
- Audio input classification for microphone vs system-audio loopback sources
- Custom region settings, drag-to-select overlay, and target injection on supported backends
- System-audio mix toggle with per-platform support guards
- Real capture backends per OS:
  - macOS: direct `ScreenCaptureKit / SCRecordingOutput` on supported runtimes, with older runtimes failing explicitly instead of dropping back to ffmpeg
  - Windows: `gdigrab + dshow + ffmpeg`
  - Linux X11/XWayland: native `GStreamer ximagesrc + pulsesrc`
- Target preview overlay when choosing a display or custom region
- Runtime diagnostics for active backend path and readiness
- Local runtime crash/error logging
- Cross-platform launch-on-login integration
- Linux Wayland ScreenCast portal / PipeWire readiness and lifecycle client
- Experimental Linux Wayland GStreamer PipeWire runtime path

## Next Integration Targets

- Native backend migration now tracks in `docs/roadmap/native-backend-plan.md`
- The native migration now explicitly includes legacy cleanup and architecture-boundary tightening, not only feature parity
- macOS direct-lane hardening now has an explicit scenario checklist in `/Users/tran_van_bach/Desktop/project/record-screen/docs/roadmap/macos-native-backend-qa.md` instead of relying only on informal notes
- Production-grade Linux pure Wayland capture hardening beyond the current experimental GStreamer PipeWire path
- Windows native capture/audio backend work beyond the current ffmpeg stack
- macOS native encode/system-audio backend work beyond the current ffmpeg runtime
- Richer diagnostics and benchmark telemetry
- Richer export workflow
