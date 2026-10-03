import { expect, test } from 'bun:test'
import { resolve } from 'node:path'

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
