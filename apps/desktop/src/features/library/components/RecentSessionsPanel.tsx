import { convertFileSrc } from '@tauri-apps/api/core'
import {
  AlertCircle,
  FileVideo,
  FolderOpen,
  LoaderCircle,
  Play,
  X,
} from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import type { SessionSummary } from '../../../types/desktop'

interface RecentSessionsPanelProps {
  onOpenRecording: (recordingPath: string) => Promise<void>
  onRevealRecordingInFolder: (recordingPath: string) => Promise<void>
  sessions: SessionSummary[]
}

function filenameFromPath(location: string) {
  return location.split(/[\\/]/).filter(Boolean).at(-1) ?? location
}

function canPreviewInApp(location: string) {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window && !location.startsWith('~/')
}

export function RecentSessionsPanel({
  onOpenRecording,
  onRevealRecordingInFolder,
  sessions,
}: RecentSessionsPanelProps) {
  const [selectedSessionId, setSelectedSessionId] = useState<string | null>(null)
  const [playbackSource, setPlaybackSource] = useState<string | null>(null)
  const [previewState, setPreviewState] = useState<'idle' | 'loading' | 'ready' | 'error'>('idle')
  const [isPlaying, setIsPlaying] = useState(false)
  const [previewErrorDetail, setPreviewErrorDetail] = useState<string | null>(null)
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

  return (
    <section className="sessions-panel">
      <div className="sessions-panel__shell">
        <div className="sessions-panel__header">
          <div>
            <h3>Recent Sessions</h3>
            <p className="subtle-copy">Your latest recordings saved to disk.</p>
          </div>
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

                  <button
                    aria-label={`Open folder for ${filenameFromPath(session.location)}`}
                    className="sessions-panel__row-action"
                    onClick={() => void onRevealRecordingInFolder(session.location)}
                    type="button"
                  >
                    <FolderOpen size={17} strokeWidth={1.9} />
                  </button>
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
    </section>
  )
}
