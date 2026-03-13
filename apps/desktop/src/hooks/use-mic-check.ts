import { useCallback, useEffect, useRef, useState } from 'react'

interface MicCheckState {
  active: boolean
  error: string | null
  hasSignal: boolean
  level: number
  supported: boolean
}

const SIGNAL_THRESHOLD = 0.045
const LEVEL_SCALE = 4.8

export function useMicCheck() {
  const [state, setState] = useState<MicCheckState>({
    active: false,
    error: null,
    hasSignal: false,
    level: 0,
    supported:
      typeof navigator !== 'undefined' &&
      !!navigator.mediaDevices?.getUserMedia &&
      typeof window !== 'undefined' &&
      !!window.AudioContext,
  })
  const streamRef = useRef<MediaStream | null>(null)
  const audioContextRef = useRef<AudioContext | null>(null)
  const sourceRef = useRef<MediaStreamAudioSourceNode | null>(null)
  const analyserRef = useRef<AnalyserNode | null>(null)
  const frameRef = useRef<number | null>(null)

  const stop = useCallback(async () => {
    if (frameRef.current !== null) {
      window.cancelAnimationFrame(frameRef.current)
      frameRef.current = null
    }

    if (sourceRef.current) {
      sourceRef.current.disconnect()
      sourceRef.current = null
    }

    if (streamRef.current) {
      streamRef.current.getTracks().forEach((track) => track.stop())
      streamRef.current = null
    }

    if (audioContextRef.current) {
      try {
        await audioContextRef.current.close()
      } catch {
        // Ignore shutdown races during teardown.
      }
      audioContextRef.current = null
    }

    analyserRef.current = null

    setState((current) => ({
      ...current,
      active: false,
      hasSignal: false,
      level: 0,
    }))
  }, [])

  const start = useCallback(async () => {
    if (
      typeof navigator === 'undefined' ||
      !navigator.mediaDevices?.getUserMedia ||
      typeof window === 'undefined' ||
      !window.AudioContext
    ) {
      setState((current) => ({
        ...current,
        active: false,
        error: 'Mic testing is not supported on this device.',
        hasSignal: false,
        level: 0,
        supported: false,
      }))
      return
    }

    await stop()

    try {
      const stream = await navigator.mediaDevices.getUserMedia({
        audio: {
          autoGainControl: true,
          echoCancellation: true,
          noiseSuppression: true,
        },
        video: false,
      })
      const audioContext = new window.AudioContext()
      const source = audioContext.createMediaStreamSource(stream)
      const analyser = audioContext.createAnalyser()
      analyser.fftSize = 512
      analyser.smoothingTimeConstant = 0.82
      source.connect(analyser)

      streamRef.current = stream
      audioContextRef.current = audioContext
      sourceRef.current = source
      analyserRef.current = analyser

      const samples = new Uint8Array(analyser.fftSize)

      const tick = () => {
        const activeAnalyser = analyserRef.current
        if (!activeAnalyser) {
          return
        }

        activeAnalyser.getByteTimeDomainData(samples)

        let sumSquares = 0
        for (const sample of samples) {
          const normalized = (sample - 128) / 128
          sumSquares += normalized * normalized
        }

        const rms = Math.sqrt(sumSquares / samples.length)
        const normalizedLevel = Math.min(1, rms * LEVEL_SCALE)
        const hasSignal = normalizedLevel >= SIGNAL_THRESHOLD

        setState((current) => ({
          ...current,
          active: true,
          error: null,
          hasSignal,
          level: normalizedLevel,
          supported: true,
        }))

        frameRef.current = window.requestAnimationFrame(tick)
      }

      frameRef.current = window.requestAnimationFrame(tick)
    } catch (error) {
      await stop()
      setState((current) => ({
        ...current,
        error:
          error instanceof Error
            ? error.message
            : 'Unable to access the default microphone.',
      }))
    }
  }, [stop])

  const toggle = useCallback(async () => {
    if (state.active) {
      await stop()
      return
    }

    await start()
  }, [start, state.active, stop])

  useEffect(() => {
    return () => {
      void stop()
    }
  }, [stop])

  return {
    ...state,
    start,
    stop,
    toggle,
  }
}
