import type { SessionSummary } from '../../../types/desktop'

interface RecentSessionsPanelProps {
  sessions: SessionSummary[]
}

export function RecentSessionsPanel({ sessions }: RecentSessionsPanelProps) {
  return (
    <section className="panel sessions-panel">
      <div className="panel__header">
        <div>
          <p className="eyebrow">Recent sessions</p>
          <h2>Fast access to what you recorded last</h2>
          <p className="subtle-copy">
            Keep this section about retrieval, not product planning.
          </p>
        </div>
      </div>

      <div className="sessions-panel__list">
        {sessions.map((session) => (
          <article className="sessions-panel__item" key={session.id}>
            <div>
              <strong>{session.title}</strong>
              <p>{session.startedAt}</p>
            </div>
            <div className="sessions-panel__meta">
              <span>{session.durationLabel}</span>
              <span>{session.sizeLabel}</span>
            </div>
            <p className="subtle-copy">{session.location}</p>
          </article>
        ))}
      </div>
    </section>
  )
}
