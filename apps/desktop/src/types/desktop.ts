export type RecorderStatus = 'idle' | 'recording' | 'paused'

export interface RecorderSnapshot {
  status: RecorderStatus
  elapsedLabel: string
  activeTarget: string
  activeOutputPath: string | null
  activeEncoderLabel: string | null
  qualityPreset: string
  outputDirectory: string
  micEnabled: boolean
}

export interface MicCheckSnapshot {
  active: boolean
  level: number
  hasSignal: boolean
  error: string | null
}

export interface AppSettings {
  outputDirectory: string
  qualityPreset: string
  micEnabled: boolean
  audioInputId: string
  launchOnLogin: boolean
  showHudDuringRecording: boolean
  captureTargetId: string
}

export interface CaptureTargetOption {
  id: string
  label: string
  description: string
}

export interface AudioInputOption {
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

export interface RuntimeDiagnostics {
  summary: string
  backendPath: string
  readiness: string
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
  appAuthor: string
  appLicense: string
  platform: string
  launcherWindowLabel: string
  recorder: RecorderSnapshot
  settings: AppSettings
  captureTargets: CaptureTargetOption[]
  audioInputs: AudioInputOption[]
  qualityPresets: string[]
  shortcuts: ShortcutBinding[]
  permissions: PermissionCheck[]
  diagnostics: RuntimeDiagnostics
  recentSessions: SessionSummary[]
  roadmap: string[]
}
