import { listen } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import type {
  AudioInputOption,
  AppSettings,
  BootstrapSnapshot,
  CaptureTargetOption,
  MicCheckSnapshot,
  PermissionCheck,
  RecorderSnapshot,
  SessionSummary,
  ShortcutBinding,
} from '../types/desktop'

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown
  }
}

const mockSnapshot: BootstrapSnapshot = {
  appName: 'Record Screen',
  appVersion: '0.1.14',
  appAuthor: 'Tran Van Bach',
  appLicense: 'MIT',
  platform: 'web-preview',
  launcherWindowLabel: 'main',
  recorder: {
    status: 'idle',
    elapsedLabel: 'Ready when you are',
    activeTarget: 'Full desktop',
    activeOutputPath: null,
    activeEncoderLabel: null,
    qualityPreset: '1080p / 30 fps',
    outputDirectory: '~/Movies/Record Screen',
    micEnabled: true,
  },
  settings: {
    outputDirectory: '~/Movies/Record Screen',
    qualityPreset: '1080p / 30 fps',
    micEnabled: true,
    systemAudioEnabled: false,
    audioInputId: 'default',
    launchOnLogin: false,
    showHudDuringRecording: true,
    captureTargetId: 'full-desktop',
    regionX: 160,
    regionY: 120,
    regionWidth: 1280,
    regionHeight: 720,
    regionSourceCaptureTargetId: 'full-desktop',
    regionSourceOriginX: 0,
    regionSourceOriginY: 0,
    regionSourceScaleFactorMilli: 1000,
  },
  captureTargets: [
    {
      id: 'full-desktop',
      label: 'Full desktop',
      description: 'Record the entire active desktop layout.',
    },
    {
      id: 'monitor:display-1',
      label: 'Display 1',
      description: 'Record only the primary display.',
    },
    {
      id: 'monitor:display-2',
      label: 'Display 2',
      description: 'Record only the secondary display.',
    },
    {
      id: 'region:custom',
      label: 'Custom region',
      description: 'Capture a reusable region at 160, 120 with size 1280 x 720.',
    },
  ],
  audioInputs: [
    {
      id: 'default',
      label: 'Default input',
      description: 'Use the system default microphone.',
      kind: 'default',
    },
    {
      id: 'built-in-mic',
      label: 'Built-in Microphone',
      description: 'MacBook Pro Microphone Array',
      kind: 'microphone',
    },
    {
      id: 'usb-audio',
      label: 'USB Audio Interface',
      description: 'External microphone over USB.',
      kind: 'microphone',
    },
    {
      id: 'system-loopback',
      label: 'System audio · Built-in Output',
      description: 'Loopback monitor source for desktop audio.',
      kind: 'system',
    },
  ],
  qualityPresets: [
    '720p / 30 fps',
    '1080p / 30 fps',
    '1080p / 60 fps',
    '1440p / 60 fps',
    '4K / 60 fps',
  ],
  shortcuts: [
    {
      action: 'toggleRecording',
      label: 'Start / stop recording',
      accelerator: 'CmdOrCtrl+Shift+R',
      enabled: true,
      description: 'Instantly begin or finalize the current recording session.',
    },
    {
      action: 'pauseRecording',
      label: 'Pause / resume recording',
      accelerator: 'CmdOrCtrl+Shift+P',
      enabled: true,
      description: 'Freeze capture without losing the current session.',
    },
    {
      action: 'openLauncher',
      label: 'Open launcher',
      accelerator: 'CmdOrCtrl+Shift+L',
      enabled: true,
      description: 'Bring the command launcher back into focus from anywhere.',
    },
    {
      action: 'toggleMicrophone',
      label: 'Mute / unmute microphone',
      accelerator: 'CmdOrCtrl+Shift+M',
      enabled: true,
      description: 'Flip the microphone state while keeping the session alive.',
    },
  ],
  permissions: [
    {
      name: 'Launcher readiness',
      status: 'granted',
      guidance: 'The shell and keyboard controls are available in preview mode.',
    },
    {
      name: 'Screen recording',
      status: 'pending',
      guidance: 'Native permission probing will activate when the Tauri shell is running.',
    },
  ],
  diagnostics: {
    summary: 'Preview runtime',
    backendPath: 'Mock desktop client',
    readiness: 'Web preview uses mocked launcher state and does not start a native recorder.',
    supportsCustomRegion: true,
    customRegionNote: 'Preview mode can show the custom-region flow.',
    supportsSystemAudio: true,
    systemAudioNote: 'Preview mode exposes a mock system-audio loopback source.',
  },
  recentSessions: [
    {
      id: 'preview-1',
      title: 'Product walkthrough',
      startedAt: 'Mar 12, 2026 · 20:30',
      durationLabel: '14 min',
      location: '~/Movies/Record Screen/product-walkthrough.mp4',
      sizeLabel: '426 MB',
    },
    {
      id: 'preview-2',
      title: 'Bug repro clip',
      startedAt: 'Mar 11, 2026 · 17:05',
      durationLabel: '5 min',
      location: '~/Movies/Record Screen/bug-repro.mov',
      sizeLabel: '118 MB',
    },
  ],
  roadmap: [
    'Launcher shell and global shortcuts',
    'Permission-aware recording bootstrap',
    'Cross-platform capture backends',
    'Review and export workflow',
  ],
}

