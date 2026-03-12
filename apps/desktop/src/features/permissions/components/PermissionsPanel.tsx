import type { PermissionCheck } from '../../../types/desktop'

interface PermissionsPanelProps {
  permissions: PermissionCheck[]
}

export function PermissionsPanel({ permissions }: PermissionsPanelProps) {
  const pendingCount = permissions.filter(
    (permission) => permission.status === 'pending',
  ).length

  return (
    <section className="panel">
      <div className="panel-header">
        <div>
          <p className="eyebrow">Permissions</p>
          <h2>What still blocks a real capture</h2>
          <p className="subtle-copy">
            {pendingCount === 0
              ? 'Everything important is already ready.'
              : `${pendingCount} item${pendingCount === 1 ? '' : 's'} still need attention before the first real recording.`}
          </p>
        </div>
      </div>

      <div className="permission-summary">
        <strong>{pendingCount === 0 ? 'Ready to test' : 'Needs setup'}</strong>
        <span className={`pill pill-${pendingCount === 0 ? 'granted' : 'pending'}`}>
          {pendingCount === 0 ? '0 pending' : `${pendingCount} pending`}
        </span>
      </div>

      <div className="permission-list">
        {permissions.map((permission) => (
          <article className="permission-item" key={permission.name}>
            <div className="permission-headline">
              <strong>{permission.name}</strong>
              <span className={`pill pill-${permission.status}`}>
                {permission.status}
              </span>
            </div>
            <p>{permission.guidance}</p>
          </article>
        ))}
      </div>
    </section>
  )
}
