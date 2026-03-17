# Bao Cao Doc Hieu Du An Record Screen

Ngay cap nhat: 2026-03-13

## 1. Tong quan

`Record Screen` la mot ung dung quay man hinh desktop da nen tang, duoc xay dung bang `Tauri v2 + React 19 + Rust`. Muc tieu cua repo hien tai khong chi la tao mot cua so UI de bat dau quay, ma la tao mot shell desktop hoan chinh gom:

- launcher de cau hinh va dieu khien recorder
- HUD nho gon hien khi dang quay
- global shortcut
- system tray
- flow readiness va permission theo tung he dieu hanh
- backend capture tach rieng theo OS
- pipeline dong goi phat hanh cho macOS, Windows, Linux

Trang thai hien tai la **MVP da co duong chay that** tren cac backend capture theo tung he, nhung chua phai mot san pham da hoan thien o muc production. Kien truc da duoc tach lop kha ro, nhung van con mot so khoang trong nhu persistence that, telemetry, export workflow, launch-on-login that, va kha nang review/chinh sua sau khi quay.

## 2. Muc tieu san pham va triet ly kien truc

Qua README, ADR va roadmap trong `docs/`, repo dang theo cac nguyen tac sau:

- `Rust` la **source of truth** cho recorder state va runtime decisions.
- `React` dong vai tro **control surface**, tap trung vao hien thi va gui lenh.
- `Tauri` quan ly **window lifecycle, tray, global shortcuts, invoke commands, event bridge**.
- Capture backend duoc tach rieng theo tung he dieu hanh de tranh mot lop abstraction "fake cross-platform" qua mong manh.
- Repo da chuan bi san cho nhung pha sau: encoder, export, telemetry, review workflow.

Noi ngan gon: day khong phai mot app web duoc nhung vao desktop, ma la mot desktop shell thuc thu, trong do web UI chi la lop giao tiep.

## 3. Cau truc workspace

Workspace Rust duoc khai bao o `Cargo.toml` cap goc, gom cac thanh phan chinh:

- `src-tauri`: shell desktop, command bridge, tray, windows, runtime orchestration
- `apps/desktop`: giao dien React/Vite
- `crates/app-core`: state machine va snapshot dung chung
- `crates/capture`: type chung cho he capture
- `crates/capture-macos`: backend quay man hinh macOS
- `crates/capture-linux`: backend quay man hinh Linux
- `crates/capture-windows`: backend quay man hinh Windows
- `crates/permissions`: probing/request permission
- `crates/shortcuts`: danh muc action + default shortcut
- `crates/storage`: output path, app settings co ban
- `crates/audio`, `encoder`, `export`, `telemetry`: hien dang la placeholder

Danh gia:

- Tach module tot, de mo rong.
- Phan su dung that hien nay tap trung chu yeu o `src-tauri`, `app-core`, `capture-*`, `permissions`, `storage`, `shortcuts`, `apps/desktop`.
- Cac crate `audio`, `encoder`, `export`, `telemetry` chua co nghiep vu that; hien tai moi tra ve chuoi `capability_summary()`.

## 4. Kien truc runtime tong the

Flow runtime cua app hien tai nhu sau:

1. Tauri khoi dong va tao `AppState`.
2. App dang ky global shortcut plugin.
3. App tao tray, dam bao HUD window ton tai, va dua launcher vao focus.
4. Frontend React goi `get_bootstrap` de lay snapshot ban dau.
5. Mọi thay doi recorder/settings se di qua command Rust.
6. Rust phat event `recorder://state-changed` va `recorder://runtime-error` ve UI.
7. UI cap nhat state cuc bo dua tren snapshot/event do.

Thanh phan trung tam la:

- `src-tauri/src/lib.rs`
- `src-tauri/src/recording.rs`
- `crates/app-core/src/lib.rs`

Trong `AppState` co 2 thanh phan chinh:

- `core: Mutex<AppCore)` de giu state logic
- `recorder: Mutex<Option<Box<dyn CaptureController>>>` de giu backend capture dang chay

Dieu nay cho thay logic duoc tach ro:

- `AppCore` luu state muc san pham
- `CaptureController` lo viec tuong tac voi process/backend quay that

## 5. AppCore: state machine va snapshot trung tam

