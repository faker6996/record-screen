import { Mic, MicOff, Monitor, Pause, Play } from 'lucide-react'
import { Combobox } from '../../../components/Combobox'
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
  const recordLabel = isIdle ? 'REC' : 'STOP'

  return (
    <section className="recorder-panel" aria-label={title}>
      <div className="recorder-panel__hero">
        <div className="recorder-panel__record-shell">
          <div
            className={`recorder-panel__record-glow recorder-panel__record-glow--${recorder.status}`}
          />
          <button
            className={`recorder-panel__record-button recorder-panel__record-button--${recorder.status}`}
            onClick={() => void onToggleRecording()}
            type="button"
          >
            <span className="recorder-panel__record-core" />
            <span className="recorder-panel__record-label">{recordLabel}</span>
          </button>
        </div>

        <div className="recorder-panel__hero-meta">
          <strong className="recorder-panel__timer">{recorder.elapsedLabel}</strong>
          <div className="recorder-panel__hero-actions">
            <button
              className="button button--secondary recorder-panel__hero-button"
              disabled={isIdle}
              onClick={() => void onPauseResume()}
              type="button"
            >
              {isPaused ? (
                <Play aria-hidden="true" size={16} strokeWidth={1.9} />
              ) : (
                <Pause aria-hidden="true" size={16} strokeWidth={1.9} />
              )}
              {isPaused ? 'Resume' : 'Pause'}
            </button>
            <button
              className="button button--secondary recorder-panel__hero-button"
              onClick={() => void onToggleMicrophone()}
              type="button"
            >
              {recorder.micEnabled ? (
                <Mic aria-hidden="true" size={16} strokeWidth={1.9} />
              ) : (
                <MicOff aria-hidden="true" size={16} strokeWidth={1.9} />
              )}
              {recorder.micEnabled ? 'Mic on' : 'Mic off'}
            </button>
          </div>
        </div>
      </div>

      <div className="recorder-panel__control-grid">
        <section className="recorder-panel__control-card">
          <div className="recorder-panel__control-header">
            <div>
              <span className="metric-label recorder-panel__metric-label">
                <Monitor aria-hidden="true" size={14} strokeWidth={1.9} />
                Capture target
              </span>
              <strong>What to record</strong>
            </div>
          </div>
          <Combobox
            ariaLabel="Capture target"
            className="recorder-panel__target-combobox"
            disabled={!isIdle}
            onChange={(nextCaptureTargetId) => {
              void onUpdateCaptureTarget(nextCaptureTargetId)
            }}
            options={captureTargets.map((target) => ({
              value: target.id,
              label: target.label,
            }))}
            value={selectedCaptureTargetId}
          />
          <p className="subtle-copy recorder-panel__helper">
            {captureTargets.find((target) => target.id === selectedCaptureTargetId)?.description}
          </p>
        </section>

        <section className="recorder-panel__control-card">
          <div className="recorder-panel__control-header">
            <div>
              <span className="metric-label recorder-panel__metric-label">
                <Mic aria-hidden="true" size={14} strokeWidth={1.9} />
                Microphone
              </span>
              <strong>Mic input</strong>
            </div>
          </div>

          <button
            className={`recorder-panel__mic-toggle ${
              recorder.micEnabled ? 'recorder-panel__mic-toggle--active' : ''
            }`}
            onClick={() => void onToggleMicrophone()}
            type="button"
          >
            <span className="recorder-panel__mic-copy">
              {recorder.micEnabled ? (
                <Mic aria-hidden="true" size={16} strokeWidth={1.9} />
              ) : (
                <MicOff aria-hidden="true" size={16} strokeWidth={1.9} />
              )}
              {recorder.micEnabled ? 'Default Input' : 'Microphone muted'}
            </span>
            <strong>{recorder.micEnabled ? 'On' : 'Off'}</strong>
          </button>
        </section>
      </div>

      {recorder.activeOutputPath ? (
        <div className="recorder-panel__active-file">
          <span className="metric-label">Active file</span>
          <strong>{recorder.activeOutputPath}</strong>
        </div>
      ) : null}

      <div className="recorder-panel__footer">
        <span className="recorder-panel__shortcut">
          Global start/stop <Kbd>CmdOrCtrl</Kbd> <Kbd>Shift</Kbd> <Kbd>R</Kbd>
        </span>
        <span className="subtle-copy">
          {recorder.qualityPreset} · {recorder.outputDirectory}
        </span>
      </div>
    </section>
  )
}
