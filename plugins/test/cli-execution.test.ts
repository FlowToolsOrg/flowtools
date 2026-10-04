import { expect, test } from 'bun:test'
import { resolve } from 'node:path'

import {
  validateOperationValue,
  type CommandManifestV1,
} from '@flowtools/sdk/manifest'

async function invoke(args: string[]) {
  const child = Bun.spawn(
    [
      process.execPath,
      resolve(import.meta.dir, '../../packages/cli/dist/cli.mjs'),
      ...args,
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
