import { useEffect, useMemo, useState, type KeyboardEvent } from 'react'
import { AppWindow, Keyboard, RotateCcw, Save, X } from 'lucide-react'
import { Kbd } from '../../../components/Kbd'
import type { ShortcutAction, ShortcutBinding } from '../../../types/desktop'

interface ShortcutPanelProps {
  shortcuts: ShortcutBinding[]
  onFocusLauncher: () => Promise<void>
  onReset: () => Promise<void>
  onUpdateShortcut: (
    action: ShortcutAction,
    accelerator: string,
  ) => Promise<void>
}

function renderAccelerator(accelerator: string) {
  return accelerator
    .split('+')
    .map((part) => part.replace('CmdOrCtrl', 'Cmd'))
    .map((part) => <Kbd key={part}>{part}</Kbd>)
}

function normalizeShortcutKey(key: string) {
  if (key.length === 1 && /^[a-z0-9]$/i.test(key)) {
    return key.toUpperCase()
  }

  switch (key) {
    case ' ':
      return 'Space'
    case 'ArrowUp':
      return 'Up'
    case 'ArrowDown':
      return 'Down'
    case 'ArrowLeft':
      return 'Left'
    case 'ArrowRight':
      return 'Right'
    case 'Escape':
      return 'Esc'
    default:
      return key.length > 1 ? key : null
  }
}

function captureAccelerator(event: KeyboardEvent<HTMLInputElement>) {
  const key = normalizeShortcutKey(event.key)
  if (!key || ['Meta', 'Control', 'Alt', 'Shift'].includes(event.key)) {
    return null
  }

  const modifiers: string[] = []
  if (event.metaKey || event.ctrlKey) {
    modifiers.push('CmdOrCtrl')
  }
  if (event.altKey) {
    modifiers.push('Alt')
  }
  if (event.shiftKey) {
    modifiers.push('Shift')
  }

  if (modifiers.length === 0) {
    return null
  }

  return [...modifiers, key].join('+')
}

export function ShortcutPanel({
  shortcuts,
  onFocusLauncher,
  onReset,
  onUpdateShortcut,
}: ShortcutPanelProps) {
  const [editingAction, setEditingAction] = useState<ShortcutAction | null>(null)
  const [draftAccelerator, setDraftAccelerator] = useState('')
  const [isSaving, setIsSaving] = useState(false)
  const activeShortcut = useMemo(
    () => shortcuts.find((shortcut) => shortcut.action === editingAction) ?? null,
    [editingAction, shortcuts],
  )

  useEffect(() => {
    if (!activeShortcut) {
      return
    }

    setDraftAccelerator(activeShortcut.accelerator)
  }, [activeShortcut])

  async function saveShortcut() {
    if (!editingAction || !draftAccelerator.trim()) {
      return
    }

    setIsSaving(true)
    try {
      await onUpdateShortcut(editingAction, draftAccelerator.trim())
      setEditingAction(null)
      setDraftAccelerator('')
    } finally {
      setIsSaving(false)
    }
  }

  return (
    <section className="shortcut-panel">
      <header className="shortcut-panel__header">
        <div>
          <h3>Global Shortcuts</h3>
          <p className="subtle-copy">
            Edit each binding directly, then save to re-register it with the desktop shell.
          </p>
        </div>
      </header>

      <div className="shortcut-panel__list">
        {shortcuts.map((shortcut, index) => {
          const isEditing = editingAction === shortcut.action

          return (
            <article className="shortcut-panel__item" key={shortcut.action}>
              <div className="shortcut-panel__copy">
                <strong>{shortcut.label}</strong>
                <p className="subtle-copy">{shortcut.description}</p>
              </div>

              {!isEditing ? (
                <>
                  <div
                    aria-label={shortcut.accelerator}
                    className="shortcut-panel__keys"
                  >
                    {renderAccelerator(shortcut.accelerator)}
                  </div>
                  <div className="shortcut-panel__row-actions">
                    <button
                      className="button button--secondary"
                      onClick={() => {
                        setEditingAction(shortcut.action)
                        setDraftAccelerator(shortcut.accelerator)
                      }}
                      type="button"
                    >
                      <Keyboard aria-hidden="true" size={16} strokeWidth={1.9} />
                      Edit
                    </button>
                  </div>
                </>
              ) : (
                <div className="shortcut-panel__editor">
                  <label className="shortcut-panel__capture">
                    <span className="field-label">Press the new shortcut</span>
                    <input
                      autoFocus
                      className="text-input shortcut-panel__capture-input"
                      onKeyDown={(event) => {
                        event.preventDefault()
                        const accelerator = captureAccelerator(event)
                        if (accelerator) {
                          setDraftAccelerator(accelerator)
                        }
                      }}
                      onChange={() => undefined}
                      readOnly
                      type="text"
                      value={draftAccelerator}
                    />
                  </label>
                  <div className="shortcut-panel__editor-actions">
                    <button
                      className="button button--primary"
                      disabled={!draftAccelerator.trim() || isSaving}
                      onClick={() => {
                        void saveShortcut()
                      }}
                      type="button"
                    >
                      <Save aria-hidden="true" size={16} strokeWidth={1.9} />
                      Save
                    </button>
                    <button
                      className="button button--secondary"
                      onClick={() => {
                        setEditingAction(null)
                        setDraftAccelerator('')
                      }}
                      type="button"
                    >
                      <X aria-hidden="true" size={16} strokeWidth={1.9} />
                      Cancel
                    </button>
                  </div>
                  <p className="subtle-copy shortcut-panel__hint">
                    Use at least one modifier key like <code>CmdOrCtrl</code>, <code>Shift</code>,
                    or <code>Alt</code>.
                  </p>
                </div>
              )}

              {index < shortcuts.length - 1 ? (
                <span className="shortcut-panel__divider" aria-hidden="true" />
              ) : null}
            </article>
          )
        })}
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
