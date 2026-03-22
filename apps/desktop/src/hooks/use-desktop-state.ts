import { startTransition, useCallback, useEffect, useRef, useState } from 'react'
import { desktopClient } from '../services/desktop-client'
import { useRecordingCountdown } from './use-recording-countdown'
import type {
  AppSettings,
  BootstrapSnapshot,
  PermissionCheck,
  RecorderSnapshot,
  RuntimeDiagnostics,
  SessionSummary,
  ShortcutAction,
  ShortcutBinding,
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

function optimisticRecorderStart(
  snapshot: BootstrapSnapshot | null,
  recorder: RecorderSnapshot | null,
): RecorderSnapshot | null {
  if (!snapshot) {
    return null
  }

  const currentRecorder = recorder ?? snapshot.recorder

  const activeTarget =
    snapshot.captureTargets.find(
      (target) => target.id === snapshot.settings.captureTargetId,
    )?.label ?? currentRecorder.activeTarget

  return {
    ...currentRecorder,
    status: 'recording',
    elapsedLabel: '00:00:00',
    activeTarget,
    canPause: false,
    pauseNote: 'Recorder is still preparing the native capture session.',
    micEnabled: snapshot.settings.micEnabled,
    qualityPreset: snapshot.settings.qualityPreset,
    outputDirectory: snapshot.settings.outputDirectory,
  }
}

function optimisticRecorderFinalizing(
  snapshot: BootstrapSnapshot | null,
  recorder: RecorderSnapshot | null,
): RecorderSnapshot | null {
  if (!snapshot) {
    return null
  }

  return {
    ...(recorder ?? snapshot.recorder),
    status: 'finalizing',
    canPause: false,
    pauseNote: 'Recording is finalizing the output file. Pause is unavailable right now.',
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

function updateCaptureTargetSelectionSnapshot(
  snapshot: BootstrapSnapshot | null,
  captureTargetId: string,
  captureTargetLabel: string,
) {
  if (!snapshot) {
    return snapshot
  }

  return {
    ...snapshot,
    settings: {
      ...snapshot.settings,
      captureTargetId,
    },
    recorder: {
      ...snapshot.recorder,
      activeTarget: captureTargetLabel,
    },
  }
}

function updateRegionSourceCaptureTargetSelectionSnapshot(
  snapshot: BootstrapSnapshot | null,
  regionSourceCaptureTargetId: string,
) {
  if (!snapshot) {
    return snapshot
  }

  return {
    ...snapshot,
    settings: {
      ...snapshot.settings,
      captureTargetId: 'region:custom',
      regionSourceCaptureTargetId,
    },
  }
}

function updateAudioInputSelectionSnapshot(
  snapshot: BootstrapSnapshot | null,
  audioInputId: string,
) {
  if (!snapshot) {
    return snapshot
  }

  return {
    ...snapshot,
    settings: {
      ...snapshot.settings,
      audioInputId,
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

function updateDiagnosticsSnapshot(
  snapshot: BootstrapSnapshot | null,
  diagnostics: RuntimeDiagnostics,
) {
  if (!snapshot) {
    return snapshot
  }

  return {
    ...snapshot,
    diagnostics,
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

function updateShortcutsSnapshot(
  snapshot: BootstrapSnapshot | null,
  shortcuts: ShortcutBinding[],
) {
  if (!snapshot) {
    return snapshot
  }

  return {
    ...snapshot,
    shortcuts,
  }
}

export function useDesktopState() {
  const [snapshot, setSnapshot] = useState<BootstrapSnapshot | null>(null)
  const [recorder, setRecorder] = useState<RecorderSnapshot | null>(null)
  const [isLoading, setIsLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [actionError, setActionError] = useState<string | null>(null)
  const snapshotRef = useRef<BootstrapSnapshot | null>(null)
  const recorderRef = useRef<RecorderSnapshot | null>(null)

  useEffect(() => {
    snapshotRef.current = snapshot
  }, [snapshot])

  useEffect(() => {
    recorderRef.current = recorder
  }, [recorder])

  useEffect(() => {
    let isDisposed = false
    let deferredLoadTimer: number | null = null
    const deferredRefreshTimers = new Set<number>()
    let unlistenRecorder: () => void = () => undefined
    let unlistenRuntimeError: () => void = () => undefined
    let unlistenRecentSessionsRefresh: () => void = () => undefined
    let unlistenBootstrapRefresh: () => void = () => undefined

    async function refreshRecentSessionsInBackground(options?: {
      reportError?: boolean
    }) {
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

    async function refreshRuntimeDiagnostics(options?: {
      reportError?: boolean
    }) {
      try {
        const diagnostics = await desktopClient.getRuntimeDiagnostics()
        if (isDisposed) {
          return
        }

        startTransition(() => {
          setSnapshot((current) =>
            updateDiagnosticsSnapshot(current, diagnostics),
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
              : 'Unable to refresh runtime diagnostics.',
          )
        })
      }
    }

    async function refreshDeviceDiscovery(options?: { reportError?: boolean }) {
      try {
        const [captureTargets, audioInputs] = await Promise.all([
          desktopClient.getCaptureTargets(),
          desktopClient.getAudioInputs(),
        ])
        if (isDisposed) {
          return
        }

        startTransition(() => {
          setSnapshot((current) => {
            if (!current) {
              return current
            }

            return {
              ...current,
              captureTargets,
              audioInputs,
            }
          })
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
              : 'Unable to refresh capture devices.',
          )
        })
      }
    }

    function scheduleDeferredStartupRefresh() {
      if (deferredLoadTimer !== null) {
        window.clearTimeout(deferredLoadTimer)
      }

      for (const timer of deferredRefreshTimers) {
        window.clearTimeout(timer)
      }
      deferredRefreshTimers.clear()

      deferredLoadTimer = window.setTimeout(() => {
        deferredLoadTimer = null
        if (isDisposed) {
          return
        }

        const tasks = [
          () => void refreshDeviceDiscovery(),
          () => void refreshRuntimeDiagnostics(),
        ]

        tasks.forEach((task, index) => {
          const timer = window.setTimeout(() => {
            deferredRefreshTimers.delete(timer)
            if (isDisposed) {
              return
            }
            task()
          }, 1200 + index * 1000)

          deferredRefreshTimers.add(timer)
        })
      }, 240)
    }

    async function loadSnapshot() {
      try {
        const nextSnapshot = await desktopClient.getBootstrap()
        if (isDisposed) {
          return
        }
        startTransition(() => {
          setSnapshot(nextSnapshot)
          setRecorder(nextSnapshot.recorder)
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

    async function refreshSnapshot() {
      try {
        const nextSnapshot = await desktopClient.getBootstrap()
        if (isDisposed) {
          return
        }

        startTransition(() => {
          setSnapshot(nextSnapshot)
          setRecorder(nextSnapshot.recorder)
          setError(null)
          setActionError(null)
        })
      } catch {
        // Ignore snapshot refresh failures and keep the last known launcher state.
      }
    }

    function applyRecorderSnapshot(recorder: RecorderSnapshot) {
      startTransition(() => {
        setRecorder(recorder)
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

        void refreshRecentSessionsInBackground({ reportError: true })
      })
      .then((dispose) => {
        if (isDisposed) {
          dispose()
          return
        }

        unlistenRecentSessionsRefresh = dispose
      })

    function handleWindowFocus() {
      void refreshSnapshot()
    }

    window.addEventListener('focus', handleWindowFocus)
    void desktopClient.subscribeBootstrapRefreshRequest(() => {
      void refreshSnapshot()
    }).then((dispose) => {
      unlistenBootstrapRefresh = dispose
    })

    return () => {
      isDisposed = true
      window.removeEventListener('focus', handleWindowFocus)
      if (deferredLoadTimer !== null) {
        window.clearTimeout(deferredLoadTimer)
      }
      for (const timer of deferredRefreshTimers) {
        window.clearTimeout(timer)
      }
      deferredRefreshTimers.clear()
      unlistenRecorder()
      unlistenRuntimeError()
      unlistenRecentSessionsRefresh()
      unlistenBootstrapRefresh()
    }
  }, [])

  const toggleRecordingNow = useCallback(async () => {
    const currentSnapshot = snapshotRef.current
    const optimisticRecorder =
      recorderRef.current?.status === 'idle'
        ? optimisticRecorderStart(currentSnapshot, recorderRef.current)
        : recorderRef.current?.status === 'recording' ||
            recorderRef.current?.status === 'paused'
          ? optimisticRecorderFinalizing(currentSnapshot, recorderRef.current)
          : null

    if (optimisticRecorder) {
      startTransition(() => {
        setRecorder(optimisticRecorder)
        setActionError(null)
      })
    }

    const recorder = await desktopClient.toggleRecording()

    startTransition(() => {
      setRecorder(recorder)
      setActionError(null)
    })
  }, [])

  const {
    isStartupDelayed,
    isStartingRecording,
    toggleRecording,
  } = useRecordingCountdown({
    onClearError: () => {
      startTransition(() => {
        setActionError(null)
      })
    },
    onError: (message) => {
      startTransition(() => {
        setActionError(message)
      })
    },
    status: recorder?.status ?? 'idle',
    toggleRecordingNow,
  })

  const pauseResume = useCallback(async () => {
    try {
      const recorder = await desktopClient.pauseResume()
      if (recorder) {
        startTransition(() => {
          setRecorder(recorder)
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
  }, [])

  const toggleMicrophone = useCallback(async () => {
    try {
      const recorder = await desktopClient.toggleMicrophone()
      startTransition(() => {
        setRecorder(recorder)
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
  }, [])

  async function resetShortcuts() {
    try {
      const shortcuts = await desktopClient.resetShortcuts()
      startTransition(() => {
        setSnapshot((current) => updateShortcutsSnapshot(current, shortcuts))
        setActionError(null)
      })
    } catch (actionLoadError) {
      startTransition(() => {
        setActionError(
          actionLoadError instanceof Error
            ? actionLoadError.message
            : 'Unable to reset shortcuts.',
        )
      })
    }
  }

  async function updateShortcut(action: ShortcutAction, accelerator: string) {
    try {
      const shortcuts = await desktopClient.updateShortcut(action, accelerator)
      startTransition(() => {
        setSnapshot((current) => updateShortcutsSnapshot(current, shortcuts))
        setActionError(null)
      })
    } catch (actionLoadError) {
      startTransition(() => {
        setActionError(
          actionLoadError instanceof Error
            ? actionLoadError.message
            : 'Unable to update shortcut.',
        )
      })
    }
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

  async function updateSystemAudioEnabled(systemAudioEnabled: boolean) {
    try {
      const settings =
        await desktopClient.updateSystemAudioEnabled(systemAudioEnabled)
      startTransition(() => {
        setSnapshot((current) => updateSettingsSnapshot(current, settings))
        setActionError(null)
      })
    } catch (actionLoadError) {
      startTransition(() => {
        setActionError(
          actionLoadError instanceof Error
            ? actionLoadError.message
            : 'Unable to update system-audio capture.',
        )
      })
    }
  }

  const updateCaptureTarget = useCallback(async (captureTargetId: string) => {
    const currentSnapshot = snapshotRef.current
    const captureTargetLabel =
      currentSnapshot?.captureTargets.find((target) => target.id === captureTargetId)
        ?.label ?? 'Display'

    startTransition(() => {
      setSnapshot((current) =>
        updateCaptureTargetSelectionSnapshot(
          current,
          captureTargetId,
          captureTargetLabel,
        ),
      )
      setActionError(null)
    })

    try {
      const settings = await desktopClient.updateCaptureTarget(
        captureTargetId,
        captureTargetLabel,
      )
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
  }, [])

  const updateAudioInput = useCallback(async (audioInputId: string) => {
    startTransition(() => {
      setSnapshot((current) =>
        updateAudioInputSelectionSnapshot(current, audioInputId),
      )
      setActionError(null)
    })

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
  }, [])

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

  async function updateCustomRegion(
    regionX: number,
    regionY: number,
    regionWidth: number,
    regionHeight: number,
    regionSourceCaptureTargetId?: string,
    regionSourceOriginX?: number,
    regionSourceOriginY?: number,
    regionSourceScaleFactorMilli?: number,
  ) {
    try {
      const settings = await desktopClient.updateCustomRegion(
        regionX,
        regionY,
        regionWidth,
        regionHeight,
        regionSourceCaptureTargetId,
        regionSourceOriginX,
        regionSourceOriginY,
        regionSourceScaleFactorMilli,
      )
      const captureTargets = await desktopClient.getCaptureTargets()
      startTransition(() => {
        setSnapshot((current) => {
          const withSettings = updateSettingsSnapshot(current, settings)
          if (!withSettings) {
            return withSettings
          }

          return {
            ...withSettings,
            captureTargets,
          }
        })
        setActionError(null)
      })
    } catch (actionLoadError) {
      startTransition(() => {
        setActionError(
          actionLoadError instanceof Error
            ? actionLoadError.message
            : 'Unable to update custom region.',
        )
      })
    }
  }

  const updateRegionSourceCaptureTarget = useCallback(
    async (regionSourceCaptureTargetId: string) => {
      const currentSnapshot = snapshotRef.current
      if (!currentSnapshot) {
        return
      }

      startTransition(() => {
        setSnapshot((current) =>
          updateRegionSourceCaptureTargetSelectionSnapshot(
            current,
            regionSourceCaptureTargetId,
          ),
        )
        setActionError(null)
      })

      try {
        const settings = await desktopClient.updateRegionSourceCaptureTarget(
          regionSourceCaptureTargetId,
        )
        startTransition(() => {
          setSnapshot((current) => updateSettingsSnapshot(current, settings))
          setActionError(null)
        })
      } catch (actionLoadError) {
        startTransition(() => {
          setActionError(
            actionLoadError instanceof Error
              ? actionLoadError.message
              : 'Unable to update custom-region source display.',
          )
        })
      }
    },
    [],
  )

  const refreshPermissions = useCallback(async () => {
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
  }, [])

  const refreshRecentSessions = useCallback(async () => {
    try {
      const recentSessions = await desktopClient.getRecentRecordings()
      startTransition(() => {
        setSnapshot((current) =>
          updateRecentSessionsSnapshot(current, recentSessions),
        )
        setActionError(null)
      })
    } catch (actionLoadError) {
      startTransition(() => {
        setActionError(
          actionLoadError instanceof Error
            ? actionLoadError.message
            : 'Unable to refresh recent recordings.',
        )
      })
    }
  }, [])

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

  async function trashRecordings(recordingPaths: string[]) {
    try {
      const recentSessions = await desktopClient.trashRecordings(recordingPaths)
      startTransition(() => {
        setSnapshot((current) =>
          updateRecentSessionsSnapshot(current, recentSessions),
        )
        setActionError(null)
      })
    } catch (actionLoadError) {
      startTransition(() => {
        setActionError(
          actionLoadError instanceof Error
            ? actionLoadError.message
            : 'Unable to move recordings to Trash.',
        )
      })
    }
  }

  const showRegionSelector = useCallback(async () => {
    try {
      await desktopClient.showRegionSelector()
      startTransition(() => {
        setActionError(null)
      })
    } catch (actionLoadError) {
      startTransition(() => {
        setActionError(
          actionLoadError instanceof Error
            ? actionLoadError.message
            : 'Unable to open the region selector.',
        )
      })
    }
  }, [])

  return {
    actionError,
    error,
    isLoading,
    isStartupDelayed,
    isStartingRecording,
    focusLauncher: desktopClient.focusLauncher,
    hideHud: desktopClient.hideHud,
    hideTargetPreview: desktopClient.hideTargetPreview,
    openPermissionSettings,
    openRecording,
    pauseResume,
    refreshPermissions,
    refreshRecentSessions,
    revealRecordingInFolder,
    resetShortcuts,
    updateShortcut,
    requestPermission,
    saveRecordingCopy,
    showCustomRegionPreview: desktopClient.showCustomRegionPreview,
    trashRecordings,
    showHud: desktopClient.showHud,
    showRegionSelector,
    recorder,
    snapshot,
    toggleMicrophone,
    toggleRecording,
    updateCaptureTarget,
    updateCustomRegion,
    updateRegionSourceCaptureTarget,
    updateAudioInput,
    updateShowHudDuringRecording,
    updateSystemAudioEnabled,
    updateLaunchOnLogin,
    updateOutputDirectory,
    updateQualityPreset,
    pickOutputDirectory,
  }
}
