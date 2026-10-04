import { expect, test } from 'bun:test'
import { resolve } from 'node:path'

test('Web, Desktop and CLI inventories exactly match sorted built-in metadata', async () => {
  const child = Bun.spawn(
    [
      process.execPath,
      resolve(import.meta.dir, 'generate-manifests.ts'),
      '--check',
    ],
    { cwd: resolve(import.meta.dir, '..'), stdout: 'pipe', stderr: 'pipe' }
  )
  const [stdout, stderr, code] = await Promise.all([
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
    child.exited,
  ])
  expect(stderr).toBe('')
  expect(stdout).toBe('')
  expect(code).toBe(0)
}, 30_000)
