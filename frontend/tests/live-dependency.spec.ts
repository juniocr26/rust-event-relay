import { test, expect } from '@playwright/test'
import { readFileSync } from 'node:fs'
test('dependency unavailable never fabricates producer or consumer success', async ({ page }) => {
  await page.goto('/'); await page.getByRole('button', { name: 'Open experience' }).click()
  await page.getByLabel('Local demo token', { exact: true }).fill(readFileSync('/tmp/demo61-browser-token','utf8').trim())
  await page.getByRole('button', { name: 'Run real delivery' }).click()
  await expect(page.getByText('Submission outcome may be unknown.', { exact: false }).first()).toBeVisible()
  await expect(page.getByRole('button', { name: 'Copy Run ID' })).toHaveCount(0)
  await expect(page.getByText('Confirmed: receipt committed', { exact: false })).toHaveCount(0)
  await expect(page.getByText('No fresh evidence is available.').first()).toBeVisible()
  const key = await page.evaluate(() => sessionStorage.getItem('relay-request:normal'))
  await page.keyboard.press('Escape'); await page.getByRole('button', { name: 'Open experience' }).click()
  expect(await page.evaluate(() => sessionStorage.getItem('relay-request:normal'))).toBe(key)
  await expect(page.getByRole('button', { name: 'Recover same request' })).toBeDisabled()
})
