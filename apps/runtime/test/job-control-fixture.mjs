import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import { createHash } from 'node:crypto'
import { mkdtemp, readFile, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'

const root = resolve(import.meta.dirname, '../../..')
const cli = join(root, 'packages/cli/dist/cli.mjs')
const scratch = await mkdtemp(join(tmpdir(), 'flowtools-validation-control-'))
const profile = join(scratch, 'profile')
/** @type {{ plugins: { id: string }[] }} */
const catalog = /** @type {{ plugins: { id: string }[] }} */ (
  /** @type {unknown} */ (
    JSON.parse(
      await readFile(
        join(root, 'packages/runtime-core/.generated/catalog.json'),
        'utf8'
      )
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
const grants = ['plugin-base64-encoder', 'plugin-todo-list'].flatMap(pluginId =>
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
    effects: pluginId === 'plugin-todo-list' ? ['data-read', 'data-write'] : [],
    scopes:
      pluginId === 'plugin-todo-list'
        ? [{ kind: 'plugin-data', scope: { key_prefix: 'todos' } }]
        : [],
    expiresAt: Date.now() + 3600000,
    maxCalls: 32,
    coldStart: true,
    background: true,
  }))
)
const policy = join(scratch, 'policy.json')
await writeFile(
  policy,
  JSON.stringify({ formatVersion: 1, coldStart: true, grants })
)
/** @param {string[]} args */
async function invoke(args) {
  if (process.env.FLOWTOOLS_VALIDATE_TRACE === '1')
    process.stderr.write('Stage: ' + args.slice(0, 2).join(' ') + '\n')
  const child = spawn(process.execPath, [cli, ...args, '--profile', profile], {
    windowsHide: true,
    env: { SystemRoot: process.env.SystemRoot },
    stdio: ['ignore', 'pipe', 'pipe'],
  })
  /** @type {Promise<number | null>} */
  const exit = new Promise(resolve => child.once('exit', code => resolve(code)))
  let stdout = '',
    stderr = ''
  child.stdout.on('data', bytes => (stdout += bytes))
  child.stderr.on('data', bytes => (stderr += bytes))
  const timer = setTimeout(() => child.kill(), 45000)
  try {
    return {
      code: await exit,
      stdout,
      stderr,
      lines: stdout
        .trim()
        .split(/\r?\n/)
        .filter(Boolean)
        .map(
          line =>
            /** @type {unknown} */ (
              line.startsWith('{') ? JSON.parse(line) : line
            )
        ),
    }
  } finally {
    clearTimeout(timer)
  }
}
/** @param {unknown} value @returns {Record<string, unknown>} */
function object(value) {
  assert.ok(value && typeof value === 'object' && !Array.isArray(value))
  return /** @type {Record<string, unknown>} */ (value)
}
/** @param {Awaited<ReturnType<typeof invoke>>} result @returns {unknown} */
function success(result) {
  assert.equal(result.code, 0, result.stdout + result.stderr)
  return object(result.lines[0]).data
}
try {
  success(await invoke(['init', '--policy', policy]))
  const started = object(success(await invoke(['runtime', 'start'])))
  assert.equal(started.activeJobs, 0)
  const deadline = String(Date.now() + 25000)
  const args = [
    'jobs',
    'submit',
    'plugin-todo-list',
    '--background',
    '--idempotency-key',
    'one-write',
    '--deadline',
    deadline,
    '--input',
    JSON.stringify({ todo: 'CONTROL_PRIVATE_CANARY' }),
  ]
  const receipt = object(success(await invoke(args)))
  assert.equal(receipt.receiptType, 'job')
  assert.equal(
    object(success(await invoke(['jobs', 'lookup', 'one-write']))).runId,
    receipt.runId
  )
  const watched = await invoke(['jobs', 'watch', String(receipt.runId)])
  assert.equal(watched.code, 0, watched.stdout + watched.stderr)
  assert.equal(object(object(watched.lines.at(-1)).data).state, 'succeeded')
  assert.equal(object(success(await invoke(args))).runId, receipt.runId)
  const changed = await invoke([
    ...args.slice(0, -1),
    JSON.stringify({ todo: 'second' }),
  ])
  assert.equal(changed.code, 1)
  assert.equal(
    object(object(changed.lines[0]).error).code,
    'IDEMPOTENCY_CONFLICT'
  )
  const listing = success(await invoke(['jobs', 'list']))
  assert.ok(Array.isArray(listing))
  const listed = listing.map(object).find(item => item.runId === receipt.runId)
  assert.ok(listed)
  assert.equal(listed.result, null)
  assert.ok(!JSON.stringify(listing).includes('CONTROL_PRIVATE_CANARY'))
  const status = object(
    success(await invoke(['jobs', 'status', String(receipt.runId)]))
  )
  assert.equal(
    object(object(object(object(status.result).data).value).result).total,
    1
  )
  const actual = await invoke([
    'run',
    'plugin-base64-encoder',
    '--format',
    'text',
    '--text',
    'hello',
  ])
  assert.equal(actual.code, 0, actual.stdout + actual.stderr)
  assert.equal(actual.stdout.trim(), 'aGVsbG8=')
  const malformed = await invoke([
    'jobs',
    'submit',
    'plugin-todo-list',
    '--background',
    '--idempotency-key',
    'invalid',
    '--input',
    '{"todo":3}',
  ])
  assert.equal(malformed.code, 1)
  assert.equal(object(object(malformed.lines[0]).error).code, 'INPUT_INVALID')
  const missing = await invoke(['jobs', 'status', 'unknown-run'])
  assert.equal(missing.code, 1)
  assert.equal(object(object(missing.lines[0]).error).code, 'JOB_NOT_FOUND')
  success(await invoke(['permissions', 'revoke', 'plugin-todo-list']))
  const revoked = await invoke([
    'jobs',
    'submit',
    'plugin-todo-list',
    '--background',
    '--idempotency-key',
    'revoked',
    '--input',
    '{}',
  ])
  assert.equal(revoked.code, 1)
  assert.equal(object(object(revoked.lines[0]).error).code, 'APPROVAL_REQUIRED')
  assert.equal(
    object(success(await invoke(['jobs', 'lookup', 'one-write']))).runId,
    receipt.runId
  )
  success(await invoke(['jobs', 'cancel', String(receipt.runId)]))
  process.stdout.write('Compiled shared job control fixture passed\n')
} finally {
  await invoke(['runtime', 'stop']).catch(() => {})
}
