import {
  AlertCircle,
  CheckCircle2,
  RefreshCw,
  Settings2,
  ShieldAlert,
} from 'lucide-react'
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
  const grantedCount = permissions.filter(
    (permission) => permission.status === 'granted',
  ).length

  return (
    <section className="permissions-panel">
      <article className="permissions-panel__hero">
        <div className="permissions-panel__hero-copy">
          <p className="eyebrow">Readiness</p>
          <h3>{pendingCount === 0 ? 'Ready to record' : 'Setup needed'}</h3>
          <p className="subtle-copy">Check access.</p>
        </div>

        <div className="permissions-panel__hero-pills">
          <span className="pill pill-granted">
            <CheckCircle2 aria-hidden="true" size={14} strokeWidth={1.9} />
            {grantedCount} granted
          </span>
          <span className={`pill pill-${pendingCount === 0 ? 'granted' : 'pending'}`}>
            <AlertCircle aria-hidden="true" size={14} strokeWidth={1.9} />
            {pendingCount === 0 ? 'All clear' : `${pendingCount} pending`}
          </span>
        </div>
      </article>

      <div className="permissions-panel__toolbar">
        <div className="permissions-panel__summary-copy">
          <strong>{pendingCount === 0 ? 'Ready to test' : 'Needs setup'}</strong>
          <span className="subtle-copy">
            {pendingCount === 0
              ? 'All clear.'
              : `${pendingCount} item${pendingCount === 1 ? '' : 's'} pending.`}
          </span>
        </div>
        <button
          className="button button--secondary"
          onClick={() => void onRefreshPermissions()}
          type="button"
        >
          <RefreshCw aria-hidden="true" size={16} strokeWidth={1.9} />
          Refresh
        </button>
      </div>

      <div className="permissions-panel__summary">
        <strong>{pendingCount === 0 ? 'System clear' : 'Action needed'}</strong>
        <span className={`pill pill-${pendingCount === 0 ? 'granted' : 'pending'}`}>
          {pendingCount === 0 ? '0 pending' : `${pendingCount} pending`}
        </span>
      </div>

      <div className="permissions-panel__list">
        {permissions.map((permission) => (
          <article className="permissions-panel__item" key={permission.name}>
            <div className="permissions-panel__headline">
              <div className="permissions-panel__status-mark">
                <span className="permissions-panel__status-icon" aria-hidden="true">
                  {permission.status === 'granted' ? (
                    <CheckCircle2 size={18} strokeWidth={1.9} />
                  ) : permission.status === 'pending' ? (
                    <AlertCircle size={18} strokeWidth={1.9} />
                  ) : (
                    <ShieldAlert size={18} strokeWidth={1.9} />
                  )}
                </span>
                <strong>{permission.name}</strong>
              </div>
              <span className={`pill pill-${permission.status}`}>{permission.status}</span>
            </div>
            <p>{permission.guidance}</p>
            {permission.status !== 'granted' ? (
              <div className="permissions-panel__actions">
                <button
                  className="button button--secondary"
                  onClick={() => void onRequestPermission(permission.name)}
                  type="button"
                >
                  <ShieldAlert aria-hidden="true" size={16} strokeWidth={1.9} />
                  {actionLabel(permission.name)}
                </button>
                <button
                  className="button button--secondary"
                  onClick={() => void onOpenPermissionSettings(permission.name)}
                  type="button"
                >
                  <Settings2 aria-hidden="true" size={16} strokeWidth={1.9} />
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
