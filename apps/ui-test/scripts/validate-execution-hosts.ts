import { mkdir, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'

import { chromium, type Page, type Route } from 'playwright'

// Controlled acceptance fixture, not a compatibility or sandbox certification.
// Native CDP must belong to the isolated validation identifier, never a user app.
const webUrl = process.env.FLOWTOOLS_VALIDATION_WEB_URL
const nativeCdp = process.env.FLOWTOOLS_VALIDATION_DESKTOP_CDP
if (!webUrl || !nativeCdp)
  throw new Error('Both validation host URLs are required')
for (const value of [webUrl, nativeCdp]) {
  const url = new URL(value)
  if (url.hostname !== '127.0.0.1' || url.protocol !== 'http:') {
    throw new Error('Validation endpoints must be loopback HTTP')
  }
}
const output = resolve(import.meta.dirname, '../../../execution-validation')
await mkdir(output, { recursive: true })
const web = await chromium.launch({ headless: true })
const desktop = await chromium.connectOverCDP(nativeCdp)
const receipts: Record<string, unknown>[] = []

async function waitForText(page: Page, selector: string, text: string) {
  await page.locator(selector).filter({ hasText: text }).first().waitFor()
}

async function verifyExecution(page: Page, host: string, historyKey: string) {
  const section = page.getByRole('region', { name: 'SDK execution' })
  await section.waitFor()
  await section.scrollIntoViewIfNeeded()
  const input = section.getByRole('textbox', { name: 'JSON input' })
  await input.fill('{"text":"hello"}')
  await input.focus()
  await page.keyboard.press('Tab')
  if ((await page.locator(':focus').innerText()) !== 'Run JSON') {
    throw new Error(host + ': run control is not keyboard reachable')
  }
  await page.keyboard.press('Enter')
  await waitForText(page, '[data-testid="execution-output"]', 'aGVsbG8=')
  const success = JSON.parse(
    await page.getByTestId('execution-output').innerText()
  ) as {
    success: boolean
    pluginVersion: string
    durationMs: number
    data: { value: { result: string; mode: string } }
  }
  if (
    !success.success ||
    success.pluginVersion !== '0.1.0' ||
    success.data.value.result !== 'aGVsbG8=' ||
    success.data.value.mode !== 'encode'
  ) {
    throw new Error(host + ': real plugin result mismatch')
  }
  await page.screenshot({
    path: resolve(output, host + '-success.png'),
    fullPage: true,
  })
  await input.fill('{"text":3}')
  await section.getByRole('button', { name: 'Run JSON', exact: true }).click()
  await waitForText(page, '[role="status"]', 'INPUT_INVALID')
  await input.fill('{fixture-secret-input')
  await section.getByRole('button', { name: 'Run JSON', exact: true }).click()
  await waitForText(
    page,
    '[data-testid="execution-output"]',
    'Input must be valid JSON'
  )
  const stored = await page.evaluate(
    key => localStorage.getItem(key),
    historyKey
  )
  if (
    !stored ||
    stored.includes('fixture-secret-input') ||
    stored.includes('aGVsbG8=') ||
    stored.includes('"text"')
  ) {
    throw new Error(host + ': history missing or contains private data')
  }
  const history = JSON.parse(stored) as {
    formatVersion: number
    entries: { status: string }[]
  }
  if (
    history.formatVersion !== 1 ||
    history.entries.length !== 3 ||
    history.entries[0]?.status !== 'error'
  ) {
    throw new Error(host + ': history does not reflect actual attempts')
  }
  await page.reload()
  if (host === 'web')
    await page.getByRole('tab', { name: 'Run', exact: true }).click()
  await page.getByRole('region', { name: 'SDK execution' }).waitFor()
  await page
    .getByRole('heading', { name: 'Execution history (metadata only)' })
    .waitFor()
  const restored = await page.evaluate(
    key => localStorage.getItem(key),
    historyKey
  )
  if (restored !== stored) throw new Error(host + ': reload changed history')
  await page.getByRole('button', { name: 'Clear history' }).click()
  const cleared = await page.evaluate(
    key => localStorage.getItem(key),
    historyKey
  )
  if (JSON.parse(cleared ?? '{}').entries?.length !== 0)
    throw new Error(host + ': clear failed')
  receipts.push({
    host,
    success,
    schemaFailure: 'INPUT_INVALID',
    malformedJson: 'INPUT_INVALID',
    privateHistory: true,
    reload: true,
    clear: true,
  })
}

async function openNativePlugin(page: Page, name: string) {
  await page.goto(new URL('/plugins', page.url()).href)
  const row = page
    .locator('div')
    .filter({
      has: page.locator('strong').filter({ hasText: name }),
    })
    .last()
  await row.getByRole('button', { name: '安装', exact: true }).click()
  await row.getByRole('button', { name: '启用并启动', exact: true }).click()
}

async function verifyCancel(page: Page, host: string) {
  let pending: Route | undefined
  const url = 'https://flowtools-execution.fixture.invalid/'
  await page.route(url, route => {
    pending = route
  })
  const section = page.getByRole('region', { name: 'SDK execution' })
  await section.waitFor()
  await section
    .getByRole('textbox', { name: 'JSON input' })
    .fill(JSON.stringify({ urls: [url] }))
  const request = page.waitForRequest(url)
  await section.getByRole('button', { name: 'Run JSON', exact: true }).click()
  await request
  await section.getByRole('button', { name: 'Cancel', exact: true }).click()
  await waitForText(page, '[role="status"]', 'ABORTED')
  if (
    await section
      .getByRole('button', { name: 'Run JSON', exact: true })
      .isDisabled()
  ) {
    throw new Error(host + ': cancellation did not restore the controls')
  }
  if (pending) await pending.abort().catch(() => {})
  await page.unroute(url)
  await page.screenshot({
    path: resolve(output, host + '-cancel.png'),
    fullPage: true,
  })
  receipts.push({
    host,
    plugin: 'plugin-website-latency',
    cancelled: 'ABORTED',
    network: 'intercepted reserved fixture; no public request',
  })
}

try {
  const webPage = await web.newPage({ viewport: { width: 1200, height: 900 } })
  await webPage.goto(new URL('/tools/plugin-base64-encoder', webUrl).href)
  await webPage.getByRole('tab', { name: 'Run', exact: true }).click()
  await verifyExecution(webPage, 'web', 'flowtools-web-run-history-v1')
  await webPage.goto(new URL('/tools/plugin-website-latency', webUrl).href)
  await webPage.getByRole('tab', { name: 'Run', exact: true }).click()
  await verifyCancel(webPage, 'web')

  const native = desktop
    .contexts()[0]
    ?.pages()
    .find(
      page =>
        page.url().includes('tauri.localhost') ||
        page.url().startsWith('tauri://')
    )
  if (!native) throw new Error('The isolated Tauri WebView was not found')
  if (
    new URL(native.url()).searchParams.get('execution-validation') !==
    '20261003'
  ) {
    throw new Error('Refusing to control a non-validation desktop window')
  }
  await openNativePlugin(native, 'Base64 编解码')
  await verifyExecution(native, 'desktop', 'flowtools-desktop-run-history-v1')
  await openNativePlugin(native, '网站延迟测试')
  await verifyCancel(native, 'desktop')
  await writeFile(
    resolve(output, 'receipt.json'),
    JSON.stringify({ checkedAt: new Date().toISOString(), receipts }, null, 2)
  )
  process.stdout.write(
    'Real Web and isolated Tauri execution validation passed\n'
  )
} finally {
  await web.close()
  // CDP close disconnects this test client; lifecycle is owned by the launcher.
  await desktop.close()
}
