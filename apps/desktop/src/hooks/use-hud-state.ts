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
    let unlistenHudShown: () => void = () => undefined
    let refreshTimer: number | null = null

    async function loadRecorder(options?: { silent?: boolean }) {
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

        if (options?.silent) {
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

    refreshTimer = window.setInterval(() => {
      void loadRecorder({ silent: true })
    }, 1000)

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

    void desktopClient
      .subscribeHudShown(() => {
        if (isDisposed) {
          return
        }

        void loadRecorder()
      })
      .then((dispose) => {
        if (isDisposed) {
          dispose()
          return
        }

        unlistenHudShown = dispose
      })

    return () => {
      isDisposed = true
      if (refreshTimer !== null) {
        window.clearInterval(refreshTimer)
      }
      unlistenRecorder()
      unlistenRuntimeError()
      unlistenHudShown()
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
      const nextRecorder = await desktopClient.pauseResume()
      if (nextRecorder) {
        startTransition(() => {
          setRecorder(nextRecorder)
          setError(null)
        })
      }
    },
    toggleMicrophone: async () => {
      const nextRecorder = await desktopClient.toggleMicrophone()
      startTransition(() => {
        setRecorder(nextRecorder)
        setError(null)
      })
    },
    toggleRecording: async () => {
      const nextRecorder = await desktopClient.toggleRecording()
      startTransition(() => {
        setRecorder(nextRecorder)
        setError(null)
      })
    },
  }
}
