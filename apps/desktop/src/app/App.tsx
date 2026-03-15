import type { ReactElement } from 'react'
import { useEffect, useState } from 'react'
import {
  AppWindow,
  Clock3,
  Keyboard,
  Settings,
  ShieldCheck,
  Video,
} from 'lucide-react'
import { getAppSurface } from './surfaces'
import { HudSurface } from '../features/launcher/components/HudSurface'
import { PermissionsPanel } from '../features/permissions/components/PermissionsPanel'
import { RecorderPanel } from '../features/recorder/components/RecorderPanel'
import { RegionSelectorSurface } from '../features/recorder/components/RegionSelectorSurface'
import { TargetPreviewSurface } from '../features/recorder/components/TargetPreviewSurface'
import { RecentSessionsPanel } from '../features/library/components/RecentSessionsPanel'
import { SettingsPanel } from '../features/settings/components/SettingsPanel'
import { ShortcutPanel } from '../features/settings/components/ShortcutPanel'
import { useDesktopState } from '../hooks/use-desktop-state'
import { useHudState } from '../hooks/use-hud-state'

type LauncherTab =
  | 'recorder'
  | 'recent'
  | 'settings'
  | 'shortcuts'
  | 'permissions'

type ThemeMode = 'dark' | 'light'

const launcherTabs: Array<{
  id: LauncherTab
  label: string
  eyebrow: string
  icon: ReactElement
  title: string
  description: string
}> = [
  {
    id: 'recorder',
    label: 'Record',
    eyebrow: 'Recorder',
    icon: <Video aria-hidden="true" size={16} strokeWidth={1.9} />,
    title: 'Ready to record',
    description: 'Select your target and start capturing.',
  },
  {
    id: 'recent',
    label: 'Recent',
    eyebrow: 'Recent sessions',
    icon: <Clock3 aria-hidden="true" size={16} strokeWidth={1.9} />,
    title: 'Recent clips',
    description: 'Open recent recordings.',
  },
  {
    id: 'settings',
    label: 'Settings',
    eyebrow: 'Defaults',
    icon: <Settings aria-hidden="true" size={16} strokeWidth={1.9} />,
    title: 'Defaults',
    description: 'Theme, quality, output.',
  },
  {
    id: 'shortcuts',
    label: 'Shortcuts',
    eyebrow: 'Keyboard control',
    icon: <Keyboard aria-hidden="true" size={16} strokeWidth={1.9} />,
    title: 'Shortcuts',
    description: 'Control the app anywhere.',
  },
  {
    id: 'permissions',
    label: 'Permissions',
    eyebrow: 'Readiness',
    icon: <ShieldCheck aria-hidden="true" size={16} strokeWidth={1.9} />,
    title: 'Permissions',
    description: 'Check what is ready.',
  },
]

function LoadingState() {
  return (
    <main className="launcher-shell launcher--loading">
      <section className="panel launcher__state-panel">
        <p className="eyebrow">Record Screen</p>
        <h1>Preparing launcher</h1>
        <p>Loading app state.</p>
      </section>
    </main>
  )
}

function ErrorState({ message }: { message: string }) {
  return (
    <main className="launcher-shell launcher--loading">
      <section className="panel launcher__state-panel">
        <p className="eyebrow">Launcher error</p>
        <h1>Unable to load desktop state</h1>
        <p>{message}</p>
      </section>
    </main>
  )
}

function HudLoadingState() {
  return (
    <main className="hud">
      <section className="hud__card">
        <div className="hud__elapsed hud__drag-region" data-tauri-drag-region>
          <span className="status-dot status-idle" />
          <strong>00:00:00</strong>
        </div>
      </section>
    </main>
  )
}

