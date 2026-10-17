import { test, expect } from '@playwright/test'

test('presentation loads its resources and favicon without runtime errors or API calls', async ({ page }) => {
  const errors: string[] = []
  const apiCalls: string[] = []
  page.on('pageerror', error => errors.push(error.message))
  page.on('console', message => { if (message.type() === 'error') errors.push(message.text()) })
  page.on('requestfailed', request => errors.push(`failed resource: ${request.url()}`))
  page.on('response', response => {
    if (response.status() >= 400) errors.push(`HTTP ${response.status()}: ${response.url()}`)
  })
  page.on('request', request => {
    if (['fetch', 'xhr'].includes(request.resourceType())) apiCalls.push(request.url())
  })
  await page.goto('/')
  await page.getByLabel('Choose an experience').selectOption('ack')
  await expect(page.getByRole('button', { name: 'Open experience' })).toBeVisible()
  const favicon = page.locator('link[rel="icon"]')
  await expect(favicon).toHaveAttribute('href', '/favicon.svg')
  const response = await page.request.get((await favicon.getAttribute('href'))!)
  expect(response.ok()).toBe(true)
  expect(await response.text()).toContain('<svg')
  await page.clock.install()
  await page.getByRole('button', { name: 'Open experience' }).click()
  await page.getByRole('button', { name: 'Run scenario' }).click()
  await page.clock.runFor(14400)
  await expect(page.locator('.result strong')).toHaveText('? Uncertain response · committed outcome observed')
  await page.keyboard.press('Escape')
  expect(errors).toEqual([])
  expect(apiCalls).toEqual([])
})

for (const width of [320, 375, 768, 1024, 1920]) {
  test(`viewport ${width}: full landing, readable modal, accessible controls`, async ({ page }) => {
    await page.setViewportSize({ width, height: 900 })
    await page.goto('/')
    await page.getByLabel('Choose an experience').selectOption('ack')
    const landing = await page.locator('.landing').boundingBox()
    expect(landing?.width).toBe(width)
    expect(landing!.height).toBeGreaterThanOrEqual(900)
    await expect(page.locator('body')).toHaveCSS('margin', '0px')
    await page.screenshot({ path: `/tmp/relay-landing-${width}.png`, fullPage: true })
    await page.getByRole('button', { name: 'Open experience' }).click()
    await expect(page.getByRole('dialog')).toBeVisible()
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(width)
    expect(await page.locator('.modal').evaluate((el) => el.scrollWidth <= el.clientWidth)).toBe(
      true,
    )
    await expect(page.getByRole('button', { name: 'Close', exact: true })).toBeFocused()
    await expect(page.getByRole('heading', { name: 'Illustrative · Lose a confirmation' })).toBeInViewport()
    await page.screenshot({ path: `/tmp/relay-modal-top-${width}.png`, fullPage: true })
    await page.getByRole('button', { name: 'Run scenario' }).scrollIntoViewIfNeeded()
    await expect(page.getByRole('button', { name: 'Run scenario' })).toBeInViewport()
    await page.screenshot({ path: `/tmp/relay-${width}.png`, fullPage: true })
    await page.clock.install()
    await page.getByRole('button', { name: 'Run scenario' }).click()
    await page.clock.runFor(14400)
    await expect(page.locator('.output-line')).toHaveCount(4)
    await page.getByRole('button', { name: 'Run again' }).scrollIntoViewIfNeeded()
    await expect(page.getByRole('button', { name: 'Run again' })).toBeInViewport()
    expect(await page.locator('.modal').evaluate((el) => el.scrollWidth <= el.clientWidth)).toBe(true)
    await page.screenshot({ path: `/tmp/relay-completed-${width}.png`, fullPage: true })
    // Stress layout with unbroken identifiers and output, beyond the short demo envelope.
    await page.locator('.envelope code').first().evaluate(el => { el.textContent = 'event_'.repeat(100) })
    await page.locator('.output-line').first().evaluate(el => { el.textContent = 'output_'.repeat(100) })
    expect(await page.locator('.modal').evaluate((el) => el.scrollWidth <= el.clientWidth)).toBe(true)
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(width)
    await page.keyboard.press('Escape')
    await expect(page.getByRole('button', { name: 'Open experience' })).toBeFocused()
    await expect(page.getByRole('dialog')).toHaveCount(0)
  })
}

