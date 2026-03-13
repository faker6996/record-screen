import { AppWindow, Circle, Mic, MicOff, Pause, Play, Square } from 'lucide-react'
import { Kbd } from '../../../components/Kbd'
import type { RecorderSnapshot } from '../../../types/desktop'

interface HudSurfaceProps {
  onFocusLauncher: () => Promise<void>
  onPauseResume: () => Promise<void>
  onToggleMicrophone: () => Promise<void>
  onToggleRecording: () => Promise<void>
  recorder: RecorderSnapshot
}

export function HudSurface({
  onFocusLauncher,
  onPauseResume,
  onToggleMicrophone,
  onToggleRecording,
  recorder,
}: HudSurfaceProps) {
  const isIdle = recorder.status === 'idle'
  const pauseLabel = recorder.status === 'paused' ? 'Resume' : 'Pause'

  return (
    <main className="hud">
      <section className="hud__card">
        <div className="hud__elapsed">
          <span className={`status-dot status-${recorder.status}`} />
          <strong>{recorder.elapsedLabel}</strong>
        </div>

        <div className="hud__divider" />

        <div className="hud__actions">
          <button
            className="button button--secondary hud__icon-button"
            disabled={isIdle}
            onClick={() => void onPauseResume()}
            type="button"
          >
            {recorder.status === 'paused' ? (
              <Play aria-hidden="true" size={16} strokeWidth={1.9} />
            ) : (
              <Pause aria-hidden="true" size={16} strokeWidth={1.9} />
            )}
            {pauseLabel}
          </button>
          <button
            className={`button ${isIdle ? 'button--primary button--record' : 'button--primary button--stop'} hud__icon-button`}
            onClick={() => void onToggleRecording()}
            type="button"
          >
            {isIdle ? (
              <Circle aria-hidden="true" size={15} strokeWidth={1.9} />
            ) : (
              <Square aria-hidden="true" size={15} strokeWidth={1.9} />
            )}
            {isIdle ? 'Start' : 'Stop'}
          </button>
        </div>

        <div className="hud__divider" />

        <div className="hud__meta">
          <button
            className="button button--secondary hud__icon-button"
            onClick={() => void onToggleMicrophone()}
            type="button"
          >
            {recorder.micEnabled ? (
              <Mic aria-hidden="true" size={16} strokeWidth={1.9} />
            ) : (
              <MicOff aria-hidden="true" size={16} strokeWidth={1.9} />
            )}
            Mic {recorder.micEnabled ? 'on' : 'off'}
          </button>
          <button
            className="button button--secondary hud__icon-button"
            onClick={() => void onFocusLauncher()}
            type="button"
          >
            <AppWindow aria-hidden="true" size={16} strokeWidth={1.9} />
            Launcher
          </button>
        </div>

        <div className="hud__divider" />

        <div className="hud__hint">
          <Kbd>CmdOrCtrl</Kbd>
          <Kbd>Shift</Kbd>
          <Kbd>R</Kbd>
        </div>
      </section>
    </main>
  )
}
