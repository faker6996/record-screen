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
    <section className="panel shortcut-panel">
      <div className="panel__header">
        <div>
          <p className="eyebrow">Shortcuts</p>
          <h2>Keyboard controls for the moments you are already busy</h2>
          <p className="subtle-copy">
            These should feel like memory, not another setup surface.
          </p>
        </div>
        <div className="panel__actions">
          <button
            className="button button--secondary"
            onClick={() => void onFocusLauncher()}
            type="button"
          >
            Reveal launcher
          </button>
          <button
            className="button button--secondary"
            onClick={() => void onReset()}
            type="button"
          >
            Reset defaults
          </button>
        </div>
      </div>

      <div className="shortcut-panel__list">
        {shortcuts.map((shortcut) => (
          <article className="shortcut-panel__item" key={shortcut.action}>
            <div>
              <strong>{shortcut.label}</strong>
              <p>{shortcut.description}</p>
            </div>
            <div
              aria-label={shortcut.accelerator}
              className="shortcut-panel__keys"
            >
              {renderAccelerator(shortcut.accelerator)}
            </div>
          </article>
        ))}
      </div>
    </section>
  )
}
