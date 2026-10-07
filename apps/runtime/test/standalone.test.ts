import { expect, test } from 'bun:test'
import { fileURLToPath } from 'node:url'

test('relocated GUI-free bundle rejects tampering and fifty concurrent cold starts share one Host', async () => {
  const fixture = Bun.spawn(
    [
      'node',
      fileURLToPath(new URL('./standalone-fixture.mjs', import.meta.url)),
    ],
    { stdout: 'pipe', stderr: 'pipe' }
  )
  const [stdout, stderr, code] = await Promise.all([
    new Response(fixture.stdout).text(),
    new Response(fixture.stderr).text(),
    fixture.exited,
  ])
  expect({ code, stderr }).toEqual({ code: 0, stderr: '' })
  expect(stdout.trim()).toBe('Standalone fifty-start fixture passed')
}, 300000)
