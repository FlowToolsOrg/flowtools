import { expect, test } from 'bun:test'
import { fileURLToPath } from 'node:url'
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