let mockMicCheckState: MicCheckSnapshot = {
  active: false,
  level: 0,
  hasSignal: false,
  error: null,
}

let mockMicCheckTimer: number | null = null

function stopMockMicCheck() {
  if (mockMicCheckTimer !== null) {
    window.clearInterval(mockMicCheckTimer)
    mockMicCheckTimer = null
  }
  mockMicCheckState = {
    active: false,
    level: 0,
    hasSignal: false,
    error: null,
  }
}

function isTauriRuntime() {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}

async function command<T>(
  name: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (!isTauriRuntime()) {
    switch (name) {
      case 'get_bootstrap':
        return structuredClone(mockSnapshot) as T
      case 'get_recorder_snapshot':
        return structuredClone(mockSnapshot.recorder) as T
      case 'get_capture_targets':
        return structuredClone(mockSnapshot.captureTargets) as T
      case 'get_audio_inputs':
        return structuredClone(mockSnapshot.audioInputs) as T
      case 'toggle_recording':
        mockSnapshot.recorder.status =
          mockSnapshot.recorder.status === 'idle' ? 'recording' : 'idle'
        mockSnapshot.recorder.activeOutputPath =
          mockSnapshot.recorder.status === 'idle'
            ? null
            : '~/Movies/Record Screen/recording-preview.mp4'
        mockSnapshot.recorder.activeEncoderLabel =
          mockSnapshot.recorder.status === 'idle' ? null : 'h264_videotoolbox'
        mockSnapshot.recorder.elapsedLabel =
          mockSnapshot.recorder.status === 'idle'
            ? 'Ready when you are'
            : '00:12:41'
        return structuredClone(mockSnapshot.recorder) as T
      case 'pause_resume':
        if (mockSnapshot.recorder.status === 'recording') {
          mockSnapshot.recorder.status = 'paused'
          mockSnapshot.recorder.elapsedLabel = 'Paused at 00:12:41'
          return structuredClone(mockSnapshot.recorder) as T
        }
        if (mockSnapshot.recorder.status === 'paused') {
          mockSnapshot.recorder.status = 'recording'
          mockSnapshot.recorder.elapsedLabel = '00:12:41'
          return structuredClone(mockSnapshot.recorder) as T
        }
        return null as T
      case 'toggle_microphone':
        mockSnapshot.recorder.micEnabled = !mockSnapshot.recorder.micEnabled
        mockSnapshot.settings.micEnabled = mockSnapshot.recorder.micEnabled
        return structuredClone(mockSnapshot.recorder) as T
      case 'start_mic_check':
        stopMockMicCheck()
        mockMicCheckState = {
          active: true,
          level: 0.14,
          hasSignal: true,
          error: null,
        }
        return structuredClone(mockMicCheckState) as T
      case 'stop_mic_check':
        stopMockMicCheck()
        return structuredClone(mockMicCheckState) as T
      case 'update_quality_preset': {
        const qualityPreset = String(args?.qualityPreset ?? '')
        if (mockSnapshot.qualityPresets.includes(qualityPreset)) {
          mockSnapshot.settings.qualityPreset = qualityPreset
          mockSnapshot.recorder.qualityPreset = qualityPreset
        }
        return structuredClone(mockSnapshot.settings) as T
      }
      case 'update_launch_on_login':
        mockSnapshot.settings.launchOnLogin = Boolean(args?.launchOnLogin)
        return structuredClone(mockSnapshot.settings) as T
      case 'update_system_audio_enabled':
        mockSnapshot.settings.systemAudioEnabled = Boolean(args?.systemAudioEnabled)
        return structuredClone(mockSnapshot.settings) as T
      case 'update_show_hud_during_recording':
        mockSnapshot.settings.showHudDuringRecording = Boolean(
          args?.showHudDuringRecording,
        )
        return structuredClone(mockSnapshot.settings) as T
      case 'update_capture_target': {
        const captureTargetId = String(args?.captureTargetId ?? '')
        const captureTarget = mockSnapshot.captureTargets.find(
          (target) => target.id === captureTargetId,
        )
        if (captureTarget) {
          mockSnapshot.settings.captureTargetId = captureTarget.id
          mockSnapshot.recorder.activeTarget = captureTarget.label
        }
        return structuredClone(mockSnapshot.settings) as T
      }
      case 'update_audio_input': {
        const audioInputId = String(args?.audioInputId ?? '')
        const audioInput = mockSnapshot.audioInputs.find(
          (input) => input.id === audioInputId,
        )
        if (audioInput) {
          mockSnapshot.settings.audioInputId = audioInput.id
        }
        return structuredClone(mockSnapshot.settings) as T
      }
      case 'update_output_directory': {
        const outputDirectory = String(args?.outputDirectory ?? '').trim()
        if (outputDirectory) {
          mockSnapshot.settings.outputDirectory = outputDirectory
          mockSnapshot.recorder.outputDirectory = outputDirectory
        }
        return structuredClone(mockSnapshot.settings) as T
      }
      case 'update_custom_region': {
        mockSnapshot.settings.regionX = Number(args?.regionX ?? mockSnapshot.settings.regionX)
        mockSnapshot.settings.regionY = Number(args?.regionY ?? mockSnapshot.settings.regionY)
        mockSnapshot.settings.regionWidth = Number(
          args?.regionWidth ?? mockSnapshot.settings.regionWidth,
        )
        mockSnapshot.settings.regionHeight = Number(
          args?.regionHeight ?? mockSnapshot.settings.regionHeight,
        )
        const customRegionTarget = mockSnapshot.captureTargets.find(
          (target) => target.id === 'region:custom',
        )
        if (customRegionTarget) {
          customRegionTarget.description = `Capture a reusable region at ${mockSnapshot.settings.regionX}, ${mockSnapshot.settings.regionY} with size ${mockSnapshot.settings.regionWidth} x ${mockSnapshot.settings.regionHeight}.`
        }
        return structuredClone(mockSnapshot.settings) as T
      }
      case 'pick_output_directory':
        return structuredClone(mockSnapshot.settings) as T
      case 'reset_shortcuts':
        mockSnapshot.shortcuts = structuredClone(mockSnapshot.shortcuts.map((shortcut) => ({
          ...shortcut,
          accelerator:
            shortcut.action === 'toggleRecording'
              ? 'CmdOrCtrl+Shift+R'
              : shortcut.action === 'pauseRecording'
                ? 'CmdOrCtrl+Shift+P'
                : shortcut.action === 'openLauncher'
                  ? 'CmdOrCtrl+Shift+L'
                  : 'CmdOrCtrl+Shift+M',
        })))
        return structuredClone(mockSnapshot.shortcuts) as T
      case 'update_shortcut': {
        const action = String(args?.action ?? '') as ShortcutBinding['action']
        const accelerator = String(args?.accelerator ?? '').trim()
        if (!accelerator) {
          throw new Error('Shortcut accelerator is empty.')
        }
        const existingShortcut = mockSnapshot.shortcuts.find(
          (shortcut) => shortcut.action !== action && shortcut.accelerator === accelerator,
        )
        if (existingShortcut) {
          throw new Error(
            `shortcut conflict: \`${accelerator}\` is already assigned to ${existingShortcut.label}.`,
          )
        }
        mockSnapshot.shortcuts = mockSnapshot.shortcuts.map((shortcut) =>
          shortcut.action === action
            ? { ...shortcut, accelerator }
            : shortcut,
        )
        return structuredClone(mockSnapshot.shortcuts) as T
      }
      case 'focus_launcher':
      case 'show_hud':
      case 'show_region_selector':
      case 'hide_hud':
      case 'hide_region_selector':
      case 'open_recording':
      case 'reveal_recording_in_folder':
      case 'open_permission_settings':
        return undefined as T
      case 'save_recording_copy':
        return `~/Downloads/${String(args?.recordingPath ?? '').split(/[\\/]/).filter(Boolean).at(-1) ?? 'recording.mp4'}` as T
      case 'trash_recordings': {
        const recordingPaths = Array.isArray(args?.recordingPaths)
          ? args.recordingPaths.map((value) => String(value))
          : []
        mockSnapshot.recentSessions = mockSnapshot.recentSessions.filter(
          (session) => !recordingPaths.includes(session.location),
        )
        return structuredClone(mockSnapshot.recentSessions) as T
      }
      case 'get_permissions':
        return structuredClone(mockSnapshot.permissions) as T
      case 'get_recent_recordings':
        return structuredClone(mockSnapshot.recentSessions) as T
      case 'request_permission': {
        const permissionName = String(args?.permissionName ?? '')
        mockSnapshot.permissions = mockSnapshot.permissions.map((permission) =>
          permission.name === permissionName
            ? { ...permission, status: 'granted' }
            : permission,
        )
        return structuredClone(mockSnapshot.permissions) as T
      }
      default:
        throw new Error(`Unsupported preview command: ${name}`)
    }
  }

  try {
    return await invoke<T>(name, args)
  } catch (error) {
    throw new Error(extractCommandErrorMessage(error, name))
  }
}

