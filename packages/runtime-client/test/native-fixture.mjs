import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import { randomBytes } from 'node:crypto'
import { once } from 'node:events'
import { mkdtemp, readFile } from 'node:fs/promises'
import { createConnection } from 'node:net'
import { tmpdir } from 'node:os'
import { resolve, join } from 'node:path'
import { createInterface } from 'node:readline'

import { RuntimeClient } from '../dist/index.js'
import { connectNamedPipe } from '../dist/node.js'
const root = resolve(import.meta.dirname, '../../..')
const executable = resolve(
  process.env.CARGO_TARGET_DIR ?? join(root, 'target'),
  'debug/flowtools-runtime.exe'
)
const profile = await mkdtemp(join(tmpdir(), 'flowtools-validation-'))
const cliToken = randomBytes(32).toString('hex')
const desktopToken = randomBytes(32).toString('hex')
const host = spawn(
  executable,
  ['--validation-profile', profile, '--validation'],
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
const exit = once(host, 'exit')

const lines = createInterface({ input: host.stdout })
const ready = /** @type {{ pipe: string }} */ (
  await Promise.race([
    once(lines, 'line').then(([line]) => JSON.parse(String(line))),
    exit.then(() => {
      throw new Error('Host refused startup')
    }),
  ])
)
const cli = new RuntimeClient(await connectNamedPipe(ready.pipe))
const desktop = new RuntimeClient(await connectNamedPipe(ready.pipe))
try {
  const mismatch = await rawExchange(ready.pipe, {
    version: 2,
    requestId: 'wrong-version',
    session: null,
    call: { method: 'runtime.status' },
  })
  partial(mismatch, {
    outcome: { type: 'error', data: { code: 'PROTOCOL_MISMATCH' } },
  })
  const injected = await rawExchange(ready.pipe, {
    version: 1,
    requestId: 'spoof',
    session: null,
    call: {
      method: 'session.open',
      payload: {
        token: cliToken,
        clientVersion: '0.1.0',
        expectedInstanceId: null,
        agentId: 'admin',
      },
    },
  })
  partial(injected, {
    outcome: { type: 'error', data: { code: 'INVALID_REQUEST' } },
  })
  const oversized = await rawExchange(ready.pipe, null, true)
  partial(oversized, {
    outcome: { type: 'error', data: { code: 'FRAME_TOO_LARGE' } },
  })
  await cli.connect(cliToken)
  await desktop.connect(desktopToken)
  const obsolete = new RuntimeClient(await connectNamedPipe(ready.pipe))
  await assert.rejects(obsolete.connect(cliToken, 'old-instance'), {
    code: 'INSTANCE_MISMATCH',
  })
  obsolete.close()
  assert.equal(cli.instanceId, desktop.instanceId)
  const job = {
    pluginId: 'plugin-base64-encoder',
    commandId: 'run',
    input: { text: 'hello' },
    idempotencyKey: 'actual-base64',
    background: true,
    deadline: Date.now() + 10000,
  }
  const receipt = await cli.submit(job)
  await assert.rejects(
    cli.submit({ ...job, idempotencyKey: 'expired', deadline: Date.now() - 1 }),
    { code: 'TIMEOUT' }
  )
  assert.equal(receipt.receiptType, 'job')
  assert.equal('success' in receipt, false)
  const retried = await cli.submit(job)
  assert.equal(retried.runId, receipt.runId)
  partial(await desktop.waitForResult(receipt.runId), {
    rootCaller: 'validation-cli',
    state: 'succeeded',
    result: { success: true, data: { value: { result: 'aGVsbG8=' } } },
  })
  assert.deepEqual(
    await cli.job(receipt.runId),
    await desktop.job(receipt.runId)
  )
  await assert.rejects(
    cli.submit({ ...job, input: { text: 3 }, idempotencyKey: 'bad' }),
    { code: 'INPUT_INVALID' }
  )
  await assert.rejects(
    cli.submit({
      ...job,
      pluginId: 'plugin-todo-list',
      input: { todo: 'private' },
      idempotencyKey: 'denied',
    }),
    { code: 'APPROVAL_REQUIRED' }
  )
  await assert.rejects(cli.submit({ ...job, input: { text: 'different' } }), {
    code: 'IDEMPOTENCY_CONFLICT',
  })
  await assert.rejects(desktop.cancel(receipt.runId), {
    code: 'SESSION_INVALID',
  })
  const cancelled = await cli.submit({ ...job, idempotencyKey: 'cancel' })
  await cli.cancel(cancelled.runId)
  partial(await desktop.waitForResult(cancelled.runId), {
    state: 'cancelled',
    result: { success: false, error: { code: 'ABORTED' } },
  })
  const background = await cli.submit({ ...job, idempotencyKey: 'background' })
  cli.close()
  partial(await desktop.waitForResult(background.runId), { state: 'succeeded' })
  const reconnect = new RuntimeClient(await connectNamedPipe(ready.pipe))
  await reconnect.connect(cliToken, desktop.instanceId)
  const foreground = await reconnect.submit({
    ...job,
    idempotencyKey: 'foreground',
    background: false,
    deadline: Date.now() + 10000,
  })
  reconnect.close()
  partial(await desktop.waitForResult(foreground.runId), {
    state: 'cancelled',
    result: { success: false, error: { code: 'ABORTED' } },
  })
  const events = await desktop.call({
    method: 'jobs.events',
    payload: { runId: background.runId, afterSequence: 0 },
  })
  assert.ok(!JSON.stringify(events).includes('hello'))
  const outputBudget = new RuntimeClient(await connectNamedPipe(ready.pipe))
  await outputBudget.connect(cliToken, desktop.instanceId)
  const oversizedOutput = await outputBudget.submit({
    ...job,
    input: { text: 'x'.repeat(500000) },
    idempotencyKey: 'output-budget',
    deadline: Date.now() + 10000,
  })
  partial(await desktop.waitForResult(oversizedOutput.runId), {
    state: 'failed',
    result: { success: false, error: { code: 'OUTPUT_INVALID' } },
  })
  outputBudget.close()
  assert.equal(
    await readFile(join(profile, 'validation-profile-v1'), 'utf8'),
    'flowtools-disposable-v1'
  )
} finally {
  cli.close()
  desktop.close()
  lines.close()
  host.stdin.end()
  assert.equal((await exit)[0], 0)
}
process.stdout.write('Native fixture passed\n')

function partial(actual, expected) {
  for (const [key, value] of Object.entries(expected)) {
    if (value && typeof value === 'object') partial(actual[key], value)
    else assert.equal(actual[key], value)
  }
}

/** @returns {Promise<unknown>} */
async function rawExchange(pipe, request, oversized = false) {
  const socket = createConnection(pipe)
  socket.on('error', () => {})
  await once(socket, 'connect')
  try {
    return await new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        socket.destroy()
        reject(new Error('Raw frame timeout'))
      }, 5000)
      let buffer = Buffer.alloc(0)
      socket.on('data', chunk => {
        buffer = Buffer.concat([buffer, chunk])
        if (buffer.length < 4 || buffer.length < buffer.readUInt32LE(0) + 4)
          return
        clearTimeout(timer)
        resolve(JSON.parse(buffer.subarray(4).toString('utf8')))
      })
      const bytes = Buffer.from(JSON.stringify(request))
      const header = Buffer.alloc(4)
      header.writeUInt32LE(oversized ? 1_048_577 : bytes.length)
      socket.write(oversized ? header : Buffer.concat([header, bytes]))
    })
  } finally {
    socket.destroy()
  }
}
