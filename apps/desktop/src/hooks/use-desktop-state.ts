import { startTransition, useEffect, useState } from 'react'
import { desktopClient } from '../services/desktop-client'
import type {
  AppSettings,
  BootstrapSnapshot,
  RecorderSnapshot,
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
      outputDirectory: recorder.outputDirectory,
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

export function useDesktopState() {
  const [snapshot, setSnapshot] = useState<BootstrapSnapshot | null>(null)
  const [currentWindowLabel, setCurrentWindowLabel] = useState('main')
  const [isLoading, setIsLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    let isDisposed = false
    let unlisten: () => void = () => undefined

    async function loadSnapshot() {
      try {
        const [nextSnapshot, nextWindowLabel] = await Promise.all([
          desktopClient.getBootstrap(),
          desktopClient.getCurrentWindowLabel(),
        ])
        if (isDisposed) {
          return
        }
        startTransition(() => {
          setSnapshot(nextSnapshot)
          setCurrentWindowLabel(nextWindowLabel)
          setError(null)
          setIsLoading(false)
        })
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

    void desktopClient.subscribeRecorderState((recorder) => {
      if (!isDisposed) {
        applyRecorderSnapshot(recorder)
      }
    }).then((dispose) => {
      if (isDisposed) {
        dispose()
        return
      }
      unlisten = dispose
    })

    return () => {
      isDisposed = true
      unlisten()
    }
  }, [])

  async function toggleRecording() {
    const recorder = await desktopClient.toggleRecording()
    startTransition(() => {
      setSnapshot((current) => updateRecorderSnapshot(current, recorder))
    })
  }

  async function pauseResume() {
    const recorder = await desktopClient.pauseResume()
    if (recorder) {
      startTransition(() => {
        setSnapshot((current) => updateRecorderSnapshot(current, recorder))
      })
    }
  }

  async function toggleMicrophone() {
    const recorder = await desktopClient.toggleMicrophone()
    startTransition(() => {
      setSnapshot((current) => updateRecorderSnapshot(current, recorder))
    })
  }

  async function resetShortcuts() {
    await desktopClient.resetShortcuts()
    const nextSnapshot = await desktopClient.getBootstrap()
    startTransition(() => {
      setSnapshot(nextSnapshot)
      setError(null)
      setIsLoading(false)
    })
  }

  async function updateQualityPreset(qualityPreset: string) {
    const settings = await desktopClient.updateQualityPreset(qualityPreset)
    startTransition(() => {
      setSnapshot((current) => updateSettingsSnapshot(current, settings))
    })
  }

  async function updateLaunchOnLogin(launchOnLogin: boolean) {
    const settings = await desktopClient.updateLaunchOnLogin(launchOnLogin)
    startTransition(() => {
      setSnapshot((current) => updateSettingsSnapshot(current, settings))
    })
  }

  async function updateOutputDirectory(outputDirectory: string) {
    const settings = await desktopClient.updateOutputDirectory(outputDirectory)
    startTransition(() => {
      setSnapshot((current) => updateSettingsSnapshot(current, settings))
    })
  }

  return {
    currentWindowLabel,
    snapshot,
    isLoading,
    error,
    focusLauncher: desktopClient.focusLauncher,
    hideHud: desktopClient.hideHud,
    pauseResume,
    resetShortcuts,
    showHud: desktopClient.showHud,
    toggleMicrophone,
    toggleRecording,
    updateLaunchOnLogin,
    updateOutputDirectory,
    updateQualityPreset,
  }
}
