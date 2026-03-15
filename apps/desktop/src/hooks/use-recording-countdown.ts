import { useCallback, useEffect, useRef, useState } from 'react'
import type { RecorderSnapshot } from '../types/desktop'

interface UseRecordingCountdownOptions {
  onClearError?: () => void
  onError: (message: string) => void
  seconds?: number
  status: RecorderSnapshot['status']
  toggleRecordingNow: () => Promise<void>
}

export function useRecordingCountdown({
  onClearError,
  onError,
  seconds = 3,
  status,
  toggleRecordingNow,
}: UseRecordingCountdownOptions) {
  const [countdownValue, setCountdownValue] = useState<number | null>(null)
  const [isStartingRecording, setIsStartingRecording] = useState(false)
  const timerRef = useRef<number | null>(null)

  const clearCountdownTimer = useCallback(() => {
    if (timerRef.current !== null) {
      window.clearTimeout(timerRef.current)
      timerRef.current = null
    }
  }, [])

  const resetCountdownState = useCallback(() => {
    clearCountdownTimer()
    setCountdownValue(null)
    setIsStartingRecording(false)
  }, [clearCountdownTimer])

  useEffect(() => {
    if (status !== 'idle') {
      resetCountdownState()
    }
  }, [resetCountdownState, status])

  useEffect(() => {
    return () => {
      clearCountdownTimer()
    }
  }, [clearCountdownTimer])

  const commitRecordingStart = useCallback(async () => {
    setIsStartingRecording(true)

    try {
      await toggleRecordingNow()
      onClearError?.()
    } catch (error) {
      onError(
        error instanceof Error
          ? error.message
          : 'Unable to start or stop the recorder.',
      )
    } finally {
      setIsStartingRecording(false)
    }
  }, [onClearError, onError, toggleRecordingNow])

  useEffect(() => {
    if (countdownValue === null) {
      return
    }

    timerRef.current = window.setTimeout(() => {
      if (countdownValue > 1) {
        setCountdownValue(countdownValue - 1)
        return
      }

      setCountdownValue(null)
      void commitRecordingStart()
    }, 1000)

    return () => {
      clearCountdownTimer()
    }
  }, [clearCountdownTimer, commitRecordingStart, countdownValue])

  const toggleRecording = useCallback(async () => {
    if (status !== 'idle') {
      resetCountdownState()
      await toggleRecordingNow()
      return
    }

    if (isStartingRecording) {
      return
    }

    if (countdownValue !== null) {
      resetCountdownState()
      onClearError?.()
      return
    }

    onClearError?.()
    setCountdownValue(seconds)
  }, [
    countdownValue,
    isStartingRecording,
    onClearError,
    resetCountdownState,
    seconds,
    status,
    toggleRecordingNow,
  ])

  return {
    countdownValue,
    isCountingDown: countdownValue !== null,
    isStartingRecording,
    toggleRecording,
  }
}
