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

  return (
    <section className="panel recorder-panel">
      <div className="panel-header">
        <span className={`status-dot status-${recorder.status}`} />
        <div>
          <p className="eyebrow">Recorder</p>
          <h2>
            {isIdle
              ? 'Ready for a capture burst'
              : isPaused
                ? 'Paused with session preserved'
                : 'Recording in progress'}
          </h2>
        </div>
      </div>

      <div className="hero-metrics">
        <div>
          <span className="metric-label">Elapsed</span>
          <strong>{recorder.elapsedLabel}</strong>
        </div>
        <div>
          <span className="metric-label">Target</span>
          <strong>{recorder.activeTarget}</strong>
        </div>
        <div>
          <span className="metric-label">Quality</span>
          <strong>{recorder.qualityPreset}</strong>
        </div>
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
          Trigger anytime with <Kbd>CmdOrCtrl</Kbd> <Kbd>Shift</Kbd>{' '}
          <Kbd>R</Kbd>
        </span>
        <span className="subtle-copy">{recorder.outputDirectory}</span>
      </div>
    </section>
  )
}
