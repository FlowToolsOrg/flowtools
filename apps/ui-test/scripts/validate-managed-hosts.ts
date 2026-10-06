import type { CreatePluginDto } from '../../desktop/src/utils/bindings'
import type {
  Call,
  JobSnapshot,
  Request,
  Response,
} from '@flowtools/runtime-client'

import assert from 'node:assert/strict'
import { spawn, execFileSync, execFile } from 'node:child_process'
import { createHash } from 'node:crypto'
import { once } from 'node:events'
import { mkdir, mkdtemp, readFile, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { promisify } from 'node:util'

import { chromium } from 'playwright'

import { assertRuntimeValidationPreflight } from './runtime-validation-preflight.ts'

// Run with Node. CLI policy is an explicit disposable fixture, never GUI consent.
const root = resolve(import.meta.dirname, '../../..')
const executable = join(root, 'apps/desktop/src-tauri/target/debug/desktop.exe')
const identity = 'com.flowtools.g2-validation-20261004'
assertRuntimeValidationPreflight(
  await readFile(executable),
  [identity, 'http://127.0.0.1:1420/', 'FlowTools G2 Runtime validation'],
  () =>
    execFileSync(executable, ['--runtime-validation-identity'], {
      windowsHide: true,
      encoding: 'utf8',
      maxBuffer: 4096,
      env: { SystemRoot: process.env.SystemRoot },
    })
)
const scratch = await mkdtemp(
  join(tmpdir(), 'flowtools-validation-managed-ui-')
)
const profile = join(scratch, 'flowtools-validation-profile')
const output = join(root, 'execution-validation/g3')
await mkdir(output, { recursive: true })
for (const name of ['appdata', 'localappdata', 'webview'])
  await mkdir(join(scratch, name))
function canonical(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(canonical)
  if (value && typeof value === 'object')
    return Object.fromEntries(
      Object.entries(value)
        .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
        .map(([key, item]) => [key, canonical(item)])
    )
  return value
}
const catalog = JSON.parse(
  await readFile(
    join(root, 'packages/runtime-core/.generated/catalog.json'),
    'utf8'
  )
) as { plugins: { id: string }[] }
const policy = join(scratch, 'policy.json')
await writeFile(
  policy,
  JSON.stringify({
    formatVersion: 1,
    coldStart: true,
    grants: ['plugin-todo-list', 'plugin-base64-encoder'].flatMap(pluginId =>
      ['cli', 'desktop'].map(target => ({
        pluginId,
        commandId: 'run',
        target,
        packageDigest: createHash('sha256')
          .update(
            JSON.stringify(
              canonical(catalog.plugins.find(item => item.id === pluginId))
            )
          )
          .digest('hex'),
        effects:
          pluginId === 'plugin-todo-list' ? ['data-read', 'data-write'] : [],
        scopes:
          pluginId === 'plugin-todo-list'
            ? [{ kind: 'plugin-data', scope: { key_prefix: 'todos' } }]
            : [],
        expiresAt: Date.now() + 3600000,
        maxCalls: 128,
        coldStart: true,
        background: true,
      }))
    ),
  })
)
async function cli(args: string[]) {
  const result = await promisify(execFile)(
    process.execPath,
    [join(root, 'packages/cli/dist/cli.mjs'), ...args, '--profile', profile],
    {
      windowsHide: true,
      encoding: 'utf8',
      maxBuffer: 1048576,
      timeout: 45000,
      env: { SystemRoot: process.env.SystemRoot },
    }
  )
  assert.equal(result.stderr, '')
  return JSON.parse(
    args[0] === 'jobs' && args[1] === 'watch'
      ? result.stdout.trim().split(/\r?\n/).at(-1)!
      : result.stdout
  ) as {
    success: boolean
    data: unknown
  }
}
await cli(['init', '--policy', policy])
const started = await cli(['runtime', 'start'])
const desktop = spawn(executable, [], {
  windowsHide: true,
  stdio: 'ignore',
  env: {
    SystemRoot: process.env.SystemRoot,
    APPDATA: join(scratch, 'appdata'),
    LOCALAPPDATA: join(scratch, 'localappdata'),
    USERPROFILE: process.env.USERPROFILE,
    TEMP: process.env.TEMP,
    TMP: process.env.TMP,
    FLOWTOOLS_RUNTIME_VALIDATION: '1',
    FLOWTOOLS_MANAGED_VALIDATION_PROFILE: profile,
    FLOWTOOLS_DESKTOP_VALIDATION_DATA_ROOT: join(
      profile,
      'flowtools-validation-desktop'
    ),
    WEBVIEW2_USER_DATA_FOLDER: join(scratch, 'webview'),
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:
      '--remote-debugging-address=127.0.0.1 --remote-debugging-port=9224',
  },
})
const desktopExit = once(desktop, 'exit')
let browser: Awaited<ReturnType<typeof chromium.connectOverCDP>> | undefined
let failed = false
try {
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
    { windowsHide: true, encoding: 'utf8' }
  )
  assert.ok(
    addresses
      .trim()
      .split(/\s+/)
      .every(address => address === '127.0.0.1' || address === '::1')
  )
  browser = await chromium.connectOverCDP('http://127.0.0.1:9224')
  const page = browser.contexts()[0]!.pages()[0]!
  const nativeData = (call: Call) =>
    page.evaluate(async call => {
      const invoke = (
        window as unknown as {
          __TAURI_INTERNALS__: {
            invoke<T>(this: void, command: string, args?: unknown): Promise<T>
          }
        }
      ).__TAURI_INTERNALS__.invoke
      const exchange = (request: Request) =>
        invoke<Response>('managed_runtime', { request })
      const opened = await exchange({
        version: 1,
        requestId: crypto.randomUUID(),
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
      try {
        return (
          await exchange({
            version: 1,
            requestId: crypto.randomUUID(),
            session: opened.outcome.data,
            call,
          })
        ).outcome
      } finally {
        await invoke('managed_runtime_disconnect', {
          sessionId: opened.outcome.data.sessionId,
        })
      }
    }, call)
  await page
    .locator('input[name="launcher-search"]')
    .waitFor({ timeout: 30000 })
  assert.equal(new URL(page.url()).pathname, '/')
  assert.equal(new URL(page.url()).hostname, '127.0.0.1')
  // Fixture activation is metadata only; grants were independently installed by T0 CLI.
  const plugin: CreatePluginDto = {
    id: 'plugin-todo-list',
    name: 'Todo List',
    version: '0.1.0',
    description: null,
    author: null,
    link: null,
    type: 'app',
    permissions: [],
    tags: [],
    status: 'installed',
    category: null,
    icon: null,
    cliAvailable: true,
    state: 'enabled',
  }
  await page.evaluate(async plugin => {
    const invoke = (
      window as unknown as {
        __TAURI_INTERNALS__: {
          invoke<T>(this: void, command: string, args?: unknown): Promise<T>
        }
      }
    ).__TAURI_INTERNALS__.invoke
    await invoke('add_plugin', { plugin })
  }, plugin)
  await page.goto('http://127.0.0.1:1420/run/plugin-todo-list%3Aopen')
  await page
    .getByTestId('shared-todo-revision')
    .filter({ hasText: 'Runtime revision 0' })
    .waitFor()
  await page
    .getByRole('region', { name: '共享 Todo 数据' })
    .getByRole('textbox', { name: /^Todo/ })
    .fill('GUI shared fixture')
  await page.getByRole('button', { name: 'Add', exact: true }).focus()
  await page.keyboard.press('Enter')
  await page
    .getByTestId('shared-todo-revision')
    .filter({ hasText: 'Runtime revision 1' })
    .waitFor()
  const first = await cli(['run', 'plugin-todo-list', '--input', '{}'])
  assert.equal(first.success, true)
  assert.deepEqual(
    (first.data as { value: { result: string[]; count: number } }).value,
    { result: ['1. [No Deadline] GUI shared fixture'], count: 1 }
  )
  const seeded = await nativeData({
    method: 'data.write',
    payload: {
      pluginId: 'plugin-todo-list',
      mutation: {
        key: 'todos',
        expectedRevision: 1,
        value: [
          {
            todo: 'GUI shared fixture',
            deadline: '',
            fixtureExtension: 'preserve-me',
          },
        ],
      },
    },
  })
  assert.equal(seeded.type, 'data')
  await page
    .getByTestId('shared-todo-revision')
    .filter({ hasText: 'Runtime revision 2' })
    .waitFor()
  await page
    .getByRole('region', { name: '共享 Todo 数据' })
    .getByRole('textbox', { name: /^Todo/ })
    .fill('GUI retained fixture')
  await page.getByRole('button', { name: 'Add', exact: true }).click()
  await page
    .getByTestId('shared-todo-revision')
    .filter({ hasText: 'Runtime revision 3' })
    .waitFor()
  const preserved = await nativeData({
    method: 'data.read',
    payload: { pluginId: 'plugin-todo-list', key: 'todos' },
  })
  assert.equal(preserved.type, 'data')
  if (preserved.type === 'data')
    assert.equal(
      (preserved.data.value as { fixtureExtension?: string }[])[0]!
        .fixtureExtension,
      'preserve-me'
    )
  await cli([
    'run',
    'plugin-todo-list',
    '--input',
    '{"todo":"CLI shared fixture"}',
  ])
  await page
    .getByRole('list', { name: '共享 Todo Items' })
    .getByText('CLI shared fixture', { exact: true })
    .waitFor()
  await page
    .getByTestId('shared-todo-revision')
    .filter({ hasText: 'Runtime revision 4' })
    .waitFor()
  await page.screenshot({
    path: join(output, 'desktop-shared-todo.png'),
    fullPage: true,
  })
  await page.goto('http://127.0.0.1:1420/settings')
  assert.equal(
    await page.locator('option[value="plugin-website-latency"]').count(),
    0
  )
  await page.getByRole('button', { name: '启动 / 刷新 Runtime' }).focus()
  await page.keyboard.press('Enter')
  await page
    .getByTestId('managed-runtime-status')
    .filter({ hasText: '已连接共享 Runtime' })
    .waitFor()
  assert.ok(
    (await page.getByTestId('managed-runtime-instance').innerText()).includes(
      (started.data as { instanceId: string }).instanceId
    )
  )
  const cliJobs = (await cli(['jobs', 'list'])).data as JobSnapshot[]
  for (const job of cliJobs)
    await page
      .getByTestId('managed-job')
      .filter({ hasText: job.runId })
      .waitFor()
  await page
    .getByRole('checkbox', { name: '批准后台执行，并以后台任务提交' })
    .check()
  await page
    .getByRole('textbox', { name: '任务输入 JSON' })
    .fill('{"todo":"GUI background fixture"}')
  await page.getByRole('button', { name: '提交任务', exact: true }).click()
  await page
    .getByTestId('managed-job')
    .filter({ hasText: 'local-desktop' })
    .waitFor()
  const background = ((await cli(['jobs', 'list'])).data as JobSnapshot[]).find(
    job => job.rootCaller === 'local-desktop'
  )!
  assert.ok(background)
  await cli(['jobs', 'watch', background.runId])
  await page.getByRole('button', { name: '启动 / 刷新 Runtime' }).click()
  await page
    .getByTestId('managed-job')
    .filter({ hasText: background.runId })
    .filter({ hasText: 'succeeded' })
    .waitFor()
  await page.waitForFunction(
    () =>
      !document.querySelector('[aria-label="共享 Runtime"] button[disabled]')
  )
  await page
    .getByTestId('managed-job')
    .filter({ hasText: background.runId })
    .scrollIntoViewIfNeeded()
  await page.screenshot({
    path: join(output, 'desktop-shared-jobs.png'),
    fullPage: true,
  })
  // Business transport cannot obtain manager rights, even in the trusted main WebView.
  const refused = await page.evaluate(async () => {
    const invoke = (
      window as unknown as {
        __TAURI_INTERNALS__: {
          invoke<T>(this: void, command: string, args?: unknown): Promise<T>
        }
      }
    ).__TAURI_INTERNALS__.invoke
    try {
      await invoke('managed_runtime', {
        request: {
          version: 1,
          requestId: 'manager-refusal',
          session: null,
          call: { method: 'policy.set', payload: { coldStart: true } },
        },
      })
      return 'unexpected'
    } catch (error) {
      return (error as { code: string }).code
    }
  })
  assert.equal(refused, 'SESSION_INVALID')
  // Real Desktop-bound submit then GUI process exit; Host owns the explicit background work.
  const receipt = await page.evaluate(async () => {
    const invoke = (
      window as unknown as {
        __TAURI_INTERNALS__: {
          invoke<T>(this: void, command: string, args?: unknown): Promise<T>
        }
      }
    ).__TAURI_INTERNALS__.invoke
    const exchange = (request: Request) =>
      invoke<Response>('managed_runtime', { request })
    const opened = await exchange({
      version: 1,
      requestId: 'background-open',
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
    const result = await exchange({
      version: 1,
      requestId: 'background-submit',
      session: opened.outcome.data,
      call: {
        method: 'jobs.submit',
        payload: {
          pluginId: 'plugin-todo-list',
          commandId: 'run',
          input: { todo: 'after GUI exit fixture' },
          idempotencyKey: 'desktop-exit-background',
          background: true,
          deadline: Date.now() + 30000,
        },
      },
    })
    if (result.outcome.type !== 'receipt') throw new Error('Native receipt')
    return result.outcome.data
  })
  desktop.kill()
  await desktopExit
  // watch emits several JSON lines; use a bounded status poll for this cross-client check.
  let terminal: JobSnapshot | undefined
  for (let attempt = 0; attempt < 100; attempt++) {
    terminal = (await cli(['jobs', 'status', receipt.runId]))
      .data as JobSnapshot
    if (terminal.result) break
    await new Promise(resolve => setTimeout(resolve, 25))
  }
  assert.equal(terminal?.state, 'succeeded')
  const listing = (await cli(['jobs', 'list'])).data as JobSnapshot[]
  assert.ok(listing.every(job => job.result === null))
  assert.ok(!JSON.stringify(listing).includes('shared fixture'))
  await writeFile(
    join(output, 'managed-native-receipt.json'),
    JSON.stringify(
      {
        identity,
        initialLauncher: true,
        keyboard: true,
        sharedRevision: 4,
        unknownDataFieldsPreserved: true,
        sharedRunIds: listing.map(job => job.runId),
        backgroundAfterGuiExit: receipt.runId,
        state: terminal?.state,
        businessManagerRefusal: refused,
        nativeConsentClick: 'pending - Computer Use unavailable',
        scope: 'Windows T1 prototype',
      },
      null,
      2
    )
  )
  process.stdout.write('Native managed GUI/CLI shared acceptance passed\n')
} catch (error) {
  failed = true
  const page = browser?.contexts()[0]?.pages()[0]
  if (page && !page.isClosed()) {
    await writeFile(
      join(output, 'desktop-managed-accessibility.txt'),
      await page.locator('body').ariaSnapshot()
    )
    await page.screenshot({
      path: join(output, 'desktop-managed-failure.png'),
      fullPage: true,
    })
    process.stderr.write(
      (await page.locator('body').innerText()).slice(0, 4096) + '\n'
    )
  }
  throw error
} finally {
  await browser?.close()
  desktop.kill()
  await desktopExit
  await cli(['runtime', 'stop']).catch((error: unknown) => {
    if (!failed) throw error
    process.stderr.write('Fixture cleanup: Runtime already disconnected\n')
  })
}
