import assert from 'node:assert/strict'
import { spawn, execFileSync } from 'node:child_process'
import { randomBytes } from 'node:crypto'
import { once } from 'node:events'
import { mkdir, mkdtemp, readFile, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { resolve, join } from 'node:path'
import { createInterface } from 'node:readline'

import {
  PluginDataClient,
  RuntimeClient,
  type Request,
  type Response,
} from '@flowtools/runtime-client'
import { connectNamedPipe } from '@flowtools/runtime-client/node'
import { chromium } from 'playwright'

import { assertRuntimeValidationPreflight } from './runtime-validation-preflight.ts'

// Run with Node. Dedicated config/profile, child-only loopback CDP, no user DB.
const root = resolve(import.meta.dirname, '../../..')
const desktopExe = join(root, 'apps/desktop/src-tauri/target/debug/desktop.exe')
const config = JSON.parse(
  await readFile(
    join(root, 'apps/desktop/tauri.runtime-validation.conf.json'),
    'utf8'
  )
) as { identifier: string; app: { windows: { title: string }[] } }
assert.equal(config.identifier, 'com.flowtools.g2-validation-20261004')
// Old binaries do not support the safe probe and must never be executed.
assertRuntimeValidationPreflight(
  await readFile(desktopExe),
  [config.identifier, 'http://127.0.0.1:1420/', config.app.windows[0]!.title],
  () =>
    execFileSync(desktopExe, ['--runtime-validation-identity'], {
      windowsHide: true,
      encoding: 'utf8',
      maxBuffer: 4096,
      env: { SystemRoot: process.env.SystemRoot },
    })
)
const output = join(root, 'execution-validation/g2')
await mkdir(output, { recursive: true })
const runtimeProfile = await mkdtemp(join(tmpdir(), 'flowtools-validation-g2-'))
const webviewProfile = await mkdtemp(join(output, 'webview-'))
const cliToken = randomBytes(32).toString('hex')
const desktopToken = randomBytes(32).toString('hex')
const host = spawn(
  join(root, 'target/debug/flowtools-runtime.exe'),
  ['--validation-profile', runtimeProfile, '--validation', '--validation-data'],
  {
    windowsHide: true,
    stdio: ['pipe', 'pipe', 'pipe'],
    env: {
      SystemRoot: process.env.SystemRoot,
      FLOWTOOLS_RUNTIME_VALIDATION_CLI_TOKEN: cliToken,
      FLOWTOOLS_RUNTIME_VALIDATION_DESKTOP_TOKEN: desktopToken,
      FLOWTOOLS_RUNTIME_VALIDATION_STDIN_LIFETIME: '1',
    },
  }
)
const hostExit = once(host, 'exit')
const lines = createInterface({ input: host.stdout })
const [line] = (await once(lines, 'line')) as unknown[]
const ready = JSON.parse(String(line)) as { pipe: string }
const cli = new RuntimeClient(await connectNamedPipe(ready.pipe))
const desktop = spawn(desktopExe, [], {
  windowsHide: true,
  stdio: 'ignore',
  env: {
    SystemRoot: process.env.SystemRoot,
    APPDATA: process.env.APPDATA,
    LOCALAPPDATA: process.env.LOCALAPPDATA,
    USERPROFILE: process.env.USERPROFILE,
    TEMP: process.env.TEMP,
    TMP: process.env.TMP,
    FLOWTOOLS_RUNTIME_VALIDATION: '1',
    FLOWTOOLS_DESKTOP_VALIDATION_DATA_ROOT: join(
      runtimeProfile,
      'flowtools-validation-desktop'
    ),
    FLOWTOOLS_RUNTIME_VALIDATION_PIPE: ready.pipe,
    FLOWTOOLS_RUNTIME_VALIDATION_DESKTOP_TOKEN: desktopToken,
    WEBVIEW2_USER_DATA_FOLDER: webviewProfile,
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:
      '--remote-debugging-address=127.0.0.1 --remote-debugging-port=9224',
  },
})
const desktopExit = once(desktop, 'exit')
let browser: Awaited<ReturnType<typeof chromium.connectOverCDP>> | undefined
try {
  await cli.connect(cliToken)
  for (let attempt = 0; attempt < 100; attempt++) {
    try {
      await fetch('http://127.0.0.1:9224/json/version')
      break
    } catch {
      await new Promise(resolve => setTimeout(resolve, 250))
    }
  }
  const addresses = execFileSync(
    'pwsh',
    [
      '-NoProfile',
      '-Command',
      '(Get-NetTCPConnection -State Listen -LocalPort 9224).LocalAddress',
    ],
    { encoding: 'utf8', windowsHide: true }
  )
  assert.ok(
    addresses
      .trim()
      .split(/\s+/)
      .every(address => address === '127.0.0.1' || address === '::1')
  )
  browser = await chromium.connectOverCDP('http://127.0.0.1:9224')
  const page = browser.contexts()[0]!.pages()[0]!
  await page
    .locator('input[name="launcher-search"]')
    .waitFor({ timeout: 30000 })
  assert.equal(new URL(page.url()).pathname, '/')
  assert.equal(new URL(page.url()).hostname, '127.0.0.1')
  await page.goto(
    'http://127.0.0.1:1420/?execution-validation=20261003&runtime-validation=1'
  )
  await page
    .getByTestId('runtime-job-status')
    .filter({ hasText: 'Connected:' })
    .waitFor()
  const receipt = await cli.submit({
    pluginId: 'plugin-base64-encoder',
    commandId: 'run',
    input: { text: 'hello' },
    idempotencyKey: 'g2-native-ui',
    background: true,
    deadline: Date.now() + 10000,
  })
  const job = await cli.waitForResult(receipt.runId)
  assert.equal(job.state, 'succeeded')
  await page
    .getByRole('textbox', { name: 'Run ID', exact: true })
    .fill(receipt.runId)
  await page.getByRole('button', { name: 'Inspect Runtime job' }).focus()
  await page.keyboard.press('Enter')
  await page
    .getByTestId('runtime-job-status')
    .filter({ hasText: `${receipt.runId} | succeeded | validation-cli` })
    .waitFor()
  await page
    .getByRole('region', { name: 'Runtime validation' })
    .scrollIntoViewIfNeeded()
  await page.screenshot({
    path: join(output, 'desktop-runtime-job.png'),
    fullPage: true,
  })
  await page
    .getByRole('textbox', { name: 'Run ID', exact: true })
    .fill('missing-fixture-run')
  await page.getByRole('button', { name: 'Inspect Runtime job' }).click()
  await page
    .getByTestId('runtime-job-status')
    .filter({ hasText: 'JOB_NOT_FOUND:' })
    .waitFor()
  assert.match(
    await page.getByTestId('runtime-job-status').innerText(),
    /不自动重新提交/
  )
  // Actual WebView -> native-bound Desktop session -> same SQLite writer.
  const data = new PluginDataClient(cli, 'plugin-todo-list')
  await data.write({
    key: 'todos',
    expectedRevision: 0,
    value: [{ todo: 'native shared fixture', deadline: '' }],
  })
  const nativeData = await page.evaluate(async () => {
    const invoke = (
      window as unknown as {
        __TAURI_INTERNALS__: {
          invoke<T>(this: void, command: string, args?: unknown): Promise<T>
        }
      }
    ).__TAURI_INTERNALS__.invoke
    await invoke('validation_runtime_disconnect')
    const exchange = (request: Request) =>
      invoke<Response>('validation_runtime', { request })
    const opened = await exchange({
      version: 1,
      requestId: 'native-data-open',
      session: null,
      call: {
        method: 'session.open',
        payload: {
          token: '',
          clientVersion: '0.2.0',
          expectedInstanceId: null,
        },
      },
    })
    if (opened.outcome.type !== 'session') throw new Error('Native handshake')
    const session = opened.outcome.data
    const read = await exchange({
      version: 1,
      requestId: 'native-data-read',
      session,
      call: {
        method: 'data.read',
        payload: { pluginId: 'plugin-todo-list', key: 'todos' },
      },
    })
    const write = await exchange({
      version: 1,
      requestId: 'native-data-write',
      session,
      call: {
        method: 'data.write',
        payload: {
          pluginId: 'plugin-todo-list',
          mutation: {
            key: 'todos',
            expectedRevision: 1,
            value: [{ todo: 'native edit fixture', deadline: '' }],
          },
        },
      },
    })
    return { read, write }
  })
  assert.equal(nativeData.read.outcome.type, 'data')
  assert.equal(nativeData.write.outcome.type, 'data')
  const sharedData = await data.read('todos')
  assert.equal(sharedData.revision, 2)
  assert.deepEqual(sharedData.value, [
    { todo: 'native edit fixture', deadline: '' },
  ])
  // Only the fixture Host is stopped; reload exercises a real handshake failure.
  host.stdin.end()
  const [hostExitCode] = (await hostExit) as unknown[]
  assert.equal(hostExitCode, 0)
  await page.reload()
  await page
    .getByTestId('runtime-job-status')
    .filter({ hasText: 'RUNTIME_DISCONNECTED:' })
    .waitFor()
  assert.match(
    await page.getByTestId('runtime-job-status').innerText(),
    /重连同一实例并查询 runId/
  )
  await writeFile(
    join(output, 'native-receipt.json'),
    JSON.stringify(
      {
        initialLauncher: true,
        keyboard: true,
        sharedRunId: receipt.runId,
        state: job.state,
        nativeSharedDataRevision: sharedData.revision,
        connectionDiagnostic: 'RUNTIME_DISCONNECTED',
        identity: config.identifier,
        scope: 'prototype validation only',
      },
      null,
      2
    )
  )
  process.stdout.write('Native GUI/CLI shared Runtime acceptance passed\n')
} catch (error) {
  const page = browser?.contexts()[0]?.pages()[0]
  if (page) {
    await page.screenshot({
      path: join(output, 'desktop-runtime-failure.png'),
      fullPage: true,
    })
    process.stderr.write(
      (await page.getByTestId('runtime-job-status').innerText()) + '\n'
    )
  }
  throw error
} finally {
  await browser?.close()
  desktop.kill()
  await desktopExit
  cli.close()
  lines.close()
  if (!host.stdin.writableEnded) host.stdin.end()
  await hostExit
}
