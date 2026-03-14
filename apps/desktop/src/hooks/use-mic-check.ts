import { useCallback, useEffect, useState } from 'react'
import { desktopClient } from '../services/desktop-client'
import type { MicCheckSnapshot } from '../types/desktop'

interface MicCheckState {
  active: boolean
  error: string | null
  hasSignal: boolean
  level: number
  supported: boolean
}

export function useMicCheck() {
  const [state, setState] = useState<MicCheckState>({
    active: false,
    error: null,
    hasSignal: false,
    level: 0,
    supported: true,
  })

  const applySnapshot = useCallback((snapshot: MicCheckSnapshot) => {
    setState((current) => ({
      ...current,
      active: snapshot.active,
      error: snapshot.error,
      hasSignal: snapshot.hasSignal,
      level: snapshot.level,
    }))
  }, [])

  const stop = useCallback(async () => {
    try {
      const snapshot = await desktopClient.stopMicCheck()
      applySnapshot(snapshot)
    } catch (error) {
      setState((current) => ({
        ...current,
        active: false,
        error:
          error instanceof Error ? error.message : 'Unable to stop microphone test.',
        hasSignal: false,
        level: 0,
      }))
    }
  }, [applySnapshot])

  const start = useCallback(async () => {
    try {
      const snapshot = await desktopClient.startMicCheck()
      applySnapshot(snapshot)
    } catch (error) {
      setState((current) => ({
        ...current,
        active: false,
        error:
          error instanceof Error
            ? error.message
            : 'Unable to access the selected microphone.',
        hasSignal: false,
        level: 0,
      }))
    }
  }, [applySnapshot])

  const toggle = useCallback(async () => {
    if (state.active) {
      await stop()
      return
    }

    await start()
  }, [start, state.active, stop])

  useEffect(() => {
    let unlisten: (() => void) | undefined

    void desktopClient.subscribeMicCheckState((snapshot) => {
      applySnapshot(snapshot)
    }).then((dispose) => {
      unlisten = dispose
    })

    return () => {
      unlisten?.()
      void stop()
    }
  }, [applySnapshot, stop])

  return {
    ...state,
    start,
    stop,
    toggle,
  }
}
