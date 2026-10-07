import { expect, test } from 'bun:test'
import { fileURLToPath } from 'node:url'

test('compiled CLI diagnoses actual runs and retries real Windows blocked recovery without replay or grants', async () => {
  const fixture = Bun.spawn(
    ['node', fileURLToPath(new URL('./recovery-fixture.ts', import.meta.url))],
    { stdout: 'pipe', stderr: 'pipe' }
  )
  const [stdout, stderr, code] = await Promise.all([
    new Response(fixture.stdout).text(),
    new Response(fixture.stderr).text(),
    fixture.exited,
  ])
  expect({ code, stderr }).toEqual({ code: 0, stderr: '' })
  expect(stdout.trim()).toBe(
    'Native recovery and private diagnostics fixture passed'
  )
}, 60000)

test('actual managed Host durably executes Todo and recovers lost ACK, crash and revoke', async () => {
  const fixture = Bun.spawn(
    ['node', fileURLToPath(new URL('./jobs-fixture.mjs', import.meta.url))],
    { stdout: 'pipe', stderr: 'pipe' }
  )
  const [stdout, stderr, code] = await Promise.all([
    new Response(fixture.stdout).text(),
    new Response(fixture.stderr).text(),
    fixture.exited,
  ])
  expect({ code, stderr }).toEqual({ code: 0, stderr: '' })
  expect(stdout.trim()).toBe('Durable jobs fixture passed')
}, 30000)
test('actual Node clients and native Host share real T1 tasks and cancellation', async () => {
  // Bun's Windows net.Socket missed a response after switching pipes. Exercise the
  // supported Node transport in a real Node process, with every assertion intact.
  const fixture = Bun.spawn(
    [
      'node',
      fileURLToPath(
        new URL(
          '../../../packages/runtime-client/test/native-fixture.mjs',
          import.meta.url
        )
      ),
    ],
    {
      stdout: 'pipe',
      stderr: 'pipe',
    }
  )
  const [stdout, stderr, code] = await Promise.all([
    new Response(fixture.stdout).text(),
    new Response(fixture.stderr).text(),
    fixture.exited,
  ])
  expect({ code, stderr }).toEqual({ code: 0, stderr: '' })
  expect(stdout.trim()).toBe('Native fixture passed')
}, 30000)

test('actual GUI and CLI Host clients share durable revisions and reject unauthorized data', async () => {
  const fixture = Bun.spawn(
    [
      'node',
      fileURLToPath(
        new URL(
          '../../../packages/runtime-client/test/data-fixture.mjs',
          import.meta.url
        )
      ),
    ],
    {
      stdout: 'pipe',
      stderr: 'pipe',
    }
  )
  const [stdout, stderr, code] = await Promise.all([
    new Response(fixture.stdout).text(),
    new Response(fixture.stderr).text(),
    fixture.exited,
  ])
  expect({ code, stderr }).toEqual({ code: 0, stderr: '' })
  expect(stdout.trim()).toBe('Shared data fixture passed')
}, 30000)

test('compiled CLI shares receipts, metadata, terminal formats and revocation with native Host', async () => {
  const fixture = Bun.spawn(
    [
      'node',
      fileURLToPath(new URL('./job-control-fixture.mjs', import.meta.url)),
    ],
    { stdout: 'pipe', stderr: 'pipe' }
  )
  const [stdout, stderr, code] = await Promise.all([
    new Response(fixture.stdout).text(),
    new Response(fixture.stderr).text(),
    fixture.exited,
  ])
  expect({ code, stderr }).toEqual({ code: 0, stderr: '' })
  expect(stdout.trim()).toBe('Compiled shared job control fixture passed')
}, 60000)
