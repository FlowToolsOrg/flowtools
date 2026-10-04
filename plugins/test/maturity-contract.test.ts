import { expect, test } from 'bun:test'
import { resolve } from 'node:path'

import { loadPlugin, scanPlugins } from '@flowtools/cli'
import { z } from 'zod'

import { builtInManifests as desktop } from '../../apps/desktop/src/plugin/manifests'
import { builtInManifests as web } from '../../apps/web-vite/src/plugin/manifests'

test('built-in metadata, both manifests and CLI all declare only prototype maturity', async () => {
  expect(web).toHaveLength(12)
  expect(desktop.map(({ loader: _loader, ...meta }) => meta)).toEqual(
    web.map(({ loader: _loader, ...meta }) => meta)
  )
  for (const cli of scanPlugins()) {
    const plugin = await loadPlugin(cli.id)
    expect(plugin?.meta.maturity).toBe('prototype')
    expect(web.find(manifest => manifest.id === cli.id)?.maturity).toBe(
      'prototype'
    )
    expect(cli.maturity).toBe('prototype')
  }
}, 30_000)

test('compiled CLI list/info report maturity in JSON and text', async () => {
  const run = async (args: string[]) => {
    const child = Bun.spawn(
      [
        process.execPath,
        resolve(import.meta.dir, '../../packages/cli/dist/cli.mjs'),
        ...args,
      ],
      { stdout: 'pipe', stderr: 'pipe' }
    )
    const [stdout, stderr, code] = await Promise.all([
      new Response(child.stdout).text(),
      new Response(child.stderr).text(),
      child.exited,
    ])
    expect(code).toBe(0)
    expect(stderr).toBe('')
    return stdout
  }
  const listed = z
    .array(z.object({ id: z.string(), maturity: z.literal('prototype') }))
    .parse(JSON.parse(await run(['list', '--format', 'json'])))
  expect(listed).toHaveLength(12)
  expect(await run(['list', '--format', 'text'])).toContain('[prototype]')
  expect(
    JSON.parse(await run(['info', 'plugin-base64-encoder', '--format', 'json']))
  ).toMatchObject({ maturity: 'prototype' })
  expect(
    await run(['info', 'plugin-base64-encoder', '--format', 'text'])
  ).toContain('Maturity: prototype')
}, 30_000)
