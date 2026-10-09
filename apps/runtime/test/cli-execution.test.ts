import type { DependencyPlan } from '@flowtools/runtime-client'

import { beforeAll, afterAll, expect, test } from 'bun:test'
import { createHash } from 'node:crypto'
import { mkdtemp, readFile, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { resolve } from 'node:path'

import {
  validateOperationValue,
  type CommandManifestV1,
} from '@flowtools/sdk/manifest'

const scratch = await mkdtemp(
  join(tmpdir(), 'flowtools-validation-cli-contract-')
)
const profile = join(scratch, 'profile')
const root = resolve(import.meta.dir, '../../..')
function canonical(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(canonical)
  if (value && typeof value === 'object')
    return Object.fromEntries(
      Object.entries(value)
        .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
        .map(([key, item]) => [key, canonical(item)])
    )
  return value
}
beforeAll(async () => {
  const catalog = JSON.parse(
    await readFile(
      join(root, 'packages/runtime-core/.generated/catalog.json'),
      'utf8'
    )
  ) as { plugins: { id: string }[] }
  const policy = join(scratch, 'policy.json')
  const grants = ['plugin-base64-encoder', 'plugin-random-picker'].map(
    pluginId => ({
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
      effects: [],
      scopes: [],
      expiresAt: Date.now() + 3600000,
      maxCalls: 32,
      coldStart: true,
      background: false,
    })
  )
  await writeFile(
    policy,
    JSON.stringify({ formatVersion: 1, coldStart: true, grants })
  )
  const output = await invoke([
    'init',
    '--policy',
    policy,
    '--profile',
    profile,
  ])
  expect(output).toMatchObject({ code: 0, stderr: '' })
}, 30000)
afterAll(async () => {
  const output = await invoke(['runtime', 'stop', '--profile', profile])
  expect(output).toMatchObject({ code: 0, stderr: '' })
}, 30000)
async function invoke(args: string[]) {
  const child = Bun.spawn(
    [
      'node',
      join(root, 'packages/cli/dist/cli.mjs'),
      ...args,
      ...(args[0] === 'run' ? ['--profile', profile] : []),
    ],
    {
      stdout: 'pipe',
      stderr: 'pipe',
    }
  )
  const [stdout, stderr, code] = await Promise.all([
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
    child.exited,
  ])
  return { stdout, stderr, code }
}

test('compiled CLI reads deterministic dependency plans from the actual Host without jobs or grants', async () => {
  expect((await invoke(['runtime', 'start', '--profile', profile])).code).toBe(
    0
  )
  const permissions = await invoke([
    'permissions',
    'list',
    '--profile',
    profile,
  ])
  const jobs = await invoke(['jobs', 'list', '--profile', profile])
  const args = [
    'dependencies',
    'plan',
    'plugin-base64-encoder',
    '--profile',
    profile,
    '--format',
    'json',
  ]
  const first = await invoke(args)
  const second = await invoke(args)
  expect(first).toMatchObject({ code: 0, stderr: '' })
  expect(second.stdout).toBe(first.stdout)
  const plan = JSON.parse(first.stdout) as {
    success: true
    data: DependencyPlan
  }
  expect(plan).toMatchObject({
    success: true,
    data: {
      formatVersion: 1,
      mode: 'plan-only',
      lock: {
        roots: [{ publisher: 'flowtools', id: 'plugin-base64-encoder' }],
      },
    },
  })
  expect(plan.data.lock.digest).toMatch(/^[a-f0-9]{64}$/)
  expect(plan.data.lock.packages).toHaveLength(1)
  expect(
    (await invoke(['permissions', 'list', '--profile', profile])).stdout
  ).toBe(permissions.stdout)
  expect((await invoke(['jobs', 'list', '--profile', profile])).stdout).toBe(
    jobs.stdout
  )
  const text = await invoke([
    'dependencies',
    'plan',
    'plugin-base64-encoder',
    '--profile',
    profile,
  ])
  expect(text).toMatchObject({ code: 0, stderr: '' })
  expect(text.stdout).toContain(plan.data.lock.digest)
  expect(text.stdout).toContain('plan-only')
  expect(text.stdout).toContain('does not install, activate or grant')
  const unload = await invoke([
    'dependencies',
    'unload-plan',
    'plugin-base64-encoder',
    '--profile',
    profile,
    '--format',
    'json',
  ])
  expect(unload).toMatchObject({ code: 0, stderr: '' })
  expect(JSON.parse(unload.stdout)).toMatchObject({
    success: true,
    data: {
      mode: 'plan-only',
      provider: { id: 'plugin-base64-encoder' },
      replacement: null,
      consumers: [],
    },
  })
  expect((await invoke(['jobs', 'list', '--profile', profile])).stdout).toBe(
    jobs.stdout
  )
  expect(
    (await invoke(['permissions', 'list', '--profile', profile])).stdout
  ).toBe(permissions.stdout)
}, 30_000)

test('real built-in CLI run produces the SDK envelope from generated flags', async () => {
  const output = await invoke([
    'run',
    'plugin-base64-encoder',
    '--text',
    'hello',
    '--format',
    'json',
  ])
  expect(output.code).toBe(0)
  expect(output.stderr).toBe('')
  expect(JSON.parse(output.stdout)).toMatchObject({
    success: true,
    pluginId: 'plugin-base64-encoder',
    pluginVersion: '0.1.0',
    data: { type: 'json', value: { result: 'aGVsbG8=' } },
  })
  const jobs = await invoke(['jobs', 'list', '--profile', profile])
  const rows = (
    JSON.parse(jobs.stdout) as { data: { runId: string; pluginId: string }[] }
  ).data
  const run = rows.find(job => job.pluginId === 'plugin-base64-encoder')!
  const calls = await invoke([
    'dependencies',
    'calls',
    run.runId,
    '--profile',
    profile,
    '--format',
    'json',
  ])
  expect(calls).toMatchObject({ code: 0, stderr: '' })
  expect(JSON.parse(calls.stdout)).toMatchObject({ success: true, data: [] })
  expect((await invoke(['jobs', 'list', '--profile', profile])).stdout).toBe(
    jobs.stdout
  )
}, 30_000)

test('real built-in CLI schema error is failed JSON with nonzero status', async () => {
  const output = await invoke([
    'run',
    'plugin-base64-encoder',
    '--input',
    '{"text":3}',
    '--format',
    'json',
  ])
  expect(output.code).toBe(1)
  expect(JSON.parse(output.stdout)).toMatchObject({
    success: false,
    error: { code: 'INPUT_INVALID' },
  })
}, 30_000)

test('real built-in CLI rejects invalid timeout instead of silently defaulting', async () => {
  const output = await invoke([
    'run',
    'plugin-base64-encoder',
    '--text',
    'hello',
    '--timeout',
    'NaN',
    '--format',
    'json',
  ])
  expect(output.code).toBe(1)
  expect(JSON.parse(output.stdout)).toMatchObject({
    success: false,
    error: { code: 'TIMEOUT_INVALID' },
  })
}, 30_000)

test('real built-in CLI text output stays human-readable', async () => {
  const output = await invoke([
    'run',
    'plugin-base64-encoder',
    '--text',
    'hello',
    '--format',
    'text',
  ])
  expect(output.code).toBe(0)
  expect(output.stdout.trim()).toBe('aGVsbG8=')
}, 30_000)

test('an agent can prepare a real run from describe alone and validate its output', async () => {
  const described = await invoke([
    'describe',
    'plugin-base64-encoder',
    'run',
    '--format',
    'json',
  ])
  expect(described.code).toBe(0)
  const contract = JSON.parse(described.stdout) as {
    pluginId: string
    command: CommandManifestV1
    example: { text: string }
  }
  expect(contract.command.inputSchema.properties?.text).toMatchObject({
    type: 'string',
  })
  const output = await invoke([
    'run',
    contract.pluginId,
    '--command',
    contract.command.id,
    '--input',
    JSON.stringify(contract.example),
  ])
  expect(output.code).toBe(0)
  const result = JSON.parse(output.stdout) as {
    success: boolean
    data: unknown
  }
  expect(result.success).toBe(true)
  expect(result.data).toMatchObject({
    value: { result: Buffer.from(contract.example.text).toString('base64') },
  })
  expect(
    validateOperationValue(contract.command.outputSchema, result.data).success
  ).toBe(true)
}, 30_000)

test('real compiled CLI batch keeps input order and the shared result envelopes', async () => {
  const output = await invoke([
    'run',
    'plugin-base64-encoder',
    '--batch-input',
    '[{"text":"hello"},{"text":"world"}]',
  ])
  expect(output.code).toBe(0)
  const batch = JSON.parse(output.stdout) as { results: unknown[] }
  expect(batch).toMatchObject({
    formatVersion: 1,
    type: 'batch',
    identity: 'flowtools/plugin-base64-encoder/run',
    success: true,
  })
  expect(batch.results).toMatchObject([
    { success: true, data: { value: { result: 'aGVsbG8=' } } },
    { success: true, data: { value: { result: 'd29ybGQ=' } } },
  ])
}, 30_000)

test('real compiled CLI negative number is a numeric command input', async () => {
  const output = await invoke([
    'run',
    'plugin-random-picker',
    '--names',
    'only-fixture',
    '--count',
    '-2',
    '--format',
    'json',
  ])
  expect(output.code).toBe(0)
  expect(JSON.parse(output.stdout)).toMatchObject({
    success: true,
    data: { value: { result: [], total: 1 } },
  })
}, 30_000)

test('compiled describe rejects unknown operation identities', async () => {
  const output = await invoke([
    'describe',
    'plugin-base64-encoder',
    '../other',
    '--format',
    'json',
  ])
  expect(output.code).toBe(1)
  expect(JSON.parse(output.stdout)).toMatchObject({
    error: { code: 'NOT_RUNNABLE' },
  })
}, 30_000)
