import { expect, test } from 'bun:test'
import { fileURLToPath } from 'node:url'
test('actual resource reader shares GUI/CLI binding, refuses role and restart reuse, and keeps history private', async () => {
  const child = Bun.spawn(
    [
      'node',
      fileURLToPath(new URL('./resources-fixture.mjs', import.meta.url)),
    ],
    { stdout: 'pipe', stderr: 'pipe' }
  )
  const [stdout, stderr, code] = await Promise.all([
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
    child.exited,
  ])
  expect({ code, stderr }).toEqual({ code: 0, stderr: '' })
  expect(stdout.trim()).toBe(
    'NA1 actual CLI and Desktop protocol resource selection/read and restart refusal passed'
  )
}, 90000)
