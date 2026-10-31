import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import { once } from 'node:events'
import { mkdtemp, readFile, realpath, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { dirname, basename, join, resolve } from 'node:path'
import { createInterface } from 'node:readline'

import { RuntimeClient } from '@flowtools/runtime-client'
import { connectNamedPipe } from '@flowtools/runtime-client/node'

const root = resolve(import.meta.dirname, '../../..')
const scratch = await mkdtemp(
  join(tmpdir(), 'flowtools-validation-scheduling-')
)
const profile = join(scratch, 'profile')
const executable = join(
  process.env.CARGO_TARGET_DIR ?? join(root, 'target'),
  'debug/flowtools-runtime.exe'
)
const pause = (/** @type {number} */ milliseconds) =>
  new Promise(resolve => setTimeout(resolve, milliseconds))
/** @type {import('node:child_process').ChildProcessWithoutNullStreams | undefined} */
let child
/** @type {Promise<unknown> | undefined} */
let exited
/** @type {RuntimeClient | undefined} */
let manager
/** @type {RuntimeClient | undefined} */
let cli
/** @type {{cliToken:string,managementToken:string}} */
let bootstrap
let timedOut = false
const watchdog = setTimeout(() => {
  timedOut = true
  cli?.close()
  manager?.close()
  if (child && child.exitCode === null) {
    child.stdin.end()
    child.kill()
  }
}, 75_000)
async function start(/** @type {boolean} */ initialize) {
  child = spawn(
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
  exited = once(child, 'exit')
  const lines = createInterface({ input: child.stdout })
  let failure = ''
  child.stderr.on('data', bytes => {
    if (failure.length < 256) failure += bytes.toString()
  })
  const timer = setTimeout(() => child?.kill(), 15_000)
  try {
    const line = await Promise.race([
      once(lines, 'line').then(([line]) => String(line)),
      exited.then(() => {
        throw new Error(
          /^[A-Z_]+\s*$/.test(failure) ? failure.trim() : 'STARTUP_FAILED'
        )
      }),
    ])
    const ready = /** @type {unknown} */ (JSON.parse(line))
    assert.ok(
      ready &&
        typeof ready === 'object' &&
        'type' in ready &&
        ready.type === 'ready' &&
        'pipe' in ready &&
        typeof ready.pipe === 'string'
    )
    const stored = /** @type {unknown} */ (
      JSON.parse(await readFile(join(profile, 'bootstrap.json'), 'utf8'))
    )
    assert.ok(
      stored &&
        typeof stored === 'object' &&
        'formatVersion' in stored &&
        stored.formatVersion === 1 &&
        'cliToken' in stored &&
        typeof stored.cliToken === 'string' &&
        /^[a-f0-9]{64}$/.test(stored.cliToken) &&
        'managementToken' in stored &&
        typeof stored.managementToken === 'string' &&
        /^[a-f0-9]{64}$/.test(stored.managementToken)
    )
    bootstrap = {
      cliToken: stored.cliToken,
      managementToken: stored.managementToken,
    }
    return ready.pipe
  } finally {
    clearTimeout(timer)
    lines.close()
  }
}
async function connect(
  /** @type {string} */ pipe,
  /** @type {boolean} */ management
) {
  const client = new RuntimeClient(await connectNamedPipe(pipe))
  await client.connect(
    management ? bootstrap.managementToken : bootstrap.cliToken
  )
  return client
}
try {
  let pipe = await start(true)
  manager = await connect(pipe, true)
  const plugins = await manager.call({ method: 'plugins.list' })
  assert.equal(plugins.type, 'plugins')
  if (plugins.type !== 'plugins') throw new Error('INVALID_RESPONSE')
  const hash = plugins.data.find(
    item => item.pluginId === 'plugin-base64-encoder'
  )?.packageDigest
  assert.ok(hash)
  /** @type {import('@flowtools/runtime-client').PermissionGrant} */
  const grant = {
    pluginId: 'plugin-base64-encoder',
    commandId: 'run',
    target: 'cli',
    packageDigest: hash,
    effects: [],
    scopes: [],
    expiresAt: Date.now() + 180_000,
    maxCalls: 8,
    coldStart: false,
    background: true,
  }
  await manager.call({ method: 'permissions.grant', payload: grant })
  manager.close()
  manager = undefined
  child?.stdin.end()
  await exited
  pipe = await start(false)
  manager = await connect(pipe, true)
  cli = await connect(pipe, false)
  /** @returns {import('@flowtools/runtime-client').CreateSchedule} */
  const operation = () => ({
    requestKey: crypto.randomUUID(),
    target: 'cli',
    pluginId: 'plugin-base64-encoder',
    commandId: 'run',
    input: { text: 'SCHEDULE_REAL_PLUGIN_E2E' },
    timeoutMs: 30_000,
    definition: {
      firstDueMs: Date.now() + 1500,
      intervalMs: null,
      occurrences: 1,
      graceMs: 30_000,
      missed: 'coalesce-latest',
    },
  })
  await assert.rejects(cli.createSchedule(operation()), {
    code: 'SESSION_INVALID',
  })
  const create = operation()
  const schedule = await manager.createSchedule(create)
  const secret = Buffer.from('SCHEDULE_REAL_PLUGIN_E2E')
  assert.ok(!(await readFile(join(profile, 'runtime.sqlite'))).includes(secret))
  assert.ok(
    !(
      await readFile(join(profile, 'private-jobs', `${schedule.id}.input.bin`))
    ).includes(secret)
  )
  assert.equal((await manager.createSchedule(create)).id, schedule.id)
  manager.close()
  manager = undefined
  cli.close()
  cli = undefined
  // The Host has no connected GUI or CLI when the schedule becomes due.
  await pause(2500)
  cli = await connect(pipe, false)
  const deadline = Date.now() + 35_000
  let scheduled
  while (Date.now() < deadline) {
    scheduled = (await cli.schedules()).find(item => item.id === schedule.id)
    if (scheduled?.lastClaim?.result.state === 'submitted') break
    await pause(100)
  }
  assert.equal(scheduled?.lastClaim?.result.state, 'submitted')
  if (scheduled?.lastClaim?.result.state !== 'submitted')
    throw new Error('SCHEDULE_NOT_SUBMITTED')
  const run = scheduled.lastClaim.result.runId
  const completed = await cli.waitForResult(run)
  assert.equal(completed.state, 'succeeded')
  assert.equal(completed.rootCaller, 'local-cli')
  assert.ok(
    JSON.stringify(completed.result).includes(
      Buffer.from('SCHEDULE_REAL_PLUGIN_E2E').toString('base64')
    )
  )
  await pause(1100)
  assert.equal((await cli.jobs()).length, 1)
  manager = await connect(pipe, true)
  const revoked = await manager.createSchedule({
    ...operation(),
    definition: { ...create.definition, firstDueMs: Date.now() + 2500 },
  })
  await manager.call({
    method: 'permissions.revoke',
    payload: { pluginId: grant.pluginId, commandId: 'run', target: 'cli' },
  })
  await pause(4000)
  const after = (await manager.schedules()).find(item => item.id === revoked.id)
  assert.equal(after?.cancelled, true)
  assert.equal(after?.lastClaim?.result.state, 'rejected')
  assert.equal((await cli.jobs()).length, 1)
  await manager.call({ method: 'permissions.grant', payload: grant })
  const cancelled = await manager.createSchedule({
    ...operation(),
    definition: { ...create.definition, firstDueMs: Date.now() + 1500 },
  })
  const replacement = {
    previous: { id: cancelled.id, revision: cancelled.revision },
    create: {
      ...operation(),
      definition: { ...create.definition, firstDueMs: Date.now() + 60_000 },
    },
  }
  await assert.rejects(cli.replaceSchedule(replacement), {
    code: 'SESSION_INVALID',
  })
  const edited = await manager.replaceSchedule(replacement)
  assert.equal((await manager.replaceSchedule(replacement)).id, edited.id)
  assert.equal(
    (await manager.schedules()).find(item => item.id === cancelled.id)
      ?.cancelled,
    true
  )
  await assert.rejects(
    manager.replaceSchedule({
      ...replacement,
      create: { ...replacement.create, requestKey: crypto.randomUUID() },
    }),
    { code: 'REVISION_CONFLICT' }
  )
  await manager.cancelSchedule(edited.id, edited.revision)
  await pause(2600)
  assert.equal((await cli.jobs()).length, 1)
  const reminderDigest = plugins.data.find(
    item => item.pluginId === 'plugin-local-reminder'
  )?.packageDigest
  assert.ok(reminderDigest)
  await manager.call({
    method: 'permissions.grant',
    payload: {
      ...grant,
      pluginId: 'plugin-local-reminder',
      packageDigest: reminderDigest,
      effects: ['notification'],
      scopes: [{ kind: 'notification' }],
    },
  })
  const reminder = await manager.createSchedule({
    ...operation(),
    pluginId: 'plugin-local-reminder',
    input: {
      title: 'FlowTools 自动验证',
      message: '无界面 Runtime 提醒发送测试。',
    },
    definition: { ...create.definition, firstDueMs: Date.now() + 1500 },
  })
  manager.close()
  manager = undefined
  cli.close()
  cli = undefined
  await pause(2500)
  cli = await connect(pipe, false)
  const reminderRecord = (await cli.schedules()).find(
    item => item.id === reminder.id
  )
  assert.equal(reminderRecord?.lastClaim?.result.state, 'submitted')
  if (reminderRecord?.lastClaim?.result.state !== 'submitted')
    throw new Error('REMINDER_NOT_SUBMITTED')
  const notification = await cli.waitForResult(
    reminderRecord.lastClaim.result.runId
  )
  assert.equal(notification.state, 'succeeded')
  assert.ok(
    (await readFile(join(profile, 'runtime.sqlite'))).includes(
      Buffer.from('notification-attempts-v1')
    ),
    'The actual broker/native adapter must persist its attempt; no missing-capability fallback'
  )
  assert.match(
    JSON.stringify(notification.result),
    /"status":"(?:requested|failed|unsupported|unknown)"/
  )
  process.stdout.write(
    'Reminder OS request status: ' +
      JSON.stringify(notification.result).match(
        /"status":"(requested|failed|unsupported|unknown)"/
      )?.[1] +
      '\n'
  )
  assert.ok(
    !(await readFile(join(profile, 'runtime.sqlite'))).includes(
      Buffer.from('无界面 Runtime 提醒发送测试。')
    )
  )
  await pause(1100)
  assert.equal((await cli.jobs()).length, 2)
  manager = await connect(pipe, true)
  await manager.call({ method: 'runtime.stop' })
  manager.close()
  manager = undefined
  cli.close()
  cli = undefined
  child?.stdin.end()
  await exited
  assert.equal(timedOut, false)
  process.stdout.write(
    'Native scheduling executes the actual plugin without clients and refuses revoke/cancel replay\n'
  )
} finally {
  clearTimeout(watchdog)
  cli?.close()
  manager?.close()
  if (child && child.exitCode === null) {
    child.stdin.end()
    child.kill()
    await exited
  }
  const owned = await realpath(scratch)
  assert.equal(
    dirname(owned).toLowerCase(),
    (await realpath(tmpdir())).toLowerCase()
  )
  assert.ok(basename(owned).startsWith('flowtools-validation-scheduling-'))
  await rm(owned, { recursive: true, force: true })
}
