import { expect, test } from 'bun:test'
import { fileURLToPath } from 'node:url'
test('actual CLI/native workflows retain one file/media plan across consent and restart', async () => {
  const fixture = Bun.spawn(
    [
      'node',
      fileURLToPath(new URL('./g7-workflows-fixture.mjs', import.meta.url)),
    ],
    { stdout: 'pipe', stderr: 'pipe' }
  )
  const [stdout, stderr, code] = await Promise.all([
    new Response(fixture.stdout).text(),
    new Response(fixture.stderr).text(),
    fixture.exited,
  ])
  expect({ code, stderr }).toEqual({ code: 0, stderr: '' })
  expect(stdout.trim()).toBe(
    'G7 actual CLI and native Desktop protocol share file plans, consent, commit/undo, signed media publication and restart-safe receipts'
  )
}, 90000)
