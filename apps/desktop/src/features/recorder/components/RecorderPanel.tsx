import { Kbd } from '../../../components/Kbd'
import type {
  CaptureTargetOption,
  RecorderSnapshot,
} from '../../../types/desktop'

interface RecorderPanelProps {
  captureTargets: CaptureTargetOption[]
  recorder: RecorderSnapshot
  onPauseResume: () => Promise<void>
  onUpdateCaptureTarget: (captureTargetId: string) => Promise<void>
  onToggleMicrophone: () => Promise<void>
  onToggleRecording: () => Promise<void>
  selectedCaptureTargetId: string
}

export function RecorderPanel({
  captureTargets,
  recorder,
  onPauseResume,
  onUpdateCaptureTarget,
  onToggleMicrophone,
  onToggleRecording,
  selectedCaptureTargetId,
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
      <div className="panel__header">
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

      <div className="recorder-panel__metrics">
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

      <div className="recorder-panel__checklist">
        <article className="recorder-panel__checklist-item">
          <span className="metric-label">Quality preset</span>
          <strong>{recorder.qualityPreset}</strong>
        </article>
        <article className="recorder-panel__checklist-item">
          <span className="metric-label">Output folder</span>
          <strong>{recorder.outputDirectory}</strong>
        </article>
      </div>

      <div className="recorder-panel__targets">
        <div className="recorder-panel__targets-copy">
          <span className="metric-label">Capture target</span>
          <strong>Choose full desktop or a single display before recording</strong>
        </div>
        <div className="recorder-panel__target-grid">
          {captureTargets.map((target) => (
            <button
              className={`chip recorder-panel__target-chip ${
                target.id === selectedCaptureTargetId ? 'chip--active' : ''
              }`}
              disabled={!isIdle}
              key={target.id}
              onClick={() => void onUpdateCaptureTarget(target.id)}
              type="button"
            >
              <span>{target.label}</span>
              <small>{target.description}</small>
            </button>
          ))}
        </div>
      </div>

      {recorder.activeOutputPath ? (
        <p className="subtle-copy recorder-panel__file">
          Active file: {recorder.activeOutputPath}
        </p>
      ) : null}

      <div className="recorder-panel__actions">
        <button
          className={`button button--primary ${isIdle ? 'button--record' : 'button--stop'}`}
          onClick={() => void onToggleRecording()}
          type="button"
        >
          {isIdle ? 'Start recording' : 'Stop recording'}
        </button>
        <button
          className="button button--secondary"
          disabled={isIdle}
          onClick={() => void onPauseResume()}
          type="button"
        >
          {isPaused ? 'Resume' : 'Pause'}
        </button>
        <button
          className="button button--secondary"
          onClick={() => void onToggleMicrophone()}
          type="button"
        >
          {recorder.micEnabled ? 'Mute mic' : 'Enable mic'}
        </button>
      </div>

      <div className="recorder-panel__shortcuts">
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