function HudApp() {
  const {
    error,
    isLoading,
    isRecordingCountdownActive,
    isStartingRecording,
    pauseResume,
    recorder,
    recordingStartCountdown,
    toggleMicrophone,
    toggleRecording,
  } =
    useHudState()

  if (isLoading || !recorder) {
    return <HudLoadingState />
  }

  if (error) {
    return (
      <main className="hud">
        <section className="hud__card">
          <div className="hud__elapsed hud__drag-region" data-tauri-drag-region>
            <span className="status-dot status-idle" />
            <strong>HUD unavailable</strong>
          </div>
        </section>
      </main>
    )
  }

  return (
    <HudSurface
      countdownValue={
        isRecordingCountdownActive ? recordingStartCountdown : null
      }
      isStartingRecording={isStartingRecording}
      onPauseResume={pauseResume}
      onToggleMicrophone={toggleMicrophone}
      onToggleRecording={toggleRecording}
      recorder={recorder}
    />
  )
}

function RegionSelectorApp() {
  return <RegionSelectorSurface />
}

function TargetPreviewApp() {
  return <TargetPreviewSurface />
}

function LauncherApp() {
  const [activeTab, setActiveTab] = useState<LauncherTab>('recorder')
  const [themeMode, setThemeMode] = useState<ThemeMode>(() => {
    if (typeof window !== 'undefined' && window.matchMedia) {
      return window.matchMedia('(prefers-color-scheme: light)').matches
        ? 'light'
        : 'dark'
    }

    return 'dark'
  })
  const {
    actionError,
    error,
    focusLauncher,
    isRecordingCountdownActive,
    isStartingRecording,
    isLoading,
    openPermissionSettings,
    openRecording,
    pauseResume,
    pickOutputDirectory,
    refreshPermissions,
    revealRecordingInFolder,
    resetShortcuts,
    requestPermission,
    saveRecordingCopy,
    trashRecordings,
    showHud,
    showRegionSelector,
    snapshot,
    recordingStartCountdown,
    toggleMicrophone,
    toggleRecording,
    updateAudioInput,
    updateCaptureTarget,
    updateCustomRegion,
    updateLaunchOnLogin,
    updateOutputDirectory,
    updateQualityPreset,
    updateShortcut,
    updateShowHudDuringRecording,
    updateSystemAudioEnabled,
  } = useDesktopState()

  useEffect(() => {
    document.body.dataset.theme = themeMode
    return () => {
      delete document.body.dataset.theme
    }
  }, [themeMode])

  if (isLoading || !snapshot) {
    return <LoadingState />
  }

  if (error) {
    return <ErrorState message={error} />
  }

  const currentSnapshot = snapshot

  const activeTabConfig =
    launcherTabs.find((tab) => tab.id === activeTab) ?? launcherTabs[0]

  function renderActiveTab() {
    switch (activeTab) {
      case 'recorder':
        return (
          <RecorderPanel
            audioInputs={currentSnapshot.audioInputs}
            captureTargets={currentSnapshot.captureTargets}
            countdownValue={
              isRecordingCountdownActive ? recordingStartCountdown : null
            }
            diagnostics={currentSnapshot.diagnostics}
            isStartingRecording={isStartingRecording}
            onOpenRegionSelector={showRegionSelector}
            onPauseResume={pauseResume}
            onUpdateAudioInput={updateAudioInput}
            onUpdateCaptureTarget={updateCaptureTarget}
            onToggleMicrophone={toggleMicrophone}
            onToggleRecording={toggleRecording}
            recorder={currentSnapshot.recorder}
            runtimeError={actionError}
            selectedAudioInputId={currentSnapshot.settings.audioInputId}
            selectedCaptureTargetId={currentSnapshot.settings.captureTargetId}
            systemAudioEnabled={currentSnapshot.settings.systemAudioEnabled}
          />
        )
      case 'recent':
        return (
          <RecentSessionsPanel
            onTrashRecordings={trashRecordings}
            onOpenRecording={openRecording}
            onRevealRecordingInFolder={revealRecordingInFolder}
            onSaveRecordingCopy={saveRecordingCopy}
            sessions={currentSnapshot.recentSessions}
          />
        )
      case 'settings':
        return (
          <SettingsPanel
            diagnostics={currentSnapshot.diagnostics}
            hasSystemAudioSource={currentSnapshot.audioInputs.some((input) => input.kind === 'system')}
            onOpenRegionSelector={showRegionSelector}
            onPickOutputDirectory={pickOutputDirectory}
            onUpdateThemeMode={setThemeMode}
            onUpdateLaunchOnLogin={updateLaunchOnLogin}
            onUpdateOutputDirectory={updateOutputDirectory}
            onUpdateQualityPreset={updateQualityPreset}
            onUpdateCustomRegion={updateCustomRegion}
            onUpdateShowHudDuringRecording={updateShowHudDuringRecording}
            onUpdateSystemAudioEnabled={updateSystemAudioEnabled}
            qualityPresets={currentSnapshot.qualityPresets}
            settings={currentSnapshot.settings}
            themeMode={themeMode}
          />
        )
      case 'shortcuts':
        return (
          <ShortcutPanel
            onFocusLauncher={focusLauncher}
            onReset={resetShortcuts}
            onUpdateShortcut={updateShortcut}
            shortcuts={currentSnapshot.shortcuts}
          />
        )
      case 'permissions':
        return (
          <PermissionsPanel
            onOpenPermissionSettings={openPermissionSettings}
            onRefreshPermissions={refreshPermissions}
            onRequestPermission={requestPermission}
            permissions={currentSnapshot.permissions}
          />
        )
    }
  }

  return (
    <main className="launcher-shell">
      <section className="launcher-frame">
        <aside className="launcher-sidebar">
          <div className="launcher-sidebar__brand">
            <div className="launcher-sidebar__brand-mark">
              <Video aria-hidden="true" size={17} strokeWidth={2} />
            </div>
            <div>
              <strong>{currentSnapshot.appName}</strong>
              <p>{activeTab === 'recorder' ? 'Desktop recorder' : currentSnapshot.platform}</p>
            </div>
          </div>

          <nav className="launcher-nav" aria-label="Launcher sections">
            {launcherTabs.map((tab) => (
              <button
                className={`launcher-nav__item ${
                  activeTab === tab.id ? 'launcher-nav__item--active' : ''
                }`}
                key={tab.id}
                onClick={() => {
                  setActiveTab(tab.id)
                }}
                type="button"
              >
                <span className="launcher-nav__marker">
                  {tab.icon}
                </span>
                <span>{tab.label}</span>
              </button>
            ))}
          </nav>

          <div className="launcher-sidebar__footer">
            <button
              className="button button--secondary launcher-sidebar__preview"
              onClick={() => void showHud()}
              type="button"
            >
              <AppWindow aria-hidden="true" size={15} strokeWidth={1.9} />
              Preview HUD Mode
            </button>
            <div className="launcher-sidebar__meta">
              <span>v{currentSnapshot.appVersion}</span>
              <span aria-hidden="true">•</span>
              <span>{currentSnapshot.appLicense}</span>
              <span aria-hidden="true">•</span>
              <span>{currentSnapshot.appAuthor}</span>
            </div>
          </div>
        </aside>

        <section
          className={`launcher-stage ${
            activeTab === 'recorder' ? 'launcher-stage--recorder' : ''
          }`}
        >
          {activeTab !== 'recorder' ? (
            <header className="launcher-stage__header launcher-stage__header--compact">
              <div>
                <p className="eyebrow">{activeTabConfig.eyebrow}</p>
                <h1>{activeTabConfig.title}</h1>
                <p className="launcher-stage__copy">{activeTabConfig.description}</p>
              </div>
            </header>
          ) : null}

          {actionError && activeTab !== 'recorder' ? (
            <section className="panel launcher__error">
              <p className="eyebrow">Recorder issue</p>
              <p>{actionError}</p>
            </section>
          ) : null}

          <div className={`launcher-stage__body launcher-stage__body--${activeTab}`}>
            {renderActiveTab()}
          </div>
        </section>
      </section>
    </main>
  )
}

export default function App() {
  const surface = getAppSurface()

  useEffect(() => {
    document.body.dataset.surface = surface
    return () => {
      delete document.body.dataset.surface
    }
  }, [surface])

  if (surface === 'hud') {
    return <HudApp />
  }

  if (surface === 'region-selector') {
    return <RegionSelectorApp />
  }

  if (surface === 'target-preview') {
    return <TargetPreviewApp />
  }

  return <LauncherApp />
}
