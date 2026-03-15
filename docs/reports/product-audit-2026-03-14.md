# Product Audit Report

Date: 2026-03-14

## Executive Summary

`Record Screen` da vuot moc scaffold va dang o muc `MVP co duong chay that` tren ca macOS, Windows, va Linux. App da co launcher, HUD, tray, global shortcuts, recent sessions, mic testing, diagnostics runtime, va backend quay that theo tung OS.

Tuy nhien, san pham hien tai van chua dat muc "production-ready cross-platform recorder" vi con 3 khoang trong lon:

1. Linux `Wayland-only` chua quay end-to-end on dinh nhu `X11/XWayland`
2. Workflow sau khi quay van con nhe, chua co export/editor/automation manh
3. Nhieu tinh nang differentiator ma thi truong hien dai dang coi la chuan van chua co

Neu muon nang cap phan mem theo huong hop ly, repo nay nen di theo lane:

- `local-first polished desktop recorder` truoc
- chua nen nha vao `cloud collaboration suite` qua som

Ly do: kien truc hien tai phu hop hon voi app quay va xu ly cuc bo chat luong cao, khong phai mot he thong share-video + cloud comments + workspace tu ngay dau.

## Current State

### Da co trong san pham

- Launcher desktop voi `Record`, `Recent`, `Settings`, `Shortcuts`, `Permissions`
- HUD nho, trong suot, co the keo duoc
- Tray menu va global shortcuts
- Recent sessions:
  - preview/open folder
  - export copy co ban
  - move to Trash / Recycle Bin
  - chon nhieu file va xoa hang loat
- Capture target:
  - full desktop
  - display
  - window, tuy theo OS/session
- custom region tren backend/session duoc ho tro
- Chon microphone input + `Test mic`
- Toggle mix them system audio tren backend/session duoc ho tro
- Runtime diagnostics trong UI
- CI/CD:
  - macOS installer
  - Windows installer
  - Linux package
  - APT publishing
  - Homebrew tap publishing

### Maturity theo tung he dieu hanh

| Platform | Trang thai hien tai | Danh gia |
| --- | --- | --- |
| macOS | `AVFoundation + ffmpeg`, permission flow that, mic select/test, encoder uu tien `h264_videotoolbox`, custom region crop da co | Tot nhat hien tai |
| Windows | `gdigrab + dshow + ffmpeg`, mic diagnostics, multi-encoder selection, window/monitor targeting | Dung duoc, can hardening them |
| Linux X11 | `x11grab + pulse + ffmpeg`, mic test, window discovery | Dung duoc |
| Linux XWayland | co duong quay that qua compatibility path | Dung duoc |
| Linux Wayland-only | da co ScreenCast portal lifecycle client va PipeWire/GStreamer readiness path, nhung chua dat muc on dinh nhu X11 | Chua hoan tat |

## Features Chua Hoan Thien Hoac Chua Phat Trien

### 1. Linux pure Wayland chua xong

Day la khoang trong ky thuat lon nhat neu muon goi day la app quay man hinh da nen tang thuc su.

Tinh trang hien tai:

- da co `ScreenCast portal` lifecycle: `CreateSession`, `SelectSources`, `Start`, `OpenPipeWireRemote`
- da co probing `PipeWire` va `GStreamer`
- da co diagnostics va handoff doc
- nhung `Wayland-only` van chua dat muc quay end-to-end on dinh ngang `X11/XWayland`

### 2. Shortcut remapping da co co ban, nhung chua sau

Da co:

- edit tung shortcut trong UI
- validate syntax va conflict
- luu shortcut tuy chinh
- dang ky lai global shortcut ngay trong runtime

Chua co:

- profile shortcut theo tung OS
- import/export preset
- UX cho disabled bindings

### 3. Launch on login da co path that tren 3 OS, nhung can hardening

Da co:

- Linux: `.desktop autostart`
- macOS: `LaunchAgent`
- Windows: `HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run`

Chua co:

- verify/readback status trong UI
- startup mode tuy chon nhu `silent` hay `tray-only`
- matrix test thuc chien tren nhieu may that

