import { useEffect } from 'react'
import { HudSurface } from '../features/launcher/components/HudSurface'
import { PermissionsPanel } from '../features/permissions/components/PermissionsPanel'
import { RecorderPanel } from '../features/recorder/components/RecorderPanel'
import { RecentSessionsPanel } from '../features/library/components/RecentSessionsPanel'
import { SettingsPanel } from '../features/settings/components/SettingsPanel'
import { ShortcutPanel } from '../features/settings/components/ShortcutPanel'
import { useDesktopState } from '../hooks/use-desktop-state'

function LoadingState() {
  return (
    <main className="app-shell loading-shell">
      <section className="panel loading-panel">
        <p className="eyebrow">Record Screen</p>
        <h1>Preparing launcher</h1>
        <p>Loading recorder state, shortcuts, and first-run readiness checks.</p>
      </section>
    </main>
  )
}

function ErrorState({ message }: { message: string }) {
  return (
    <main className="app-shell loading-shell">
      <section className="panel loading-panel">
        <p className="eyebrow">Launcher error</p>
        <h1>Unable to load desktop state</h1>
        <p>{message}</p>
      </section>
    </main>
  )
}

export default function App() {
  const {
    currentWindowLabel,
    error,
    focusLauncher,
    hideHud,
    isLoading,
    pauseResume,
    resetShortcuts,
    showHud,
    snapshot,
    toggleMicrophone,
    toggleRecording,
    updateLaunchOnLogin,
    updateOutputDirectory,
    updateQualityPreset,
  } = useDesktopState()

  useEffect(() => {
    document.body.dataset.surface = currentWindowLabel
    return () => {
      delete document.body.dataset.surface
    }
  }, [currentWindowLabel])

  if (isLoading || !snapshot) {
    return <LoadingState />
  }

  if (error) {
    return <ErrorState message={error} />
  }

  if (currentWindowLabel === 'hud') {
    return (
      <HudSurface
        onPauseResume={pauseResume}
        onToggleRecording={toggleRecording}
        recorder={snapshot.recorder}
      />
    )
  }

  return (
    <main className="app-shell">
      <section className="hero-strip">
        <div>
          <p className="eyebrow">Cross-platform recorder</p>
          <h1>{snapshot.appName}</h1>
          <p className="hero-copy">
            A compact launcher for rapid capture, global shortcuts, and a Rust
            core that will own the recording pipeline.
          </p>
        </div>
        <div className="hero-badges">
          <span className="badge">Platform: {snapshot.platform}</span>
          <span className="badge">Window: {snapshot.launcherWindowLabel}</span>
        </div>
      </section>

      <div className="grid-layout">
        <div className="primary-column">
          <RecorderPanel
            onPauseResume={pauseResume}
            onToggleMicrophone={toggleMicrophone}
            onToggleRecording={toggleRecording}
            recorder={snapshot.recorder}
          />
          <ShortcutPanel
            onFocusLauncher={focusLauncher}
            onReset={resetShortcuts}
            shortcuts={snapshot.shortcuts}
          />
          <SettingsPanel
            onHideHud={hideHud}
            onShowHud={showHud}
            onUpdateLaunchOnLogin={updateLaunchOnLogin}
            onUpdateOutputDirectory={updateOutputDirectory}
            onUpdateQualityPreset={updateQualityPreset}
            qualityPresets={snapshot.qualityPresets}
            settings={snapshot.settings}
          />
        </div>

        <div className="secondary-column">
          <PermissionsPanel permissions={snapshot.permissions} />
          <RecentSessionsPanel
            roadmap={snapshot.roadmap}
            sessions={snapshot.recentSessions}
          />
        </div>
      </div>
    </main>
  )
}
