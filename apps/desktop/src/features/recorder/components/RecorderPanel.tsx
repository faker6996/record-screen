import { Kbd } from '../../../components/Kbd'
import type { RecorderSnapshot } from '../../../types/desktop'

interface RecorderPanelProps {
  recorder: RecorderSnapshot
  onPauseResume: () => Promise<void>
  onToggleMicrophone: () => Promise<void>
  onToggleRecording: () => Promise<void>
}

export function RecorderPanel({
  recorder,
  onPauseResume,
  onToggleMicrophone,
  onToggleRecording,
}: RecorderPanelProps) {
  const isIdle = recorder.status === 'idle'
  const isPaused = recorder.status === 'paused'
  const title = isIdle
    ? 'Ready to record'
    : isPaused
      ? 'Paused and waiting'
      : 'Recording right now'

  return (
    <section className="panel recorder-panel">
      <div className="panel-header">
        <div>
          <p className="eyebrow">Recorder</p>
          <h2>{title}</h2>
          <p className="subtle-copy">
            Review the target and defaults below, then hit the main button.
          </p>
        </div>
        <span className={`status-pill status-${recorder.status}`}>
          <span className={`status-dot status-${recorder.status}`} />
          {recorder.status}
        </span>
      </div>

      <div className="hero-metrics recorder-overview">
        <article>
          <span className="metric-label">Elapsed</span>
          <strong>{recorder.elapsedLabel}</strong>
        </article>
        <article>
          <span className="metric-label">Target</span>
          <strong>{recorder.activeTarget}</strong>
        </article>
        <article>
          <span className="metric-label">Mic</span>
          <strong>{recorder.micEnabled ? 'Enabled' : 'Muted'}</strong>
        </article>
      </div>

      <div className="recorder-checklist">
        <article className="checklist-item">
          <span className="metric-label">Quality preset</span>
          <strong>{recorder.qualityPreset}</strong>
        </article>
        <article className="checklist-item">
          <span className="metric-label">Output folder</span>
          <strong>{recorder.outputDirectory}</strong>
        </article>
      </div>

      <div className="hero-actions">
        <button
          className={`primary-button ${isIdle ? 'record' : 'stop'}`}
          onClick={() => void onToggleRecording()}
          type="button"
        >
          {isIdle ? 'Start recording' : 'Stop recording'}
        </button>
        <button
          className="secondary-button"
          disabled={isIdle}
          onClick={() => void onPauseResume()}
          type="button"
        >
          {isPaused ? 'Resume' : 'Pause'}
        </button>
        <button
          className="secondary-button"
          onClick={() => void onToggleMicrophone()}
          type="button"
        >
          {recorder.micEnabled ? 'Mute mic' : 'Enable mic'}
        </button>
      </div>

      <div className="inline-shortcuts">
        <span>
          Global start/stop <Kbd>CmdOrCtrl</Kbd> <Kbd>Shift</Kbd> <Kbd>R</Kbd>
        </span>
        <span className="subtle-copy">
          {isIdle
            ? 'Launcher can be hidden after setup.'
            : 'Use HUD or shortcut to control the session.'}
        </span>
      </div>
    </section>
  )
}
