import { startTransition, useEffect, useRef, useState } from 'react'
import { desktopClient } from '../services/desktop-client'
import type {
  AppSettings,
  BootstrapSnapshot,
  PermissionCheck,
  RecorderSnapshot,
  SessionSummary,
} from '../types/desktop'

function updateRecorderSnapshot(
  snapshot: BootstrapSnapshot | null,
  recorder: RecorderSnapshot,
) {
  if (!snapshot) {
    return snapshot
  }

  return {
    ...snapshot,
    recorder,
    settings: {
      ...snapshot.settings,
      micEnabled: recorder.micEnabled,
      qualityPreset: recorder.qualityPreset,
    },
  }
}

function updateSettingsSnapshot(
  snapshot: BootstrapSnapshot | null,
  settings: AppSettings,
) {
  if (!snapshot) {
    return snapshot
  }

  return {
    ...snapshot,
    settings,
    recorder: {
      ...snapshot.recorder,
      micEnabled: settings.micEnabled,
      outputDirectory: settings.outputDirectory,
      qualityPreset: settings.qualityPreset,
    },
  }
}

function updatePermissionsSnapshot(
  snapshot: BootstrapSnapshot | null,
  permissions: PermissionCheck[],
) {
  if (!snapshot) {
    return snapshot
  }

  return {
    ...snapshot,
    permissions,
  }
}

function updateRecentSessionsSnapshot(
  snapshot: BootstrapSnapshot | null,
  recentSessions: SessionSummary[],
) {
  if (!snapshot) {
    return snapshot
  }

  return {
    ...snapshot,
    recentSessions,
  }
}

