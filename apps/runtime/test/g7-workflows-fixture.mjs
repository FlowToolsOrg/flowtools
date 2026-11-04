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
import { join, resolve, dirname, basename } from 'node:path'
import { createInterface } from 'node:readline'

import { RuntimeClient } from '@flowtools/runtime-client'
import { connectNamedPipe } from '@flowtools/runtime-client/node'

const root = resolve(import.meta.dirname, '../../..')
/** @param {unknown} value @returns {Record<string,unknown>} */
function object(value) {
  assert.ok(value && typeof value === 'object' && !Array.isArray(value))
  return /** @type {Record<string,unknown>} */ (value)
}
const scratch = await mkdtemp(
  join(tmpdir(), 'flowtools-validation-g7-workflows-')
)
const profile = join(scratch, 'profile'),
  selected = join(scratch, 'selected')
const runtime = join(
  process.env.CARGO_TARGET_DIR ?? join(root, 'target'),
  'debug/flowtools-runtime.exe'
)
const cli = join(root, 'packages/cli/dist/cli.mjs')
const bun = /** @type {{bun:{path:string}}} */ (
  JSON.parse(
    await readFile(join(root, 'apps/runtime/.generated/runner.json'), 'utf8')
  )
).bun.path
/** @type {import('node:child_process').ChildProcessWithoutNullStreams | undefined} */ let child
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
    if (failure.length < 256) failure += b.toString()
  })
  const timer = setTimeout(() => child?.kill(), 15000)
  try {
    const line = await Promise.race([
      once(lines, 'line').then(([s]) => String(s)),
      exited.then(() => {
        throw new Error(
          /^[A-Z_]+\s*$/.test(failure) ? failure : 'STARTUP_FAILED'
        )
      }),
    ])
    const ready = object(JSON.parse(line))
    assert.equal(ready.type, 'ready')
    const tokens = object(
      JSON.parse(await readFile(join(profile, 'bootstrap.json'), 'utf8'))
    )
    assert.equal(typeof ready.pipe, 'string')
    assert.equal(typeof tokens.managementToken, 'string')
    assert.equal(typeof tokens.desktopToken, 'string')
    if (
      typeof ready.pipe !== 'string' ||
      typeof tokens.managementToken !== 'string' ||
      typeof tokens.desktopToken !== 'string'
    )
      throw new Error('INVALID_RESPONSE')
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
async function invoke(/** @type {string[]} */ args, approve = true) {
  const p = spawn(bun, [cli, ...args, '--profile', profile], {
    windowsHide: true,
    stdio: ['ignore', 'pipe', 'pipe'],
    env: { SystemRoot: process.env.SystemRoot, PATHEXT: process.env.PATHEXT },
  })
  let out = '',
    err = ''
  p.stdout.on('data', b => (out += b))
  p.stderr.on('data', b => (err += b))
  const timer = setTimeout(() => p.kill(), 40000)
  try {
    const code = await new Promise(
      (/** @type {(value:number|null)=>void} */ resolve) =>
        p.once('exit', resolve)
    )
    assert.equal(code, approve ? 0 : 1, err + ' ' + out)
    return object(JSON.parse(out))
  } finally {
    clearTimeout(timer)
  }
}
const watchdog = setTimeout(() => {
  child?.kill()
  process.exitCode = 1
}, 85000)
try {
  await mkdir(selected)
  await writeFile(join(selected, 'notes.txt'), 'preserved input')
  await start(true)
  const draft = await manager.call({
    method: 'files.plan',
    payload: {
      requestKey: crypto.randomUUID(),
      directory: selected,
      prefix: 'done_',
    },
  })
  assert.equal(draft.type, 'file-plan')
  if (draft.type !== 'file-plan') throw new Error('INVALID_RESPONSE')
  const p = draft.data.record
  const observed = await invoke(['files', 'preview', p.id, '--revision', '1'])
  assert.equal(object(object(observed.data).record).digest, p.digest)
  const denied = await invoke(
    ['files', 'commit', p.id, '--revision', '1', '--digest', p.digest],
    false
  )
  assert.equal(object(denied.error).code, 'APPROVAL_REQUIRED')
  assert.equal(
    await readFile(join(selected, 'notes.txt'), 'utf8'),
    'preserved input'
  )
  await invoke([
    'files',
    'commit',
    p.id,
    '--revision',
    '1',
    '--digest',
    p.digest,
    '--approve',
  ])
  await invoke([
    'files',
    'commit',
    p.id,
    '--revision',
    '1',
    '--digest',
    p.digest,
    '--approve',
  ])
  const undone = await manager.call({
    method: 'files.undo',
    payload: { id: p.id, revision: 1, digest: p.digest },
  })
  assert.equal(undone.type, 'file-execution')
  assert.equal(
    await readFile(join(selected, 'notes.txt'), 'utf8'),
    'preserved input'
  )
  await stop()
  await start(false)
  const pcm = Buffer.alloc(32000)
  for (let n = 0; n < 16000; n++)
    pcm.writeInt16LE(((n % 257) - 128) * 128, n * 2)
  const wav = Buffer.alloc(44)
  wav.write('RIFF')
  wav.writeUInt32LE(32036, 4)
  wav.write('WAVEfmt ', 8)
  wav.writeUInt32LE(16, 16)
  wav.writeUInt16LE(1, 20)
  wav.writeUInt16LE(1, 22)
  wav.writeUInt32LE(16000, 24)
  wav.writeUInt32LE(32000, 28)
  wav.writeUInt16LE(2, 32)
  wav.writeUInt16LE(16, 34)
  wav.write('data', 36)
  wav.writeUInt32LE(pcm.length, 40)
  const input = join(scratch, 'input.wav'),
    output = join(scratch, 'output.wav')
  await writeFile(input, Buffer.concat([wav, pcm]))
  const media = await invoke([
    'media',
    'plan',
    '--input',
    input,
    '--output',
    output,
    '--action',
    'audio',
    '--approve-read',
  ])
  const mediaPlan = object(media.data)
  if (typeof mediaPlan.id !== 'string' || typeof mediaPlan.digest !== 'string')
    throw new Error('INVALID_RESPONSE')
  const plan = await manager.call({
    method: 'media.preview',
    payload: { id: mediaPlan.id, revision: 1 },
  })
  assert.equal(plan.type, 'media-plan')
  const result = await invoke([
    'media',
    'start',
    mediaPlan.id,
    '--digest',
    mediaPlan.digest,
    '--approve',
    '--wait',
  ])
  const finished = object(result.data)
  if (typeof finished.runId !== 'string') throw new Error('INVALID_RESPONSE')
  assert.equal(finished.state, 'succeeded')
  assert.equal(
    object(object(object(object(finished.result).data).value).result).published,
    true
  )
  const bytes = await readFile(output)
  assert.equal(bytes.subarray(0, 4).toString(), 'RIFF')
  assert.ok(bytes.includes(pcm))
  const desktopResult = await desktop.call({
    method: 'jobs.status',
    payload: { runId: finished.runId },
  })
  assert.equal(desktopResult.type, 'job')
  const retry = await manager.call({
    method: 'media.start',
    payload: {
      id: mediaPlan.id,
      digest: mediaPlan.digest,
      target: 'desktop',
    },
  })
  assert.equal(retry.type, 'receipt')
  if (retry.type === 'receipt') assert.equal(retry.data.runId, finished.runId)
  await stop()
  await start(false)
  const after = await desktop.call({
    method: 'jobs.status',
    payload: { runId: finished.runId },
  })
  assert.equal(after.type, 'job')
  if (after.type === 'job') assert.equal(after.data.state, 'succeeded')
  assert.deepEqual(await readFile(output), bytes)
  process.stdout.write(
    'G7 actual CLI and native Desktop protocol share file plans, consent, commit/undo, signed media publication and restart-safe receipts\n'
  )
} finally {
  clearTimeout(watchdog)
  await stop()
  const actual = await realpath(scratch)
  assert.equal(
    dirname(actual).toLowerCase(),
    (await realpath(tmpdir())).toLowerCase()
  )
  assert.ok(basename(actual).startsWith('flowtools-validation-g7-workflows-'))
  await rm(actual, { recursive: true })
}