function extractCommandErrorMessage(error: unknown, commandName: string): string {
  if (typeof error === 'string' && error.trim()) {
    return error
  }

  if (error instanceof Error && error.message.trim()) {
    return error.message
  }

  if (error && typeof error === 'object') {
    const record = error as Record<string, unknown>

    const directMessage = firstString([
      record.message,
      record.error,
      record.details,
      record.reason,
    ])
    if (directMessage) {
      return directMessage
    }

    const cause = record.cause
    if (cause && typeof cause === 'object') {
      const causeMessage = firstString([
        (cause as Record<string, unknown>).message,
        (cause as Record<string, unknown>).error,
      ])
      if (causeMessage) {
        return causeMessage
      }
    }
  }

  return `Command \`${commandName}\` failed.`
}

function firstString(values: unknown[]): string | null {
  for (const value of values) {
    if (typeof value === 'string' && value.trim()) {
      return value
    }
  }

  return null
}

export const desktopClient = {
  getBootstrap() {
    return command<BootstrapSnapshot>('get_bootstrap')
  },
  getRecorderSnapshot() {
    return command<RecorderSnapshot>('get_recorder_snapshot')
  },
  getCaptureTargets() {
    return command<CaptureTargetOption[]>('get_capture_targets')
  },
  getAudioInputs() {
    return command<AudioInputOption[]>('get_audio_inputs')
  },
  focusLauncher() {
    return command<void>('focus_launcher')
  },
  showHud() {
    return command<void>('show_hud')
  },
  showRegionSelector() {
    return command<void>('show_region_selector')
  },
  hideHud() {
    return command<void>('hide_hud')
  },
  hideRegionSelector() {
    return command<void>('hide_region_selector')
  },
  startHudDrag() {
    return command<void>('start_hud_drag')
  },
  toggleRecording() {
    return command<RecorderSnapshot>('toggle_recording')
  },
  pauseResume() {
    return command<RecorderSnapshot | null>('pause_resume')
  },
  startMicCheck() {
    return command<MicCheckSnapshot>('start_mic_check')
  },
  stopMicCheck() {
    return command<MicCheckSnapshot>('stop_mic_check')
  },
  toggleMicrophone() {
    return command<RecorderSnapshot>('toggle_microphone')
  },
  resetShortcuts() {
    return command<ShortcutBinding[]>('reset_shortcuts')
  },
  updateShortcut(action: ShortcutBinding['action'], accelerator: string) {
    return command<ShortcutBinding[]>('update_shortcut', {
      action,
      accelerator,
    })
  },
  updateQualityPreset(qualityPreset: string) {
    return command<AppSettings>('update_quality_preset', { qualityPreset })
  },
  updateLaunchOnLogin(launchOnLogin: boolean) {
    return command<AppSettings>('update_launch_on_login', { launchOnLogin })
  },
  updateSystemAudioEnabled(systemAudioEnabled: boolean) {
    return command<AppSettings>('update_system_audio_enabled', {
      systemAudioEnabled,
    })
  },
  updateShowHudDuringRecording(showHudDuringRecording: boolean) {
    return command<AppSettings>('update_show_hud_during_recording', {
      showHudDuringRecording,
    })
  },
  updateCaptureTarget(captureTargetId: string) {
    return command<AppSettings>('update_capture_target', { captureTargetId })
  },
  updateAudioInput(audioInputId: string) {
    return command<AppSettings>('update_audio_input', { audioInputId })
  },
  updateOutputDirectory(outputDirectory: string) {
    return command<AppSettings>('update_output_directory', { outputDirectory })
  },
  updateCustomRegion(
    regionX: number,
    regionY: number,
    regionWidth: number,
    regionHeight: number,
    regionSourceCaptureTargetId?: string,
    regionSourceOriginX?: number,
    regionSourceOriginY?: number,
    regionSourceScaleFactorMilli?: number,
  ) {
    return command<AppSettings>('update_custom_region', {
      regionX,
      regionY,
      regionWidth,
      regionHeight,
      regionSourceCaptureTargetId,
      regionSourceOriginX,
      regionSourceOriginY,
      regionSourceScaleFactorMilli,
    })
  },
  pickOutputDirectory() {
    return command<AppSettings | null>('pick_output_directory')
  },
  getPermissions() {
    return command<PermissionCheck[]>('get_permissions')
  },
  getRecentRecordings() {
    return command<SessionSummary[]>('get_recent_recordings')
  },
  requestPermission(permissionName: string) {
    return command<PermissionCheck[]>('request_permission', { permissionName })
  },
  openPermissionSettings(permissionName: string) {
    return command<void>('open_permission_settings', { permissionName })
  },
  openRecording(recordingPath: string) {
    return command<void>('open_recording', { recordingPath })
  },
  revealRecordingInFolder(recordingPath: string) {
    return command<void>('reveal_recording_in_folder', { recordingPath })
  },
  saveRecordingCopy(recordingPath: string) {
    return command<string | null>('save_recording_copy', { recordingPath })
  },
  trashRecordings(recordingPaths: string[]) {
    return command<SessionSummary[]>('trash_recordings', { recordingPaths })
  },
  async subscribeRecorderState(
    listener: (snapshot: RecorderSnapshot) => void,
  ) {
    if (!isTauriRuntime()) {
      return () => undefined
    }

    const unlisten = await listen<RecorderSnapshot>(
      'recorder://state-changed',
      (event) => {
        listener(event.payload)
      },
    )
    return unlisten
  },
  async subscribeRuntimeError(listener: (message: string) => void) {
    if (!isTauriRuntime()) {
      return () => undefined
    }

    const unlisten = await listen<string>('recorder://runtime-error', (event) => {
      listener(event.payload)
    })
    return unlisten
  },
  async subscribeHudShown(listener: () => void) {
    if (!isTauriRuntime()) {
      return () => undefined
    }

    const unlisten = await listen('recorder://hud-shown', () => {
      listener()
    })
    return unlisten
  },
  async subscribeRecentSessionsRefreshRequest(listener: () => void) {
    if (!isTauriRuntime()) {
      return () => undefined
    }

    const unlisten = await listen('recorder://recent-sessions-refresh-requested', () => {
      listener()
    })
    return unlisten
  },
  async subscribeMicCheckState(listener: (snapshot: MicCheckSnapshot) => void) {
    if (!isTauriRuntime()) {
      listener(structuredClone(mockMicCheckState))
      mockMicCheckTimer = window.setInterval(() => {
        if (!mockMicCheckState.active) {
          return
        }

        const nextLevel = 0.08 + Math.random() * 0.72
        mockMicCheckState = {
          active: true,
          level: nextLevel,
          hasSignal: nextLevel >= 0.1,
          error: null,
        }
        listener(structuredClone(mockMicCheckState))
      }, 180)

      return () => {
        stopMockMicCheck()
      }
    }

    const unlisten = await listen<MicCheckSnapshot>(
      'recorder://mic-check-state',
      (event) => {
        listener(event.payload)
      },
    )
    return unlisten
  },
}
