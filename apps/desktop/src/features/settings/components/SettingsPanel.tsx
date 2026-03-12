import { useState } from 'react'
import type { AppSettings } from '../../../types/desktop'

interface SettingsPanelProps {
  qualityPresets: string[]
  settings: AppSettings
  onHideHud: () => Promise<void>
  onShowHud: () => Promise<void>
  onUpdateLaunchOnLogin: (enabled: boolean) => Promise<void>
  onUpdateOutputDirectory: (outputDirectory: string) => Promise<void>
  onUpdateQualityPreset: (qualityPreset: string) => Promise<void>
}

export function SettingsPanel({
  qualityPresets,
  settings,
  onHideHud,
  onShowHud,
  onUpdateLaunchOnLogin,
  onUpdateOutputDirectory,
  onUpdateQualityPreset,
}: SettingsPanelProps) {
  const [draftOutputDirectory, setDraftOutputDirectory] = useState(settings.outputDirectory)

  function commitOutputDirectory() {
    const nextOutputDirectory = draftOutputDirectory.trim()
    if (!nextOutputDirectory || nextOutputDirectory === settings.outputDirectory) {
      return
    }
    void onUpdateOutputDirectory(nextOutputDirectory)
  }

  return (
    <section className="panel settings-panel">
      <div className="panel__header">
        <div>
          <p className="eyebrow">Settings</p>
          <h2>Recording defaults you should not have to hunt for</h2>
          <p className="subtle-copy">
            Adjust the essentials here once, then keep using the shortcut.
          </p>
        </div>
        <div className="panel__actions">
          <button
            className="button button--secondary"
            onClick={() => void onShowHud()}
            type="button"
          >
            Show HUD
          </button>
          <button
            className="button button--secondary"
            onClick={() => void onHideHud()}
            type="button"
          >
            Hide HUD
          </button>
        </div>
      </div>

      <div className="settings-panel__grid">
        <div className="settings-panel__group">
          <span className="metric-label">Quality preset</span>
          <div className="settings-panel__preset-row">
            {qualityPresets.map((preset) => (
              <button
                className={`chip ${
                  preset === settings.qualityPreset ? 'chip--active' : ''
                }`}
                key={preset}
                onClick={() => void onUpdateQualityPreset(preset)}
                type="button"
              >
                {preset}
              </button>
            ))}
          </div>
        </div>

        <div className="settings-panel__group">
          <label className="field-label" htmlFor="output-directory">
            Output folder
          </label>
          <div className="settings-panel__field-row">
            <input
              autoComplete="off"
              className="text-input"
              id="output-directory"
              key={settings.outputDirectory}
              name="output-directory"
              onBlur={commitOutputDirectory}
              onChange={(event) => {
                setDraftOutputDirectory(event.target.value)
              }}
              onKeyDown={(event) => {
                if (event.key === 'Enter') {
                  commitOutputDirectory()
                }
              }}
              placeholder="~/Movies/Record Screen"
              type="text"
              value={draftOutputDirectory}
            />
            <button
              className="button button--secondary"
              onClick={() => {
                commitOutputDirectory()
              }}
              type="button"
            >
              Save
            </button>
          </div>
        </div>

        <label className="settings-panel__toggle" htmlFor="launch-on-login">
          <div>
            <strong>Launch on login</strong>
            <p>
              Keep the recorder and its shortcuts available right after sign in.
            </p>
          </div>
          <input
            checked={settings.launchOnLogin}
            id="launch-on-login"
            onChange={(event) => {
              void onUpdateLaunchOnLogin(event.target.checked)
            }}
            type="checkbox"
          />
        </label>
      </div>
    </section>
  )
}