`crates/app-core/src/lib.rs` la lop model chung cua ung dung.

### 5.1. Cac kieu du lieu chinh

- `RecorderStatus`: `Idle | Recording | Paused`
- `RecorderSnapshot`: snapshot UI can hien
- `BootstrapSnapshot`: snapshot day du cho launcher

`BootstrapSnapshot` chua:

- ten app
- platform
- launcher window label
- snapshot recorder
- app settings
- capture targets
- quality presets
- shortcuts
- permissions
- recent sessions
- roadmap

### 5.2. Trach nhiem

`AppCore`:

- giu `settings`
- giu `status` recorder
- giu target dang quay
- giu active output path
- giu moc thoi gian start/pause/resume
- giu shortcut bindings
- giu recent sessions

### 5.3. Gioi han hien tai

- `settings` chua co persistence that; chu yeu song trong memory.
- `recentSessions` hien la du lieu demo/sample, khong phai lich su that tu disk.
- `launchOnLogin` moi la state trong app, chua thay co implementation OS-level de dang ky startup item.

Day la mot trong nhung khoang trong quan trong nhat neu muon dua app len muc production.

## 6. Shell desktop Tauri

`src-tauri/src/lib.rs` la entrypoint thuc te cua desktop app.

### 6.1. Shortcut toan cuc

Repo dang hardcode 4 action:

- `CmdOrCtrl+Shift+R`: start/stop recording
- `CmdOrCtrl+Shift+P`: pause/resume
- `CmdOrCtrl+Shift+L`: focus launcher
- `CmdOrCtrl+Shift+M`: mute/unmute microphone

Khi shortcut duoc bam, `handle_shortcut_action()` se map sang command recorder/window tuong ung.

### 6.2. Window model

Trong `src-tauri/src/window/mod.rs` co 2 cua so logic:

- `main`: launcher
- `hud`: cua so HUD nho, always-on-top, khong decoration

Close request cua `main` va `hud` khong destroy cua so, ma chi hide. Day la quyet dinh dung voi desktop recorder, vi app can tiep tuc song de global shortcut va tray van hoat dong.

### 6.3. Tray

`src-tauri/src/tray.rs` cung cap:

- Show launcher
- Start/Stop recording
- Pause/Resume
- Toggle microphone
- Show/Hide HUD
- Quit

Day la mot diem manh cua repo, vi tray khong bi coi la phan phu; no da duoc dua vao runtime flow ngay tu dau.

## 7. Recording orchestration

`src-tauri/src/recording.rs` la noi noi command UI/shortcut voi backend capture that.

### 7.1. Start recording

Khi `toggle_recording()` duoc goi trong luc app `Idle`:

- doc `settings` tu `AppCore`
- tao output path bang `storage::next_recording_path`
- tao `RecordingOptions`
- chon backend theo OS (`macos`, `linux`, `windows`)
- start backend capture
- ghi controller vao `AppState.recorder`
- cap nhat state `AppCore`
- emit recorder snapshot moi
- dong bo HUD visibility
- spawn ticker moi giay de phat state update

### 7.2. Stop recording

Khi goi stop:

- lay controller hien tai
- stop backend
- nhan `RecordingArtifact`
- chuyen artifact thanh `CompletedRecording`
- cap nhat recent session trong `AppCore`
- phat event ve UI
- an HUD neu ve `Idle`

### 7.3. Pause / Resume

Pause/resume duoc uy quyen cho tung backend. App chi:

- goi `pause()` hoac `resume()`
- cap nhat `AppCore`
- emit snapshot moi

### 7.4. Runtime error

Ticker moi giay se goi `poll_finished()`. Neu backend bi loi trong luc recording:

- state se duoc day ve `Idle`
- event `recorder://runtime-error` se duoc emit

Do do UI co the hien thong bao su co runtime ma khong can crash app.

## 8. Frontend React

Frontend nam o `apps/desktop`. Kien truc UI la nhe va dung vai tro cua no.

### 8.1. Surface model

`apps/desktop/src/app/App.tsx` render 2 giao dien tu cung mot bundle:

- neu `currentWindowLabel === 'hud'` thi render `HudSurface`
- nguoc lai render launcher day du

