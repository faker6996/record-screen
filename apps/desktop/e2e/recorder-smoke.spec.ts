import { expect, test } from '@playwright/test'

test.describe('recorder launcher smoke', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/')
    await expect(page.getByRole('heading', { name: 'Ready to Record' })).toBeVisible()
  })

  test('starts countdown and can cancel before recording begins', async ({ page }) => {
    const recordButton = page.getByTestId('recorder-record-button')
    const statusPill = page.getByTestId('recorder-status-pill')

    await expect(statusPill).toHaveText('idle')
    await expect(recordButton).toContainText('REC')

    await recordButton.click()
    await expect(page.getByTestId('recorder-countdown-copy')).toContainText('Starting in 3')
    await expect(recordButton).toContainText('CANCEL')

    await recordButton.click()
    await expect(page.getByRole('heading', { name: 'Ready to Record' })).toBeVisible()
    await expect(page.getByTestId('recorder-countdown-copy')).toHaveCount(0)
    await expect(statusPill).toHaveText('idle')
    await expect(recordButton).toContainText('REC')
  })

  test('records after countdown and moves through finalizing before idle', async ({ page }) => {
    const recordButton = page.getByTestId('recorder-record-button')
    const statusPill = page.getByTestId('recorder-status-pill')

    await recordButton.click()
    await expect(page.getByTestId('recorder-countdown-copy')).toContainText('Starting in 3')
    await expect(statusPill).toHaveText('recording', { timeout: 8_000 })
    await expect(page.getByRole('heading', { name: 'Recording' })).toBeVisible()

    await recordButton.click()
    await expect(statusPill).toHaveText('finalizing')
    await expect(page.getByRole('heading', { name: 'Finalizing' })).toBeVisible()
    await expect(recordButton).toBeDisabled()

    await expect(statusPill).toHaveText('idle', { timeout: 5_000 })
    await expect(page.getByRole('heading', { name: 'Ready to Record' })).toBeVisible()
    await expect(recordButton).toContainText('REC')
  })
})
