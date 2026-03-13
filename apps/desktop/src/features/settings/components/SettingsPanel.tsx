import {
  FolderOpen,
  MoonStar,
  SunMedium,
} from 'lucide-react'
import { useMemo, useRef } from 'react'
import { Combobox } from '../../../components/Combobox'
import type { AppSettings } from '../../../types/desktop'

type ThemeMode = 'dark' | 'light'

interface SettingsPanelProps {
  qualityPresets: string[]
  settings: AppSettings
  onPickOutputDirectory: () => Promise<void>
  onUpdateThemeMode: (themeMode: ThemeMode) => void
  onUpdateLaunchOnLogin: (enabled: boolean) => Promise<void>
  onUpdateOutputDirectory: (outputDirectory: string) => Promise<void>
  onUpdateQualityPreset: (qualityPreset: string) => Promise<void>
  onUpdateShowHudDuringRecording: (enabled: boolean) => Promise<void>
  themeMode: ThemeMode
}

function qualityPresetLabel(preset: string) {
  return preset
    .replace(' / ', ' · ')
    .replace('fps', ' fps')
}

export function SettingsPanel({
  qualityPresets,
  settings,
  onPickOutputDirectory,
  onUpdateThemeMode,
  onUpdateLaunchOnLogin,
  onUpdateOutputDirectory,
  onUpdateQualityPreset,
  onUpdateShowHudDuringRecording,
  themeMode,
}: SettingsPanelProps) {
  const outputDirectoryRef = useRef<HTMLInputElement | null>(null)

  const qualityOptions = useMemo(
    () =>
      qualityPresets.map((preset) => ({
        value: preset,
        label: qualityPresetLabel(preset),
      })),
    [qualityPresets],
  )

  function commitOutputDirectory() {
    const nextOutputDirectory = outputDirectoryRef.current?.value.trim() ?? ''
    if (!nextOutputDirectory || nextOutputDirectory === settings.outputDirectory) {
      return
    }

    void onUpdateOutputDirectory(nextOutputDirectory)
  }

  return (
    <section className="settings-panel">
      <header className="settings-panel__header">
        <div>
          <h3>Settings</h3>
          <p className="subtle-copy">Configure recording quality and app behavior.</p>
        </div>

        <div className="settings-panel__theme-switcher" aria-label="Theme mode">
          <button
            className={`settings-panel__theme-button ${
              themeMode === 'dark' ? 'settings-panel__theme-button--active' : ''
            }`}
            onClick={() => {
              onUpdateThemeMode('dark')
            }}
            type="button"
          >
            <MoonStar aria-hidden="true" size={15} strokeWidth={1.9} />
            Dark
          </button>
          <button
            className={`settings-panel__theme-button ${
              themeMode === 'light' ? 'settings-panel__theme-button--active' : ''
            }`}
            onClick={() => {
              onUpdateThemeMode('light')
            }}
            type="button"
          >
            <SunMedium aria-hidden="true" size={15} strokeWidth={1.9} />
            Light
          </button>
        </div>
      </header>

      <div className="settings-panel__section">
        <p className="eyebrow">Output</p>

        <div className="settings-panel__group">
          <label className="field-label" htmlFor="settings-quality-preset">
            Quality Preset
          </label>
          <Combobox
            ariaLabel="Quality preset"
            className="settings-panel__combobox"
            onChange={(value) => {
              void onUpdateQualityPreset(value)
            }}
            options={qualityOptions}
            value={settings.qualityPreset}
          />
        </div>

        <div className="settings-panel__group">
          <label className="field-label" htmlFor="settings-output-directory">
            Save Location
          </label>

          <div className="settings-panel__field-row">
            <input
              autoComplete="off"
              className="text-input settings-panel__directory-input"
              defaultValue={settings.outputDirectory}
              id="settings-output-directory"
              key={settings.outputDirectory}
              onBlur={commitOutputDirectory}
              onKeyDown={(event) => {
                if (event.key === 'Enter') {
                  commitOutputDirectory()
                }
              }}
              placeholder="~/Movies/Record Screen"
              ref={outputDirectoryRef}
              type="text"
            />
            <button
              className="button button--secondary settings-panel__browse-button"
              onClick={() => {
                void onPickOutputDirectory()
              }}
              type="button"
            >
              <FolderOpen aria-hidden="true" size={16} strokeWidth={1.9} />
              Browse
            </button>
          </div>
        </div>
      </div>

      <div className="settings-panel__section settings-panel__section--behavior">
        <p className="eyebrow">Behavior</p>

        <label className="settings-panel__toggle" htmlFor="settings-show-hud">
          <div className="settings-panel__toggle-copy">
            <strong>Show HUD while recording</strong>
            <p>Display a small floating control bar.</p>
          </div>
          <span
            className={`settings-panel__switch ${
              settings.showHudDuringRecording ? 'settings-panel__switch--active' : ''
            }`}
            aria-hidden="true"
          >
            <span className="settings-panel__switch-thumb" />
          </span>
          <input
            checked={settings.showHudDuringRecording}
            id="settings-show-hud"
            onChange={(event) => {
              void onUpdateShowHudDuringRecording(event.target.checked)
            }}
            type="checkbox"
          />
        </label>

        <label className="settings-panel__toggle" htmlFor="settings-launch-on-login">
          <div className="settings-panel__toggle-copy">
            <strong>Launch on Login</strong>
            <p>Start app silently in system tray.</p>
          </div>
          <span
            className={`settings-panel__switch ${
              settings.launchOnLogin ? 'settings-panel__switch--active' : ''
            }`}
            aria-hidden="true"
          >
            <span className="settings-panel__switch-thumb" />
          </span>
          <input
            checked={settings.launchOnLogin}
            id="settings-launch-on-login"
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
