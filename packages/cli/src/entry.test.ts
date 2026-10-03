import { expect, test } from 'bun:test'
import { resolve } from 'node:path'

async function invoke(args: string[]) {
  const process_ = Bun.spawn(
    [process.execPath, resolve(import.meta.dir, 'cli.ts'), ...args],
    {
      stdout: 'pipe',
      stderr: 'pipe',
    }
  )
  const [stdout, stderr, code] = await Promise.all([
    new Response(process_.stdout).text(),
    new Response(process_.stderr).text(),
    process_.exited,
  ])
  return { stdout, stderr, code }
}

test('CLI missing plugin emits JSON failure and exits nonzero', async () => {
  const output = await invoke([
    'run',
    'missing-plugin-fixture',
    '--format',
    'json',
  ])
  expect(output.code).toBe(1)
  expect(JSON.parse(output.stdout)).toMatchObject({
    success: false,
    pluginId: 'missing-plugin-fixture',
    error: { code: 'PLUGIN_NOT_FOUND' },
  })
  expect(output.stderr).toContain('[PLUGIN_NOT_FOUND]')
}, 30_000)

test('CLI malformed JSON emits INPUT_INVALID without leaking raw input', async () => {
  const output = await invoke([
    'run',
    'plugin-base64-encoder',
    '--input',
    '{secret-token',
    '--format',
    'json',
  ])
  expect(output.code).toBe(1)
  expect(JSON.parse(output.stdout)).toMatchObject({
    success: false,
    error: { code: 'INPUT_INVALID' },
  })
  expect(output.stdout + output.stderr).not.toContain('secret-token')
}, 30_000)

test('CLI JSON serialization failure is explicit and cannot print success', async () => {
  const runnerUrl = new URL('./runner.ts', import.meta.url).href
  const source = `import { printExecutionResult } from ${JSON.stringify(runnerUrl)};
    const data = {}; data.circular = data;
    process.exitCode = printExecutionResult({success: true, data,
      pluginId:'serialize-fixture',pluginVersion:'1.0.0',startedAt:1,finishedAt:2,
      durationMs:1,inputSummary:{kind:'object',size:0}},'json');`
  const child = Bun.spawn([process.execPath, '--eval', source], {
    stdout: 'pipe',
    stderr: 'pipe',
  })
  const [stdout, code] = await Promise.all([
    new Response(child.stdout).text(),
    child.exited,
  ])
  expect(code).toBe(1)
  expect(JSON.parse(stdout)).toMatchObject({
    success: false,
    error: { code: 'OUTPUT_INVALID' },
  })
}, 30_000)
