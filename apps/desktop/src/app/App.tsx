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

  const pendingPermissions = snapshot.permissions.filter(
    (permission) => permission.status === 'pending',
  ).length

  return (
    <main className="app-shell">
      <section className="app-header">
        <div>
          <p className="eyebrow">Cross-platform recorder</p>
          <h1>{snapshot.appName}</h1>
          <p className="hero-copy">
            Start a recording in one move, keep setup obvious, and pull the
            launcher back instantly with shortcuts when it is hidden.
          </p>
        </div>
        <div className="hero-summary">
          <article className="summary-card">
            <span className="metric-label">Current state</span>
            <strong>{snapshot.recorder.elapsedLabel}</strong>
          </article>
          <article className="summary-card">
            <span className="metric-label">Ready checks</span>
            <strong>
              {pendingPermissions === 0
                ? 'All clear'
                : `${pendingPermissions} still pending`}
            </strong>
          </article>
          <article className="summary-card">
            <span className="metric-label">Quick recall</span>
            <strong>CmdOrCtrl + Shift + L</strong>
          </article>
        </div>
      </section>

      <div className="launch-layout">
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
          <ShortcutPanel
            onFocusLauncher={focusLauncher}
            onReset={resetShortcuts}
            shortcuts={snapshot.shortcuts}
          />
        </div>
      </div>

      <div className="support-layout">
        <div className="support-card">
          <RecentSessionsPanel
            sessions={snapshot.recentSessions}
          />
        </div>
        <div className="support-card plan-card">
          <section className="panel panel-stack">
            <div className="panel-header">
              <div>
                <p className="eyebrow">What comes next</p>
                <h2>Build plan already wired into the repo</h2>
              </div>
            </div>
            <ul className="roadmap-list">
              {snapshot.roadmap.map((item) => (
                <li key={item}>{item}</li>
              ))}
            </ul>
          </section>
        </div>
      </div>
    </main>
  )
}
