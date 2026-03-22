import { AudioLines, LoaderCircle, Mic, Monitor, Pause, Play } from 'lucide-react'
import { memo, useEffect, useMemo } from 'react'
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

function parseElapsedSeconds(label: string, status: RecorderSnapshot['status']) {
  if (status === 'idle') {
    return 0
  }

  const normalized = label.replace(/^(Paused at|Finalizing)\s+/i, '').trim()
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

function formatElapsed(totalSeconds: number) {
  const hours = Math.floor(totalSeconds / 3600)
  const minutes = Math.floor((totalSeconds % 3600) / 60)
  const seconds = totalSeconds % 60

  return [hours, minutes, seconds]
    .map((value) => value.toString().padStart(2, '0'))
    .join(':')
}

const RecorderControlGrid = memo(function RecorderControlGrid({
  captureTargetDescription,
  captureTargetOptions,
  customRegionSelected,
  diagnostics,
  isIdle,
  microphoneInputOptions,
  micCheckActive,
  micCheckDisabled,
  micCheckLabel,
  micEnumerationUnavailable,
  micLevelPercent,
  onOpenRegionSelector,
  onToggleMicrophone,
  onUpdateAudioInput,
  onUpdateCaptureTarget,
  recorderMicEnabled,
  selectedAudioDescription,
  selectedAudioInputId,
  selectedCaptureTargetId,
  systemAudioEnabled,
  toggleMicCheck,
}: {
  captureTargetDescription?: string
  captureTargetOptions: Array<{ value: string; label: string }>
  customRegionSelected: boolean
  diagnostics: RuntimeDiagnostics
  isIdle: boolean
  microphoneInputOptions: Array<{ value: string; label: string }>
  micCheckActive: boolean
  micCheckDisabled: boolean
  micCheckLabel: string
  micEnumerationUnavailable: boolean
  micLevelPercent: number
  onOpenRegionSelector: () => Promise<void>
  onToggleMicrophone: () => Promise<void>
  onUpdateAudioInput: (audioInputId: string) => Promise<void>
  onUpdateCaptureTarget: (captureTargetId: string) => Promise<void>
  recorderMicEnabled: boolean
  selectedAudioDescription?: string
  selectedAudioInputId: string
  selectedCaptureTargetId: string
  systemAudioEnabled: boolean
  toggleMicCheck: () => Promise<void>
}) {
  return (
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
          triggerTestId="recorder-capture-target-trigger"
          onChange={(nextCaptureTargetId) => {
            if (nextCaptureTargetId === 'region:custom') {
              void onOpenRegionSelector()
              return
            }

            void onUpdateCaptureTarget(nextCaptureTargetId)
          }}
          options={captureTargetOptions}
          value={selectedCaptureTargetId}
        />
        <p className="subtle-copy recorder-panel__helper">{captureTargetDescription}</p>
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
            aria-label={recorderMicEnabled ? 'Disable audio capture' : 'Enable audio capture'}
            aria-pressed={recorderMicEnabled}
            className={`recorder-panel__switch ${
              recorderMicEnabled ? 'recorder-panel__switch--active' : ''
            }`}
            data-testid="recorder-audio-toggle-button"
            disabled={!isIdle}
            onClick={() => void onToggleMicrophone()}
            type="button"
          >
            <span className="recorder-panel__switch-thumb" />
          </button>
        </div>

        <Combobox
          ariaLabel="Audio input"
          className="recorder-panel__target-combobox"
          disabled={!recorderMicEnabled || !isIdle}
          triggerTestId="recorder-audio-input-trigger"
          onChange={(nextAudioInputId) => {
            void onUpdateAudioInput(nextAudioInputId)
          }}
          options={microphoneInputOptions}
          value={selectedAudioInputId}
        />
        <p className="subtle-copy recorder-panel__helper">
          {systemAudioEnabled && diagnostics.supportsSystemAudio
            ? `${selectedAudioDescription ?? 'Choose a microphone input.'} System audio will be mixed in from the current loopback source.`
            : selectedAudioDescription ?? 'Choose a microphone input for narration.'}
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
  )
})

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
  const isPreparingRecording =
    recorder.status === 'recording' &&
    !recorder.canPause &&
    recorder.activeEncoderLabel === null &&
    recorder.pauseNote?.includes('preparing the native capture session') === true
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
  const pauseDisabled = !recorder.canPause || isFinalizing
  const recordLabel = isIdle ? 'REC' : isFinalizing || isPreparingRecording ? 'WAIT' : 'STOP'
  const recordVisualState = isPreparingRecording ? 'starting' : recorder.status
  const selectedCaptureTarget = useMemo(
    () => captureTargets.find((target) => target.id === selectedCaptureTargetId) ?? null,
    [captureTargets, selectedCaptureTargetId],
  )
  const selectedAudioInput = useMemo(
    () => audioInputs.find((input) => input.id === selectedAudioInputId) ?? null,
    [audioInputs, selectedAudioInputId],
  )
  const microphoneOptions = useMemo(
    () => audioInputs.filter((input) => input.kind !== 'system'),
    [audioInputs],
  )
  const captureTargetOptions = useMemo(
    () =>
      captureTargets.map((target) => ({
        value: target.id,
        label: target.label,
      })),
    [captureTargets],
  )
  const microphoneInputOptions = useMemo(
    () =>
      microphoneOptions.map((input) => ({
        value: input.id,
        label: input.label,
      })),
    [microphoneOptions],
  )
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
  const title = isIdle
    ? 'Ready to Record'
    : isPreparingRecording
      ? 'Preparing Recorder'
    : isPaused
      ? 'Paused'
      : isFinalizing
        ? 'Finalizing'
        : 'Recording'
  const copy = isIdle
    ? 'Select your target and start capturing.'
    : isPreparingRecording
      ? 'Starting the native capture session. You can stop now if you changed your mind.'
    : isPaused
      ? 'Capture is paused. Resume when you are ready.'
      : isFinalizing
        ? 'Writing the recording file. Wait a moment before starting another capture.'
      : 'Recording is in progress. Stop when your session is complete.'
  const customRegionSelected = selectedCaptureTargetId === 'region:custom'

  function RecorderElapsed({
    label,
    status,
  }: {
    label: string
    status: RecorderSnapshot['status']
  }) {
    const displayElapsedSeconds = parseElapsedSeconds(label, status)

    if (status === 'paused') {
      return <>Paused at {formatElapsed(displayElapsedSeconds)}</>
    }

    if (status === 'finalizing') {
      return <>Finalizing {formatElapsed(displayElapsedSeconds)}</>
    }

    return <>{formatElapsed(displayElapsedSeconds)}</>
  }

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
            className={`recorder-panel__status-pill recorder-panel__status-pill--${recordVisualState}`}
            data-testid="recorder-status-pill"
          >
            {isPreparingRecording ? 'starting' : recorder.status}
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
            className={`recorder-panel__record-glow recorder-panel__record-glow--${recordVisualState}`}
          />
          <button
            className={`recorder-panel__record-button recorder-panel__record-button--${recordVisualState}`}
            data-testid="recorder-record-button"
            disabled={isFinalizing}
            onClick={() => void onToggleRecording()}
            type="button"
          >
            {isPreparingRecording ? (
              <span className="recorder-panel__record-core recorder-panel__record-core--starting">
                <LoaderCircle aria-hidden="true" className="recorder-panel__loading-spinner" />
              </span>
            ) : (
              <span className="recorder-panel__record-core" />
            )}
            <span className="recorder-panel__record-label">{recordLabel}</span>
          </button>
        </div>

        {!isIdle ? (
          <div className="recorder-panel__runtime">
            {isPreparingRecording ? (
              <strong className="recorder-panel__timer recorder-panel__timer--starting">
                <LoaderCircle aria-hidden="true" className="recorder-panel__loading-spinner" />
                <span>Preparing…</span>
              </strong>
            ) : (
              <strong className="recorder-panel__timer">
                <RecorderElapsed
                  key={`${recorder.activeOutputPath ?? 'idle'}:${recorder.status}:${recorder.elapsedLabel}`}
                  label={recorder.elapsedLabel}
                  status={recorder.status}
                />
              </strong>
            )}
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

      <RecorderControlGrid
        captureTargetDescription={selectedCaptureTarget?.description}
        captureTargetOptions={captureTargetOptions}
        customRegionSelected={customRegionSelected}
        diagnostics={diagnostics}
        isIdle={isIdle}
        microphoneInputOptions={microphoneInputOptions}
        micCheckActive={micCheckActive}
        micCheckDisabled={micCheckDisabled}
        micCheckLabel={micCheckLabel}
        micEnumerationUnavailable={micEnumerationUnavailable}
        micLevelPercent={micLevelPercent}
        onOpenRegionSelector={onOpenRegionSelector}
        onToggleMicrophone={onToggleMicrophone}
        onUpdateAudioInput={onUpdateAudioInput}
        onUpdateCaptureTarget={onUpdateCaptureTarget}
        recorderMicEnabled={recorder.micEnabled}
        selectedAudioDescription={selectedAudioInput?.description}
        selectedAudioInputId={selectedAudioInputId}
        selectedCaptureTargetId={selectedCaptureTargetId}
        systemAudioEnabled={systemAudioEnabled}
        toggleMicCheck={toggleMicCheck}
      />

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
