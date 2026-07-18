import type {
  PluginManifestEntry,
  PluginRegistryEvent,
} from '../src/registry/types'
import type { PluginLifecycle, FlowToolPlugin } from '../src/types/plugin'

import { describe, expect, test } from 'bun:test'

import { PluginLoader } from '../src/registry/plugin-loader'
import { PluginRegistry } from '../src/registry/plugin-registry'

async function getError(promise: Promise<unknown>): Promise<Error> {
  try {
    await promise
  } catch (error) {
    return error instanceof Error ? error : new Error(String(error))
  }

  throw new Error('Expected promise to reject')
}

function createPlugin(id: string, lifecycle?: PluginLifecycle): FlowToolPlugin {
  return {
    type: 'tool',
    meta: { id, name: id, version: '1.0.0' },
    lifecycle,
    run: () => undefined,
  }
}

function createManifest(
  id: string,
  loader: PluginManifestEntry['loader']
): PluginManifestEntry {
  return {
    id,
    name: id,
    version: '1.0.0',
    type: 'tool',
    loader,
  }
}

describe('PluginLoader loading', () => {
  test('rejects an unregistered plugin', async () => {
    const loader = new PluginLoader(new PluginRegistry())

    expect((await getError(loader.load('missing-plugin'))).message).toContain(
      'Plugin "missing-plugin" is not registered'
    )
  })

  test('marks and rethrows loader failures', async () => {
    const registry = new PluginRegistry()
    const error = new Error('loader failed')
    registry.register(
      createManifest('broken-plugin', async () => {
        throw error
      })
    )
    const loader = new PluginLoader(registry)

    expect(await getError(loader.load('broken-plugin'))).toBe(error)

    expect(registry.get('broken-plugin')?.state).toBe('error')
    expect(registry.get('broken-plugin')?.error).toBe(error)
  })

  test('rejects an invalid default export', async () => {
    const registry = new PluginRegistry()
    registry.register(
      createManifest('invalid-plugin', async () => ({
        default: {} as FlowToolPlugin,
      }))
    )
    const loader = new PluginLoader(registry)

    expect((await getError(loader.load('invalid-plugin'))).message).toContain(
      'invalid default export'
    )
    expect(registry.get('invalid-plugin')?.state).toBe('error')
  })

  test('isolates failures while loading all plugins', async () => {
    const registry = new PluginRegistry()
    const error = new Error('one loader failed')
    registry.register(
      createManifest('healthy-plugin', async () => ({
        default: createPlugin('healthy-plugin'),
      }))
    )
    registry.register(
      createManifest('broken-plugin', async () => {
        throw error
      })
    )
    const loader = new PluginLoader(registry)

    await loader.loadAll()

    expect(registry.get('healthy-plugin')?.state).toBe('loaded')
    expect(registry.get('broken-plugin')?.state).toBe('error')
    expect(registry.get('broken-plugin')?.error).toBe(error)
  })
})

describe('PluginLoader activation', () => {
  test('enables and disables idempotently', async () => {
    const registry = new PluginRegistry()
    const events: PluginRegistryEvent[] = []
    registry.subscribe(event => events.push(event))
    let loaderCalls = 0
    let activateCalls = 0
    let deactivateCalls = 0
    const plugin = createPlugin('lifecycle-plugin', {
      onActivate: () => {
        activateCalls += 1
      },
      onDeactivate: () => {
        deactivateCalls += 1
      },
    })
    registry.register(
      createManifest('lifecycle-plugin', async () => {
        loaderCalls += 1
        return { default: plugin }
      })
    )
    const loader = new PluginLoader(registry)

    await loader.enable('lifecycle-plugin')
    await loader.enable('lifecycle-plugin')
    expect(registry.get('lifecycle-plugin')?.state).toBe('enabled')
    expect(loaderCalls).toBe(1)
    expect(activateCalls).toBe(1)

    await loader.disable('lifecycle-plugin')
    await loader.disable('lifecycle-plugin')
    expect(registry.get('lifecycle-plugin')?.state).toBe('disabled')
    expect(deactivateCalls).toBe(1)
    expect(events.map(event => event.type)).toEqual([
      'registered',
      'loading',
      'loaded',
      'enabled',
      'disabled',
    ])
  })

  test('preserves and propagates activation failures', async () => {
    const registry = new PluginRegistry()
    const error = new Error('activation failed')
    registry.register(
      createManifest('activation-plugin', async () => ({
        default: createPlugin('activation-plugin', {
          onActivate: () => {
            throw error
          },
        }),
      }))
    )
    const loader = new PluginLoader(registry)

    expect(await getError(loader.enable('activation-plugin'))).toBe(error)

    const entry = registry.get('activation-plugin')
    expect(entry?.state).toBe('error')
    expect(entry?.error).toBe(error)
    expect(entry?.enabledAt).toBeUndefined()
  })

  test('preserves and propagates deactivation failures', async () => {
    const registry = new PluginRegistry()
    const error = new Error('deactivation failed')
    registry.register(
      createManifest('deactivation-plugin', async () => ({
        default: createPlugin('deactivation-plugin', {
          onDeactivate: () => {
            throw error
          },
        }),
      }))
    )
    const loader = new PluginLoader(registry)
    await loader.enable('deactivation-plugin')

    expect(await getError(loader.disable('deactivation-plugin'))).toBe(error)

    const entry = registry.get('deactivation-plugin')
    expect(entry?.state).toBe('error')
    expect(entry?.error).toBe(error)
  })
})
