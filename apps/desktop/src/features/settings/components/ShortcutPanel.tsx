import {
  AppWindow,
  Command,
  Mic,
  Pause,
  RotateCcw,
  Video,
} from 'lucide-react'
import { Kbd } from '../../../components/Kbd'
import type {
  ShortcutAction,
  ShortcutBinding,
} from '../../../types/desktop'

interface ShortcutPanelProps {
  shortcuts: ShortcutBinding[]
  onFocusLauncher: () => Promise<void>
  onReset: () => Promise<void>
}

function renderAccelerator(accelerator: string) {
  return accelerator.split('+').map((part) => <Kbd key={part}>{part}</Kbd>)
}

function shortcutIcon(action: ShortcutAction) {
  switch (action) {
    case 'toggleRecording':
      return <Video aria-hidden="true" size={18} strokeWidth={1.9} />
    case 'pauseRecording':
      return <Pause aria-hidden="true" size={18} strokeWidth={1.9} />
    case 'openLauncher':
      return <AppWindow aria-hidden="true" size={18} strokeWidth={1.9} />
    case 'toggleMicrophone':
      return <Mic aria-hidden="true" size={18} strokeWidth={1.9} />
  }
}

export function ShortcutPanel({
  shortcuts,
  onFocusLauncher,
  onReset,
}: ShortcutPanelProps) {
  return (
    <section className="shortcut-panel">
      <article className="shortcut-panel__hero">
        <div className="shortcut-panel__hero-copy">
          <p className="eyebrow">Keyboard control</p>
          <h3>Control it by keyboard</h3>
          <p className="subtle-copy">Fast actions.</p>
        </div>

        <div className="shortcut-panel__hero-pills">
          <span className="pill">
            <Command aria-hidden="true" size={14} strokeWidth={1.9} />
            {shortcuts.length} bindings
          </span>
          <span className="pill">
            <Video aria-hidden="true" size={14} strokeWidth={1.9} />
            Global
          </span>
        </div>
      </article>

      <div className="shortcut-panel__toolbar">
        <button
          className="button button--secondary"
          onClick={() => void onFocusLauncher()}
          type="button"
        >
          <AppWindow aria-hidden="true" size={16} strokeWidth={1.9} />
          Reveal launcher
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

      <div className="shortcut-panel__list">
        {shortcuts.map((shortcut) => (
          <article className="shortcut-panel__item" key={shortcut.action}>
            <div className="shortcut-panel__copy">
              <span className="shortcut-panel__marker">
                {shortcutIcon(shortcut.action)}
              </span>
              <div>
                <strong>{shortcut.label}</strong>
                <p>{shortcut.description}</p>
              </div>
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
