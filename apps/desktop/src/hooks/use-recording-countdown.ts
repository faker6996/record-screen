import { useCallback, useEffect, useRef, useState } from 'react'
import type { RecorderSnapshot } from '../types/desktop'

interface UseRecordingCountdownOptions {
  onClearError?: () => void
  onError: (message: string) => void
  seconds?: number
  startupTimeoutMs?: number
  status: RecorderSnapshot['status']
  toggleRecordingNow: () => Promise<void>
}

export function useRecordingCountdown({
  onClearError,
  onError,
  seconds = 3,
  startupTimeoutMs = 15000,
  status,
  toggleRecordingNow,
}: UseRecordingCountdownOptions) {
  const [countdownValue, setCountdownValue] = useState<number | null>(null)
  const [isStartingRecording, setIsStartingRecording] = useState(false)
  const [isStartupDelayed, setIsStartupDelayed] = useState(false)
  const timerRef = useRef<number | null>(null)
  const startTimeoutRef = useRef<number | null>(null)
  const toggleInFlightRef = useRef(false)
  const lastToggleAtRef = useRef(0)

  const beginToggleAttempt = useCallback(() => {
    const now = Date.now()
    if (toggleInFlightRef.current || now - lastToggleAtRef.current < 800) {
      return false
    }

    toggleInFlightRef.current = true
    lastToggleAtRef.current = now
    return true
  }, [])

  const finishToggleAttempt = useCallback(() => {
    toggleInFlightRef.current = false
  }, [])

  const clearCountdownTimer = useCallback(() => {
    if (timerRef.current !== null) {
      window.clearTimeout(timerRef.current)
      timerRef.current = null
    }
  }, [])

  const clearStartTimeout = useCallback(() => {
    if (startTimeoutRef.current !== null) {
      window.clearTimeout(startTimeoutRef.current)
      startTimeoutRef.current = null
    }
  }, [])

  const resetCountdownState = useCallback(() => {
    clearCountdownTimer()
    clearStartTimeout()
    setCountdownValue(null)
    setIsStartingRecording(false)
    setIsStartupDelayed(false)
  }, [clearCountdownTimer, clearStartTimeout])

  useEffect(() => {
    if (status !== 'idle') {
      resetCountdownState()
    }
  }, [resetCountdownState, status])

  useEffect(() => {
    return () => {
      clearCountdownTimer()
      clearStartTimeout()
    }
  }, [clearCountdownTimer, clearStartTimeout])

  const commitRecordingStart = useCallback(async () => {
    if (!beginToggleAttempt()) {
      return
    }

    setIsStartingRecording(true)
    setIsStartupDelayed(false)
    clearStartTimeout()
    startTimeoutRef.current = window.setTimeout(() => {
      setIsStartupDelayed(true)
      onError(
        'Recorder startup is taking longer than expected. The native capture backend may be stuck while preparing the session.',
      )
    }, startupTimeoutMs)

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
      finishToggleAttempt()
      clearStartTimeout()
      setIsStartingRecording(false)
    }
  }, [
    beginToggleAttempt,
    clearStartTimeout,
    finishToggleAttempt,
    onClearError,
    onError,
    startupTimeoutMs,
    toggleRecordingNow,
  ])

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
    if (status === 'finalizing') {
      return
    }

    if (status !== 'idle') {
      if (!beginToggleAttempt()) {
        return
      }

      resetCountdownState()
      try {
        await toggleRecordingNow()
      } finally {
        finishToggleAttempt()
      }
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
    beginToggleAttempt,
    countdownValue,
    finishToggleAttempt,
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
    isStartupDelayed,
    isStartingRecording,
    toggleRecording,
  }
}
