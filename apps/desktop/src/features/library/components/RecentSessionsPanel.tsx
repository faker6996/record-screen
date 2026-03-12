import type { SessionSummary } from '../../../types/desktop'

interface RecentSessionsPanelProps {
  roadmap: string[]
  sessions: SessionSummary[]
}

export function RecentSessionsPanel({
  roadmap,
  sessions,
}: RecentSessionsPanelProps) {
  return (
    <section className="panel panel-stack">
      <div>
        <div className="panel-header">
          <div>
            <p className="eyebrow">Recent sessions</p>
            <h2>Fast access to the last recordings</h2>
          </div>
        </div>

        <div className="session-list">
          {sessions.map((session) => (
            <article className="session-item" key={session.id}>
              <div>
                <strong>{session.title}</strong>
                <p>{session.startedAt}</p>
              </div>
              <div className="session-meta">
                <span>{session.durationLabel}</span>
                <span>{session.sizeLabel}</span>
              </div>
              <p className="subtle-copy">{session.location}</p>
            </article>
          ))}
        </div>
      </div>

      <div className="roadmap-block">
        <p className="eyebrow">Roadmap</p>
        <ul className="roadmap-list">
          {roadmap.map((item) => (
            <li key={item}>{item}</li>
          ))}
        </ul>
      </div>
    </section>
  )
}
