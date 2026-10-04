import { mkdir, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'

import { chromium, type Page, type Route } from 'playwright'

// Controlled acceptance fixture, not a compatibility or sandbox certification.
// Native CDP must belong to the isolated validation identifier, never a user app.
const webUrl = process.env.FLOWTOOLS_VALIDATION_WEB_URL
const nativeCdp = process.env.FLOWTOOLS_VALIDATION_DESKTOP_CDP
const requestedHosts = process.env.FLOWTOOLS_VALIDATION_HOSTS ?? 'both'
if (!['web', 'both'].includes(requestedHosts))
  throw new Error('Validation hosts must be web or both')
if (!webUrl || (requestedHosts === 'both' && !nativeCdp))
  throw new Error('URLs for all requested validation hosts are required')
for (const value of [webUrl, ...(nativeCdp ? [nativeCdp] : [])]) {
  const url = new URL(value)
  if (url.hostname !== '127.0.0.1' || url.protocol !== 'http:') {
    throw new Error('Validation endpoints must be loopback HTTP')
  }
}
const output = resolve(import.meta.dirname, '../../../execution-validation')
await mkdir(output, { recursive: true })
const web = await chromium.launch({ headless: true })
const desktop =
  requestedHosts === 'both' && nativeCdp
    ? await chromium.connectOverCDP(nativeCdp)
    : undefined
const receipts: Record<string, unknown>[] = []

async function waitForText(page: Page, selector: string, text: string) {
  await page.locator(selector).filter({ hasText: text }).first().waitFor()
}

async function verifyExecution(page: Page, host: string, historyKey: string) {
  await page.getByText('Prototype', { exact: true }).first().waitFor()
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
    maturity: 'prototype',
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

async function verifyTodo(page: Page, host: string) {
  const section = page.getByRole('region', { name: 'SDK execution' })
  await section.waitFor()
  await section
    .getByRole('textbox', { name: 'JSON input' })
    .fill('{"todo":"p0 shared-store fixture","deadline":"2026-10-04"}')
  await section.getByRole('button', { name: 'Run JSON', exact: true }).click()
  await waitForText(page, '[role="status"]', 'Success')
  await page.getByText('p0 shared-store fixture', { exact: true }).waitFor()
  await page.screenshot({
    path: resolve(output, host + '-todo.png'),
    fullPage: true,
  })
  receipts.push({ host, plugin: 'plugin-todo-list', sharedAppStore: true })
}

try {
  const webPage = await web.newPage({ viewport: { width: 1200, height: 900 } })
  await webPage.goto(webUrl)
  await webPage
    .getByRole('heading', { name: 'Dashboard', exact: true })
    .waitFor()
  if (
    (await webPage.getByText('Prototype', { exact: true }).count()) < 12 ||
    (await webPage.getByText('Stable', { exact: true }).count()) !== 0
  )
    throw new Error('Web dashboard maturity does not match actual built-ins')
  await webPage.screenshot({
    path: resolve(output, 'web-maturity-dashboard.png'),
    fullPage: true,
  })
  await webPage.goto(new URL('/plugins', webUrl).href)
  await webPage.getByText('Built-in Plugins (12)', { exact: true }).waitFor()
  if ((await webPage.getByText('Prototype', { exact: true }).count()) !== 12)
    throw new Error('Web plugin inventory maturity mismatch')
  receipts.push({
    host: 'web',
    routes: ['/', '/plugins'],
    actualBuiltInMaturity: 'prototype',
    builtInCount: 12,
  })
  await webPage.goto(new URL('/tools/plugin-base64-encoder', webUrl).href)
  await webPage.getByRole('tab', { name: 'Run', exact: true }).click()
  await verifyExecution(webPage, 'web', 'flowtools-web-run-history-v1')
  await webPage.goto(new URL('/tools/plugin-website-latency', webUrl).href)
  await webPage.getByRole('tab', { name: 'Run', exact: true }).click()
  await verifyCancel(webPage, 'web')
  await webPage.goto(new URL('/tools/plugin-todo-list', webUrl).href)
  await webPage.getByRole('tab', { name: 'Run', exact: true }).click()
  await verifyTodo(webPage, 'web')

  if (desktop) {
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
    if (new URL(native.url()).pathname !== '/')
      throw new Error('Native validation must initially load the launcher root')
    await native
      .getByPlaceholder('搜索应用、插件、命令或输入内容', { exact: true })
      .waitFor()
    await native.goto(new URL('/plugins', native.url()).href)
    await native.getByTestId('catalog-evidence-notice').waitFor()
    await native.getByText('indexed', { exact: true }).first().waitFor()
    await native.getByText('entry-resolved', { exact: true }).first().waitFor()
    await native.screenshot({
      path: resolve(output, 'desktop-maturity-catalog.png'),
      fullPage: true,
    })
    receipts.push({
      host: 'desktop',
      maturity: 'prototype',
      evidence: ['indexed', 'entry-resolved'],
      fileEvidenceOnly: true,
    })
    await openNativePlugin(native, 'Base64 编解码')
    await verifyExecution(native, 'desktop', 'flowtools-desktop-run-history-v1')
    await openNativePlugin(native, '网站延迟测试')
    await verifyCancel(native, 'desktop')
    await openNativePlugin(native, 'Todo List')
    await verifyTodo(native, 'desktop')
  }
  await writeFile(
    resolve(output, 'receipt.json'),
    JSON.stringify(
      { checkedAt: new Date().toISOString(), hosts: requestedHosts, receipts },
      null,
      2
    )
  )
  process.stdout.write(
    `Real execution validation passed for requested hosts: ${requestedHosts}\n`
  )
} finally {
  await web.close()
  // CDP close disconnects this test client; lifecycle is owned by the launcher.
  await desktop?.close()
}
