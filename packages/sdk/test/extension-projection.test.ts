import type { PluginManifestEntry } from '../src/registry/types'
import type { FlowToolPlugin } from '../src/types/plugin'

import { expect, test } from 'bun:test'

import {
  ExtensionContributionError,
  ExtensionContributionRegistry,
  projectPluginContributions,
} from '../src/extensions'
import { PluginLoader } from '../src/registry/plugin-loader'
import { PluginRegistry } from '../src/registry/plugin-registry'

const document = (value: unknown = 'original') => ({
  formatVersion: 1,
  contributions: [{ id: 'main', kind: 'theme', value }],
})

function fixture(id = 'fixture') {
  const registry = new PluginRegistry()
  const contributions = new ExtensionContributionRegistry()
  const loader = new PluginLoader(registry)
  let loads = 0
  let runs = 0
  let reads = 0
  let declarations: unknown = document()
  const plugin: FlowToolPlugin = {
    type: 'tool',
    meta: { id, name: 'Fixture', version: '1.0.0' },
    run: () => {
      runs++
    },
  }
  const manifest: PluginManifestEntry = {
    id,
    name: 'Fixture',
    type: 'tool',
    version: '1.0.0',
    loader: async () => {
      loads++
      return { default: plugin }
    },
  }
  registry.register(manifest)
  const dispose = projectPluginContributions(registry, contributions, () => {
    reads++
    return declarations
  })
  return {
    registry,
    contributions,
    loader,
    manifest,
    plugin,
    dispose,
    calls: () => ({ loads, runs, reads }),
    setDocument: (value: unknown) => {
      declarations = value
    },
  }
}

test('inspection does not load or execute; contributions follow enable/disable/unload', async () => {
  const state = fixture()
  expect(state.calls()).toEqual({ loads: 0, runs: 0, reads: 0 })
  expect(state.contributions.getSnapshot()).toHaveLength(0)
  await state.loader.load('fixture')
  expect(state.calls()).toEqual({ loads: 1, runs: 0, reads: 0 })
  expect(state.contributions.getSnapshot()).toHaveLength(0)
  await state.loader.enable('fixture')
  const generation = state.registry.get('fixture')!.generation
  expect(state.contributions.getSnapshot()[0]!.owner).toEqual({
    pluginId: 'fixture',
    version: '1.0.0',
    generation,
  })
  await state.loader.enable('fixture')
  expect(state.calls().reads).toBe(1)
  await state.loader.disable('fixture')
  expect(state.contributions.getSnapshot()).toHaveLength(0)
  await state.loader.enable('fixture')
  expect(state.contributions.getSnapshot()).toHaveLength(1)
  expect(state.registry.get('fixture')!.generation).toBe(generation)
  await state.loader.unload('fixture')
  expect(state.contributions.getSnapshot()).toHaveLength(0)
  expect(state.calls().runs).toBe(0)
  state.dispose()
})

test('reload/update replace owner generations and uninstall revokes only its plugin', async () => {
  const state = fixture()
  await state.loader.enable('fixture')
  const original = state.contributions.getSnapshot()[0]!.owner
  await state.loader.reload('fixture')
  expect(
    state.contributions.getSnapshot()[0]!.owner.generation
  ).toBeGreaterThan(original.generation)
  state.setDocument(document('updated'))
  await state.loader.update('fixture', {
    ...state.manifest,
    version: '2.0.0',
    loader: async () => ({
      default: {
        ...state.plugin,
        meta: { ...state.plugin.meta, version: '2.0.0' },
      },
    }),
  })
  expect(state.contributions.getSnapshot()[0]!.owner.version).toBe('2.0.0')
  expect(state.contributions.getSnapshot()[0]!.value).toBe('updated')
  state.contributions.replace(
    { pluginId: 'other-plugin', version: '1.0.0', generation: 1 },
    document('other')
  )
  expect(state.contributions.revoke(original)).toBe(false)
  await state.loader.uninstall('fixture')
  expect(state.contributions.getSnapshot().map(item => item.key)).toEqual([
    'other-plugin:main',
  ])
  state.dispose()
  expect(state.contributions.getSnapshot()).toHaveLength(1)
})

test('dependency refusal never reads or publishes declarations', async () => {
  const state = fixture()
  await state.loader.update('fixture', {
    ...state.manifest,
    dependenciesSatisfied: false,
  })
  expect(
    await state.loader.enable('fixture').catch((error: unknown) => error)
  ).toBeInstanceOf(Error)
  expect(state.registry.get('fixture')!.state).toBe('error')
  expect(state.calls()).toEqual({ loads: 1, runs: 0, reads: 0 })
  expect(state.contributions.getSnapshot()).toHaveLength(0)
  state.dispose()
})

