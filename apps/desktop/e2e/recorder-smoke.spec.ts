import { expect, test, type Page } from '@playwright/test'

async function selectComboboxOption(
  page: Page,
  triggerTestId: string,
  optionLabel: string,
) {
  await page.getByTestId(triggerTestId).click()
  await page.getByRole('option', { name: optionLabel }).click()
}

test.describe('recorder launcher smoke', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/')
    await expect(page.getByRole('heading', { name: 'Ready to Record' })).toBeVisible()
  })

  test('starts preparing and can stop before active recording settles', async ({ page }) => {
    const recordButton = page.getByTestId('recorder-record-button')
    const statusPill = page.getByTestId('recorder-status-pill')
    const timer = page.locator('.recorder-panel__timer')

    await expect(statusPill).toHaveText('idle')
    await expect(recordButton).toContainText('REC')

    await recordButton.click()
    await expect(statusPill).toHaveText('starting')
    await expect(page.getByRole('heading', { name: 'Preparing Recorder' })).toBeVisible()
    await expect(timer).toContainText('Preparing…')
    await expect(recordButton).toContainText('WAIT')

    await page.waitForTimeout(900)
    await recordButton.click()
    await expect(statusPill).toHaveText('finalizing')
    await expect(page.getByRole('heading', { name: 'Finalizing' })).toBeVisible()
    await expect(recordButton).toBeDisabled()

    await expect(page.getByRole('heading', { name: 'Ready to Record' })).toBeVisible({
      timeout: 5_000,
    })
    await expect(timer).toHaveCount(0)
    await expect(statusPill).toHaveText('idle', { timeout: 5_000 })
    await expect(recordButton).toContainText('REC')
  })

  test('records after preparing and moves through finalizing before idle', async ({ page }) => {
    const recordButton = page.getByTestId('recorder-record-button')
    const statusPill = page.getByTestId('recorder-status-pill')
    const timer = page.locator('.recorder-panel__timer')

    await recordButton.click()
    await expect(statusPill).toHaveText('starting')
    await expect(page.getByRole('heading', { name: 'Preparing Recorder' })).toBeVisible()
    await expect(timer).toContainText('Preparing…')
    await expect(statusPill).toHaveText('recording', { timeout: 8_000 })
    await expect(page.getByRole('heading', { name: 'Recording' })).toBeVisible()
    await expect(timer).toContainText('00:00:00')

    await recordButton.click()
    await expect(statusPill).toHaveText('finalizing')
    await expect(page.getByRole('heading', { name: 'Finalizing' })).toBeVisible()
    await expect(recordButton).toBeDisabled()

    await expect(statusPill).toHaveText('idle', { timeout: 5_000 })
    await expect(page.getByRole('heading', { name: 'Ready to Record' })).toBeVisible()
    await expect(recordButton).toContainText('REC')
  })

  test('simulates a full GUI recorder flow with mic controls and pause resume', async ({ page }) => {
    const recordButton = page.getByTestId('recorder-record-button')
    const statusPill = page.getByTestId('recorder-status-pill')
    const audioToggle = page.getByTestId('recorder-audio-toggle-button')
    const micCheckButton = page.getByTestId('recorder-mic-check-button')
    const pauseButton = page.getByTestId('recorder-pause-button')

    await expect(audioToggle).toHaveAttribute('aria-pressed', 'true')

    await audioToggle.click()
    await expect(audioToggle).toHaveAttribute('aria-pressed', 'false')
    await expect(page.getByText('Audio off')).toBeVisible()
    await expect(page.getByTestId('recorder-audio-input-trigger')).toBeDisabled()

    await audioToggle.click()
    await expect(audioToggle).toHaveAttribute('aria-pressed', 'true')
    await expect(page.getByTestId('recorder-audio-input-trigger')).toBeEnabled()

    await selectComboboxOption(page, 'recorder-capture-target-trigger', 'Display 2')
    await selectComboboxOption(page, 'recorder-audio-input-trigger', 'USB Audio Interface')

    await expect(page.getByTestId('recorder-capture-target-trigger')).toContainText('Display 2')
    await expect(page.getByTestId('recorder-audio-input-trigger')).toContainText('USB Audio Interface')

    await expect(micCheckButton).toContainText('Test input')
    await micCheckButton.click()
    await expect(micCheckButton).toContainText('Stop test')
    await expect(page.getByText('Mic detected')).toBeVisible()
    await micCheckButton.click()
    await expect(micCheckButton).toContainText('Test input')

    await recordButton.click()
    await expect(statusPill).toHaveText('starting')
    await expect(page.getByRole('heading', { name: 'Preparing Recorder' })).toBeVisible()
    await expect(statusPill).toHaveText('recording', { timeout: 8_000 })
    await expect(page.getByRole('heading', { name: 'Recording' })).toBeVisible()
    await expect(page.getByTestId('recorder-active-file')).toContainText('recording-preview.mp4')

    await pauseButton.click()
    await expect(statusPill).toHaveText('paused')
    await expect(page.getByRole('heading', { name: 'Paused' })).toBeVisible()
    await expect(pauseButton).toContainText('Resume')

    await pauseButton.click()
    await expect(statusPill).toHaveText('recording')
    await expect(page.getByRole('heading', { name: 'Recording' })).toBeVisible()
    await expect(pauseButton).toContainText('Pause')

    await recordButton.click()
    await expect(statusPill).toHaveText('finalizing')
    await expect(page.getByRole('heading', { name: 'Finalizing' })).toBeVisible()

    await expect(statusPill).toHaveText('idle', { timeout: 5_000 })
    await expect(page.getByRole('heading', { name: 'Ready to Record' })).toBeVisible()
    await expect(recordButton).toContainText('REC')
  })
})
