import { listen } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import type {
  AppSettings,
  BootstrapSnapshot,
  CaptureTargetOption,
  PermissionCheck,
  RecorderSnapshot,
  ShortcutBinding,
} from '../types/desktop'
import type { BootstrapRefreshRequestPayload } from '../types/events'

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown
  }
}

const mockSnapshot: BootstrapSnapshot = {
  appName: 'Record Screen',
  appAuthor: 'Tran Van Bach',
  appLicense: 'MIT',
  platform: 'web-preview',
  launcherWindowLabel: 'main',
  recorder: {
    status: 'idle',
    elapsedLabel: 'Ready when you are',
    activeTarget: 'Full desktop',
    activeOutputPath: null,
    qualityPreset: '1080p / 60 fps',
    outputDirectory: '~/Movies/Record Screen',
    micEnabled: true,
  },
  settings: {
    outputDirectory: '~/Movies/Record Screen',
    qualityPreset: '1080p / 60 fps',
    micEnabled: true,
    launchOnLogin: true,
    showHudDuringRecording: true,
    captureTargetId: 'full-desktop',
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
  ],
  qualityPresets: [
    '720p / 30 fps',
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
      case 'get_capture_targets':
        return structuredClone(mockSnapshot.captureTargets) as T
      case 'toggle_recording':
        mockSnapshot.recorder.status =
          mockSnapshot.recorder.status === 'idle' ? 'recording' : 'idle'
        mockSnapshot.recorder.activeOutputPath =
          mockSnapshot.recorder.status === 'idle'
            ? null
            : '~/Movies/Record Screen/recording-preview.mp4'
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
      case 'update_output_directory': {
        const outputDirectory = String(args?.outputDirectory ?? '').trim()
        if (outputDirectory) {
          mockSnapshot.settings.outputDirectory = outputDirectory
          mockSnapshot.recorder.outputDirectory = outputDirectory
        }
        return structuredClone(mockSnapshot.settings) as T
      }
      case 'pick_output_directory':
        return structuredClone(mockSnapshot.settings) as T
      case 'reset_shortcuts':
        return structuredClone(mockSnapshot.shortcuts) as T
      case 'focus_launcher':
      case 'show_hud':
      case 'hide_hud':
      case 'open_recording':
      case 'reveal_recording_in_folder':
      case 'open_permission_settings':
        return undefined as T
      case 'get_permissions':
        return structuredClone(mockSnapshot.permissions) as T
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

  return invoke<T>(name, args)
}

export const desktopClient = {
  getBootstrap() {
    return command<BootstrapSnapshot>('get_bootstrap')
  },
  getCaptureTargets() {
    return command<CaptureTargetOption[]>('get_capture_targets')
  },
  async getCurrentWindowLabel() {
    if (!isTauriRuntime()) {
      return 'main'
    }
    return getCurrentWindow().label
  },
  focusLauncher() {
    return command<void>('focus_launcher')
  },
  showHud() {
    return command<void>('show_hud')
  },
  hideHud() {
    return command<void>('hide_hud')
  },
  toggleRecording() {
    return command<RecorderSnapshot>('toggle_recording')
  },
  pauseResume() {
    return command<RecorderSnapshot | null>('pause_resume')
  },
  toggleMicrophone() {
    return command<RecorderSnapshot>('toggle_microphone')
  },
  resetShortcuts() {
    return command<ShortcutBinding[]>('reset_shortcuts')
  },
  updateQualityPreset(qualityPreset: string) {
    return command<AppSettings>('update_quality_preset', { qualityPreset })
  },
  updateLaunchOnLogin(launchOnLogin: boolean) {
    return command<AppSettings>('update_launch_on_login', { launchOnLogin })
  },
  updateShowHudDuringRecording(showHudDuringRecording: boolean) {
    return command<AppSettings>('update_show_hud_during_recording', {
      showHudDuringRecording,
    })
  },
  updateCaptureTarget(captureTargetId: string) {
    return command<AppSettings>('update_capture_target', { captureTargetId })
  },
  updateOutputDirectory(outputDirectory: string) {
    return command<AppSettings>('update_output_directory', { outputDirectory })
  },
  pickOutputDirectory() {
    return command<AppSettings | null>('pick_output_directory')
  },
  getPermissions() {
    return command<PermissionCheck[]>('get_permissions')
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
  async subscribeBootstrapRefreshRequest(
    listener: (payload: BootstrapRefreshRequestPayload) => void,
  ) {
    if (!isTauriRuntime()) {
      return () => undefined
    }

    const unlisten = await listen<BootstrapRefreshRequestPayload>(
      'recorder://bootstrap-refresh-requested',
      (event) => {
        listener(event.payload)
      },
    )
    return unlisten
  },
}
