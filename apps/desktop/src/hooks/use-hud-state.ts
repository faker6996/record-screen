import { startTransition, useEffect, useState } from 'react'
import { desktopClient } from '../services/desktop-client'
import { useRecordingCountdown } from './use-recording-countdown'
import type { RecorderSnapshot } from '../types/desktop'

function optimisticRecorderStart(recorder: RecorderSnapshot | null) {
  if (!recorder) {
    return null
  }

  return {
    ...recorder,
    status: 'recording' as const,
    elapsedLabel: '00:00:00',
    canPause: false,
    pauseNote: 'Recorder is still preparing the native capture session.',
  }
}

function optimisticRecorderFinalizing(recorder: RecorderSnapshot | null) {
  if (!recorder) {
    return null
  }

  return {
    ...recorder,
    status: 'finalizing' as const,
    canPause: false,
    pauseNote: 'Recording is finalizing the output file. Pause is unavailable right now.',
  }
}

export function useHudState() {
  const [recorder, setRecorder] = useState<RecorderSnapshot | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [isLoading, setIsLoading] = useState(true)

  async function toggleRecordingNow() {
    const optimisticRecorder =
      recorder?.status === 'idle'
        ? optimisticRecorderStart(recorder)
        : recorder?.status === 'recording' || recorder?.status === 'paused'
          ? optimisticRecorderFinalizing(recorder)
          : null

    if (optimisticRecorder) {
      startTransition(() => {
        setRecorder(optimisticRecorder)
        setError(null)
      })
    }

    const nextRecorder = await desktopClient.toggleRecording()
    startTransition(() => {
      setRecorder(nextRecorder)
      setError(null)
    })
  }

  const {
    isStartupDelayed,
    isStartingRecording,
    toggleRecording,
  } = useRecordingCountdown({
    onClearError: () => {
      startTransition(() => {
        setError(null)
      })
    },
    onError: (message) => {
      startTransition(() => {
        setError(message)
      })
    },
    status: recorder?.status ?? 'idle',
    toggleRecordingNow,
  })

  useEffect(() => {
    let isDisposed = false
    let unlistenRecorder: () => void = () => undefined
    let unlistenRuntimeError: () => void = () => undefined
    let unlistenHudShown: () => void = () => undefined

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
      unlistenRecorder()
      unlistenRuntimeError()
      unlistenHudShown()
    }
  }, [])

  useEffect(() => {
    if (!recorder || recorder.status === 'idle') {
      return undefined
    }

    const timer = window.setInterval(() => {
      void desktopClient
        .getRecorderSnapshot()
        .then((nextRecorder) => {
          startTransition(() => {
            setRecorder((current) => {
              if (!current) {
                return nextRecorder
              }

              const unchanged =
                current.status === nextRecorder.status &&
                current.elapsedLabel === nextRecorder.elapsedLabel &&
                current.activeOutputPath === nextRecorder.activeOutputPath &&
                current.activeEncoderLabel === nextRecorder.activeEncoderLabel &&
                current.canPause === nextRecorder.canPause &&
                current.pauseNote === nextRecorder.pauseNote &&
                current.micEnabled === nextRecorder.micEnabled

              return unchanged ? current : nextRecorder
            })
          })
        })
        .catch(() => undefined)
    }, 750)

    return () => {
      window.clearInterval(timer)
    }
  }, [recorder])

  return {
    error,
    isLoading,
    isStartupDelayed,
    isStartingRecording,
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
    toggleRecording,
  }
}
