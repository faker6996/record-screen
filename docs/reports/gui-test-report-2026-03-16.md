# GUI Test Report 2026-03-16

## Scope

This report covers the new automated GUI smoke-test layer added for the desktop launcher UI on macOS development machines.

The current automation target is the React launcher/web surface, not the native Tauri shell itself. That means these tests validate recorder UI flow and state transitions in the mocked desktop client path, while native capture/output still requires separate manual QA on macOS hardware.

## Commands Run

- `PATH="/opt/homebrew/bin:$PATH" npm run lint --workspace @record-screen/desktop`
- `PATH="/opt/homebrew/bin:$PATH" npm run build:web`
- `PATH="/opt/homebrew/bin:$PATH" npm run test:e2e:web`
- `RUSTFLAGS='-D warnings' cargo check -p record-screen-desktop`

## Automated GUI Results

### Playwright smoke suite

File:

- [apps/desktop/e2e/recorder-smoke.spec.ts](/Users/tran_van_bach/Desktop/project/record-screen/apps/desktop/e2e/recorder-smoke.spec.ts)

Result:

- `2 passed`

Covered scenarios:

- countdown starts with `3 2 1`
- countdown can be cancelled before recording begins
- recorder enters `recording`
- stop transitions through `finalizing`
- recorder returns to `idle` after finalization completes

### Build and static checks

Result:

- desktop lint: pass
- web build: pass
- Rust desktop check with `-D warnings`: pass

## Supporting Test Harness Added

- [apps/desktop/playwright.config.ts](/Users/tran_van_bach/Desktop/project/record-screen/apps/desktop/playwright.config.ts)
- [apps/desktop/src/services/desktop-client.ts](/Users/tran_van_bach/Desktop/project/record-screen/apps/desktop/src/services/desktop-client.ts)
- [apps/desktop/src/features/recorder/components/RecorderPanel.tsx](/Users/tran_van_bach/Desktop/project/record-screen/apps/desktop/src/features/recorder/components/RecorderPanel.tsx)

Key points:

- Playwright now boots the Vite app automatically.
- Web-preview mock state now emits recorder-state updates so GUI tests can observe `recording -> finalizing -> idle`.
- Recorder UI has stable test hooks via `data-testid`.

## What This Does Not Prove Yet

- native Tauri shell window behavior on macOS
- real `ScreenCaptureKit` capture success
- real file output/finalization timing in native runtime
- multi-monitor native target routing
- permission revoke / sleep-wake / disk-full behavior

Those remain covered by the manual QA checklist in:

- [docs/roadmap/macos-native-backend-qa.md](/Users/tran_van_bach/Desktop/project/record-screen/docs/roadmap/macos-native-backend-qa.md)

## Current Assessment

The launcher-level GUI path is now automatically testable and green for the main recorder interaction loop. This is a meaningful improvement because regressions in countdown, start/stop UX, and `Finalizing` state can now be caught automatically before manual macOS runtime QA.
