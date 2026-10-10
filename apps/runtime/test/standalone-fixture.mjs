import assert from 'node:assert/strict'
import { spawn, spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { once } from 'node:events'
import {
  copyFile,
  cp,
  mkdtemp,
  readFile,
  rename,
  lstat,
  rmdir,
  writeFile,
} from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'

const root = resolve(import.meta.dirname, '../../..')
/** @param {string} exe @param {string[]} args @param {string} cwd @param {NodeJS.ProcessEnv} env */
async function processResult(exe, args, cwd, env) {
  const child = spawn(exe, args, {
    cwd,
    env,
    windowsHide: true,
    stdio: ['ignore', 'pipe', 'pipe'],
  })
  const exit = once(child, 'exit')
  let stdout = '',
    stderr = ''
  child.stdout.on('data', bytes => (stdout += bytes))
  child.stderr.on('data', bytes => (stderr += bytes))
  const timer = setTimeout(
    () => child.kill(),
    args.includes('pack.ts') ? 240000 : 120000
  )
  try {
    return { code: Number((await exit)[0]), stdout, stderr }
  } finally {
    clearTimeout(timer)
  }
}
const pack = await processResult(
  'bun',
  ['run', '--cwd', 'apps/runtime', 'pack.ts'],
  root,
  process.env
)
assert.equal(pack.code, 0, pack.stderr)
const source = pack.stdout.trim().split(/\r?\n/).at(-1)
assert.ok(
  source && source.startsWith(join(root, 'execution-validation', 'standalone'))
)
const scratch = await mkdtemp(
  join(tmpdir(), 'flowtools-validation-standalone-')
)
const bundle = join(scratch, 'relocated')
await cp(source, bundle, { recursive: true, dereference: false })
const exe = join(bundle, 'flowtools.exe')
// Read the actual PE imports, rather than inferring CRT independence from flags.
for (const name of [
  'flowtools.exe',
  'native/flowtools-runtime.exe',
  'native/node.exe',
  'native/bun.exe',
]) {
  const bytes = await readFile(join(bundle, name))
  const pe = bytes.readUInt32LE(0x3c),
    optional = pe + 24
  assert.equal(bytes.readUInt16LE(optional), 0x20b)
  const count = bytes.readUInt16LE(pe + 6)
  const sections = optional + bytes.readUInt16LE(pe + 20)
  /** @param {number} rva */
  const offset = rva => {
    for (let i = 0; i < count; i++) {
      const section = sections + i * 40,
        address = bytes.readUInt32LE(section + 12)
      const size = Math.max(
        bytes.readUInt32LE(section + 8),
        bytes.readUInt32LE(section + 16)
      )
      if (rva >= address && rva < address + size)
        return bytes.readUInt32LE(section + 20) + rva - address
    }
    throw new Error('Invalid PE import RVA')
  }
  let descriptor = offset(bytes.readUInt32LE(optional + 120))
  while (bytes.readUInt32LE(descriptor + 12)) {
    const start = offset(bytes.readUInt32LE(descriptor + 12))
    const end = bytes.indexOf(0, start)
    assert.ok(end > start && end < start + 256)
    const dll = bytes.subarray(start, end).toString('ascii')
    assert.ok(!/^(?:vcruntime|msvcp)\d/i.test(dll), `${name}: ${dll}`)
    descriptor += 20
  }
}
const profile = join(scratch, 'profile')
const env = {
  SystemRoot: process.env.SystemRoot,
  PATH: join(scratch, 'no-programs'),
  LOCALAPPDATA: scratch,
}
/** @param {string[]} args */
const run = args => processResult(exe, args, scratch, env)
/**
 * @param {Awaited<ReturnType<typeof run>>} result
 * @param {string} [diagnostics]
 */
const success = (result, diagnostics) => {
  assert.equal(result.code, 0, diagnostics ?? result.stdout + result.stderr)
  return /** @type {{command:{supportsColdStart:boolean},data:{instanceId:string,mode:string,value:{result:string}},success:boolean,runId:string}} */ (
    JSON.parse(result.stdout)
  )
}
// No Desktop, source checkout or system Node/Bun is reachable through cwd/PATH.
assert.equal(
  success(
    await run(['describe', 'plugin-base64-encoder', 'run', '--format', 'json'])
  ).command.supportsColdStart,
  true
)
let policy = { formatVersion: 1, coldStart: false, grants: [] }
const policyPath = join(scratch, 'policy.json')
await writeFile(policyPath, JSON.stringify(policy))
success(await run(['init', '--profile', profile, '--policy', policyPath]))
const offlineStatus = await run(['runtime', 'status', '--profile', profile])
assert.equal(offlineStatus.code, 1)
assert.deepEqual(JSON.parse(offlineStatus.stdout).error, {
  code: 'RUNTIME_DISCONNECTED',
  phase: 'connect',
  stage: 'pipe',
})
let denied = await run(['runtime', 'start', '--profile', profile])
assert.equal(denied.code, 1)
assert.equal(JSON.parse(denied.stdout).error.code, 'COLD_START_DENIED')
const catalog = /** @type {{plugins:{id:string}[]}} */ (
  JSON.parse(
    await readFile(
      join(bundle, 'plugins/.generated/builtin-manifests.json'),
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
policy = {
  formatVersion: 1,
  coldStart: true,
  grants: [
    {
      pluginId: 'plugin-base64-encoder',
      commandId: 'run',
      target: 'cli',
      packageDigest: createHash('sha256')
        .update(
          JSON.stringify(
            canonical(
              catalog.plugins.find(p => p.id === 'plugin-base64-encoder')
            )
          )
        )
        .digest('hex'),
      effects: [],
      scopes: [],
      expiresAt: Date.now() + 3600000,
      maxCalls: 64,
      coldStart: true,
      background: true,
    },
  ],
}
await writeFile(policyPath, JSON.stringify(policy))
success(
  await run([
    'permissions',
    'grant',
    '--profile',
    profile,
    '--policy',
    policyPath,
  ])
)
let running = false
try {
  running = true
  // Launch all fifty independent native launchers before awaiting any result.
  const fifty = await Promise.all(
    Array.from({ length: 50 }, (_, index) => {
      const startedAt = performance.now()
      return run(['runtime', 'start', '--profile', profile]).then(result => ({
        ...result,
        index,
        elapsedMs: Math.round(performance.now() - startedAt),
      }))
    })
  )
  const diagnostics = JSON.stringify(
    fifty.map(result => {
      let errorCode = null,
        phase = null,
        stage = null
      try {
        const value =
          /** @type {{error?:{code?:unknown,phase?:unknown,stage?:unknown}}} */ (
            JSON.parse(result.stdout)
          )
        if (
          typeof value?.error?.code === 'string' &&
          /^[A-Z][A-Z_]{0,63}$/.test(value.error.code)
        )
          errorCode = value.error.code
        if (
          typeof value?.error?.phase === 'string' &&
          ['connect', 'query', 'reconnect'].includes(value.error.phase)
        )
          phase = value.error.phase
        if (
          typeof value?.error?.stage === 'string' &&
          ['pipe', 'authenticate'].includes(value.error.stage)
        )
          stage = value.error.stage
      } catch {
        // Malformed output adds no untrusted text to the aggregate diagnostic.
      }
      return {
        index: result.index,
        code: result.code,
        errorCode,
        phase,
        stage,
        elapsedMs: result.elapsedMs,
      }
    })
  )
  const statuses = fifty.map(result => success(result, diagnostics))
  assert.equal(statuses.length, 50, diagnostics)
  assert.equal(
    new Set(statuses.map(result => result.data.instanceId)).size,
    1,
    diagnostics
  )
  assert.ok(
    statuses.every(result => result.data.mode === 'managed'),
    diagnostics
  )
  const json = success(
    await run([
      'run',
      'plugin-base64-encoder',
      '--profile',
      profile,
      '--format',
      'json',
      '--text',
      'standalone-canary',
    ])
  )
  assert.equal(json.success, true)
  assert.ok(json.runId)
  assert.equal(
    json.data.value.result,
    Buffer.from('standalone-canary').toString('base64')
  )
  const text = await run([
    'run',
    'plugin-base64-encoder',
    '--profile',
    profile,
    '--text',
    'standalone-canary',
  ])
  assert.equal(text.code, 0, text.stdout + text.stderr)
  assert.equal(
    text.stdout.trim(),
    Buffer.from('standalone-canary').toString('base64')
  )
  const noGrant = await run([
    'run',
    'plugin-todo-list',
    '--profile',
    profile,
    '--input',
    '{"todo":"must-not-write"}',
  ])
  assert.equal(noGrant.code, 1)
  assert.equal(JSON.parse(noGrant.stdout).error.code, 'APPROVAL_REQUIRED')
  const invalid = await run([
    'run',
    'plugin-base64-encoder',
    '--profile',
    profile,
    '--input',
    '{"text":3}',
  ])
  assert.equal(invalid.code, 1)
  assert.equal(JSON.parse(invalid.stdout).error.code, 'INPUT_INVALID')
  success(await run(['runtime', 'stop', '--profile', profile]))
  running = false
  await new Promise(resolve => setTimeout(resolve, 250))
  const cli = join(bundle, 'packages/cli/dist/cli.mjs')
  const savedCli = join(scratch, 'cli.saved')
  await copyFile(cli, savedCli)
  await writeFile(cli, 'throw new Error("tamper")')
  denied = await run(['runtime', 'start', '--profile', profile])
  assert.equal(denied.code, 1)
  assert.equal(JSON.parse(denied.stdout).error.code, 'BUNDLE_INTEGRITY_FAILED')
  await copyFile(savedCli, cli)
  const runtime = join(bundle, 'native/flowtools-runtime.exe')
  await rename(runtime, runtime + '.missing')
  denied = await run(['runtime', 'start', '--profile', profile])
  assert.equal(denied.code, 1)
  await rename(runtime + '.missing', runtime)
  const runner = join(bundle, 'packages/plugin-runner/dist/runner.js')
  const savedRunner = join(scratch, 'runner.saved')
  await copyFile(runner, savedRunner)
  await writeFile(runner, 'throw new Error("tamper")')
  denied = await run(['runtime', 'start', '--profile', profile])
  assert.equal(denied.code, 1)
  assert.equal(JSON.parse(denied.stdout).error.code, 'BUNDLE_INTEGRITY_FAILED')
  assert.equal((await run(['runtime', 'status', '--profile', profile])).code, 1)
  await copyFile(savedRunner, runner)
  // A fresh redirected fixture avoids renaming a directory whose executables
  // were just mapped by Windows/AV. The same pinned launcher must reject it.
  const redirected = join(scratch, 'redirected')
  await cp(bundle, redirected, {
    recursive: true,
    filter: source => source !== join(bundle, 'native'),
  })
  const native = join(redirected, 'native'),
    nativeReal = join(bundle, 'native')
  assert.ok(
    native.startsWith(scratch + '\\') && nativeReal.startsWith(scratch + '\\')
  )
  const junction = spawnSync(
    join(process.env.SystemRoot ?? 'C:/Windows', 'System32/cmd.exe'),
    ['/d', '/c', 'mklink', '/J', native, nativeReal],
    { encoding: 'utf8', windowsHide: true, timeout: 5000 }
  )
  assert.equal(junction.status, 0, junction.stderr)
  assert.equal((await lstat(native)).isSymbolicLink(), true)
  denied = await processResult(
    join(redirected, 'flowtools.exe'),
    ['runtime', 'start', '--profile', profile],
    scratch,
    env
  )
  assert.equal(denied.code, 1)
  assert.equal(JSON.parse(denied.stdout).error.code, 'BUNDLE_INVALID')
  await rmdir(native)
} finally {
  if (running) await run(['runtime', 'stop', '--profile', profile])
}
process.stdout.write('Standalone fifty-start fixture passed\n')
