import { Kbd } from '../../../components/Kbd'
import type { ShortcutBinding } from '../../../types/desktop'

interface ShortcutPanelProps {
  shortcuts: ShortcutBinding[]
  onFocusLauncher: () => Promise<void>
  onReset: () => Promise<void>
}

function renderAccelerator(accelerator: string) {
  return accelerator.split('+').map((part) => <Kbd key={part}>{part}</Kbd>)
}

export function ShortcutPanel({
  shortcuts,
  onFocusLauncher,
  onReset,
}: ShortcutPanelProps) {
  return (
    <section className="panel">
      <div className="panel-header">
        <div>
          <p className="eyebrow">Shortcuts</p>
          <h2>Global controls that stay close</h2>
        </div>
        <div className="panel-actions">
          <button
            className="secondary-button"
            onClick={() => void onFocusLauncher()}
            type="button"
          >
            Reveal launcher
          </button>
          <button
            className="secondary-button"
            onClick={() => void onReset()}
            type="button"
          >
            Reset defaults
          </button>
        </div>
      </div>

      <div className="shortcut-list">
        {shortcuts.map((shortcut) => (
          <article className="shortcut-item" key={shortcut.action}>
            <div>
              <strong>{shortcut.label}</strong>
              <p>{shortcut.description}</p>
            </div>
            <div className="shortcut-keys">
              {renderAccelerator(shortcut.accelerator)}
            </div>
          </article>
        ))}
      </div>
    </section>
  )
}
