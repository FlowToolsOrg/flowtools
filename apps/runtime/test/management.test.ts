import { expect, test } from 'bun:test'
import { createHash } from 'node:crypto'
import { mkdtemp, readFile, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'

test('compiled CLI initializes without GUI, requires interaction or explicit policy, and preserves revocation', async () => {
  const root = resolve(import.meta.dir, '../../..')
  const profile = await mkdtemp(
    join(tmpdir(), 'flowtools-validation-management-')
  )
  const policy = join(
    profile,
    '../flowtools-policy-' + crypto.randomUUID() + '.json'
  )
  await writeFile(
    policy,
    JSON.stringify({ formatVersion: 1, coldStart: false, grants: [] })
  )
  const cli = join(root, 'packages/cli/dist/cli.mjs')
  const run = async (args: string[]) => {
    const child = Bun.spawn([process.execPath, cli, ...args], {
      stdout: 'pipe',
      stderr: 'pipe',
      env: { SystemRoot: process.env.SystemRoot, PATH: process.env.PATH },
    })
    const [stdout, stderr, code] = await Promise.all([
      new Response(child.stdout).text(),
      new Response(child.stderr).text(),
      child.exited,
    ])
    return {
      code,
      stderr,
      value: JSON.parse(stdout) as {
        success: boolean
        error?: { code: string }
        data?: unknown
      },
    }
  }
  expect(
    (await run(['init', '--interactive', '--profile', profile])).value.error
      ?.code
  ).toBe('INTERACTION_REQUIRED')
  const initialized = await run([
    'init',
    '--policy',
    policy,
    '--profile',
    profile,
  ])
  expect(initialized).toMatchObject({
    code: 0,
    stderr: '',
    value: { success: true },
  })
  const bootstrapBefore = await readFile(
    join(profile, 'bootstrap.json'),
    'utf8'
  )
  const listed = await run(['permissions', 'list', '--profile', profile])
  expect(listed.code).toBe(0)
  expect(listed.value.data).toEqual([{ type: 'permissions', data: [] }])
  const catalog = JSON.parse(
    await readFile(
      join(root, 'packages/runtime-core/.generated/catalog.json'),
      'utf8'
    )
  ) as { plugins: Record<string, unknown>[] }
  const manifest = catalog.plugins.find(
    plugin => plugin.id === 'plugin-base64-encoder'
  )!
  const canonical = (value: unknown): unknown =>
    Array.isArray(value)
      ? value.map(canonical)
      : value && typeof value === 'object'
        ? Object.fromEntries(
            Object.entries(value)
              .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
              .map(([key, item]) => [key, canonical(item)])
          )
        : value
  const packageDigest = createHash('sha256')
    .update(JSON.stringify(canonical(manifest)))
    .digest('hex')
  await writeFile(
    policy,
    JSON.stringify({
      formatVersion: 1,
      coldStart: false,
      grants: [
        {
          pluginId: 'plugin-base64-encoder',
          commandId: 'run',
          target: 'cli',
          packageDigest,
          effects: [],
          scopes: [],
          expiresAt: Date.now() + 3600000,
          maxCalls: 16,
          coldStart: false,
          background: false,
        },
      ],
    })
  )
  expect(
    (
      await run([
        'permissions',
        'grant',
        '--policy',
        policy,
        '--profile',
        profile,
      ])
    ).value
  ).toMatchObject({
    success: true,
    data: [
      { type: 'permissions', data: [{ epoch: 1, grant: { packageDigest } }] },
    ],
  })
  expect(
    (
      await run([
        'permissions',
        'revoke',
        'plugin-base64-encoder',
        '--profile',
        profile,
      ])
    ).value.success
  ).toBe(true)
  expect(
    (await run(['permissions', 'list', '--profile', profile])).value
  ).toMatchObject({
    data: [{ type: 'permissions', data: [{ epoch: 2, grant: null }] }],
  })
  expect(await readFile(join(profile, 'bootstrap.json'), 'utf8')).toBe(
    bootstrapBefore
  )
  const cold = Bun.spawn(
    [
      resolve(
        process.env.CARGO_TARGET_DIR ?? join(root, 'target'),
        'debug/flowtools-runtime.exe'
      ),
      '--profile',
      profile,
      '--cold-start',
    ],
    {
      stdout: 'pipe',
      stderr: 'pipe',
      env: { SystemRoot: process.env.SystemRoot },
    }
  )
  expect(await new Response(cold.stderr).text()).toBe('COLD_START_DENIED\n')
  expect(await cold.exited).toBe(1)
}, 60000)
