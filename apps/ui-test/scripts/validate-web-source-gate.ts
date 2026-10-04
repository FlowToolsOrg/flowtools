/// <reference lib="dom" />

import assert from 'node:assert/strict'
import { mkdir, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'

import { chromium, type Page } from 'playwright'

// Disposable Chromium contexts only. No desktop control or real user profiles.
const endpoints = [
  ['production', process.env.FLOWTOOLS_GATE_PRODUCTION_URL, false],
  ['development-default', process.env.FLOWTOOLS_GATE_DEVELOPMENT_URL, false],
  ['development-opt-in', process.env.FLOWTOOLS_GATE_PREVIEW_URL, true],
] as const
for (const [, address] of endpoints) {
  assert(address, 'All three Web gate endpoints are required')
  const url = new URL(address)
  assert(
    url.protocol === 'http:' && url.hostname === '127.0.0.1',
    'Loopback HTTP only'
  )
}
const output = resolve(
  import.meta.dirname,
  '../../../execution-validation/p0b1-web'
)
await mkdir(output, { recursive: true })
const browser = await chromium.launch({ headless: true })
const receipts: Record<string, unknown>[] = []
const oldSource =
  'globalThis.__FLOWTOOLS_OLD_SOURCE_EXECUTED__ = true; throw new Error("must not execute");'

async function seedOldSource(page: Page) {
  await page.evaluate(async source => {
    localStorage.setItem(
      'flowtools-unverified-grants',
      JSON.stringify({ certified: true, development: true })
    )
    await new Promise<void>((done, reject) => {
      const open = indexedDB.open('flowtools-plugins', 1)
      open.onupgradeneeded = () =>
        open.result.createObjectStore('external-plugins', { keyPath: 'id' })
      open.onerror = () => reject(open.error)
      open.onsuccess = () => {
        const db = open.result
        const tx = db.transaction('external-plugins', 'readwrite')
        tx.objectStore('external-plugins').put({
          id: 'untrusted-persisted-fixture',
          name: 'Untrusted old source',
          fileName: 'old-source.tsx',
          type: 'application/javascript',
          data: new TextEncoder().encode(source).buffer,
          certified: true,
          maturity: 'production',
        })
        tx.oncomplete = () => {
          db.close()
          done()
        }
        tx.onerror = () => {
          db.close()
          reject(tx.error)
        }
      }
    })
  }, oldSource)
}

async function assertOldSourcePreserved(page: Page) {
  const record = await page.evaluate(async () => {
    const executed =
      Reflect.get(globalThis, '__FLOWTOOLS_OLD_SOURCE_EXECUTED__') === true
    const source = await new Promise<string>((done, reject) => {
      const open = indexedDB.open('flowtools-plugins', 1)
      open.onerror = () => reject(open.error)
      open.onsuccess = () => {
        const db = open.result
        const request = db
          .transaction('external-plugins', 'readonly')
          .objectStore('external-plugins')
          .get('untrusted-persisted-fixture')
        request.onsuccess = () => {
          const record = request.result as { data: ArrayBuffer } | undefined
          db.close()
          done(record ? new TextDecoder().decode(record.data) : '')
        }
        request.onerror = () => {
          db.close()
          reject(request.error)
        }
      }
    })
    return { executed, source }
  })
  assert.equal(record.executed, false, 'Old source was executed')
  assert.equal(record.source, oldSource, 'Old source was deleted or changed')
}

try {
  for (const [mode, address, enabled] of endpoints) {
    const context = await browser.newContext({
      viewport: { width: 1200, height: 900 },
    })
    try {
      const network: string[] = []
      await context.route('**/*', route => {
        const url = new URL(route.request().url())
        if (url.hostname !== '127.0.0.1') {
          network.push(url.hostname)
          return route.abort()
        }
        return route.continue()
      })
      const page = await context.newPage()
      const errors: string[] = []
      page.on('pageerror', error => errors.push(error.message))
      await page.goto(
        `${address}/plugins?certified=true&VITE_ENABLE_UNSAFE_PLUGIN_PREVIEW=1`,
        { waitUntil: 'networkidle' }
      )
      await page
        .getByRole('heading', { name: 'Built-in Plugins (12)' })
        .waitFor()
      await seedOldSource(page)
      await page.reload({ waitUntil: 'networkidle' })
      await page
        .getByRole('heading', { name: 'Built-in Plugins (12)' })
        .waitFor()
      await assertOldSourcePreserved(page)
      assert.equal(
        await page.locator('script[type=importmap]').count(),
        enabled ? 1 : 0
      )
      assert.equal(
        await page.locator('input[type=file]').count(),
        enabled ? 1 : 0
      )

      if (enabled) {
        await page
          .getByText('UNSAFE DEVELOPMENT PREVIEW', { exact: false })
          .waitFor()
        const button = page.getByRole('button', {
          name: 'Preview unsigned plugin',
        })
        for (let index = 0; index < 40; index += 1) {
          if (
            await button.evaluate(element => element === document.activeElement)
          )
            break
          await page.keyboard.press('Tab')
        }
        assert.equal(
          await button.evaluate(element => element === document.activeElement),
          true,
          'Preview button is not Tab-reachable'
        )
        const [chooser] = await Promise.all([
          page.waitForEvent('filechooser'),
          page.keyboard.press('Enter'),
        ])
        await chooser.setFiles({
          name: 'unsigned-fixture.ts',
          mimeType: 'application/typescript',
          buffer: Buffer.from(`
            const title: string = 'Unsigned fixture';
            globalThis.__FLOWTOOLS_PREVIEW_EXECUTED__ = true;
            export default {
              type: 'tool',
              meta: { id: 'unsigned-fixture', name: title, version: '0.1.0' },
              run() { return { type: 'text', value: 'actual unsigned fixture' }; }
            };
          `),
        })
        await page
          .getByText('Plugin "Unsigned fixture" loaded successfully.', {
            exact: false,
          })
          .waitFor()
        assert.equal(
          await page.evaluate(() =>
            Reflect.get(globalThis, '__FLOWTOOLS_PREVIEW_EXECUTED__')
          ),
          true
        )
        await page.screenshot({
          path: resolve(output, `${mode}.png`),
          fullPage: true,
        })
        await page.reload({ waitUntil: 'networkidle' })
        assert.equal(
          await page.evaluate(
            () =>
              Reflect.get(globalThis, '__FLOWTOOLS_PREVIEW_EXECUTED__') === true
          ),
          false
        )
        assert.equal(
          await page
            .getByRole('heading', { name: 'External Plugins (1)' })
            .count(),
          0
        )
        await assertOldSourcePreserved(page)
      } else {
        await page
          .getByRole('status')
          .filter({ hasText: 'External code execution is disabled' })
          .waitFor()
        assert.equal(
          await page
            .getByRole('button', { name: 'Preview unsigned plugin' })
            .count(),
          0
        )
        assert.equal(
          await page.evaluate(() => Reflect.has(globalThis, '__FLOWTOOLS__')),
          false
        )
        await page.screenshot({
          path: resolve(output, `${mode}.png`),
          fullPage: true,
        })
      }
      assert.deepEqual(network, [], 'Fixture attempted non-loopback networking')
      assert.deepEqual(errors, [], 'Unexpected page exception')
      receipts.push({
        mode,
        enabled,
        oldSourcePreserved: true,
        oldSourceExecuted: false,
        automaticRestore: false,
        keyboardPreview: enabled,
      })
    } finally {
      await context.close()
    }
  }
  await writeFile(
    resolve(output, 'receipt.json'),
    JSON.stringify({ date: new Date().toISOString(), receipts }, null, 2)
  )
  process.stdout.write(`${JSON.stringify(receipts, null, 2)}\n`)
} finally {
  await browser.close()
}
