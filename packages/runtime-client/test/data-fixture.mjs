import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import { randomBytes } from 'node:crypto'
import { once } from 'node:events'
import { mkdtemp, readFile, readdir } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { resolve, join } from 'node:path'
import { createInterface } from 'node:readline'

import { inspectLegacyTodos } from '../../sdk/dist/data.js'
import { RuntimeClient, PluginDataClient } from '../dist/index.js'
import { connectNamedPipe } from '../dist/node.js'

const root = resolve(import.meta.dirname, '../../..')
const executable = resolve(
  process.env.CARGO_TARGET_DIR ?? join(root, 'target'),
  'debug/flowtools-runtime.exe'
)
const profile = await mkdtemp(join(tmpdir(), 'flowtools-validation-data-'))
const cliToken = randomBytes(32).toString('hex')
const desktopToken = randomBytes(32).toString('hex')

async function start(data) {
  const child = spawn(
    executable,
    [
      '--validation-profile',
      profile,
      '--validation',
      ...(data ? ['--validation-data'] : []),
    ],
    {
      windowsHide: true,
      env: {
        SystemRoot: process.env.SystemRoot,
        FLOWTOOLS_RUNTIME_VALIDATION_CLI_TOKEN: cliToken,
        FLOWTOOLS_RUNTIME_VALIDATION_DESKTOP_TOKEN: desktopToken,
        FLOWTOOLS_RUNTIME_VALIDATION_STDIN_LIFETIME: '1',
      },
      stdio: ['pipe', 'pipe', 'pipe'],
    }
  )
  const exit = once(child, 'exit')
  const lines = createInterface({ input: child.stdout })
  /** @type {{ pipe: string }} */
  const ready = await Promise.race([
    once(lines, 'line').then(([line]) => {
      const parsed = /** @type {unknown} */ (JSON.parse(String(line)))
      if (
        !parsed ||
        typeof parsed !== 'object' ||
        !('pipe' in parsed) ||
        typeof parsed.pipe !== 'string'
      )
        throw new Error('Invalid startup receipt')
      return { pipe: parsed.pipe }
    }),
    exit.then(() => {
      throw new Error('Host refused startup')
    }),
  ])
  const cli = new RuntimeClient(await connectNamedPipe(ready.pipe))
  const desktop = new RuntimeClient(await connectNamedPipe(ready.pipe))
  await cli.connect(cliToken)
  await desktop.connect(desktopToken)
  return {
    cli,
    desktop,
    async close() {
      cli.close()
      desktop.close()
      lines.close()
      child.stdin.end()
      assert.equal((await exit)[0], 0)
    },
  }
}

let host = await start(false)
try {
  await assert.rejects(
    new PluginDataClient(host.cli, 'plugin-todo-list').read('todos'),
    { code: 'APPROVAL_REQUIRED' }
  )
} finally {
  await host.close()
}
host = await start(true)
const raw = '[{"todo":"private-original","deadline":""}]'
try {
  const cliData = new PluginDataClient(host.cli, 'plugin-todo-list')
  const guiData = new PluginDataClient(host.desktop, 'plugin-todo-list')
  const inspected = await inspectLegacyTodos(raw, 'cli-v0')
  const { source, sourceDigest, value } = inspected
  const imported = await cliData.importLegacy({ source, sourceDigest, value })
  assert.equal(imported.revision, 1)
  assert.deepEqual(await guiData.read('todos'), imported)
  assert.equal(raw, '[{"todo":"private-original","deadline":""}]')
  assert.equal(
    (await guiData.importLegacy({ source, sourceDigest, value })).revision,
    1
  )
  const update = await guiData.write({
    key: 'todos',
    expectedRevision: 1,
    value: [...value, { todo: 'from-gui', deadline: '' }],
  })
  assert.equal(update.revision, 2)
  await assert.rejects(
    cliData.write({ key: 'todos', expectedRevision: 1, value: [] }),
    { code: 'REVISION_CONFLICT' }
  )
  assert.deepEqual(await cliData.read('todos'), update)
  await assert.rejects(
    cliData.transaction([
      { key: 'todos-other', expectedRevision: 0, value: 'rollback' },
      { key: 'todos', expectedRevision: 0, value: [] },
    ]),
    { code: 'REVISION_CONFLICT' }
  )
  assert.equal((await cliData.read('todos-other')).revision, 0)
  await assert.rejects(
    cliData.write({ key: 'host-metadata', expectedRevision: 0, value: [] }),
    { code: 'SCOPE_DENIED' }
  )
  await assert.rejects(
    new PluginDataClient(host.cli, 'plugin-base64-encoder').read('todos'),
    { code: 'CAPABILITY_UNDECLARED' }
  )
  await assert.rejects(
    cliData.importLegacy({ source, sourceDigest: 'spoof', value }),
    { code: 'INPUT_INVALID' }
  )
} finally {
  await host.close()
}
host = await start(true)
try {
  assert.equal(
    (await new PluginDataClient(host.cli, 'plugin-todo-list').read('todos'))
      .revision,
    2
  )
} finally {
  await host.close()
}
const files = await readdir(profile)
assert.ok(files.includes('runtime.sqlite'))
assert.equal(
  await readFile(join(profile, 'validation-profile-v1'), 'utf8'),
  'flowtools-disposable-v1'
)
process.stdout.write('Shared data fixture passed\n')
