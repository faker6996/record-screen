import {
  FolderOpen,
  MoonStar,
  SunMedium,
} from 'lucide-react'
import { useMemo, useRef } from 'react'
import { Combobox } from '../../../components/Combobox'
import type { AppSettings, RuntimeDiagnostics } from '../../../types/desktop'

type ThemeMode = 'dark' | 'light'

interface SettingsPanelProps {
  diagnostics: RuntimeDiagnostics
  hasSystemAudioSource: boolean
  onOpenRegionSelector: () => Promise<void>
  qualityPresets: string[]
  settings: AppSettings
  onPickOutputDirectory: () => Promise<void>
  onUpdateThemeMode: (themeMode: ThemeMode) => void
  onUpdateCustomRegion: (
    regionX: number,
    regionY: number,
    regionWidth: number,
    regionHeight: number,
  ) => Promise<void>
  onUpdateLaunchOnLogin: (enabled: boolean) => Promise<void>
  onUpdateOutputDirectory: (outputDirectory: string) => Promise<void>
  onUpdateQualityPreset: (qualityPreset: string) => Promise<void>
  onUpdateShowHudDuringRecording: (enabled: boolean) => Promise<void>
  onUpdateSystemAudioEnabled: (enabled: boolean) => Promise<void>
  themeMode: ThemeMode
}

function qualityPresetLabel(preset: string) {
  return preset
    .replace(' / ', ' · ')
    .replace('fps', ' fps')
}

export function SettingsPanel({
  diagnostics,
  hasSystemAudioSource,
  onOpenRegionSelector,
  qualityPresets,
  settings,
  onPickOutputDirectory,
  onUpdateThemeMode,
  onUpdateCustomRegion,
  onUpdateLaunchOnLogin,
  onUpdateOutputDirectory,
  onUpdateQualityPreset,
  onUpdateShowHudDuringRecording,
  onUpdateSystemAudioEnabled,
  themeMode,
}: SettingsPanelProps) {
  const outputDirectoryRef = useRef<HTMLInputElement | null>(null)
  const regionXRef = useRef<HTMLInputElement | null>(null)
  const regionYRef = useRef<HTMLInputElement | null>(null)
  const regionWidthRef = useRef<HTMLInputElement | null>(null)
  const regionHeightRef = useRef<HTMLInputElement | null>(null)

  const qualityOptions = useMemo(
    () =>
      qualityPresets.map((preset) => ({
        value: preset,
        label: qualityPresetLabel(preset),
      })),
    [qualityPresets],
  )
  const systemAudioUnavailableReason = !diagnostics.supportsSystemAudio
    ? diagnostics.systemAudioNote
    : !hasSystemAudioSource
      ? 'No usable system-audio loopback source is exposed on this machine right now.'
      : null
  const systemAudioToggleDisabled = systemAudioUnavailableReason !== null
  const customRegionDisabled = !diagnostics.supportsCustomRegion

  function commitOutputDirectory() {
    const nextOutputDirectory = outputDirectoryRef.current?.value.trim() ?? ''
    if (!nextOutputDirectory || nextOutputDirectory === settings.outputDirectory) {
      return
    }

    void onUpdateOutputDirectory(nextOutputDirectory)
  }

  function commitCustomRegion() {
    const regionX = Number(regionXRef.current?.value ?? settings.regionX)
    const regionY = Number(regionYRef.current?.value ?? settings.regionY)
    const regionWidth = Number(regionWidthRef.current?.value ?? settings.regionWidth)
    const regionHeight = Number(regionHeightRef.current?.value ?? settings.regionHeight)

    if (
      regionX === settings.regionX &&
      regionY === settings.regionY &&
      regionWidth === settings.regionWidth &&
      regionHeight === settings.regionHeight
    ) {
      return
    }

    void onUpdateCustomRegion(regionX, regionY, regionWidth, regionHeight)
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

        <label
          className={`settings-panel__toggle ${
            systemAudioToggleDisabled ? 'settings-panel__toggle--disabled' : ''
          }`}
          htmlFor="settings-system-audio"
        >
          <div className="settings-panel__toggle-copy">
            <strong>Include system audio</strong>
            <p>
              {systemAudioUnavailableReason ??
                'Mix desktop loopback audio when the current platform exposes a usable source.'}
            </p>
          </div>
          <span
            className={`settings-panel__switch ${
              settings.systemAudioEnabled && !systemAudioToggleDisabled
                ? 'settings-panel__switch--active'
                : ''
            }`}
            aria-hidden="true"
          >
            <span className="settings-panel__switch-thumb" />
          </span>
          <input
            checked={settings.systemAudioEnabled && !systemAudioToggleDisabled}
            disabled={systemAudioToggleDisabled}
            id="settings-system-audio"
            onChange={(event) => {
              void onUpdateSystemAudioEnabled(event.target.checked)
            }}
            type="checkbox"
          />
        </label>
      </div>

      <div className="settings-panel__section">
        <p className="eyebrow">Custom Region</p>
        <div className="settings-panel__region-grid">
          <label className="settings-panel__group">
            <span className="field-label">X</span>
            <input
              className="text-input"
              defaultValue={settings.regionX}
              disabled={customRegionDisabled}
              key={`region-x-${settings.regionX}`}
              min={0}
              onBlur={commitCustomRegion}
              onKeyDown={(event) => {
                if (event.key === 'Enter') {
                  commitCustomRegion()
                }
              }}
              ref={regionXRef}
              type="number"
            />
          </label>
          <label className="settings-panel__group">
            <span className="field-label">Y</span>
            <input
              className="text-input"
              defaultValue={settings.regionY}
              disabled={customRegionDisabled}
              key={`region-y-${settings.regionY}`}
              min={0}
              onBlur={commitCustomRegion}
              onKeyDown={(event) => {
                if (event.key === 'Enter') {
                  commitCustomRegion()
                }
              }}
              ref={regionYRef}
              type="number"
            />
          </label>
          <label className="settings-panel__group">
            <span className="field-label">Width</span>
            <input
              className="text-input"
              defaultValue={settings.regionWidth}
              disabled={customRegionDisabled}
              key={`region-width-${settings.regionWidth}`}
              min={64}
              onBlur={commitCustomRegion}
              onKeyDown={(event) => {
                if (event.key === 'Enter') {
                  commitCustomRegion()
                }
              }}
              ref={regionWidthRef}
              type="number"
            />
          </label>
          <label className="settings-panel__group">
            <span className="field-label">Height</span>
            <input
              className="text-input"
              defaultValue={settings.regionHeight}
              disabled={customRegionDisabled}
              key={`region-height-${settings.regionHeight}`}
              min={64}
              onBlur={commitCustomRegion}
              onKeyDown={(event) => {
                if (event.key === 'Enter') {
                  commitCustomRegion()
                }
              }}
              ref={regionHeightRef}
              type="number"
            />
          </label>
        </div>
        <div className="settings-panel__field-row">
          <button
            className="button button--secondary"
            disabled={customRegionDisabled}
            onClick={() => {
              void onOpenRegionSelector()
            }}
            type="button"
          >
            Select on screen
          </button>
        </div>
        <p className="subtle-copy">
          {customRegionDisabled
            ? diagnostics.customRegionNote
            : 'Select Custom region in the recorder target list to use this area.'}
        </p>
      </div>
    </section>
  )
}