test('modal contains focus, blocks background and cleans playback on close', async ({ page }) => {
  await page.goto('/')
  await page.getByLabel('Choose an experience').selectOption('ack')
  await page.clock.install()
  await page.getByRole('button', { name: 'Open experience' }).click()
  await page.keyboard.press('Shift+Tab')
  await expect(page.getByRole('button', { name: 'Run scenario' })).toBeFocused()
  await page.keyboard.press('Tab')
  await expect(page.getByRole('button', { name: 'Close', exact: true })).toBeFocused()
  expect(await page.evaluate(() => document.querySelector('dialog')?.matches(':modal'))).toBe(true)
  await expect(page.locator('body')).toHaveCSS('overflow', 'hidden')
  await page.getByRole('button', { name: 'Run scenario' }).click()
  await page.clock.runFor(3600)
  await expect(page.locator('.current')).toHaveText('Broker accepts the event')
  await page.keyboard.press('Escape')
  await page.clock.runFor(30000)
  await page.getByRole('button', { name: 'Open experience' }).click()
  await expect(page.locator('.current')).toHaveText('Ready when you are.')
  await expect(page.locator('.output-line')).toHaveCount(0)
  await page.clock.runFor(30000)
  await expect(page.locator('.current')).toHaveText('Ready when you are.')
  await page.getByRole('button', { name: 'Close', exact: true }).click()
  await expect(page.getByRole('button', { name: 'Open experience' })).toBeFocused()
  await expect(page.locator('body')).not.toHaveCSS('overflow', 'hidden')
})

for (const [id, count, outcome] of [
  ['crash', 5, '↻ Recovered with duplicate publication'],
  ['race', 5, '✓ One live owner in this schedule'],
  ['ack', 4, '? Uncertain response · committed outcome observed'],
] as const) {
  test(`${id}: coherent states, output and replay`, async ({ page }) => {
    const calls: string[] = []
    await page.goto('/')
    page.on('request', (request) => {
      if (['fetch', 'xhr'].includes(request.resourceType())) calls.push(request.url())
    })
    await page.clock.install()
    await page.getByLabel('Choose an experience').selectOption(id)
    await page.getByRole('button', { name: 'Open experience' }).click()
    await page.getByRole('button', { name: 'Run scenario' }).click()
    await page.clock.runFor(count * 3600)
    await expect(page.locator('.result strong')).toHaveText(outcome)
    await expect(page.locator('.output-line')).toHaveCount(count)
    if (id === 'crash') {
      await expect(page.locator('.stage.interrupted')).toHaveCount(1)
      await expect(page.locator('.evidence')).toContainText('2 confirmed acceptances')
    }
    if (id === 'race') await expect(page.locator('.stage.skipped')).toHaveCount(1)
    if (id === 'ack') await expect(page.locator('.stage.uncertain')).toHaveCount(1)
    if (id === 'dedupe') {
      await expect(page.getByRole('dialog')).not.toContainText(/future|not implemented/i)
      await expect(page.locator('.envelope')).toContainText('quantity: 5')
      await expect(page.locator('.output-line')).toContainText(['SKU-1', '10 + 5 = 15', 'duplicate effect is skipped', 'one atomic local effect'])
      await expect(page.locator('.stage.preview')).toHaveCount(4)
      await expect(page.locator('.output-line').nth(2)).toContainText('[2 ms]')
    }
    await page.getByRole('button', { name: 'Run again' }).click()
    await expect(page.locator('.output-line')).toHaveCount(1)
    await page.clock.runFor(count * 3600)
    await expect(page.locator('.output-line')).toHaveCount(count)
    expect(calls).toEqual([])
  })
}

test('reduced motion keeps readable progression without animation', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' })
  await page.goto('/')
  await page.getByLabel('Choose an experience').selectOption('ack')
  await page.clock.install()
  await page.getByRole('button', { name: 'Open experience' }).click()
  await page.getByRole('button', { name: 'Run scenario' }).click()
  await page.clock.runFor(3600)
  await expect(page.locator('.message').first()).toHaveCSS('animation-name', 'none')
  await expect(page.locator('.pulse')).toHaveCSS('animation-name', 'none')
  await expect(page.locator('.current')).toHaveText('Broker accepts the event')
  await page.clock.runFor(14400)
  await expect(page.getByRole('button', { name: 'Run again' })).toBeEnabled()
})
