import {
  Eye,
  EyeOff,
  FolderOpen,
  MoonStar,
  Save,
  SlidersHorizontal,
  Sparkles,
  SunMedium,
} from 'lucide-react'
import { useState } from 'react'
import type { AppSettings } from '../../../types/desktop'

type ThemeMode = 'dark' | 'light'

interface SettingsPanelProps {
  qualityPresets: string[]
  settings: AppSettings
  onHideHud: () => Promise<void>
  onShowHud: () => Promise<void>
  onUpdateThemeMode: (themeMode: ThemeMode) => void
  onUpdateLaunchOnLogin: (enabled: boolean) => Promise<void>
  onUpdateOutputDirectory: (outputDirectory: string) => Promise<void>
  onUpdateQualityPreset: (qualityPreset: string) => Promise<void>
  themeMode: ThemeMode
}

export function SettingsPanel({
  qualityPresets,
  settings,
  onHideHud,
  onShowHud,
  onUpdateThemeMode,
  onUpdateLaunchOnLogin,
  onUpdateOutputDirectory,
  onUpdateQualityPreset,
  themeMode,
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
    <section className="settings-panel">
      <article className="settings-panel__hero">
        <div className="settings-panel__hero-copy">
          <p className="eyebrow">Recorder defaults</p>
          <h3>Keep defaults simple</h3>
          <p className="subtle-copy">Set it once.</p>
        </div>

        <div className="settings-panel__hero-metrics">
          <span className="pill">
            <Sparkles aria-hidden="true" size={14} strokeWidth={1.9} />
            {themeMode === 'dark' ? 'Dark theme' : 'Light theme'}
          </span>
          <span className="pill">
            <SlidersHorizontal aria-hidden="true" size={14} strokeWidth={1.9} />
            {settings.qualityPreset}
          </span>
          <span className="pill">
            <FolderOpen aria-hidden="true" size={14} strokeWidth={1.9} />
            {settings.outputDirectory}
          </span>
        </div>
      </article>

      <div className="settings-panel__toolbar">
        <button
          className="button button--secondary"
          onClick={() => void onShowHud()}
          type="button"
        >
          <Eye aria-hidden="true" size={16} strokeWidth={1.9} />
          Show HUD
        </button>
        <button
          className="button button--secondary"
          onClick={() => void onHideHud()}
          type="button"
        >
          <EyeOff aria-hidden="true" size={16} strokeWidth={1.9} />
          Hide HUD
        </button>
      </div>

      <div className="settings-panel__grid">
        <section className="settings-panel__card">
          <div className="settings-panel__card-copy">
            <p className="eyebrow">Appearance</p>
            <h3>Theme</h3>
            <p className="subtle-copy">Dark or light.</p>
          </div>

          <div className="settings-panel__theme-row">
            <button
              className={`settings-panel__theme-chip ${
                themeMode === 'dark' ? 'settings-panel__theme-chip--active' : ''
              }`}
              onClick={() => {
                onUpdateThemeMode('dark')
              }}
              type="button"
            >
              <MoonStar aria-hidden="true" size={18} strokeWidth={1.9} />
              <span>Dark</span>
              <small>Low glare</small>
            </button>
            <button
              className={`settings-panel__theme-chip ${
                themeMode === 'light' ? 'settings-panel__theme-chip--active' : ''
              }`}
              onClick={() => {
                onUpdateThemeMode('light')
              }}
              type="button"
            >
              <SunMedium aria-hidden="true" size={18} strokeWidth={1.9} />
              <span>Light</span>
              <small>Bright UI</small>
            </button>
          </div>
        </section>

        <section className="settings-panel__card">
          <div className="settings-panel__card-copy">
            <p className="eyebrow">Output quality</p>
            <h3>Preset</h3>
            <p className="subtle-copy">Pick one.</p>
          </div>

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
        </section>

        <section className="settings-panel__card">
          <div className="settings-panel__card-copy">
            <p className="eyebrow">Storage</p>
            <h3>Save location</h3>
            <p className="subtle-copy">Where clips go.</p>
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
                <Save aria-hidden="true" size={16} strokeWidth={1.9} />
                Save
              </button>
            </div>
          </div>
        </section>

        <section className="settings-panel__card">
          <div className="settings-panel__card-copy">
            <p className="eyebrow">Behavior</p>
            <h3>Behavior</h3>
            <p className="subtle-copy">Startup options.</p>
          </div>

          <label className="settings-panel__toggle" htmlFor="launch-on-login">
            <div>
              <strong>Launch on login</strong>
              <p>Start after sign in.</p>
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
        </section>
      </div>
    </section>
  )
}
