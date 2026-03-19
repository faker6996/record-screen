# Linux X11 Performance Report

Date: 2026-03-18

Environment:
- Session: X11
- Display: `:1`
- Capture backend: `linux-gstreamer-x11-capture`
- Test binary: `target/debug/deps/smoke-e6ddb535cebeae3a`
- Test method: `linux_smoke_recording_creates_output_file`
- Capture duration target: 8 seconds

Results:

| Preset | Mic | CPU | Max RSS | Output |
| --- | --- | ---: | ---: | --- |
| 720p / 30 fps | Off | 25% | 179564 KB | H.264, 1280x720, 30 fps, 6.70 s, 1505253 bytes |
| 1080p / 30 fps | Off | 30% | 222392 KB | H.264, 1920x1080, 30 fps, 6.73 s, 2650226 bytes |
| 1080p / 60 fps | Off | 48% | 213776 KB | H.264, 1920x1080, 60 fps, 6.70 s, 2609856 bytes |
| 1080p / 30 fps | On | 33% | 217816 KB | H.264 + AAC, 1920x1080, 30 fps, 6.73 s, 2723571 bytes |

Longer stability pass:

| Preset | Duration | CPU | Max RSS | Output |
| --- | ---: | ---: | ---: | --- |
| 1080p / 30 fps | 118.73 s | 28% | 213916 KB | H.264, 1920x1080, 30 fps, 45741886 bytes |
| 1080p / 60 fps | 58.63 s | 70% | 238180 KB | H.264, 1920x1080, 60 fps, 78022444 bytes |
| 1080p / 30 fps | 598.70 s | 26% | 221984 KB | H.264, 1920x1080, 30 fps, 176771873 bytes |

Notes:
- X11 native recording is stable across the tested presets.
- `1080p / 30 fps` is the strongest default tradeoff on this machine.
- `1080p / 60 fps` is still usable, but CPU cost jumps materially.
- Enabling microphone added a small CPU cost and produced a valid AAC audio stream.
- `1080p / 30 fps` remained stable over a 2 minute run.
- `1080p / 30 fps` also remained stable over a 10 minute soak run.
- `1080p / 60 fps` also stayed stable over a 1 minute run, but CPU cost rose sharply.
- This is now strong enough to treat `1080p / 30 fps` as the safe Linux X11 default.
- A 30 minute or 60 minute soak is still the next step if we want stronger long-session confidence.

Next checks worth running:
- 10 minute `1080p / 30 fps` stability test
- 10 minute `1080p / 60 fps` thermal test
- 1440p / 60 fps on dual-monitor workload
- dropped-frame reporting for longer captures