Day dung voi ADR da ghi trong repo: cung mot app bundle, nhung surface khac nhau dua vao label cua Tauri window.

### 8.2. useDesktopState

Hook `useDesktopState()` chiu trach nhiem:

- load bootstrap snapshot ban dau
- load current window label
- subscribe event recorder state
- subscribe runtime error
- expose cac action de UI goi

No su dung `startTransition()` de giam tac dong UI khi cap nhat state.

Quan sat quan trong:

- state "that" van o Rust
- React chi merge snapshot/event vao state render

### 8.3. desktopClient

`desktop-client.ts` dong vai tro facade cho IPC.

- Trong Tauri runtime: dung `invoke()` va `listen()`
- Ngoai Tauri runtime: dung `mockSnapshot`

Dieu nay rat huu ich cho viec:

- preview UI tren web
- phat trien giao dien nhanh hon
- debug ma khong can mo Tauri moi lan

### 8.4. Cac panel chinh

#### RecorderPanel

Cho phep:

- xem state quay
- xem elapsed/target/mic
- chon capture target
- start/stop
- pause/resume
- toggle mic

Mot chi tiet dung:

- selector capture target bi disable khi dang record, tranh thay doi target giua session.

#### HudSurface

HUD rat gon:

- elapsed
- mic state
- start/stop
- pause/resume
- shortcut hint

#### PermissionsPanel

Cho phep:

- refresh permission state
- request permission
- open system settings

#### SettingsPanel

Cho phep:

- chon quality preset
- sua output directory
- bat/tat launch on login
- show/hide HUD

### 8.5. Danh gia frontend

Diem tot:

- UI duoc tach thanh feature panel ro rang
- state model thang hang voi snapshot Rust
- co che web preview mock rat thuc dung

Can luu y:

- `App.tsx` hien render `ShortcutPanel` o ca cot trai va cot phai. Neu day khong phai chu y do CSS/responsive, thi de gay trung lap noi dung.

## 9. Permissions model

`crates/permissions/src/lib.rs` gom 2 lop:

- lop chung `PermissionCheck`, `PermissionStatus`
- implementation rieng theo platform

### 9.1. macOS

macOS la noi permission flow day du nhat:

- screen recording probe
- microphone probe
- request screen recording
- request microphone
- mo System Settings dung URL phu hop

### 9.2. Linux

Linux da co `probe_permissions("linux")`. Theo tinh than hien tai cua repo, Linux readiness dang tap trung vao kha nang quay trong moi truong X11, khong phai mot flow permission system giong macOS.

Dieu nay phu hop voi thuc te Linux:

- nhieu distro/window manager khong co permission model thong nhat giong macOS
- neu muon support Wayland day du, ve sau can di sang portal/PipeWire nghiem tuc hon

### 9.3. Windows

Windows hien duoc mo ta o muc default permission checks, chu chua thay request/open-settings flow that tuong duong macOS.

## 10. Capture backend chung

`crates/capture/src/lib.rs` dinh nghia abstraction chung:

- `CaptureTargetOption`
- `RecordingOptions`
- `ActiveRecording`
- `RecordingArtifact`
- trait `CaptureController`
- `CaptureError`

Abstraction nay du de dung chung cho ca ba backend, nhung khong co gang ep moi platform phai giong nhau tuyet doi. Day la mot lua chon hop ly.

## 11. Backend Linux

`crates/capture-linux/src/lib.rs` la backend Linux co muc hoan thien cao.

### 11.1. Cong nghe

- Video: native `GStreamer ximagesrc`
- Audio mic: native `GStreamer pulsesrc` / PipeWire-Pulse runtime
- Pause/Resume: `SIGSTOP` / `SIGCONT`
- Stop: dung process control tren lane native GStreamer

### 11.2. Target support

Linux ho tro:

- full desktop
- tung monitor
- tung window

Monitor lay tu:

- `xrandr --listmonitors`

Window lay tu:

- `xwininfo -root -tree`

Code co loc:

- bo qua cua so qua nho
- bo qua cua so cua chinh app

### 11.3. Han che

- Backend nay la **X11-first**. Neu nguoi dung chay Wayland, duong quay hien tai khong phai huong dung.
- Audio chi la mic `default`, chua co source selection chi tiet.

