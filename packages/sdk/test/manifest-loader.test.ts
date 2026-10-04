import { expect, test } from 'bun:test'

import {
  loadManifestModule,
  equalJsonValues,
  parseManifestCatalog,
} from '../src/manifest'
import { PluginLoader } from '../src/registry/plugin-loader'
import { PluginRegistry } from '../src/registry/plugin-registry'

import { manifestFixture, manifestTarget } from './fixtures/manifest-v1'

function uiFixture() {
  const manifest = manifestFixture()
  manifest.entries.ui = 'dist/ui.js'
  manifest.files.push({ path: 'dist/ui.js', size: 0, sha256: '0'.repeat(64) })
  return manifest
}

test('host catalog rejects unknown root fields, versions, duplicate and extra identities', () => {
  const manifest = uiFixture()
  const value = { formatVersion: 1, plugins: [manifest] }
  expect(
    parseManifestCatalog(value, manifestTarget, [manifest.id])
  ).toHaveLength(1)
  for (const invalid of [
    { ...value, unknownCritical: true },
    { ...value, formatVersion: 2 },
    { ...value, plugins: [manifest, manifest] },
  ])
    expect(() =>
      parseManifestCatalog(invalid, manifestTarget, [manifest.id])
    ).toThrow()
  expect(() =>
    parseManifestCatalog(value, manifestTarget, ['other-plugin'])
  ).toThrow()
})

async function expectRejected(promise: Promise<unknown>, message?: string) {
  const error: unknown = await promise.then(
    () => null,
    (error: unknown) => error
  )
  expect(error).toBeInstanceOf(Error)
  if (message)
    expect(error instanceof Error ? error.message : '').toContain(message)
}

test('runtime command equality ignores object order and rejects lossy/changed values', () => {
  expect(equalJsonValues({ a: 1, b: [false] }, { b: [false], a: 1 })).toBe(true)
  expect(equalJsonValues({ a: 1, b: [false] }, { b: [true], a: 1 })).toBe(false)
  expect(equalJsonValues({ a: undefined }, {})).toBe(false)
})

test('host-bound UI validates its pure manifest before importing code', async () => {
  const manifest = uiFixture()
  let loads = 0
  const module = {
    default: {
      type: manifest.type,
      meta: {
        id: manifest.id,
        version: manifest.version,
        maturity: manifest.maturity,
      },
      command: manifest.commands[0],
    },
  }
  expect(
    await loadManifestModule(manifest, manifestTarget, manifest, async () => {
      loads++
      return module
    })
  ).toBe(module)
  expect(loads).toBe(1)
})

for (const invalid of ['field', 'identity', 'target', 'schema', 'ui-entry']) {
  test(`invalid UI manifest ${invalid} refuses before loader/lifecycle and loaded state`, async () => {
    const manifest = uiFixture()
    const expected = structuredClone(manifest)
    if (invalid === 'field') Object.assign(manifest, { unknownCritical: true })
    if (invalid === 'identity') manifest.id = 'impostor-plugin'
    if (invalid === 'target')
      manifest.targets = [{ platform: 'linux', arch: 'x64' }]
    if (invalid === 'schema')
      Object.assign(manifest.commands[0]!.inputSchema, { $ref: '#/unsafe' })
    if (invalid === 'ui-entry') delete manifest.entries.ui
    let loads = 0,
      hooks = 0
    const registry = new PluginRegistry()
    registry.register({
      id: expected.id,
      name: expected.name,
      version: expected.version,
      type: 'tool',
      loader: () =>
        loadManifestModule(manifest, manifestTarget, expected, async () => {
          loads++
          return {
            default: {
              type: 'tool' as const,
              meta: {
                id: expected.id,
                name: expected.name,
                version: expected.version,
              },
              run: () => {},
              lifecycle: {
                onLoad: () => {
                  hooks++
                },
              },
            },
          }
        }),
    })
    await expectRejected(new PluginLoader(registry).load(expected.id))
    expect(loads).toBe(0)
    expect(hooks).toBe(0)
    expect(registry.get(expected.id)?.state).toBe('error')
    expect(registry.get(expected.id)?.plugin).toBeUndefined()
  })
}

test('UI metadata and adapted command mismatches never return a loaded module', async () => {
  const manifest = uiFixture()
  await expectRejected(
    loadManifestModule(manifest, manifestTarget, manifest, async () => ({
      default: {
        type: 'tool' as const,
        meta: { id: 'impostor', version: manifest.version },
      },
    })),
    'identity'
  )
  await expectRejected(
    loadManifestModule(manifest, manifestTarget, manifest, async () => ({
      default: {
        type: 'tool' as const,
        meta: { id: manifest.id, version: manifest.version },
        command: { id: 'wrong' },
      },
    })),
    'contract'
  )
})
