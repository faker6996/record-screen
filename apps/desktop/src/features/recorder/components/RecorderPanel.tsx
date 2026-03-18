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
  countdownValue: number | null
  isStartupDelayed: boolean
  isStartingRecording: boolean
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
  countdownValue,
  isStartupDelayed,
  isStartingRecording,
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
  const isFinalizing = recorder.status === 'finalizing'
  const isCountingDown = countdownValue !== null
  const isStartPending = isCountingDown || isStartingRecording
  const pauseDisabled = !recorder.canPause || isFinalizing
  const recordLabel = isIdle ? (isCountingDown ? 'CANCEL' : 'REC') : isFinalizing ? 'WAIT' : 'STOP'
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
    isStartPending ||
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
  const title = isIdle
    ? 'Ready to Record'
    : isPaused
      ? 'Paused'
      : isFinalizing
        ? 'Finalizing'
        : 'Recording'
  const copy = isIdle
    ? isCountingDown
      ? `Recording starts in ${countdownValue}. Click again to cancel.`
      : isStartingRecording
        ? isStartupDelayed
          ? 'Recorder startup is taking longer than expected. Check the runtime log if this screen does not advance.'
          : 'Starting capture. Hold still for a moment.'
        : 'Select your target and start capturing.'
    : isPaused
      ? 'Capture is paused. Resume when you are ready.'
      : isFinalizing
        ? 'Writing the recording file. Wait a moment before starting another capture.'
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
            data-testid="recorder-status-pill"
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
            className={`recorder-panel__record-button recorder-panel__record-button--${recorder.status} ${
              isStartPending ? 'recorder-panel__record-button--countdown' : ''
            }`}
            data-testid="recorder-record-button"
            disabled={isFinalizing}
            onClick={() => void onToggleRecording()}
            type="button"
          >
            <span
              className={`recorder-panel__record-core ${
                isCountingDown ? 'recorder-panel__record-core--countdown' : ''
              }`}
            >
              {isCountingDown ? countdownValue : null}
            </span>
            <span className="recorder-panel__record-label">{recordLabel}</span>
          </button>
        </div>

        {isStartPending ? (
          <div
            aria-live="polite"
            className="recorder-panel__countdown-copy"
            data-testid="recorder-countdown-copy"
          >
            <strong>
              {isCountingDown
                ? `Starting in ${countdownValue}`
                : isStartupDelayed
                  ? 'Startup is taking longer than expected'
                  : 'Starting capture...'}
            </strong>
            <span>
              {isCountingDown
                ? 'Click the button again if you want to cancel.'
                : isStartupDelayed
                  ? 'The native recorder has not finished starting yet. If this persists, inspect the runtime log for the startup watchdog message.'
                  : 'Preparing the recorder right now.'}
            </span>
          </div>
        ) : null}

        {!isIdle ? (
          <div className="recorder-panel__runtime">
            <strong className="recorder-panel__timer">{recorder.elapsedLabel}</strong>
            {!isFinalizing ? (
              <button
                className={`recorder-panel__pause-button ${
                  isPaused ? 'recorder-panel__pause-button--paused' : ''
                }`}
                data-testid="recorder-pause-button"
                disabled={pauseDisabled}
                onClick={() => void onPauseResume()}
                title={pauseDisabled ? recorder.pauseNote ?? 'Pause is unavailable' : undefined}
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
            ) : null}
            {pauseDisabled && recorder.pauseNote ? (
              <span className="subtle-copy recorder-panel__helper">{recorder.pauseNote}</span>
            ) : null}
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
            disabled={!isIdle || isStartPending}
            triggerTestId="recorder-capture-target-trigger"
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
                disabled={!isIdle || isStartPending}
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
              data-testid="recorder-audio-toggle-button"
              disabled={isStartPending}
              onClick={() => void onToggleMicrophone()}
              type="button"
            >
              <span className="recorder-panel__switch-thumb" />
            </button>
          </div>

          <Combobox
            ariaLabel="Audio input"
            className="recorder-panel__target-combobox"
            disabled={!recorder.micEnabled || !isIdle || isStartPending}
            triggerTestId="recorder-audio-input-trigger"
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
              <strong>Windows microphone unavailable</strong>
              <p>
                Recording will continue without microphone narration until
                the native Windows audio runtime exposes a usable microphone
                route.
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
                data-testid="recorder-mic-check-button"
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
        <div className="recorder-panel__active-file" data-testid="recorder-active-file">
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
    </section>
  )
}
