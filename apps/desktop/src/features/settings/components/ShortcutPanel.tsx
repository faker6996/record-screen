import {
  AppWindow,
  RotateCcw,
} from 'lucide-react'
import { Kbd } from '../../../components/Kbd'
import type { ShortcutBinding } from '../../../types/desktop'

interface ShortcutPanelProps {
  shortcuts: ShortcutBinding[]
  onFocusLauncher: () => Promise<void>
  onReset: () => Promise<void>
}

function renderAccelerator(accelerator: string) {
  return accelerator
    .replace('CmdOrCtrl', 'Cmd')
    .split('+')
    .map((part) => <Kbd key={part}>{part}</Kbd>)
}

export function ShortcutPanel({
  shortcuts,
  onFocusLauncher,
  onReset,
}: ShortcutPanelProps) {
  return (
    <section className="shortcut-panel">
      <header className="shortcut-panel__header">
        <div>
          <h3>Global Shortcuts</h3>
          <p className="subtle-copy">Control the recorder from anywhere.</p>
        </div>
      </header>

      <div className="shortcut-panel__list">
        {shortcuts.map((shortcut, index) => (
          <article className="shortcut-panel__item" key={shortcut.action}>
            <div className="shortcut-panel__copy">
              <strong>{shortcut.label}</strong>
            </div>
            <div
              aria-label={shortcut.accelerator}
              className="shortcut-panel__keys"
            >
              {renderAccelerator(shortcut.accelerator)}
            </div>
            {index < shortcuts.length - 1 ? (
              <span className="shortcut-panel__divider" aria-hidden="true" />
            ) : null}
          </article>
        ))}
      </div>

      <div className="shortcut-panel__actions">
        <button
          className="button button--secondary"
          onClick={() => void onFocusLauncher()}
          type="button"
        >
          <AppWindow aria-hidden="true" size={16} strokeWidth={1.9} />
          Show launcher
        </button>
        <button
          className="button button--secondary"
          onClick={() => void onReset()}
          type="button"
        >
          <RotateCcw aria-hidden="true" size={16} strokeWidth={1.9} />
          Reset defaults
        </button>
      </div>
    </section>
  )
}
