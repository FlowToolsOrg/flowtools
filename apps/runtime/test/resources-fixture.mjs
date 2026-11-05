import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import { once } from 'node:events'
import {
  mkdtemp,
  mkdir,
  readFile,
  writeFile,
  rm,
  realpath,
} from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve, basename, dirname } from 'node:path'
import { createInterface } from 'node:readline'

import { RuntimeClient, RuntimeClientError } from '@flowtools/runtime-client'
import { connectNamedPipe } from '@flowtools/runtime-client/node'
const root = resolve(import.meta.dirname, '../../..'),
  scratch = await mkdtemp(join(tmpdir(), 'flowtools-validation-na1-')),
  profile = join(scratch, 'profile'),
  files = join(scratch, 'files')
const runtime = join(
    process.env.CARGO_TARGET_DIR ?? join(root, 'target'),
    'debug/flowtools-runtime.exe'
  ),
  cli = join(root, 'packages/cli/dist/cli.mjs')
const bun = /** @type {{bun:{path:string}}} */ (
  JSON.parse(
    await readFile(join(root, 'apps/runtime/.generated/runner.json'), 'utf8')
  )
).bun.path
/** @type {import('node:child_process').ChildProcessWithoutNullStreams|undefined} */ let child
/** @type {Promise<unknown>|undefined} */ let exited
/** @type {RuntimeClient|undefined} */ let manager
/** @type {RuntimeClient|undefined} */ let desktop
async function start(/** @type {boolean} */ initialize) {
  child = spawn(
    runtime,
    [
      initialize ? '--initialize-profile' : '--profile',
      profile,
      initialize ? '--management' : '--serve',
    ],
    {
      windowsHide: true,
      stdio: ['pipe', 'pipe', 'pipe'],
      env: {
        SystemRoot: process.env.SystemRoot,
        FLOWTOOLS_RUNTIME_VALIDATION_STDIN_LIFETIME: '1',
      },
    }
  )
  exited = once(child, 'exit')
  const lines = createInterface({ input: child.stdout })
  let failure = ''
  child.stderr.on('data', b => {
    failure += b.toString()
  })
  const timer = setTimeout(() => child?.kill(), 15000)
  try {
    const line = await Promise.race([
      once(lines, 'line').then(([line]) => String(line)),
      exited.then(() => {
        throw new Error(failure || 'STARTUP_FAILED')
      }),
    ])
    const ready = /** @type {{type:string;pipe:string}} */ (JSON.parse(line)),
      tokens = /** @type {{managementToken:string;desktopToken:string}} */ (
        JSON.parse(await readFile(join(profile, 'bootstrap.json'), 'utf8'))
      )
    assert.equal(ready.type, 'ready')
    manager = new RuntimeClient(await connectNamedPipe(ready.pipe))
    await manager.connect(tokens.managementToken)
    desktop = new RuntimeClient(await connectNamedPipe(ready.pipe))
    await desktop.connect(tokens.desktopToken)
  } finally {
    clearTimeout(timer)
    lines.close()
  }
}
async function stop() {
  manager?.close()
  desktop?.close()
  manager = undefined
  desktop = undefined
  if (child) {
    child.stdin.end()
    await exited
    child = undefined
  }
}
async function invoke(/** @type {string[]} */ args, expected = 0) {
  const command = spawn(bun, [cli, ...args, '--profile', profile], {
    windowsHide: true,
    stdio: ['ignore', 'pipe', 'pipe'],
    env: { SystemRoot: process.env.SystemRoot, PATHEXT: process.env.PATHEXT },
  })
  let stdout = '',
    stderr = ''
  command.stdout.on('data', b => (stdout += b.toString()))
  command.stderr.on('data', b => (stderr += b.toString()))
  /** @type {Promise<number|null>} */
  const commandExit = new Promise((resolve, reject) => {
    command.once('exit', resolve)
    command.once('error', reject)
  })
  const timer = setTimeout(() => command.kill(), 35000)
  try {
    const code = await commandExit
    assert.equal(code, expected, stderr + ' ' + stdout)
    assert.equal(stderr, '')
    return /** @type {{success:boolean;data:Record<string,unknown>;error?:{code:string}}} */ (
      JSON.parse(stdout)
    )
  } finally {
    clearTimeout(timer)
  }
}
const watchdog = setTimeout(() => {
  child?.kill()
  process.exitCode = 1
}, 85000)
try {
  await mkdir(files)
  const path = join(files, 'PRIVATE-FILE.txt')
  await writeFile(path, 'PRIVATE-CONTENT')
  await start(true)
  await stop()
  await start(false)
  const denied = await invoke(['resources', 'select', path], 1)
  assert.equal(denied.error?.code, 'APPROVAL_REQUIRED')
  const selected = await invoke(['resources', 'select', path, '--approve-read'])
  assert.equal(typeof selected.data.owner, 'string')
  assert.ok(manager && desktop)
  await assert.rejects(
    manager.call({
      method: 'resources.submit',
      payload: {
        owner: String(selected.data.owner),
        submit: {
          pluginId: 'plugin-resource-reader',
          commandId: 'run',
          input: {},
          idempotencyKey: crypto.randomUUID(),
          deadline: Date.now() + 30000,
          background: false,
        },
      },
    }),
    e => e instanceof RuntimeClientError && e.code === 'APPROVAL_REQUIRED'
  )
  const result = await invoke(['resources', 'run', String(selected.data.owner)])
  assert.equal(result.data.state, 'succeeded')
  assert.ok(!JSON.stringify(result).includes('PRIVATE-'))
  const plugins = await manager.call({ method: 'plugins.list' })
  assert.equal(plugins.type, 'plugins')
  if (plugins.type !== 'plugins') throw new Error('INVALID_RESPONSE')
  const plugin = plugins.data.find(p => p.pluginId === 'plugin-resource-reader')
  assert.ok(plugin)
  await manager.call({
    method: 'resources.grant',
    payload: {
      formatVersion: 2,
      capabilityProfile: 'flowtools-native-v2',
      pluginId: plugin.pluginId,
      commandId: 'run',
      target: 'desktop',
      packageDigest: plugin.packageDigest,
      scopes: [{ slot: 'source', operations: ['list', 'stat'] }],
      expiresAt: Date.now() + 60000,
      maxCalls: 4096,
    },
  })
  const owner = await manager.call({
    method: 'resources.owner',
    payload: {
      key: { pluginId: plugin.pluginId, commandId: 'run', target: 'desktop' },
    },
  })
  assert.equal(owner.type, 'resource-owner')
  if (owner.type !== 'resource-owner') throw new Error('INVALID_RESPONSE')
  await assert.rejects(
    desktop.call({
      method: 'resources.select',
      payload: {
        owner: owner.data.id,
        slot: 'source',
        kind: 'directory',
        path: files,
      },
    }),
    e => e instanceof RuntimeClientError && e.code === 'SESSION_INVALID'
  )
  const chosen = await manager.call({
    method: 'resources.select',
    payload: {
      owner: owner.data.id,
      slot: 'source',
      kind: 'directory',
      path: files,
    },
  })
  assert.equal(chosen.type, 'resource')
  await assert.rejects(
    desktop.submit({
      pluginId: plugin.pluginId,
      commandId: 'run',
      input: {},
      idempotencyKey: crypto.randomUUID(),
      deadline: Date.now() + 30000,
      background: false,
    }),
    e => e instanceof RuntimeClientError && e.code === 'INTERACTION_REQUIRED'
  )
  const accepted = await desktop.call({
    method: 'resources.submit',
    payload: {
      owner: owner.data.id,
      submit: {
        pluginId: plugin.pluginId,
        commandId: 'run',
        input: {},
        idempotencyKey: crypto.randomUUID(),
        deadline: Date.now() + 30000,
        background: false,
      },
    },
  })
  assert.equal(accepted.type, 'receipt')
  if (accepted.type !== 'receipt') throw new Error('INVALID_RESPONSE')
  for (;;) {
    const job = await desktop.job(accepted.data.runId)
    if (
      ['succeeded', 'failed', 'cancelled', 'interrupted'].includes(job.state)
    ) {
      assert.equal(job.state, 'succeeded', JSON.stringify(job.result))
      assert.ok(!JSON.stringify(job).includes('PRIVATE-'))
      break
    }
    await new Promise(resolve => setTimeout(resolve, 50))
  }
  const unused = await manager.call({
    method: 'resources.owner',
    payload: {
      key: { pluginId: plugin.pluginId, commandId: 'run', target: 'desktop' },
    },
  })
  if (unused.type !== 'resource-owner') throw new Error('INVALID_RESPONSE')
  await manager.call({
    method: 'resources.select',
    payload: {
      owner: unused.data.id,
      slot: 'source',
      kind: 'directory',
      path: files,
    },
  })
  await stop()
  await start(false)
  assert.ok(desktop)
  await assert.rejects(
    desktop.call({
      method: 'resources.submit',
      payload: {
        owner: unused.data.id,
        submit: {
          pluginId: plugin.pluginId,
          commandId: 'run',
          input: {},
          idempotencyKey: crypto.randomUUID(),
          deadline: Date.now() + 30000,
          background: false,
        },
      },
    }),
    e => e instanceof RuntimeClientError && e.code === 'SCOPE_DENIED'
  )
  assert.equal(await readFile(path, 'utf8'), 'PRIVATE-CONTENT')
  process.stdout.write(
    'NA1 actual CLI and Desktop protocol resource selection/read and restart refusal passed\n'
  )
} finally {
  clearTimeout(watchdog)
  await stop()
  const actual = await realpath(scratch)
  assert.equal(
    dirname(actual).toLowerCase(),
    (await realpath(tmpdir())).toLowerCase()
  )
  assert.ok(basename(actual).startsWith('flowtools-validation-na1-'))
  await rm(actual, { recursive: true })
}
