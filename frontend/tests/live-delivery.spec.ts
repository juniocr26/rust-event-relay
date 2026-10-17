import { test, expect } from '@playwright/test'
import { readFileSync, writeFileSync } from 'node:fs'
const token = readFileSync('/tmp/demo61-browser-token', 'utf8').trim()
const headers = { Authorization: `Bearer ${token}`, 'X-Demo-Client': 'demo61' }

test('real browser → producer → relay → broker → consumer, with persistent inspectable evidence', async ({ page, request, context }) => {
  const errors: string[] = []; page.on('pageerror', e => errors.push(e.message))
  await page.goto('/')
  await page.getByRole('button', { name: 'Open experience' }).click()
  await page.getByLabel('Local demo token', { exact: true }).fill(token); await page.getByRole('button', { name: 'Verify access' }).click()
  await page.getByRole('button', { name: 'Run real delivery' }).click()
  await expect(page.getByText('Confirmed: receipt committed and stock 15; relay completion observed.').first()).toBeVisible({ timeout: 20000 })
  const key = await page.evaluate(() => sessionStorage.getItem('relay-request:normal'))
  const response = await request.get(`/demo/runs/${key}`, { headers })
  expect(response.ok()).toBeTruthy(); const e = await response.json()
  expect(e.outbox.status).toBe('processed'); expect(e.stock).toBe(15)
  expect(e.receipt.stock_before).toBe(10); expect(e.receipt.stock_after).toBe(15)
  await expect(page.getByRole('button', { name: 'Copy Run ID', exact: true })).toBeVisible()
  await expect(page.getByRole('button', { name: 'Copy Event ID', exact: true })).toBeVisible()
  await context.grantPermissions(['clipboard-read', 'clipboard-write'])
  await page.getByRole('button', { name: 'Copy Event ID', exact: true }).click()
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(e.event_id)
  await page.getByRole('button', { name: 'Copy Run ID', exact: true }).click()
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(e.run_id)
  // Values remain selectable even when the browser Clipboard API is unavailable.
  await expect(page.getByText(e.event_id, { exact: true })).toBeVisible()
  await page.getByText('Where to verify', { exact: true }).click()
  await expect(page.getByText('SELECT * FROM demo.runs', { exact: false })).toBeVisible()
  await expect(page.locator('.pulse')).toHaveCount(0)
  await page.keyboard.press('Escape')
  await expect(page.getByRole('button', { name: 'Open experience' })).toBeFocused()
  const after = await request.get(`/demo/runs/${key}`, { headers }); expect((await after.json()).stock).toBe(15)
  writeFileSync('/tmp/demo61-browser-evidence.json', JSON.stringify(e))
  expect(errors).toEqual([])
})

test('real unavailable authorization, safe same-identity recovery and close without stale updates', async ({ page }) => {
  await page.goto('/'); await page.getByRole('button', { name: 'Open experience' }).click()
  await page.getByLabel('Local demo token', { exact: true }).fill('0'.repeat(64))
  await page.getByRole('button', { name: 'Verify access' }).click()
  await expect(page.getByText('Authorization required. Check the demo token.').first()).toBeVisible()
  expect(await page.evaluate(() => sessionStorage.getItem('relay-request:normal'))).toBeNull()
  await page.getByLabel('Local demo token', { exact: true }).fill(token)
  await page.getByRole('button', { name: 'Verify access' }).click()
  await page.getByRole('button', { name: 'Run real delivery' }).click()
  await expect(page.getByText('Confirmed: receipt committed and stock 15; relay completion observed.').first()).toBeVisible({ timeout: 20000 })
})

for (const width of [320, 375, 768, 1024, 1920]) {
  test(`real modal ${width}: keyboard, reduced motion, responsive evidence`, async ({ page }) => {
    await page.setViewportSize({ width, height: 900 }); await page.emulateMedia({ reducedMotion: 'reduce' })
    await page.goto('/'); await page.getByRole('button', { name: 'Open experience' }).click()
    await expect(page.getByRole('button', { name: 'Close', exact: true })).toBeFocused()
    await page.keyboard.press('Tab'); await expect(page.getByLabel('Local demo token', { exact: true })).toBeFocused()
    await page.getByLabel('Local demo token', { exact: true }).fill(token); await page.getByRole('button', { name: 'Verify access' }).click()
    await page.getByRole('button', { name: 'Run real delivery' }).click()
    await expect(page.getByText('Confirmed: receipt committed and stock 15; relay completion observed.').first()).toBeVisible({ timeout: 20000 })
    expect(await page.locator('.modal').evaluate(el => el.scrollWidth <= el.clientWidth)).toBe(true)
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(width)
    await page.screenshot({ path: `/tmp/demo61-live-${width}.png`, fullPage: true })
    await page.keyboard.press('Escape'); await expect(page.getByRole('button', { name: 'Open experience' })).toBeFocused()
  })
}

test('exact loopback origins supported; foreign origin and invalid credentials rejected', async ({ page, request, baseURL }) => {
  await page.waitForTimeout(1100)
  for (const host of ['localhost', '127.0.0.1']) {
    const url = new URL(baseURL!); url.hostname = host
    const response = await request.get('/demo/config', { headers: { ...headers, Origin: url.origin } })
    expect(response.status()).toBe(200)
    await page.goto(url.href)
    await page.getByRole('button', { name: 'Open experience' }).click()
    await page.getByLabel('Local demo token', { exact: true }).fill(token)
    await page.getByRole('button', { name: 'Verify access' }).click()
    await expect(page.getByText('Connected. Access verified; ready to run.')).toBeVisible()
  }
  expect((await request.get('/demo/config', { headers: { ...headers, Origin: 'http://example.invalid:5173' } })).status()).toBe(403)
  expect((await request.get('/demo/config', { headers: { ...headers, Authorization: 'Bearer invalid' } })).status()).toBe(401)
})

test('configuration proxy failure submits no intent and keeps narrow recovery readable', async ({ page }) => {
  await page.setViewportSize({ width: 320, height: 800 })
  const mutations: string[] = []
  page.on('request', request => { if (request.method() === 'POST') mutations.push(request.url()) })
  await page.route('**/demo/config', route => route.fulfill({ status: 502, body: '' }))
  await page.goto('/')
  await page.getByRole('button', { name: 'Open experience' }).click()
  await page.getByLabel('Local demo token', { exact: true }).fill(token)
  await page.getByRole('button', { name: 'Verify access' }).click()
  await expect(page.getByText('Backend unreachable or proxy failure (HTTP 502). This read submitted no work.')).toBeVisible()
  await expect(page.getByRole('button', { name: 'Run real delivery' })).toBeDisabled()
  expect(await page.evaluate(() => sessionStorage.getItem('relay-request:normal'))).toBeNull()
  expect(mutations).toEqual([])
  await page.getByText('Recovery details', { exact: true }).click()
  expect(await page.locator('.modal').evaluate(el => el.scrollWidth <= el.clientWidth)).toBe(true)
  const width = await page.locator('.recovery-details').evaluate(el => el.getBoundingClientRect().width)
  expect(width).toBeGreaterThan(240)
})