### 11.4. Chat luong implementation

Backend Linux la mot trong nhung phan an tuong nhat cua repo hien nay vi:

- co target discovery that
- co test parser
- co smoke test backend
- co mapping loi ffmpeg kha ro

## 12. Backend macOS

`crates/capture-macos/src/lib.rs` dung:

- `ffmpeg` + `avfoundation`

### 12.1. Kha nang hien tai

- probe device index bang `ffmpeg -list_devices true`
- quay display + microphone
- pause/resume qua signal
- stop bang stdin `q`
- mapping loi permission/missing ffmpeg kha ro

### 12.2. Han che

- Hien tai duong target discovery cho macOS chua duoc day len layer `capture_targets`.
- O muc app, macOS van duoc coi la `full desktop` la chinh.
- Chua thay implementation chon tung monitor/window cho macOS tu layer UI/runtime hien tai.

Noi cach khac: macOS backend da quay that duoc, nhung ve capture target thi chua dong cap voi Linux/Windows.

## 13. Backend Windows

`crates/capture-windows/src/lib.rs` cho thay repo da co duong code quay man hinh Windows that.

### 13.1. Cong nghe

- Video: `ffmpeg` + `gdigrab`
- Audio mic: `ffmpeg` + `dshow`
- Pause/Resume: goi PowerShell `Suspend-Process` / `Resume-Process`
- Stop: stdin `q`

### 13.2. Target support

Windows backend ho tro:

- full desktop
- tung monitor
- tung window

Kham pha monitor:

- PowerShell + `System.Windows.Forms.Screen`

Kham pha window:

- PowerShell + Windows API `GetWindowRect`

### 13.3. Danh gia

Ve mat code, Windows hien da co muc hoan thien tot hon ky vong cua mot MVP. Tuy nhien, viec repo co code chua dong nghia da duoc runtime-verify rong rai tren may Windows that. Day la khu vuc can uu tien test khi dua ra production.

## 14. Storage va output

`crates/storage/src/lib.rs` dang giai quyet cac bai toan nho nhung thiet thuc:

- expand `~`
- dam bao output directory ton tai
- tao output path moi

Mac dinh:

- folder: `~/Movies/Record Screen`
- filename: `recording-<unix_timestamp>.mp4`

Danh gia:

- Tot cho MVP
- Chua co persistence settings vao file config
- Chua co indexing/scan thu muc de tao recent sessions that

## 15. Shortcuts

`crates/shortcuts/src/lib.rs` chua:

- action id
- label
- accelerator
- description

Day la mot crate nho nhung dung huong, vi shortcut da duoc mo ta nhu mot domain concept ro rang, khong phai string hardcode ngau nhien o khap noi.

## 16. Cac crate placeholder

Hien tai cac crate sau chua co nghiep vu thuc:

- `crates/audio`
- `crates/encoder`
- `crates/export`
- `crates/telemetry`

Tat ca moi chi tra ve mot `capability_summary()` string.

Dieu nay cho thay repo duoc thiet ke san de mo rong, nhung nhung phan sau day **chua ton tai that**:

- audio routing/chon source he thong
- encoder pipeline tach rieng khoi capture
- export/share workflow
- telemetry/diagnostics that

## 17. Packaging, release va phan phoi

Day la mot khu vuc repo da duoc day len kha xa.

### 17.1. Bundle outputs theo OS

Theo `src-tauri/tauri.conf.json` va workflow hien tai:

- macOS: `DMG`
- Windows: `NSIS setup .exe`
- Linux: `.deb`

Repo da khong con di theo huong `AppImage` la mac dinh nua trong flow moi nhat.

### 17.2. Metadata

Bundle config da co:

- icon assets cho macOS/Windows/Linux
- publisher/homepage/license
- Linux deb metadata
- Windows NSIS config

### 17.3. GitHub Actions

Workflow `build-installers.yml` hien tai dang theo huong:

- push branch nao cung build package
- neu la `main` va version moi chua co tag:
  - tao tag `vX.Y.Z`
  - publish GitHub Release trong cung workflow
  - upload `dmg`, `deb`, `exe`
  - sinh `SHA256SUMS.txt`
- neu co secrets GPG:
  - build va deploy APT repository len GitHub Pages