export function useDesktopState() {
  const [snapshot, setSnapshot] = useState<BootstrapSnapshot | null>(null)
  const [isLoading, setIsLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [actionError, setActionError] = useState<string | null>(null)
  const snapshotRef = useRef<BootstrapSnapshot | null>(null)

  useEffect(() => {
    snapshotRef.current = snapshot
  }, [snapshot])

  useEffect(() => {
    let isDisposed = false
    let deferredLoadTimer: number | null = null
    let unlistenRecorder: () => void = () => undefined
    let unlistenRuntimeError: () => void = () => undefined
    let unlistenRecentSessionsRefresh: () => void = () => undefined

    async function refreshRecentSessions(options?: { reportError?: boolean }) {
      try {
        const recentSessions = await desktopClient.getRecentRecordings()
        if (isDisposed) {
          return
        }

        startTransition(() => {
          setSnapshot((current) =>
            updateRecentSessionsSnapshot(current, recentSessions),
          )
          if (options?.reportError) {
            setActionError(null)
          }
        })
      } catch (actionLoadError) {
        if (isDisposed || !options?.reportError) {
          return
        }

        startTransition(() => {
          setActionError(
            actionLoadError instanceof Error
              ? actionLoadError.message
              : 'Unable to refresh recent recordings.',
          )
        })
      }
    }

    async function refreshPermissionsInBackground(options?: {
      reportError?: boolean
    }) {
      try {
        const permissions = await desktopClient.getPermissions()
        if (isDisposed) {
          return
        }

        startTransition(() => {
          setSnapshot((current) => updatePermissionsSnapshot(current, permissions))
          if (options?.reportError) {
            setActionError(null)
          }
        })
      } catch (actionLoadError) {
        if (isDisposed || !options?.reportError) {
          return
        }

        startTransition(() => {
          setActionError(
            actionLoadError instanceof Error
              ? actionLoadError.message
              : 'Unable to refresh permissions.',
          )
        })
      }
    }

    function scheduleDeferredStartupRefresh() {
      if (deferredLoadTimer !== null) {
        window.clearTimeout(deferredLoadTimer)
      }

      deferredLoadTimer = window.setTimeout(() => {
        deferredLoadTimer = null
        if (isDisposed) {
          return
        }

        void refreshPermissionsInBackground()
        void refreshRecentSessions()
      }, 48)
    }

    async function loadSnapshot() {
      try {
        const nextSnapshot = await desktopClient.getBootstrap()
        if (isDisposed) {
          return
        }
        startTransition(() => {
          setSnapshot(nextSnapshot)
          setError(null)
          setActionError(null)
          setIsLoading(false)
        })
        scheduleDeferredStartupRefresh()
      } catch (loadError) {
        if (isDisposed) {
          return
        }
        startTransition(() => {
          setError(
            loadError instanceof Error
              ? loadError.message
              : 'Unable to load the desktop launcher state.',
          )
          setIsLoading(false)
        })
      }
    }

    function applyRecorderSnapshot(recorder: RecorderSnapshot) {
      startTransition(() => {
        setSnapshot((current) => updateRecorderSnapshot(current, recorder))
      })
    }

    void loadSnapshot()

    void desktopClient
      .subscribeRecorderState((recorder) => {
        if (!isDisposed) {
          applyRecorderSnapshot(recorder)
        }
      })
      .then((dispose) => {
        if (isDisposed) {
          dispose()
          return
        }
        unlistenRecorder = dispose
      })

    void desktopClient
      .subscribeRuntimeError((message) => {
        if (isDisposed) {
          return
        }
        startTransition(() => {
          setActionError(message)
        })
      })
      .then((dispose) => {
        if (isDisposed) {
          dispose()
          return
        }
        unlistenRuntimeError = dispose
      })

    void desktopClient
      .subscribeRecentSessionsRefreshRequest(() => {
        if (isDisposed) {
          return
        }

        void refreshRecentSessions({ reportError: true })
      })
      .then((dispose) => {
        if (isDisposed) {
          dispose()
          return
        }

        unlistenRecentSessionsRefresh = dispose
      })

    return () => {
      isDisposed = true
      if (deferredLoadTimer !== null) {
        window.clearTimeout(deferredLoadTimer)
      }
      unlistenRecorder()
      unlistenRuntimeError()
      unlistenRecentSessionsRefresh()
    }
  }, [])

  async function toggleRecording() {
    try {
      const recorder = await desktopClient.toggleRecording()

      startTransition(() => {
        setSnapshot((current) => updateRecorderSnapshot(current, recorder))
        setActionError(null)
      })
    } catch (actionLoadError) {
      startTransition(() => {
        setActionError(
          actionLoadError instanceof Error
            ? actionLoadError.message
            : 'Unable to start or stop the recorder.',
        )
      })
    }
  }

  async function pauseResume() {
    try {
      const recorder = await desktopClient.pauseResume()
      if (recorder) {
        startTransition(() => {
          setSnapshot((current) => updateRecorderSnapshot(current, recorder))
          setActionError(null)
        })
      }
    } catch (actionLoadError) {
      startTransition(() => {
        setActionError(
          actionLoadError instanceof Error
            ? actionLoadError.message
            : 'Unable to pause or resume the recorder.',
        )
      })
    }
  }

  async function toggleMicrophone() {
    try {
      const recorder = await desktopClient.toggleMicrophone()
      startTransition(() => {
        setSnapshot((current) => updateRecorderSnapshot(current, recorder))
        setActionError(null)
      })
    } catch (actionLoadError) {
      startTransition(() => {
        setActionError(
          actionLoadError instanceof Error
            ? actionLoadError.message
            : 'Unable to update microphone state.',
        )
      })
    }
  }

  async function resetShortcuts() {
    await desktopClient.resetShortcuts()
    const nextSnapshot = await desktopClient.getBootstrap()
    startTransition(() => {
      setSnapshot(nextSnapshot)
      setError(null)
      setActionError(null)
      setIsLoading(false)
    })
  }

  async function updateQualityPreset(qualityPreset: string) {
    try {
      const settings = await desktopClient.updateQualityPreset(qualityPreset)
      startTransition(() => {
        setSnapshot((current) => updateSettingsSnapshot(current, settings))
        setActionError(null)
      })
    } catch (actionLoadError) {
      startTransition(() => {
        setActionError(
          actionLoadError instanceof Error
            ? actionLoadError.message
            : 'Unable to update quality preset.',
        )
      })
    }
  }

  async function updateLaunchOnLogin(launchOnLogin: boolean) {
    try {
      const settings = await desktopClient.updateLaunchOnLogin(launchOnLogin)
      startTransition(() => {
        setSnapshot((current) => updateSettingsSnapshot(current, settings))
        setActionError(null)
      })
    } catch (actionLoadError) {
      startTransition(() => {
        setActionError(
          actionLoadError instanceof Error
            ? actionLoadError.message
            : 'Unable to update launch-on-login.',
        )
      })
    }
  }

  async function updateShowHudDuringRecording(showHudDuringRecording: boolean) {
    try {
      const settings =
        await desktopClient.updateShowHudDuringRecording(showHudDuringRecording)
      startTransition(() => {
        setSnapshot((current) => updateSettingsSnapshot(current, settings))
        setActionError(null)
      })
    } catch (actionLoadError) {
      startTransition(() => {
        setActionError(
          actionLoadError instanceof Error
            ? actionLoadError.message
            : 'Unable to update HUD preference.',
        )
      })
    }
  }

  async function updateCaptureTarget(captureTargetId: string) {
    try {
      const settings = await desktopClient.updateCaptureTarget(captureTargetId)
      startTransition(() => {
        setSnapshot((current) => updateSettingsSnapshot(current, settings))
        setActionError(null)
      })
    } catch (actionLoadError) {
      startTransition(() => {
        setActionError(
          actionLoadError instanceof Error
            ? actionLoadError.message
            : 'Unable to update capture target.',
        )
      })
    }
  }

  async function updateAudioInput(audioInputId: string) {
    try {
      const settings = await desktopClient.updateAudioInput(audioInputId)
      startTransition(() => {
        setSnapshot((current) => updateSettingsSnapshot(current, settings))
        setActionError(null)
      })
    } catch (actionLoadError) {
      startTransition(() => {
        setActionError(
          actionLoadError instanceof Error
            ? actionLoadError.message
            : 'Unable to update microphone input.',
        )
      })
    }
  }

  async function updateOutputDirectory(outputDirectory: string) {
    try {
      const settings = await desktopClient.updateOutputDirectory(outputDirectory)
      startTransition(() => {
        setSnapshot((current) => updateSettingsSnapshot(current, settings))
        setActionError(null)
      })
    } catch (actionLoadError) {
      startTransition(() => {
        setActionError(
          actionLoadError instanceof Error
            ? actionLoadError.message
            : 'Unable to update output directory.',
        )
      })
    }
  }

  async function pickOutputDirectory() {
    try {
      const settings = await desktopClient.pickOutputDirectory()
      if (!settings) {
        return
      }

      startTransition(() => {
        setSnapshot((current) => updateSettingsSnapshot(current, settings))
        setActionError(null)
      })
    } catch (actionLoadError) {
      startTransition(() => {
        setActionError(
          actionLoadError instanceof Error
            ? actionLoadError.message
            : 'Unable to choose output directory.',
        )
      })
    }
  }

  async function refreshPermissions() {
    try {
      const permissions = await desktopClient.getPermissions()
      startTransition(() => {
        setSnapshot((current) => updatePermissionsSnapshot(current, permissions))
        setActionError(null)
      })
    } catch (actionLoadError) {
      startTransition(() => {
        setActionError(
          actionLoadError instanceof Error
            ? actionLoadError.message
            : 'Unable to refresh permissions.',
        )
      })
    }
  }

  async function requestPermission(permissionName: string) {
    try {
      const permissions = await desktopClient.requestPermission(permissionName)
      startTransition(() => {
        setSnapshot((current) => updatePermissionsSnapshot(current, permissions))
        setActionError(null)
      })
    } catch (actionLoadError) {
      startTransition(() => {
        setActionError(
          actionLoadError instanceof Error
            ? actionLoadError.message
            : `Unable to request ${permissionName.toLowerCase()} permission.`,
        )
      })
    }
  }

  async function openPermissionSettings(permissionName: string) {
    try {
      await desktopClient.openPermissionSettings(permissionName)
      setActionError(null)
    } catch (actionLoadError) {
      startTransition(() => {
        setActionError(
          actionLoadError instanceof Error
            ? actionLoadError.message
            : `Unable to open settings for ${permissionName.toLowerCase()}.`,
        )
      })
    }
  }

  async function openRecording(recordingPath: string) {
    try {
      await desktopClient.openRecording(recordingPath)
      startTransition(() => {
        setActionError(null)
      })
    } catch (actionLoadError) {
      startTransition(() => {
        setActionError(
          actionLoadError instanceof Error
            ? actionLoadError.message
            : 'Unable to open recording file.',
        )
      })
    }
  }

  async function revealRecordingInFolder(recordingPath: string) {
    try {
      await desktopClient.revealRecordingInFolder(recordingPath)
      startTransition(() => {
        setActionError(null)
      })
    } catch (actionLoadError) {
      startTransition(() => {
        setActionError(
          actionLoadError instanceof Error
            ? actionLoadError.message
            : 'Unable to reveal recording in folder.',
        )
      })
    }
  }

  async function saveRecordingCopy(recordingPath: string) {
    try {
      await desktopClient.saveRecordingCopy(recordingPath)
      startTransition(() => {
        setActionError(null)
      })
    } catch (actionLoadError) {
      startTransition(() => {
        setActionError(
          actionLoadError instanceof Error
            ? actionLoadError.message
            : 'Unable to export recording copy.',
        )
      })
    }
  }

  return {
    actionError,
    snapshot,
    isLoading,
    error,
    focusLauncher: desktopClient.focusLauncher,
    hideHud: desktopClient.hideHud,
    openPermissionSettings,
    openRecording,
    pauseResume,
    refreshPermissions,
    revealRecordingInFolder,
    resetShortcuts,
    requestPermission,
    saveRecordingCopy,
    showHud: desktopClient.showHud,
    toggleMicrophone,
    updateCaptureTarget,
    updateAudioInput,
    updateShowHudDuringRecording,
    toggleRecording,
    updateLaunchOnLogin,
    updateOutputDirectory,
    updateQualityPreset,
    pickOutputDirectory,
  }
}
