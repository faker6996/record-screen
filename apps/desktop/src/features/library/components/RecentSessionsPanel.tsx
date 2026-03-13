import {
  Clock3,
  Copy,
  FileVideo,
  FolderOpen,
  HardDrive,
  Play,
  Type,
} from 'lucide-react'
import { useState } from 'react'
import type { SessionSummary } from '../../../types/desktop'

interface RecentSessionsPanelProps {
  onOpenRecording: (recordingPath: string) => Promise<void>
  onRevealRecordingInFolder: (recordingPath: string) => Promise<void>
  sessions: SessionSummary[]
}

function filenameFromPath(location: string) {
  return location.split('/').filter(Boolean).at(-1) ?? location
}

function fileTypeLabel(location: string) {
  const filename = filenameFromPath(location)
  const extension = filename.includes('.')
    ? filename.split('.').at(-1)?.toUpperCase()
    : null

  return extension ? `${extension} clip` : 'Recording'
}

export function RecentSessionsPanel({
  onOpenRecording,
  onRevealRecordingInFolder,
  sessions,
}: RecentSessionsPanelProps) {
  const [copiedSessionId, setCopiedSessionId] = useState<string | null>(null)
  const totalSize = sessions.reduce((accumulator, session) => {
    const value = Number.parseFloat(session.sizeLabel)
    return Number.isNaN(value) ? accumulator : accumulator + value
  }, 0)
  const latestSession = sessions[0] ?? null
  const totalDuration = sessions.reduce((accumulator, session) => {
    const [minutes = '0', seconds = '0'] = session.durationLabel.split(':').slice(-2)
    const nextMinutes = Number.parseInt(minutes, 10)
    const nextSeconds = Number.parseInt(seconds, 10)

    if (Number.isNaN(nextMinutes) || Number.isNaN(nextSeconds)) {
      return accumulator
    }

    return accumulator + nextMinutes * 60 + nextSeconds
  }, 0)
  const durationLabel =
    totalDuration > 0
      ? `${Math.floor(totalDuration / 60)}m ${String(totalDuration % 60).padStart(2, '0')}s`
      : '0m 00s'

  async function copyValue(sessionId: string, value: string) {
    try {
      await navigator.clipboard.writeText(value)
      setCopiedSessionId(sessionId)
      window.setTimeout(() => {
        setCopiedSessionId((current) => (current === sessionId ? null : current))
      }, 1800)
    } catch {
      setCopiedSessionId(null)
    }
  }

  return (
    <section className="sessions-panel">
      {latestSession ? (
        <article className="sessions-panel__featured">
          <div className="sessions-panel__featured-media" aria-hidden="true">
            <FileVideo size={32} strokeWidth={1.9} />
          </div>

          <div className="sessions-panel__featured-copy">
            <p className="eyebrow">Latest recording</p>
            <h3>{latestSession.title}</h3>
            <p className="subtle-copy">Open your latest clip.</p>

            <div className="sessions-panel__featured-meta">
              <span className="pill">
                <Clock3 aria-hidden="true" size={14} strokeWidth={1.9} />
                {latestSession.durationLabel}
              </span>
              <span className="pill">
                <HardDrive aria-hidden="true" size={14} strokeWidth={1.9} />
                {latestSession.sizeLabel}
              </span>
              <span className="pill">{fileTypeLabel(latestSession.location)}</span>
            </div>

            <div className="sessions-panel__actions">
              <button
                className="button button--secondary"
                onClick={() => void onOpenRecording(latestSession.location)}
                type="button"
              >
                <Play aria-hidden="true" size={16} strokeWidth={1.9} />
                Open latest
              </button>
              <button
                className="button button--secondary"
                onClick={() => void onRevealRecordingInFolder(latestSession.location)}
                type="button"
              >
                <FolderOpen aria-hidden="true" size={16} strokeWidth={1.9} />
                Open folder
              </button>
            </div>
          </div>
        </article>
      ) : null}

      <div className="sessions-panel__summary">
        <article className="sessions-panel__summary-card">
          <span className="metric-label">Saved sessions</span>
          <strong>{sessions.length}</strong>
          <p>{latestSession ? latestSession.startedAt : 'No recordings yet'}</p>
        </article>
        <article className="sessions-panel__summary-card">
          <span className="metric-label">Approx library size</span>
          <strong>{totalSize > 0 ? `${totalSize.toFixed(0)} MB` : '0 MB'}</strong>
          <p>{latestSession ? latestSession.location : 'No clips yet'}</p>
        </article>
        <article className="sessions-panel__summary-card">
          <span className="metric-label">Captured runtime</span>
          <strong>{durationLabel}</strong>
          <p>{latestSession ? filenameFromPath(latestSession.location) : 'No clips yet'}</p>
        </article>
      </div>

      <div className="sessions-panel__library">
        <div className="sessions-panel__library-header">
          <div>
            <span className="metric-label">Library</span>
            <strong>Recorded clips</strong>
          </div>
          <span className="pill">{sessions.length} items</span>
        </div>

        <div className="sessions-panel__list">
          {sessions.map((session) => (
            <article className="sessions-panel__item" key={session.id}>
              <div className="sessions-panel__topline">
                <div className="sessions-panel__thumb" aria-hidden="true">
                  <FileVideo aria-hidden="true" size={20} strokeWidth={1.9} />
                </div>
                <div className="sessions-panel__headline">
                  <div>
                    <strong>{session.title}</strong>
                    <p>{session.startedAt}</p>
                  </div>
                  <span className="pill">{fileTypeLabel(session.location)}</span>
                </div>
              </div>

              <div className="sessions-panel__meta">
                <span className="pill">{session.durationLabel}</span>
                <span className="pill">{session.sizeLabel}</span>
                <span className="pill">{filenameFromPath(session.location)}</span>
              </div>

              <div className="sessions-panel__location">
                <span className="metric-label">Saved at</span>
                <strong>{session.location}</strong>
              </div>

              <div className="sessions-panel__actions">
                <button
                  className="button button--secondary"
                  onClick={() => void onOpenRecording(session.location)}
                  type="button"
                >
                  <Play aria-hidden="true" size={16} strokeWidth={1.9} />
                  Open clip
                </button>
                <button
                  className="button button--secondary"
                  onClick={() => void onRevealRecordingInFolder(session.location)}
                  type="button"
                >
                  <FolderOpen aria-hidden="true" size={16} strokeWidth={1.9} />
                  Show folder
                </button>
                <button
                  className="button button--secondary"
                  onClick={() => {
                    void copyValue(session.id, session.location)
                  }}
                  type="button"
                >
                  <Copy aria-hidden="true" size={16} strokeWidth={1.9} />
                  {copiedSessionId === session.id ? 'Path copied' : 'Copy path'}
                </button>
                <button
                  className="button button--secondary"
                  onClick={() => {
                    void copyValue(session.id, filenameFromPath(session.location))
                  }}
                  type="button"
                >
                  <Type aria-hidden="true" size={16} strokeWidth={1.9} />
                  Copy filename
                </button>
              </div>
            </article>
          ))}
        </div>
      </div>
    </section>
  )
}