### 4. Export workflow van rat mong

Hien tai app co:

- preview recording
- open folder
- export copy co ban
- trash management

Chua co:

- trim/cut
- transcode preset
- GIF export
- subtitle/caption export
- social preset
- queue export
- batch export

### 5. Audio/encoder/export/telemetry van chua duoc tach thanh subsystem that

Trong workspace van con cac crate placeholder:

- `crates/audio`
- `crates/encoder`
- `crates/export`
- `crates/telemetry`

Thuc te recorder hien tai van dua kha nhieu vao `capture-*` crates va `src-tauri/src/recording.rs`.

Dieu nay co nghia:

- duong chay MVP da co
- nhung kien truc domain cho audio/encoder/export/telemetry van chua duoc day day

### 6. System audio da co toggle va mix tren mot so backend, nhung chua day du

Da co:

- UI audio input khong con goi tat ca la "microphone"
- Linux hien system/loopback monitor sources ro rang
- Windows phan loai `Stereo Mix` / `loopback` / `output` thanh system audio neu ffmpeg expose duoc
- user co the bat `Include system audio`
- Windows va Linux X11/XWayland co the mix `microphone + system audio`
- launcher da guard theo support matrix hien tai

Chua co:

- macOS system audio capture path that
- Linux pure Wayland system-audio mixing tren portal/GStreamer path
- benchmark/QA ky tren nhieu may that
- UX preset cao hon kieu `Best microphone` / `Best system audio`

### 7. Region capture da co mot phan, nhung chua dong deu tren 3 OS

Da co:

- custom region settings trong launcher
- drag-to-select overlay tren man hinh
- custom region target trong recorder khi backend/session hien tai support
- macOS custom region crop qua `AVFoundation + ffmpeg`
- Windows desktop path crop theo `offset_x / offset_y / video_size`
- Linux X11/XWayland crop theo `x11grab + origin + video_size`

Chua co:

- Linux pure Wayland custom region path
- multi-monitor region selector spanning dong thoi tat ca man hinh

### 8. Camera overlay / scene composition chua co

Chua co:

- webcam bubble
- presenter layout
- split layout
- picture-in-picture
- scene composition

### 9. Cursor, click, keystroke, annotation chua co

Day la nhom tinh nang rat quan trong voi app quay tutorial/demo:

- highlight cursor
- click ripple
- zoom around cursor
- keystroke overlay
- drawing / annotation

### 10. AI va collaboration workflow chua co

Chua co:

- transcript
- caption
- AI summary
- chapter generation
- share link workflow
- comments/review online

### 11. Telemetry va crash reporting moi o muc toi thieu

Da co:

- local `runtime.log`
- ghi app launch
- ghi runtime error
- panic hook local

Chua co:

- session performance log
- structured benchmark log
- drop-frame analytics
- support bundle

### 12. Library chua thanh mot media workspace day du

Hien tai `Recent` da huu ich, nhung van la mot library nhe:

- scan thu muc output
- list file gan day
- preview / export copy / trash

Chua co:

- search
- tags
- projects
- pinned recordings
- filters
- metadata index rieng

## Thi Truong Hien Nay Dang Lam Gi

Nhung app quoc te dang tach thanh 2 lane ro rang.

### Lane 1: local-first, polished recording and editing

Tieu bieu:

- [OBS Studio](https://obsproject.com/)
- [Screen Studio](https://www.screen.studio/)

Dac diem:

- local recording manh
- scene/source composition
- camera + screen + audio mix
- visual polish sau khi quay
- plugin/power-user workflows hoac creator-first workflows

### Lane 2: async communication and AI workflow

Tieu bieu:

- [Loom](https://www.loom.com/)
- [Riverside](https://riverside.fm/)
- [Tella](https://www.tella.tv/)

Dac diem:

- quay nhanh de chia se
- link sharing
- comments/review
- transcript/captions
- AI summaries / repurposing
- social/content workflow

## Cac Mau So Chung Tren Thi Truong

Nhung pattern hien dang rat pho bien:

1. `Camera overlay` la tinh nang gan nhu mac dinh
2. `Cursor/click emphasis` rat pho bien trong creator/demo tools
3. `Auto zoom/pan` va motion polish duoc coi la differentiator lon
4. `Transcript + captions + AI summary` dang tro thanh expectation trong lane async/content
5. `Fast share workflow` hoac `fast export workflow` quan trong hon viec chi "quay xong ra file"
6. Linux Wayland support la bai test kha nang ky thuat thuc su cua mot recorder desktop hien dai

## Danh Gia Chien Luoc Cho Repo Nay

Huong di hop ly nhat cho repo nay la:

### Uu tien lane `local-first polished recorder`

Nen xay dung:

- quay on dinh
- performance tot
- review/export tot
- creator UX tot
- camera/cursor/mic/system audio hoan chinh

Khong nen som dan luc vao:

- cloud workspace
- comments online
- web review portal
- full Loom clone

Ly do:

- backend desktop + Rust/Tauri + capture abstractions hien tai rat hop voi san pham local-first
- cloud collaboration se doi hoi mot bai toan san pham va ha tang lon hon nhieu

## Roadmap Nang Cap De Xuat

### Phase 1: Hardening de dat muc recorder da nen tang that su

1. Hoan tat Linux `Wayland-only` end-to-end
2. Harden Windows readiness, mic discovery, va capture edge cases
3. Mo rong system-audio mix va custom-region support cho macOS va Linux pure Wayland
4. Nang telemetry tu local log len benchmark/support bundle that

### Phase 2: Nhu cau creator/pro user can ngay

1. Camera overlay
2. Visual region picker
3. Cursor highlight / click ripple / keystroke overlay
4. Export presets:
   - MP4 quality preset
   - GIF
   - social portrait/landscape
5. Review workflow tot hon:
   - quick trim
   - rename
   - duplicate/export variants

### Phase 3: Media workspace tot hon

1. Search trong library
2. Tags / projects
3. Better preview
4. Metadata index rieng thay vi chi scan thu muc

### Phase 4: AI / collaboration neu thuc su can

1. Transcript
2. Captions
3. AI summary
4. Chapters
5. Clip extraction
6. Cloud/share workflow

## Tinh Nang Nen Them De Nang Cap San Pham

Neu tu duy theo huong "san pham tot hon chu khong chi them cho nhieu", toi de xuat nhung tinh nang nay:

### Nhom 1: co tac dong lon, kha nang ship tot

- Hoan tat system audio + microphone mixing theo tung OS
- Visual region picker + region capture day du
- Camera overlay
- Cursor/click effects
- Crash/support bundle

### Nhom 2: tang gia tri creator rat ro

- Auto hide desktop clutter khi record
- Countdown + quick presets
- Smart file naming
- Multi-export presets
- Quick trim after stop
- Auto open preview after record

### Nhom 3: differentiator tot neu lam dep

- Auto zoom around cursor
- Follow cursor motion
- Presenter layout templates
- Branded export presets
- Local captions from offline model or server-assisted path

## Tinh Nang Chua Nen Lam Ngay

- Full cloud workspace
- Team comments/review platform
- Live streaming suite kieu OBS
- Complex multi-scene broadcasting UI

Nhung huong nay de lam repo mat tap trung khoi mot desktop recorder dep, nhanh, va de ship.

## Ket Luan

`Record Screen` hien da co nen mong rat tot cho mot screen recorder desktop da nen tang, nhung van chua toi muc san pham "hoan thien". Neu chi nhin thang vao value cao nhat cho nguoi dung, thu tu nen uu tien la:

1. Linux Wayland hoan tat
2. System audio + region capture
3. Camera overlay + cursor/click polish
4. Export/review workflow tot hon
5. Sau do moi den transcript/AI/share workflow

Huong nay giu duoc loi the cua codebase hien tai, dong thoi dua san pham den mot muc canh tranh tot hon voi cac app recorder hien dai.
