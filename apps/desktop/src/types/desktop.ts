export type RecorderStatus = 'idle' | 'recording' | 'paused'

export interface RecorderSnapshot {
  status: RecorderStatus
  elapsedLabel: string
  activeTarget: string
  activeOutputPath: string | null
  qualityPreset: string
  outputDirectory: string
  micEnabled: boolean
}

export interface AppSettings {
  outputDirectory: string
  qualityPreset: string
  micEnabled: boolean
  launchOnLogin: boolean
  captureTargetId: string
}

export interface CaptureTargetOption {
  id: string
  label: string
  description: string
}

export interface SessionSummary {
  id: string
  title: string
  startedAt: string
  durationLabel: string
  location: string
  sizeLabel: string
}

export interface PermissionCheck {
  name: string
  status: 'granted' | 'pending' | 'unsupported'
  guidance: string
}

export type ShortcutAction =
  | 'toggleRecording'
  | 'pauseRecording'
  | 'openLauncher'
  | 'toggleMicrophone'

export interface ShortcutBinding {
  action: ShortcutAction
  label: string
  accelerator: string
  enabled: boolean
  description: string
}

export interface BootstrapSnapshot {
  appName: string
  platform: string
  launcherWindowLabel: string
  recorder: RecorderSnapshot
  settings: AppSettings
  captureTargets: CaptureTargetOption[]
  qualityPresets: string[]
  shortcuts: ShortcutBinding[]
  permissions: PermissionCheck[]
  recentSessions: SessionSummary[]
  roadmap: string[]
}