test('invalid declarations fail activation atomically and allow explicit recovery', async () => {
  const state = fixture()
  state.setDocument({
    formatVersion: 1,
    contributions: [
      { id: 'valid', kind: 'theme', value: {} },
      { id: 'invalid', kind: 'locale' },
    ],
  })
  expect(
    await state.loader.enable('fixture').catch((error: unknown) => error)
  ).toBeInstanceOf(ExtensionContributionError)
  expect(state.registry.get('fixture')!.state).toBe('error')
  expect(state.contributions.getSnapshot()).toHaveLength(0)
  state.setDocument(document('recovered'))
  await state.loader.reload('fixture')
  expect(state.contributions.getSnapshot()[0]!.value).toBe('recovered')
  state.registry.markError('fixture', new Error('host failure'))
  expect(state.contributions.getSnapshot()).toHaveLength(0)
  state.dispose()
})

test('dispose removes its own subscriptions and preserves later replacement leases', async () => {
  const state = fixture()
  await state.loader.enable('fixture')
  const binding = state.contributions.getSnapshot()[0]!.owner
  state.contributions.replace(binding, document('replacement'))
  state.dispose()
  state.dispose()
  expect(state.contributions.getSnapshot()[0]!.value).toBe('replacement')
  await state.loader.reload('fixture')
  expect(state.calls().reads).toBe(1)
  expect(state.contributions.getSnapshot()[0]!.value).toBe('replacement')
})

test('already enabled plugins project immediately; absent documents contribute nothing', async () => {
  const state = fixture()
  state.dispose()
  await state.loader.enable('fixture')
  const first = projectPluginContributions(
    state.registry,
    state.contributions,
    () => undefined
  )
  expect(state.contributions.getSnapshot()).toHaveLength(0)
  first()
  const second = projectPluginContributions(
    state.registry,
    state.contributions,
    () => document('present')
  )
  expect(state.contributions.getSnapshot()[0]!.value).toBe('present')
  second()
  expect(state.contributions.getSnapshot()).toHaveLength(0)
})

test('failed initial projection releases earlier contributions and unsubscribes', async () => {
  const registry = new PluginRegistry()
  const loader = new PluginLoader(registry)
  const contributions = new ExtensionContributionRegistry()
  for (const id of ['first', 'second']) {
    registry.register({
      id,
      name: id,
      version: '1.0.0',
      type: 'tool',
      loader: async () => ({
        default: {
          type: 'tool',
          meta: { id, name: id, version: '1.0.0' },
          run: () => undefined,
        },
      }),
    })
    await loader.enable(id)
  }
  let reads = 0
  expect(() =>
    projectPluginContributions(registry, contributions, entry => {
      reads++
      return entry.id === 'first' ? document() : { formatVersion: 2 }
    })
  ).toThrow(ExtensionContributionError)
  expect(reads).toBe(2)
  expect(contributions.getSnapshot()).toHaveLength(0)
  await loader.reload('first')
  expect(reads).toBe(2)
})

test('reinstall and new projector lifetimes do not reuse retired contribution epochs', async () => {
  const state = fixture()
  await state.loader.enable('fixture')
  await state.loader.reload('fixture')
  const retired = state.contributions.getSnapshot()[0]!.owner
  await state.loader.uninstall('fixture')
  state.registry.register(state.manifest)
  await state.loader.enable('fixture')
  expect(state.registry.get('fixture')!.generation).toBe(1)
  const reinstalled = state.contributions.getSnapshot()[0]!.owner
  expect(reinstalled.generation).toBeGreaterThan(retired.generation)
  state.dispose()
  const newProjector = projectPluginContributions(
    state.registry,
    state.contributions,
    () => document('new-projector')
  )
  expect(
    state.contributions.getSnapshot()[0]!.owner.generation
  ).toBeGreaterThan(reinstalled.generation)
  expect(state.contributions.revoke(retired)).toBe(false)
  newProjector()
})

test('synchronous disable during publication cannot leave stale contributions', async () => {
  const state = fixture()
  let disabled = false
  state.contributions.subscribe(() => {
    if (!disabled && state.contributions.getSnapshot().length) {
      disabled = true
      state.registry.transition('fixture', 'disabled')
    }
  })
  await state.loader.enable('fixture')
  expect(disabled).toBe(true)
  expect(state.registry.get('fixture')!.state).toBe('disabled')
  expect(state.contributions.getSnapshot()).toHaveLength(0)
  state.dispose()
})

test('source entry is rechecked after declaration callbacks mutate lifecycle state', async () => {
  const state = fixture()
  state.dispose()
  const dispose = projectPluginContributions(
    state.registry,
    state.contributions,
    () => {
      state.registry.markError('fixture', new Error('retired while reading'))
      return document('stale')
    }
  )
  await state.loader.enable('fixture')
  expect(state.registry.get('fixture')!.state).toBe('error')
  expect(state.contributions.getSnapshot()).toHaveLength(0)
  dispose()
})
