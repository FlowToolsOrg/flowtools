import type {
  JobSnapshot,
  RunDiagnostic,
  StorageReport,
  PermissionRecord,
} from '@flowtools/runtime-client'

import assert from 'node:assert/strict'
import { execFile, spawn } from 'node:child_process'
import { createHash } from 'node:crypto'
import { once } from 'node:events'
import { mkdir, mkdtemp, readFile, writeFile, symlink } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { promisify } from 'node:util'

const root = resolve(import.meta.dirname, '../../..')
const scratch = await mkdtemp(
  join(tmpdir(), 'flowtools-validation-recovery-cli-')
)
const profile = join(scratch, 'profile')
const cli = join(root, 'packages/cli/dist/cli.mjs')
const catalog = JSON.parse(
  await readFile(
    join(root, 'packages/runtime-core/.generated/catalog.json'),
    'utf8'
  )
) as { plugins: { id: string }[] }
const canonical = (value: unknown): unknown =>
  Array.isArray(value)
    ? value.map(canonical)
    : value && typeof value === 'object'
      ? Object.fromEntries(
          Object.entries(value)
            .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
            .map(([key, value]) => [key, canonical(value)])
        )
      : value
const policy = join(scratch, 'policy.json')
await writeFile(
  policy,
  JSON.stringify({
    formatVersion: 1,
    coldStart: true,
    grants: ['plugin-todo-list', 'plugin-base64-encoder'].map(pluginId => ({
      pluginId,
      commandId: 'run',
      target: 'cli',
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
    })),
  })
)
async function invoke<T>(args: string[], expectedCode?: string): Promise<T> {
  let stdout: string
  try {
    const output = await promisify(execFile)(
      process.execPath,
      [cli, ...args, '--profile', profile],
      {
        windowsHide: true,
        timeout: 45000,
        maxBuffer: 1048576,
        env: { SystemRoot: process.env.SystemRoot },
      }
    )
    assert.equal(output.stderr, '')
    stdout = output.stdout
  } catch (error) {
    const failure = error as { stdout: string; code: number }
    assert.equal(failure.code, 1)
    const value = JSON.parse(failure.stdout) as {
      success: boolean
      error: { code: string }
    }
    assert.equal(value.success, false)
    assert.equal(value.error.code, expectedCode)
    return value as T
  }
  assert.equal(expectedCode, undefined)
  return JSON.parse(stdout) as T
}
let active = false
let locker: ReturnType<typeof spawn> | undefined
try {
  await invoke(['init', '--policy', policy])
  await invoke(['runtime', 'start'])
  active = true
  const success = await invoke<{ data: { runId?: string }; runId: string }>([
    'run',
    'plugin-base64-encoder',
    '--input',
    '{"text":"PRIVATE_DIAGNOSTIC_CANARY"}',
  ])
  const runId = success.runId
  const diagnostic = (
    await invoke<{ data: RunDiagnostic }>(['jobs', 'diagnose', runId])
  ).data
  assert.equal(diagnostic.state, 'succeeded')
  assert.equal(diagnostic.resultExpired, false)
  const exported = JSON.stringify(diagnostic)
  for (const secret of [
    'PRIVATE_DIAGNOSTIC_CANARY',
    'UFJJVkFURV9ESUFHTk9TVElDX0NBTkFSWQ==',
    profile,
    'bootstrap',
    'inputSummary',
    'resources',
    'token',
    '"result":',
  ])
    assert.ok(!exported.includes(secret), secret)
  await invoke(['run', 'plugin-todo-list', '--input', '{"todo":"BACKUP_ROW"}'])
  await invoke(['runtime', 'storage', 'create'], 'STORE_BUSY')
  await invoke(['runtime', 'stop'])
  active = false
  const snapshot = (
    await invoke<{ data: StorageReport }>(['runtime', 'storage', 'create'])
  ).data
  const backupId = snapshot.backups[0]!.id
  const outside = join(scratch, 'outside-fixture')
  await mkdir(outside)
  const redirected = crypto.randomUUID()
  await symlink(
    outside,
    join(profile, `runtime-backup-${redirected}.sqlite`),
    'junction'
  )
  await invoke(
    ['runtime', 'storage', 'restore', redirected, '--confirm'],
    'STORAGE_FAILED'
  )
  await invoke(['runtime', 'start'])
  active = true
  await invoke([
    'run',
    'plugin-todo-list',
    '--input',
    '{"todo":"POST_BACKUP_ROW"}',
  ])
  await invoke(['runtime', 'stop'])
  active = false
  await invoke(['runtime', 'storage', 'restore', backupId], 'APPROVAL_REQUIRED')
  await invoke(
    ['runtime', 'storage', 'restore', '../escape', '--confirm'],
    'INVALID_REQUEST'
  )
  // Windows file sharing permits reads but denies replacement/deletion.
  await writeFile(
    join(profile, 'runtime.sqlite'),
    Buffer.from('CORRUPT_ORIGINAL_CANARY')
  )
  const file = join(profile, 'runtime.sqlite').replaceAll("'", "''")
  locker = spawn(
    'pwsh',
    [
      '-NoProfile',
      '-Command',
      `$fixtureLock=[System.IO.File]::Open('${file}',[System.IO.FileMode]::Open,[System.IO.FileAccess]::Read,[System.IO.FileShare]::ReadWrite);[Console]::WriteLine('locked');[Console]::ReadLine() | Out-Null;$fixtureLock.Dispose()`,
    ],
    { windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] }
  )
  await once(locker.stdout!, 'data')
  await invoke(
    ['runtime', 'storage', 'restore', backupId, '--confirm'],
    'STORAGE_FAILED'
  )
  assert.equal(
    await readFile(join(profile, 'runtime.sqlite'), 'utf8'),
    'CORRUPT_ORIGINAL_CANARY'
  )
  const pending = (
    await invoke<{ data: StorageReport }>(['runtime', 'storage', 'list'])
  ).data.recovery!
  assert.equal(pending.phase, 'prepared')
  await invoke(['permissions', 'list'], 'RECOVERY_PENDING')
  const unlocked = once(locker, 'exit')
  locker.stdin!.end('\n')
  await unlocked
  locker = undefined
  const recovered = (
    await invoke<{ data: StorageReport }>([
      'runtime',
      'storage',
      'retry',
      '--confirm',
    ])
  ).data.recovery!
  assert.equal(recovered.recoveryId, pending.recoveryId)
  assert.equal(recovered.phase, 'complete')
  assert.equal(recovered.grantsRevoked, true)
  assert.equal(recovered.payloadsQuarantined, true)
  assert.equal(
    await readFile(
      join(profile, `runtime-corrupt-${recovered.recoveryId}.sqlite`),
      'utf8'
    ),
    'CORRUPT_ORIGINAL_CANARY'
  )
  const permissions = (
    await invoke<{ data: { data: PermissionRecord[] }[] }>([
      'permissions',
      'list',
    ])
  ).data[0]!.data
  assert.ok(
    permissions.length > 0 && permissions.every(record => record.grant === null)
  )
  await invoke(['runtime', 'start'], 'COLD_START_DENIED')
  await invoke(['permissions', 'grant', '--policy', policy])
  await invoke(['runtime', 'start'])
  active = true
  const restored = (
    await invoke<{ data: RunDiagnostic }>(['jobs', 'diagnose', runId])
  ).data
  assert.equal(restored.state, 'succeeded')
  assert.equal(restored.resultExpired, true)
  await invoke(['jobs', 'status', runId], 'RESULT_EXPIRED')
  await invoke(
    ['jobs', 'lookup', 'unknown-post-backup-key'],
    'ACCEPTANCE_UNKNOWN'
  )
  const todos = await invoke<{
    data: { value: { result: string[]; count: number } }
  }>(['run', 'plugin-todo-list', '--input', '{}'])
  assert.deepEqual(todos.data.value, {
    result: ['1. [No Deadline] BACKUP_ROW'],
    count: 1,
  })
  const newResult = await invoke<{
    success: boolean
    data: { value: { result: string } }
    runId: string
  }>(['run', 'plugin-base64-encoder', '--input', '{"text":"new"}'])
  assert.equal(newResult.success, true)
  assert.equal(newResult.data.value.result, 'bmV3')
  assert.equal(
    (await invoke<{ data: JobSnapshot }>(['jobs', 'status', newResult.runId]))
      .data.state,
    'succeeded'
  )
  assert.equal(
    (
      await invoke<{ data: RunDiagnostic }>([
        'jobs',
        'diagnose',
        newResult.runId,
      ])
    ).data.resultExpired,
    false
  )
  process.stdout.write(
    'Native recovery and private diagnostics fixture passed\n'
  )
} finally {
  if (locker) {
    locker.stdin!.end('\n')
    await once(locker, 'exit')
  }
  if (active) await invoke(['runtime', 'stop'])
}
