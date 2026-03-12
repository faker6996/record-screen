import { Kbd } from '../../../components/Kbd'
import type { RecorderSnapshot } from '../../../types/desktop'

interface HudSurfaceProps {
  onPauseResume: () => Promise<void>
  onToggleRecording: () => Promise<void>
  recorder: RecorderSnapshot
}

export function HudSurface({
  onPauseResume,
  onToggleRecording,
  recorder,
}: HudSurfaceProps) {
  const isIdle = recorder.status === 'idle'
  const pauseLabel = recorder.status === 'paused' ? 'Resume' : 'Pause'

  return (
    <main className="hud">
      <section className="hud__card">
        <div className="hud__topline">
          <span className={`status-dot status-${recorder.status}`} />
          <strong>{recorder.elapsedLabel}</strong>
          <span className="pill">
            Mic {recorder.micEnabled ? 'on' : 'off'}
          </span>
        </div>

        <div className="hud__actions">
          <button
            className={`button button--primary ${isIdle ? 'button--record' : 'button--stop'}`}
            onClick={() => void onToggleRecording()}
            type="button"
          >
            {isIdle ? 'Start' : 'Stop'}
          </button>
          <button
            className="button button--secondary"
            disabled={isIdle}
            onClick={() => void onPauseResume()}
            type="button"
          >
            {pauseLabel}
          </button>
        </div>

        <div className="hud__hint">
          <Kbd>CmdOrCtrl</Kbd>
          <Kbd>Shift</Kbd>
          <Kbd>R</Kbd>
        </div>
      </section>
    </main>
  )
}
