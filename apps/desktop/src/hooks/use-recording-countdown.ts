import { useCallback, useEffect, useRef, useState } from 'react'
import type { RecorderSnapshot } from '../types/desktop'

interface UseRecordingCountdownOptions {
  onClearError?: () => void
  onError: (message: string) => void
  startupTimeoutMs?: number
  status: RecorderSnapshot['status']
  toggleRecordingNow: () => Promise<void>
}

export function useRecordingCountdown({
  onClearError,
  onError,
  startupTimeoutMs = 15000,
  status,
  toggleRecordingNow,
}: UseRecordingCountdownOptions) {
  const [isStartingRecording, setIsStartingRecording] = useState(false)
  const [isStartupDelayed, setIsStartupDelayed] = useState(false)
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

  const clearStartTimeout = useCallback(() => {
    if (startTimeoutRef.current !== null) {
      window.clearTimeout(startTimeoutRef.current)
      startTimeoutRef.current = null
    }
  }, [])

  const resetCountdownState = useCallback(() => {
    clearStartTimeout()
    setIsStartingRecording(false)
    setIsStartupDelayed(false)
  }, [clearStartTimeout])

  useEffect(() => {
    if (status !== 'idle') {
      resetCountdownState()
    }
  }, [resetCountdownState, status])

  useEffect(() => {
    return () => {
      clearStartTimeout()
    }
  }, [clearStartTimeout])

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

    onClearError?.()
    await commitRecordingStart()
  }, [
    beginToggleAttempt,
    commitRecordingStart,
    finishToggleAttempt,
    isStartingRecording,
    onClearError,
    resetCountdownState,
    status,
    toggleRecordingNow,
  ])

  return {
    countdownValue: null,
    isCountingDown: false,
    isStartupDelayed,
    isStartingRecording,
    toggleRecording,
  }
}
