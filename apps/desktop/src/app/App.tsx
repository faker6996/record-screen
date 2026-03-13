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
    <main className="launcher launcher--loading">
      <section className="panel launcher__state-panel">
        <p className="eyebrow">Record Screen</p>
        <h1>Preparing launcher</h1>
        <p>Loading recorder state, shortcuts, and first-run readiness checks.</p>
      </section>
    </main>
  )
}

function ErrorState({ message }: { message: string }) {
  return (
    <main className="launcher launcher--loading">
      <section className="panel launcher__state-panel">
        <p className="eyebrow">Launcher error</p>
        <h1>Unable to load desktop state</h1>
        <p>{message}</p>
      </section>
    </main>
  )
}

export default function App() {
  const {
    actionError,
    currentWindowLabel,
    error,
    focusLauncher,
    hideHud,
    isLoading,
    openPermissionSettings,
    pauseResume,
    refreshPermissions,
    resetShortcuts,
    requestPermission,
    showHud,
    snapshot,
    toggleMicrophone,
    toggleRecording,
    updateCaptureTarget,
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
    <main className="launcher">
      <section className="launcher__header">
        <div className="launcher__intro">
          <p className="eyebrow">Cross-platform recorder</p>
          <h1>{snapshot.appName}</h1>
          <p className="launcher__copy">
            Start a recording in one move, keep setup obvious, and pull the
            launcher back instantly with shortcuts when it is hidden.
          </p>
        </div>
        <div className="launcher__summary">
          <article className="launcher__summary-card">
            <span className="metric-label">Current state</span>
            <strong>{snapshot.recorder.elapsedLabel}</strong>
          </article>
          <article className="launcher__summary-card">
            <span className="metric-label">Ready checks</span>
            <strong>
              {pendingPermissions === 0
                ? 'All clear'
                : `${pendingPermissions} still pending`}
            </strong>
          </article>
          <article className="launcher__summary-card">
            <span className="metric-label">Quick recall</span>
            <strong>CmdOrCtrl + Shift + L</strong>
          </article>
        </div>
      </section>

      {actionError ? (
        <section className="panel launcher__error">
          <p className="eyebrow">Recorder issue</p>
          <p>{actionError}</p>
        </section>
      ) : null}

      <div className="launcher__layout">
        <div className="launcher__column launcher__column--primary">
          <RecorderPanel
            captureTargets={snapshot.captureTargets}
            onPauseResume={pauseResume}
            onUpdateCaptureTarget={updateCaptureTarget}
            onToggleMicrophone={toggleMicrophone}
            onToggleRecording={toggleRecording}
            recorder={snapshot.recorder}
            selectedCaptureTargetId={snapshot.settings.captureTargetId}
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

        <div className="launcher__column launcher__column--secondary">
          <PermissionsPanel
            onOpenPermissionSettings={openPermissionSettings}
            onRefreshPermissions={refreshPermissions}
            onRequestPermission={requestPermission}
            permissions={snapshot.permissions}
          />
          <ShortcutPanel
            onFocusLauncher={focusLauncher}
            onReset={resetShortcuts}
            shortcuts={snapshot.shortcuts}
          />
        </div>
      </div>

      <div className="launcher__support">
        <div className="launcher__support-card">
          <RecentSessionsPanel sessions={snapshot.recentSessions} />
        </div>
        <div className="launcher__support-card launcher__support-card--plan">
          <section className="panel panel--stack">
            <div className="panel__header">
              <div>
                <p className="eyebrow">What comes next</p>
                <h2>Build plan already wired into the repo</h2>
              </div>
            </div>
            <ul className="launcher__roadmap">
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
