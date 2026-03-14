import { startTransition, useEffect, useState } from 'react'
import { desktopClient } from '../services/desktop-client'
import type { RecorderSnapshot } from '../types/desktop'

export function useHudState() {
  const [recorder, setRecorder] = useState<RecorderSnapshot | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [isLoading, setIsLoading] = useState(true)

  useEffect(() => {
    let isDisposed = false
    let unlistenRecorder: () => void = () => undefined
    let unlistenRuntimeError: () => void = () => undefined

    async function loadRecorder() {
      try {
        const nextRecorder = await desktopClient.getRecorderSnapshot()
        if (isDisposed) {
          return
        }

        startTransition(() => {
          setRecorder(nextRecorder)
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
              : 'Unable to load HUD state.',
          )
          setIsLoading(false)
        })
      }
    }

    void loadRecorder()

    void desktopClient
      .subscribeRecorderState((nextRecorder) => {
        if (isDisposed) {
          return
        }

        startTransition(() => {
          setRecorder(nextRecorder)
        })
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
          setError(message)
        })
      })
      .then((dispose) => {
        if (isDisposed) {
          dispose()
          return
        }

        unlistenRuntimeError = dispose
      })

    return () => {
      isDisposed = true
      unlistenRecorder()
      unlistenRuntimeError()
    }
  }, [])

  return {
    error,
    isLoading,
    recorder,
    focusLauncher: async () => {
      await desktopClient.focusLauncher()
    },
    pauseResume: async () => {
      await desktopClient.pauseResume()
    },
    toggleMicrophone: async () => {
      await desktopClient.toggleMicrophone()
    },
    toggleRecording: async () => {
      await desktopClient.toggleRecording()
    },
  }
}
