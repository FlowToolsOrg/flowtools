import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import { randomBytes } from 'node:crypto'
import { once } from 'node:events'
import { mkdtemp, readFile } from 'node:fs/promises'
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
  await cli.connect(cliToken)
  await desktop.connect(desktopToken)
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
