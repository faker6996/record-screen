import { convertFileSrc } from '@tauri-apps/api/core'
import {
  AlertCircle,
  CheckSquare,
  Download,
  FileVideo,
  FolderOpen,
  LoaderCircle,
  Play,
  Square,
  Trash2,
  X,
} from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import type { SessionSummary } from '../../../types/desktop'

interface RecentSessionsPanelProps {
  onTrashRecordings: (recordingPaths: string[]) => Promise<void>
  onOpenRecording: (recordingPath: string) => Promise<void>
  onRevealRecordingInFolder: (recordingPath: string) => Promise<void>
  onSaveRecordingCopy: (recordingPath: string) => Promise<void>
  sessions: SessionSummary[]
}

interface PendingDeleteDialog {
  mode: 'single' | 'selected' | 'all'
  recordingPaths: string[]
  message: string
}

function filenameFromPath(location: string) {
  return location.split(/[\\/]/).filter(Boolean).at(-1) ?? location
}

function canPreviewInApp(location: string) {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window && !location.startsWith('~/')
}

export function RecentSessionsPanel({
  onTrashRecordings,
  onOpenRecording,
  onRevealRecordingInFolder,
  onSaveRecordingCopy,
  sessions,
}: RecentSessionsPanelProps) {
  const [selectedSessionId, setSelectedSessionId] = useState<string | null>(null)
  const [selectedSessionIds, setSelectedSessionIds] = useState<string[]>([])
  const [playbackSource, setPlaybackSource] = useState<string | null>(null)
  const [previewState, setPreviewState] = useState<'idle' | 'loading' | 'ready' | 'error'>('idle')
  const [isPlaying, setIsPlaying] = useState(false)
  const [previewErrorDetail, setPreviewErrorDetail] = useState<string | null>(null)
  const [isDeleting, setIsDeleting] = useState(false)
  const [pendingDelete, setPendingDelete] = useState<PendingDeleteDialog | null>(null)
  const videoRef = useRef<HTMLVideoElement | null>(null)
  const objectUrlRef = useRef<string | null>(null)
  const previewRequestRef = useRef(0)

  const selectedSession =
    sessions.find((session) => session.id === selectedSessionId) ?? null

  useEffect(() => {
    return () => {
      if (objectUrlRef.current) {
        URL.revokeObjectURL(objectUrlRef.current)
        objectUrlRef.current = null
      }
    }
  }, [])

  useEffect(() => {
    setSelectedSessionIds((current) =>
      current.filter((sessionId) => sessions.some((session) => session.id === sessionId)),
    )

    if (selectedSessionId && !sessions.some((session) => session.id === selectedSessionId)) {
      closePreview()
    }
  }, [selectedSessionId, sessions])

  function revokePlaybackSource() {
    if (objectUrlRef.current) {
      URL.revokeObjectURL(objectUrlRef.current)
      objectUrlRef.current = null
    }
  }

  async function openPreview(session: SessionSummary) {
    const canPreview = canPreviewInApp(session.location)
    const requestId = previewRequestRef.current + 1
    previewRequestRef.current = requestId

    revokePlaybackSource()
    setSelectedSessionId(session.id)
    setPlaybackSource(null)
    setIsPlaying(false)
    setPreviewErrorDetail(null)
    setPreviewState(canPreview ? 'loading' : 'idle')

    if (!canPreview) {
      return
    }

    const assetUrl = convertFileSrc(session.location)

    try {
      const response = await fetch(assetUrl)
      if (!response.ok) {
        throw new Error(`Preview request failed with status ${response.status}`)
      }

      const blob = await response.blob()
      if (previewRequestRef.current != requestId) {
        return
      }

      const objectUrl = URL.createObjectURL(blob)
      objectUrlRef.current = objectUrl
      setPlaybackSource(objectUrl)
    } catch (error) {
      if (previewRequestRef.current != requestId) {
        return
      }

      const detail =
        error instanceof Error ? error.message : 'The embedded webview could not load this clip.'
      setPreviewState('error')
      setPreviewErrorDetail(detail)
      console.error('[recent-preview] unable to prepare playback source', {
        assetUrl,
        location: session.location,
        detail,
      })
    }
  }

  function closePreview() {
    previewRequestRef.current += 1
    revokePlaybackSource()
    setSelectedSessionId(null)
    setPlaybackSource(null)
    setPreviewState('idle')
    setIsPlaying(false)
    setPreviewErrorDetail(null)
  }

  async function startPreview() {
    const video = videoRef.current
    if (!video) {
      return
    }

    try {
      await video.play()
      setIsPlaying(true)
      setPreviewErrorDetail(null)
    } catch {
      setPreviewState('error')
      setPreviewErrorDetail('The embedded webview rejected playback.')
      console.error('[recent-preview] play() rejected', {
        currentSrc: video.currentSrc,
        networkState: video.networkState,
        readyState: video.readyState,
      })
    }
  }

  function toggleSessionSelection(sessionId: string) {
    setSelectedSessionIds((current) =>
      current.includes(sessionId)
        ? current.filter((item) => item !== sessionId)
        : [...current, sessionId],
    )
  }

  function toggleSelectAll() {
    if (selectedSessionIds.length === sessions.length) {
      setSelectedSessionIds([])
      return
    }

    setSelectedSessionIds(sessions.map((session) => session.id))
  }

  function requestTrash(recordingPaths: string[], mode: 'single' | 'selected' | 'all') {
    if (recordingPaths.length === 0 || isDeleting) {
      return
    }

    const message =
      mode === 'single'
        ? 'Move this recording to Trash?'
        : mode === 'selected'
          ? `Move ${recordingPaths.length} selected recordings to Trash?`
          : `Move all ${recordingPaths.length} recordings to Trash?`

    setPendingDelete({
      mode,
      recordingPaths,
      message,
    })
  }

  async function confirmTrash() {
    if (!pendingDelete || isDeleting) {
      return
    }

    setIsDeleting(true)

    try {
      await onTrashRecordings(pendingDelete.recordingPaths)
      setSelectedSessionIds((current) =>
        current.filter((sessionId) => {
          const session = sessions.find((item) => item.id === sessionId)
          return session ? !pendingDelete.recordingPaths.includes(session.location) : false
        }),
      )

      if (selectedSession && pendingDelete.recordingPaths.includes(selectedSession.location)) {
        closePreview()
      }

      setPendingDelete(null)
    } finally {
      setIsDeleting(false)
    }
  }

  return (
    <section className="sessions-panel">
      <div className="sessions-panel__shell">
        <div className="sessions-panel__header">
          <div>
            <h3>Recent Sessions</h3>
            <p className="subtle-copy">Your latest recordings saved to disk.</p>
          </div>
          {sessions.length > 0 ? (
            <div className="sessions-panel__bulk-actions">
              <button
                className="button button--secondary sessions-panel__bulk-button"
                onClick={toggleSelectAll}
                type="button"
              >
                {selectedSessionIds.length === sessions.length ? (
                  <CheckSquare aria-hidden="true" size={16} strokeWidth={1.9} />
                ) : (
                  <Square aria-hidden="true" size={16} strokeWidth={1.9} />
                )}
                {selectedSessionIds.length === sessions.length ? 'Clear all' : 'Select all'}
              </button>
              <button
                className="button button--secondary sessions-panel__bulk-button"
                disabled={selectedSessionIds.length === 0 || isDeleting}
                onClick={() => {
                  const selectedPaths = sessions
                    .filter((session) => selectedSessionIds.includes(session.id))
                    .map((session) => session.location)
                  requestTrash(selectedPaths, 'selected')
                }}
                type="button"
              >
                <Trash2 aria-hidden="true" size={16} strokeWidth={1.9} />
                Delete selected
              </button>
              <button
                className="button button--secondary sessions-panel__bulk-button sessions-panel__bulk-button--danger"
                disabled={sessions.length === 0 || isDeleting}
                onClick={() => {
                  requestTrash(
                    sessions.map((session) => session.location),
                    'all',
                  )
                }}
                type="button"
              >
                <Trash2 aria-hidden="true" size={16} strokeWidth={1.9} />
                Delete all
              </button>
            </div>
          ) : null}
        </div>

        {sessions.length === 0 ? (
          <div className="sessions-panel__empty">
            <FileVideo aria-hidden="true" size={28} strokeWidth={1.9} />
            <strong>No recordings yet</strong>
            <p className="subtle-copy">Start a clip and it will appear here.</p>
          </div>
        ) : (
          <div className="sessions-panel__list">
            {sessions.map((session) => {
              const isActive = session.id === selectedSessionId

              return (
                <article
                  className={`sessions-panel__row ${
                    isActive ? 'sessions-panel__row--active' : ''
                  }`}
                  key={session.id}
                >
                  <label className="sessions-panel__checkbox">
                    <input
                      checked={selectedSessionIds.includes(session.id)}
                      onChange={() => {
                        toggleSessionSelection(session.id)
                      }}
                      type="checkbox"
                    />
                    <span aria-hidden="true" className="sessions-panel__checkbox-ui" />
                  </label>

                  <button
                    className="sessions-panel__row-main"
                    onClick={() => {
                      void openPreview(session)
                    }}
                    type="button"
                  >
                    <span className="sessions-panel__row-icon" aria-hidden="true">
                      <FileVideo size={18} strokeWidth={1.9} />
                    </span>
                    <span className="sessions-panel__row-copy">
                      <strong>{filenameFromPath(session.location)}</strong>
                      <span>
                        {session.startedAt} • {session.sizeLabel}
                      </span>
                    </span>
                  </button>

                  <div className="sessions-panel__row-actions">
                    <button
                      aria-label={`Move ${filenameFromPath(session.location)} to Trash`}
                      className="sessions-panel__row-action sessions-panel__row-action--danger"
                      disabled={isDeleting}
                      onClick={() => {
                        requestTrash([session.location], 'single')
                      }}
                      type="button"
                    >
                      <Trash2 size={17} strokeWidth={1.9} />
                    </button>
                    <button
                      aria-label={`Save ${filenameFromPath(session.location)} as`}
                      className="sessions-panel__row-action"
                      onClick={() => void onSaveRecordingCopy(session.location)}
                      type="button"
                    >
                      <Download size={17} strokeWidth={1.9} />
                    </button>
                    <button
                      aria-label={`Open folder for ${filenameFromPath(session.location)}`}
                      className="sessions-panel__row-action"
                      onClick={() => void onRevealRecordingInFolder(session.location)}
                      type="button"
                    >
                      <FolderOpen size={17} strokeWidth={1.9} />
                    </button>
                  </div>
                </article>
              )
            })}
          </div>
        )}
      </div>

      {selectedSession ? (
        <div className="sessions-panel__viewer-backdrop" role="presentation">
          <section
            aria-label={`Preview ${filenameFromPath(selectedSession.location)}`}
            className="sessions-panel__viewer"
          >
            <div className="sessions-panel__viewer-header">
              <div>
                <strong>{filenameFromPath(selectedSession.location)}</strong>
                <p className="subtle-copy">
                  {selectedSession.startedAt} • {selectedSession.sizeLabel}
                </p>
              </div>

              <button
                className="sessions-panel__viewer-close"
                onClick={() => {
                  closePreview()
                }}
                type="button"
              >
                <X aria-hidden="true" size={16} strokeWidth={1.9} />
              </button>
            </div>

            <div className="sessions-panel__viewer-stage">
              {playbackSource ? (
                <div className="sessions-panel__video-shell">
                  <video
                    className="sessions-panel__video"
                    controls
                    onCanPlay={() => {
                      setPreviewState('ready')
                      setPreviewErrorDetail(null)
                    }}
                    onError={(event) => {
                      const mediaError = event.currentTarget.error
                      const detail = mediaError
                        ? `Media error code ${mediaError.code}`
                        : 'Unknown media playback error'
                      setPreviewState('error')
                      setIsPlaying(false)
                      setPreviewErrorDetail(detail)
                      console.error('[recent-preview] media error', {
                        detail,
                        currentSrc: event.currentTarget.currentSrc,
                        networkState: event.currentTarget.networkState,
                        readyState: event.currentTarget.readyState,
                      })
                    }}
                    onPause={() => {
                      setIsPlaying(false)
                    }}
                    onPlay={() => {
                      setIsPlaying(true)
                    }}
                    playsInline
                    preload="metadata"
                    ref={videoRef}
                    src={playbackSource}
                  />

                  {!isPlaying && previewState !== 'error' ? (
                    <button
                      className="sessions-panel__video-overlay"
                      onClick={() => {
                        void startPreview()
                      }}
                      type="button"
                    >
                      {previewState === 'loading' ? (
                        <>
                          <LoaderCircle
                            aria-hidden="true"
                            className="sessions-panel__spinner"
                            size={24}
                            strokeWidth={1.9}
                          />
                          Loading preview
                        </>
                      ) : (
                        <>
                          <Play aria-hidden="true" size={22} strokeWidth={1.9} />
                          Play preview
                        </>
                      )}
                    </button>
                  ) : null}

                  {previewState === 'error' ? (
                    <div className="sessions-panel__video-error">
                      <AlertCircle aria-hidden="true" size={26} strokeWidth={1.9} />
                      <strong>Preview failed</strong>
                      <p className="subtle-copy">
                        {previewErrorDetail ?? 'Open the clip externally or open its folder.'}
                      </p>
                    </div>
                  ) : null}
                </div>
              ) : (
                <div className="sessions-panel__viewer-empty">
                  <Play aria-hidden="true" size={26} strokeWidth={1.9} />
                  <strong>
                    {previewState === 'loading' ? 'Preparing preview' : 'Preview unavailable here'}
                  </strong>
                  <p className="subtle-copy">
                    {previewState === 'loading'
                      ? 'Loading the clip into the embedded player.'
                      : 'Open the clip externally or open its folder.'}
                  </p>
                </div>
              )}
            </div>

            <div className="sessions-panel__viewer-actions">
              <button
                className="button button--secondary sessions-panel__viewer-danger"
                disabled={isDeleting}
                onClick={() => {
                  requestTrash([selectedSession.location], 'single')
                }}
                type="button"
              >
                <Trash2 aria-hidden="true" size={16} strokeWidth={1.9} />
                Move to Trash
              </button>
              <button
                className="button button--secondary"
                onClick={() => void onSaveRecordingCopy(selectedSession.location)}
                type="button"
              >
                <Download aria-hidden="true" size={16} strokeWidth={1.9} />
                Save As
              </button>
              <button
                className="button button--secondary"
                onClick={() => void onOpenRecording(selectedSession.location)}
                type="button"
              >
                <Play aria-hidden="true" size={16} strokeWidth={1.9} />
                Open clip
              </button>
              <button
                className="button button--secondary"
                onClick={() => void onRevealRecordingInFolder(selectedSession.location)}
                type="button"
              >
                <FolderOpen aria-hidden="true" size={16} strokeWidth={1.9} />
                Open folder
              </button>
            </div>
          </section>
        </div>
      ) : null}

      {pendingDelete ? (
        <div className="sessions-panel__confirm-backdrop" role="presentation">
          <section
            aria-label="Confirm deletion"
            className="sessions-panel__confirm-dialog"
          >
            <strong>Move recordings to Trash</strong>
            <p className="subtle-copy">{pendingDelete.message}</p>
            <div className="sessions-panel__confirm-actions">
              <button
                className="button button--secondary"
                disabled={isDeleting}
                onClick={() => {
                  setPendingDelete(null)
                }}
                type="button"
              >
                Cancel
              </button>
              <button
                className="button button--secondary sessions-panel__viewer-danger"
                disabled={isDeleting}
                onClick={() => {
                  void confirmTrash()
                }}
                type="button"
              >
                <Trash2 aria-hidden="true" size={16} strokeWidth={1.9} />
                {isDeleting ? 'Moving…' : 'Move to Trash'}
              </button>
            </div>
          </section>
        </div>
      ) : null}
    </section>
  )
}
