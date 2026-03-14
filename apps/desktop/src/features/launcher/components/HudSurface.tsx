import { Mic, MicOff, Pause, Play, Scan, Square } from 'lucide-react'
import { useEffect, useState } from 'react'
import type { RecorderSnapshot } from '../../../types/desktop'

interface HudSurfaceProps {
  onFocusLauncher: () => Promise<void>
  onPauseResume: () => Promise<void>
  onToggleMicrophone: () => Promise<void>
  onToggleRecording: () => Promise<void>
  recorder: RecorderSnapshot
}

function parseElapsedSeconds(label: string, status: RecorderSnapshot['status']) {
  if (status === 'idle') {
    return 0
  }

  const normalized = label.replace(/^Paused at\s+/i, '').trim()
  const parts = normalized
    .split(':')
    .map((part) => Number.parseInt(part, 10))
    .filter((part) => Number.isFinite(part))

  if (parts.length === 3) {
    return parts[0] * 3600 + parts[1] * 60 + parts[2]
  }

  if (parts.length === 2) {
    return parts[0] * 60 + parts[1]
  }

  return 0
}

function formatHudTime(totalSeconds: number) {
  const hours = Math.floor(totalSeconds / 3600)
  const minutes = Math.floor((totalSeconds % 3600) / 60)
  const seconds = totalSeconds % 60

  return [hours, minutes, seconds]
    .map((value) => value.toString().padStart(2, '0'))
    .join(':')
}

function HudElapsed({
  initialElapsedSeconds,
  status,
}: {
  initialElapsedSeconds: number
  status: RecorderSnapshot['status']
}) {
  const [displayElapsedSeconds, setDisplayElapsedSeconds] = useState(
    initialElapsedSeconds,
  )

  useEffect(() => {
    if (status !== 'recording') {
      return undefined
    }

    const timer = window.setInterval(() => {
      setDisplayElapsedSeconds((current) => current + 1)
    }, 1000)

    return () => {
      window.clearInterval(timer)
    }
  }, [status])

  return <strong>{formatHudTime(displayElapsedSeconds)}</strong>
}

export function HudSurface({
  onFocusLauncher,
  onPauseResume,
  onToggleMicrophone,
  onToggleRecording,
  recorder,
}: HudSurfaceProps) {
  const isIdle = recorder.status === 'idle'
  const isPaused = recorder.status === 'paused'
  const parsedElapsedSeconds = parseElapsedSeconds(recorder.elapsedLabel, recorder.status)

  return (
    <main className="hud">
      <section className="hud__card">
        <div
          className="hud__elapsed hud__drag-region"
          data-tauri-drag-region
          title="Drag HUD"
        >
          <span className={`status-dot status-${recorder.status}`} />
          <HudElapsed
            initialElapsedSeconds={parsedElapsedSeconds}
            key={`${recorder.activeOutputPath ?? 'idle'}:${recorder.status}`}
            status={recorder.status}
          />
        </div>

        <div className="hud__divider" />

        <div className="hud__controls">
          <button
            aria-label={recorder.status === 'paused' ? 'Resume recording' : 'Pause recording'}
            aria-pressed={isPaused}
            className={`button button--secondary hud__action-button ${
              isPaused ? 'hud__icon-button--active' : ''
            }`}
            disabled={isIdle}
            onClick={() => void onPauseResume()}
            title={recorder.status === 'paused' ? 'Resume' : 'Pause'}
            type="button"
          >
            {isPaused ? (
              <>
                <Play aria-hidden="true" size={16} strokeWidth={1.9} />
                <span>Resume</span>
              </>
            ) : (
              <>
                <Pause aria-hidden="true" size={16} strokeWidth={1.9} />
                <span>Pause</span>
              </>
            )}
          </button>
          <button
            aria-label={isIdle ? 'Start recording' : 'Stop recording'}
            className={`button ${isIdle ? 'button--primary button--record' : 'button--primary button--stop'} hud__icon-button`}
            onClick={() => void onToggleRecording()}
            title={isIdle ? 'Start recording' : 'Stop recording'}
            type="button"
          >
            <Square
              aria-hidden="true"
              fill="currentColor"
              size={14}
              strokeWidth={1.9}
            />
          </button>
          <button
            aria-label={recorder.micEnabled ? 'Mute microphone' : 'Unmute microphone'}
            aria-pressed={recorder.micEnabled}
            className={`button button--secondary hud__icon-button ${
              recorder.micEnabled ? 'hud__icon-button--active' : ''
            }`}
            onClick={() => void onToggleMicrophone()}
            title={recorder.micEnabled ? 'Mute microphone' : 'Unmute microphone'}
            type="button"
          >
            {recorder.micEnabled ? (
              <Mic aria-hidden="true" size={16} strokeWidth={1.9} />
            ) : (
              <MicOff aria-hidden="true" size={16} strokeWidth={1.9} />
            )}
          </button>
          <button
            aria-label="Open launcher"
            className="button button--secondary hud__icon-button"
            onClick={() => void onFocusLauncher()}
            title="Open launcher"
            type="button"
          >
            <Scan aria-hidden="true" size={16} strokeWidth={1.9} />
          </button>
        </div>
      </section>
    </main>
  )
}