Day la huong dung hon so voi flow tach rieng tag workflow, vi no tranh van de `GITHUB_TOKEN` khong kich hoat workflow tiep theo.

### 17.4. apt install

Hien co 2 muc:

#### Da lam duoc

- `sudo apt install ./record-screen_<version>_amd64.deb`

#### Chua san sang cho moi may

- `sudo apt install record-screen`

Lenh ngan nay chi hoat dong tren may nguoi dung khi:

- APT repo da duoc publish that
- GitHub Pages da bat
- GPG secrets da cau hinh
- nguoi dung da them source list + key vao may

Do do can phan biet ro:

- repo da co **skeleton va workflow** cho APT repo
- nhung APT repo cong khai, ky so va san sang su dung dai tra chua phai mot trang thai mac dinh chac chan

### 17.5. Homebrew

Repo da co:

- script render cask
- template cask
- skeleton `homebrew-tap`
- tai lieu huong dan publish

Tuy nhien:

- chua co tap repo that duoc publish
- chua co release automation de cap nhat cask tu dong

Noi cach khac, Homebrew da duoc chuan bi rat tot, nhung chua hoan tat chuoi phat hanh that.

## 18. Tinh nang nguoi dung hien co

Neu chi nhin o muc app functionality, repo hien tai da co cac nang luc sau:

- mo launcher va HUD
- dieu khien recorder bang shortcut toan cuc
- tray menu hoat dong
- chon quality preset
- doi thu muc output
- bat/tat mic
- bat/tat HUD
- refresh/request/open permission settings
- quay toan bo desktop
- tren Linux va Windows, chon tung display hoac tung window
- finalize file quay thanh mp4

Day la mot MVP kha manh ve phan shell va runtime.

## 19. Nhung khoang trong / rui ro chinh

### 19.1. Persistence chua that

- settings chua save ben vung
- recent sessions chua doc tu lich su that

### 19.2. Launch on login chua thay OS integration

- UI va state da co
- implementation muc he dieu hanh chua ro rang trong code hien tai

### 19.3. Linux van la X11-first

- chua co Wayland/PipeWire path nghiem tuc

### 19.4. macOS capture target chua dong cap voi Linux/Windows

- chua co chon tung monitor/window o layer chung

### 19.5. Cac module hau ky con trong

- export
- encoder
- telemetry
- audio routing

### 19.6. Release automation can van hanh that moi xac nhan duoc

Code workflow da rat day du, nhung gia tri that chi duoc chung minh khi:

- tag/release duoc tao thanh cong
- artifact duoc upload thanh cong
- Pages/apt repo duoc deploy thanh cong

## 20. Danh gia ky thuat tong ket

Day la mot repo co kien truc tot hon muc MVP thong thuong.

Diem manh ro nhat:

- chia domain ro
- Rust giu source of truth dung huong
- Tauri shell duoc su dung dung cach
- Linux backend da co muc "that" kha cao
- packaging/release docs va workflow da duoc nghiem tuc hoa

Day chua la mot san pham hoan chinh, nhung no da vuot qua giai doan "demo UI". Repo hien tai la mot **nen tang recorder desktop nghiem tuc**, trong do phan kho nhat da duoc dat dung cho:

- window model
- tray
- shortcut
- recorder state
- backend abstraction
- installer/release flow

Neu muon dua sang giai doan tiep theo, uu tien hop ly nhat la:

1. them persistence that cho settings va recent sessions
2. xac nhan runtime test Windows/macOS o may that
3. bo sung macOS display/window target hoac xac nhan pham vi support
4. xay Wayland strategy cho Linux
5. hoan thien release publishing va apt repo ngoai doi that
6. them review/export workflow va telemetry toi thieu

## 21. Ket luan ngan

`Record Screen` hien la mot app desktop recorder da co xuong song va huong kien truc dung. Phan san pham da co the test va dong goi, nhung muc "production-ready" van phu thuoc vao:

- persistence
- OS-level polish
- release infrastructure that
- cross-platform runtime verification

Neu nhin tu goc do ky thuat, repo nay dang o trang thai:

**MVP manh, kien truc sach, backend quay da co that, nhung van can mot vong hoan thien de bien thanh san pham phat hanh on dinh.**
