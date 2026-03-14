import { AudioLines, Mic, Monitor, Pause, Play } from 'lucide-react'
import { useEffect } from 'react'
import { Combobox } from '../../../components/Combobox'
import { useMicCheck } from '../../../hooks/use-mic-check'
import type {
  AudioInputOption,
  CaptureTargetOption,
  RecorderSnapshot,
  RuntimeDiagnostics,
} from '../../../types/desktop'

interface RecorderPanelProps {
  audioInputs: AudioInputOption[]
  captureTargets: CaptureTargetOption[]
  recorder: RecorderSnapshot
  diagnostics: RuntimeDiagnostics
  onOpenRegionSelector: () => Promise<void>
  onPauseResume: () => Promise<void>
  onUpdateAudioInput: (audioInputId: string) => Promise<void>
  onUpdateCaptureTarget: (captureTargetId: string) => Promise<void>
  onToggleMicrophone: () => Promise<void>
  onToggleRecording: () => Promise<void>
  selectedAudioInputId: string
  selectedCaptureTargetId: string
  systemAudioEnabled: boolean
  runtimeError: string | null
}

export function RecorderPanel({
  audioInputs,
  captureTargets,
  recorder,
  diagnostics,
  onOpenRegionSelector,
  onPauseResume,
  onUpdateAudioInput,
  onUpdateCaptureTarget,
  onToggleMicrophone,
  onToggleRecording,
  selectedAudioInputId,
  selectedCaptureTargetId,
  systemAudioEnabled,
  runtimeError,
}: RecorderPanelProps) {
  const {
    active: micCheckActive,
    error: micCheckError,
    hasSignal,
    level: micLevel,
    supported: micCheckSupported,
    toggle: toggleMicCheck,
    stop: stopMicCheck,
  } = useMicCheck()

  const isIdle = recorder.status === 'idle'
  const isPaused = recorder.status === 'paused'
  const recordLabel = isIdle ? 'REC' : 'STOP'
  const selectedCaptureTarget =
    captureTargets.find((target) => target.id === selectedCaptureTargetId) ?? null
  const selectedAudioInput =
    audioInputs.find((input) => input.id === selectedAudioInputId) ?? null
  const microphoneOptions = audioInputs.filter((input) => input.kind !== 'system')
  const selectedAudioInputKind = selectedAudioInput?.kind ?? 'default'
  const micLevelPercent = recorder.micEnabled ? Math.round(micLevel * 100) : 0
  const micEnumerationUnavailable =
    selectedAudioInput?.id === 'default' &&
    audioInputs.length === 1 &&
    selectedAudioInput.description
      .toLowerCase()
      .includes('could not enumerate directshow microphone devices')
  const audioStatusLabel = !recorder.micEnabled
    ? 'Audio off'
    : micEnumerationUnavailable
      ? 'No mic detected'
      : selectedAudioInput?.label ?? 'Default input'
  const systemAudioSelected = selectedAudioInputKind === 'system'
  const micCheckDisabled =
    !recorder.micEnabled ||
    !micCheckSupported ||
    !isIdle ||
    micEnumerationUnavailable ||
    systemAudioSelected
  const micCheckLabel = micCheckError
    ? micCheckError
    : micCheckActive
      ? hasSignal
        ? 'Mic detected'
        : 'Listening for input'
      : systemAudioSelected
        ? 'System audio sources do not use microphone level testing.'
      : micEnumerationUnavailable
        ? 'Windows could not inspect microphone devices yet.'
      : recorder.micEnabled
        ? selectedAudioInput?.label ?? 'Default input'
        : 'Audio capture muted'
  const title = isIdle ? 'Ready to Record' : isPaused ? 'Paused' : 'Recording'
  const copy = isIdle
    ? 'Select your target and start capturing.'
    : isPaused
      ? 'Capture is paused. Resume when you are ready.'
      : 'Recording is in progress. Stop when your session is complete.'
  const customRegionSelected = selectedCaptureTargetId === 'region:custom'

  useEffect(() => {
    if (!isIdle && micCheckActive) {
      void stopMicCheck()
    }
  }, [isIdle, micCheckActive, stopMicCheck])

  return (
    <section className="recorder-panel" aria-label="Recorder controls">
      <header className="recorder-panel__intro">
        <h2 className="recorder-panel__title">{title}</h2>
        <p className="recorder-panel__copy">{copy}</p>
        <div className="recorder-panel__status-row">
          <span
            className={`recorder-panel__status-pill recorder-panel__status-pill--${recorder.status}`}
          >
            {recorder.status}
          </span>
          <span className="recorder-panel__status-meta">{selectedCaptureTarget?.label}</span>
          <span className="recorder-panel__status-meta">{audioStatusLabel}</span>
          {systemAudioEnabled ? (
            <span className="recorder-panel__status-meta">System audio on</span>
          ) : null}
        </div>
      </header>

      {runtimeError ? (
        <section className="recorder-panel__error" role="alert">
          <strong>Recorder issue</strong>
          <p>{runtimeError}</p>
        </section>
      ) : null}

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

        {!isIdle ? (
          <div className="recorder-panel__runtime">
            <strong className="recorder-panel__timer">{recorder.elapsedLabel}</strong>
            <button
              className={`recorder-panel__pause-button ${
                isPaused ? 'recorder-panel__pause-button--paused' : ''
              }`}
              onClick={() => void onPauseResume()}
              type="button"
            >
              {isPaused ? (
                <>
                  <Play aria-hidden="true" size={18} strokeWidth={2} />
                  <span>Resume</span>
                </>
              ) : (
                <>
                  <Pause aria-hidden="true" size={18} strokeWidth={2} />
                  <span>Pause</span>
                </>
              )}
            </button>
          </div>
        ) : null}
      </div>

      <div className="recorder-panel__control-grid">
        <section className="recorder-panel__control-card">
          <div className="recorder-panel__control-header">
            <span className="metric-label recorder-panel__metric-label">
              <Monitor aria-hidden="true" size={16} strokeWidth={1.9} />
              Capture target
            </span>
          </div>
          <Combobox
            ariaLabel="Capture target"
            className="recorder-panel__target-combobox"
            disabled={!isIdle}
            onChange={(nextCaptureTargetId) => {
              if (nextCaptureTargetId === 'region:custom') {
                void onOpenRegionSelector()
                return
              }

              void onUpdateCaptureTarget(nextCaptureTargetId)
            }}
            options={captureTargets.map((target) => ({
              value: target.id,
              label: target.label,
            }))}
            value={selectedCaptureTargetId}
          />
          <p className="subtle-copy recorder-panel__helper">
            {selectedCaptureTarget?.description}
          </p>
          {customRegionSelected && diagnostics.supportsCustomRegion ? (
            <div>
              <button
                className="button button--secondary"
                disabled={!isIdle}
                onClick={() => {
                  void onOpenRegionSelector()
                }}
                type="button"
              >
                Select on screen again
              </button>
            </div>
          ) : null}
        </section>

        <section className="recorder-panel__control-card">
          <div className="recorder-panel__control-header recorder-panel__control-header--split">
            <span className="metric-label recorder-panel__metric-label">
              <Mic aria-hidden="true" size={16} strokeWidth={1.9} />
              Audio input
            </span>
            <button
              aria-label={recorder.micEnabled ? 'Disable audio capture' : 'Enable audio capture'}
              aria-pressed={recorder.micEnabled}
              className={`recorder-panel__switch ${
                recorder.micEnabled ? 'recorder-panel__switch--active' : ''
              }`}
              onClick={() => void onToggleMicrophone()}
              type="button"
            >
              <span className="recorder-panel__switch-thumb" />
            </button>
          </div>

          <Combobox
            ariaLabel="Audio input"
            className="recorder-panel__target-combobox"
            disabled={!recorder.micEnabled || !isIdle}
            onChange={(nextAudioInputId) => {
              void onUpdateAudioInput(nextAudioInputId)
            }}
            options={microphoneOptions.map((input) => ({
              value: input.id,
              label: input.label,
            }))}
            value={selectedAudioInputId}
          />
          <p className="subtle-copy recorder-panel__helper">
            {systemAudioEnabled && diagnostics.supportsSystemAudio
              ? `${selectedAudioInput?.description ?? 'Choose a microphone input.'} System audio will be mixed in from the current loopback source.`
              : selectedAudioInput?.description ??
                'Choose a microphone input for narration.'}
          </p>

          {micEnumerationUnavailable ? (
            <div
              className="recorder-panel__warning"
              role="status"
              aria-live="polite"
            >
              <strong>Windows microphone fallback</strong>
              <p>
                Recording will continue without microphone narration until
                Windows exposes a usable DirectShow input.
              </p>
            </div>
          ) : null}

          <div className="recorder-panel__mic-check">
            <div className="recorder-panel__mic-level">
              <div className="recorder-panel__mic-level-copy">
                <span>Input Level</span>
                <span>{micLevelPercent}%</span>
              </div>
              <div className="recorder-panel__mic-level-track" aria-hidden="true">
                <div
                  className={`recorder-panel__mic-level-fill ${
                    micLevelPercent > 85
                      ? 'recorder-panel__mic-level-fill--hot'
                      : micLevelPercent > 65
                        ? 'recorder-panel__mic-level-fill--warm'
                        : ''
                  }`}
                  style={{ width: `${micLevelPercent}%` }}
                />
              </div>
            </div>

            <div className="recorder-panel__mic-check-meta">
              <span className="subtle-copy">{micCheckLabel}</span>
              <button
                className="button button--secondary recorder-panel__mic-check-button"
                disabled={micCheckDisabled}
                onClick={() => void toggleMicCheck()}
                type="button"
              >
                <AudioLines aria-hidden="true" size={16} strokeWidth={1.9} />
                {micCheckActive ? 'Stop test' : 'Test input'}
              </button>
            </div>
          </div>
        </section>
      </div>

      {recorder.activeOutputPath ? (
        <div className="recorder-panel__active-file">
          <span className="metric-label">Active file</span>
          <strong>{recorder.activeOutputPath}</strong>
        </div>
      ) : null}

      <div className="recorder-panel__meta">
        <span>{recorder.qualityPreset}</span>
        {recorder.activeEncoderLabel ? (
          <>
            <span aria-hidden="true">•</span>
            <span>{recorder.activeEncoderLabel}</span>
          </>
        ) : null}
        <span aria-hidden="true">•</span>
        <span>{recorder.outputDirectory}</span>
      </div>

      <div className="recorder-panel__meta">
        <span>{diagnostics.summary}</span>
        <span aria-hidden="true">•</span>
        <span>{diagnostics.backendPath}</span>
      </div>
      <p className="subtle-copy recorder-panel__helper">{diagnostics.readiness}</p>
    </section>
  )
}
