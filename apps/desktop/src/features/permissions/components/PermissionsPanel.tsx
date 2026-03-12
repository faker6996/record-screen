import type { PermissionCheck } from '../../../types/desktop'

interface PermissionsPanelProps {
  permissions: PermissionCheck[]
}

export function PermissionsPanel({ permissions }: PermissionsPanelProps) {
  return (
    <section className="panel">
      <div className="panel-header">
        <div>
          <p className="eyebrow">Permissions</p>
          <h2>Readiness before first capture</h2>
        </div>
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
