import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import { createHash } from 'node:crypto'
import { once } from 'node:events'
import { mkdtemp, readFile, readdir } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { createInterface } from 'node:readline'

import {
  RuntimeClient,
  PluginDataClient,
  RuntimeClientError,
} from '../../../packages/runtime-client/dist/index.js'
import { connectNamedPipe } from '../../../packages/runtime-client/dist/node.js'

const root = resolve(import.meta.dirname, '../../..')
const executable = resolve(
  process.env.CARGO_TARGET_DIR ?? join(root, 'target'),
  'debug/flowtools-runtime.exe'
)
const profile = await mkdtemp(join(tmpdir(), 'flowtools-validation-jobs-'))
const catalog = /** @type {{ plugins: { id: string }[] }} */ (
  JSON.parse(
    await readFile(
      join(root, 'packages/runtime-core/.generated/catalog.json'),
      'utf8'
    )
  )
)
/** @param {unknown} value @returns {unknown} */
const canonical = value =>
  Array.isArray(value)
    ? value.map(canonical)
    : value && typeof value === 'object'
      ? Object.fromEntries(
          Object.entries(value)
            .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
            .map(([key, item]) => [key, canonical(item)])
        )
      : value
const grants = ['plugin-todo-list', 'plugin-base64-encoder'].map(pluginId => ({
  pluginId,
  commandId: 'run',
  target: 'cli',
  packageDigest: createHash('sha256')
    .update(
      JSON.stringify(canonical(catalog.plugins.find(p => p.id === pluginId)))
    )
    .digest('hex'),
  effects: pluginId === 'plugin-todo-list' ? ['data-read', 'data-write'] : [],
  scopes:
    pluginId === 'plugin-todo-list'
      ? [{ kind: 'plugin-data', scope: { key_prefix: 'todos' } }]
      : [],
  expiresAt: Date.now() + 3600000,
  maxCalls: 32,
  coldStart: false,
  background: true,
}))
let credentials = { managementToken: '', cliToken: '' }
async function start(initialize = false) {
  const child = spawn(
    executable,
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
  const exit = once(child, 'exit')
  const lines = createInterface({ input: child.stdout })
  let error = ''
  child.stderr.on('data', b => (error += b))
  const ready = await Promise.race([
    once(lines, 'line').then(
      ([v]) => /** @type {{pipe:string}} */ (JSON.parse(String(v)))
    ),
    exit.then(() => {
      throw new Error(error)
    }),
  ])
  credentials = /** @type {{managementToken:string,cliToken:string}} */ (
    JSON.parse(await readFile(join(profile, 'bootstrap.json'), 'utf8'))
  )
  const connect = async (manager = false) => {
    const client = new RuntimeClient(await connectNamedPipe(ready.pipe))
    await client.connect(
      manager ? credentials.managementToken : credentials.cliToken
    )
    return client
  }
  return {
    child,
    exit,
    ready,
    connect,
    async close() {
      lines.close()
      child.stdin.end()
      assert.equal((await exit)[0], 0)
    },
  }
}
let host = await start(true)
let manager = await host.connect(true)
await manager.call({
  method: 'policy.import',
  payload: { formatVersion: 1, coldStart: false, grants },
})
manager.close()
await host.close()
host = await start()
let cli = await host.connect()
try {
  const transport = await connectNamedPipe(host.ready.pipe)
  const lost = new RuntimeClient({
    close: () => transport.close(),
    async exchange(request) {
      const response = await transport.exchange(request)
      if (request.call.method === 'jobs.submit') {
        transport.close()
        throw new RuntimeClientError('RUNTIME_DISCONNECTED')
      }
      return response
    },
  })
  await lost.connect(credentials.cliToken)
  const operation = {
    pluginId: 'plugin-todo-list',
    commandId: 'run',
    input: { todo: 'SENSITIVE_JOB_CANARY' },
    idempotencyKey: 'lost-ack',
    background: true,
    deadline: Date.now() + 25000,
  }
  await assert.rejects(
    lost.submit(operation),
    error => error.acceptanceUnknown === true
  )
  const found = await cli.call({
    method: 'jobs.lookup',
    payload: { idempotencyKey: 'lost-ack' },
  })
  assert.ok(found.type === 'receipt')
  const run = found.data.runId
  assert.equal((await cli.submit(operation)).runId, run)
  await assert.rejects(cli.submit({ ...operation, input: { todo: 'other' } }), {
    code: 'IDEMPOTENCY_CONFLICT',
  })
  const result = await cli.waitForResult(run)
  assert.equal(result.state, 'succeeded')
  assert.equal(result.result.data.value.result.total, 1)
  assert.equal(
    (await new PluginDataClient(cli, 'plugin-todo-list').read('todos'))
      .revision,
    1
  )
  const pure = await cli.submit({
    pluginId: 'plugin-base64-encoder',
    commandId: 'run',
    input: { text: 'PRIVATE_RESULT_CANARY' },
    idempotencyKey: 'pure',
    background: true,
    deadline: Date.now() + 25000,
  })
  const pureResult = await cli.waitForResult(pure.runId)
  assert.equal(pureResult.state, 'succeeded')
  const events = await cli.call({
    method: 'jobs.events',
    payload: { runId: pure.runId, afterSequence: 0 },
  })
  assert.ok(!JSON.stringify(events).includes('PRIVATE_RESULT_CANARY'))
  for (const file of await readdir(join(profile, 'private-jobs'))) {
    const bytes = await readFile(join(profile, 'private-jobs', file))
    assert.ok(!bytes.includes(Buffer.from('PRIVATE_RESULT_CANARY')))
    assert.ok(!file.endsWith('.input.bin'))
  }
  cli.close()
  await host.close()
  host = await start()
  cli = await host.connect()
  assert.deepEqual(
    (await cli.job(pure.runId)).result.data,
    pureResult.result.data
  )
  assert.equal(
    (
      await cli.call({
        method: 'jobs.lookup',
        payload: { idempotencyKey: 'lost-ack' },
      })
    ).data.runId,
    run
  )
  const crashing = await cli.submit({
    pluginId: 'plugin-todo-list',
    commandId: 'run',
    input: { todo: 'CRASH_CANARY' },
    idempotencyKey: 'crash',
    background: true,
    deadline: Date.now() + 25000,
  })
  const running = await cli.job(crashing.runId)
  assert.equal(running.state, 'running')
  host.child.kill()
  await host.exit
  cli.close()
  host = await start()
  cli = await host.connect()
  const interrupted = await cli.job(crashing.runId)
  assert.equal(interrupted.state, 'interrupted')
  assert.equal(interrupted.result.error.code, 'EXECUTION_INTERRUPTED')
  const afterCrash = await new PluginDataClient(cli, 'plugin-todo-list').read(
    'todos'
  )
  assert.ok(afterCrash.revision <= 2)
  assert.equal(
    (
      await cli.call({
        method: 'jobs.lookup',
        payload: { idempotencyKey: 'crash' },
      })
    ).data.runId,
    crashing.runId
  )
  manager = await host.connect(true)
  const revoked = await cli.submit({
    pluginId: 'plugin-todo-list',
    commandId: 'run',
    input: { todo: 'REVOKE_CANARY' },
    idempotencyKey: 'revoke',
    background: true,
    deadline: Date.now() + 25000,
  })
  await manager.call({
    method: 'permissions.revoke',
    payload: { pluginId: 'plugin-todo-list', commandId: 'run', target: 'cli' },
  })
  const failure = await cli.waitForResult(revoked.runId)
  assert.equal(failure.result.error.code, 'GRANT_REVOKED')
  // Receipt lookup remains available after revocation; it never repeats a write.
  assert.equal(
    (
      await cli.call({
        method: 'jobs.lookup',
        payload: { idempotencyKey: 'revoke' },
      })
    ).data.runId,
    revoked.runId
  )
  manager.close()
} finally {
  cli.close()
  await host.close()
}
process.stdout.write('Durable jobs fixture passed\n')
