import type { PermissionCheck } from '../../../types/desktop'

interface PermissionsPanelProps {
  permissions: PermissionCheck[]
  onOpenPermissionSettings: (permissionName: string) => Promise<void>
  onRefreshPermissions: () => Promise<void>
  onRequestPermission: (permissionName: string) => Promise<void>
}

function actionLabel(permissionName: string) {
  switch (permissionName) {
    case 'Screen recording':
      return 'Request access'
    case 'Microphone':
      return 'Request access'
    default:
      return 'Refresh'
  }
}

export function PermissionsPanel({
  onOpenPermissionSettings,
  onRefreshPermissions,
  onRequestPermission,
  permissions,
}: PermissionsPanelProps) {
  const pendingCount = permissions.filter(
    (permission) => permission.status === 'pending',
  ).length

  return (
    <section className="panel permissions-panel">
      <div className="panel__header">
        <div>
          <p className="eyebrow">Permissions</p>
          <h2>What still blocks a real capture</h2>
          <p className="subtle-copy">
            {pendingCount === 0
              ? 'Everything important is already ready.'
              : `${pendingCount} item${pendingCount === 1 ? '' : 's'} still need attention before the first real recording.`}
          </p>
        </div>
        <div className="panel__actions">
          <button
            className="button button--secondary"
            onClick={() => void onRefreshPermissions()}
            type="button"
          >
            Refresh
          </button>
        </div>
      </div>

      <div className="permissions-panel__summary">
        <strong>{pendingCount === 0 ? 'Ready to test' : 'Needs setup'}</strong>
        <span className={`pill pill-${pendingCount === 0 ? 'granted' : 'pending'}`}>
          {pendingCount === 0 ? '0 pending' : `${pendingCount} pending`}
        </span>
      </div>

      <div className="permissions-panel__list">
        {permissions.map((permission) => (
          <article className="permissions-panel__item" key={permission.name}>
            <div className="permissions-panel__headline">
              <strong>{permission.name}</strong>
              <span className={`pill pill-${permission.status}`}>
                {permission.status}
              </span>
            </div>
            <p>{permission.guidance}</p>
            {permission.status !== 'granted' ? (
              <div className="permissions-panel__actions">
                <button
                  className="button button--secondary"
                  onClick={() => void onRequestPermission(permission.name)}
                  type="button"
                >
                  {actionLabel(permission.name)}
                </button>
                <button
                  className="button button--secondary"
                  onClick={() => void onOpenPermissionSettings(permission.name)}
                  type="button"
                >
                  Open settings
                </button>
              </div>
            ) : null}
          </article>
        ))}
      </div>
    </section>
  )
}
