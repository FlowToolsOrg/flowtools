import { expect, test } from 'bun:test'
import { fileURLToPath } from 'node:url'

test('native scheduler executes real T1 commands with no connected client and refuses revoked/cancelled schedules', async () => {
  const fixture = Bun.spawn(
    [
      'node',
      fileURLToPath(new URL('./scheduling-fixture.mjs', import.meta.url)),
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
  expect(stdout.trim()).toBe(
    'Native scheduling executes the actual plugin without clients and refuses revoke/cancel replay'
  )
}, 90_000)
