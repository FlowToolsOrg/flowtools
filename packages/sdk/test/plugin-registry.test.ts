import type {
  PluginManifestEntry,
  PluginRegistryEvent,
  PluginState,
} from '../src/registry/types'
import type { FlowToolPlugin } from '../src/types/plugin'

import { describe, expect, test } from 'bun:test'

import { PluginRegistry } from '../src/registry/plugin-registry'

function createManifest(
  id: string,
  type: PluginManifestEntry['type'] = 'tool'
): PluginManifestEntry {
  const plugin: FlowToolPlugin = {
    type: 'tool',
    meta: { id, name: id, version: '1.0.0' },
    run: () => undefined,
  }

  return {
    id,
    name: id,
    version: '1.0.0',
    type,
    loader: async () => ({ default: plugin }),
  }
}

describe('PluginRegistry registration', () => {
  test('registers a duplicate id only once without replacing its manifest', () => {
    const registry = new PluginRegistry()
    const events: PluginRegistryEvent[] = []
    const first = createManifest('same-plugin')
    const duplicate = { ...first, name: 'Replacement' }
    registry.subscribe(event => events.push(event))

    registry.register(first)
    registry.register(duplicate)

    expect(registry.size).toBe(1)
    expect(registry.get('same-plugin')?.manifest).toBe(first)
    expect(events).toEqual([{ type: 'registered', pluginId: 'same-plugin' }])
  })

  test('stops delivering events after unsubscribe', () => {
    const registry = new PluginRegistry()
    const events: PluginRegistryEvent[] = []
    const unsubscribe = registry.subscribe(event => events.push(event))

    registry.register(createManifest('first-plugin'))
    unsubscribe()
    registry.register(createManifest('second-plugin'))

    expect(events.map(event => event.pluginId)).toEqual(['first-plugin'])
  })

  test('filters plugins by type and enabled state', () => {
    const registry = new PluginRegistry()
    registry.register(createManifest('app-plugin', 'app'))
    registry.register(createManifest('tool-plugin', 'tool'))
    registry.transition('tool-plugin', 'loading')
    registry.transition('tool-plugin', 'loaded')
    registry.transition('tool-plugin', 'enabled')

    expect(registry.getByType('app').map(plugin => plugin.id)).toEqual([
      'app-plugin',
    ])
    expect(registry.getByType('tool').map(plugin => plugin.id)).toEqual([
      'tool-plugin',
    ])
    expect(registry.getEnabled().map(plugin => plugin.id)).toEqual([
      'tool-plugin',
    ])
  })
})

describe('PluginRegistry state events', () => {
  test('emits every supported non-error transition once', () => {
    const registry = new PluginRegistry()
    registry.register(createManifest('stateful-plugin'))
    const events: PluginRegistryEvent[] = []
    registry.subscribe(event => events.push(event))

    const states: PluginState[] = [
      'loading',
      'loaded',
      'enabled',
      'disabled',
      'registered',
    ]
    for (const state of states) {
      registry.transition('stateful-plugin', state)
    }
    registry.transition('stateful-plugin', 'registered')

    expect(events.map(event => event.type)).toEqual(states)
    expect(registry.get('stateful-plugin')?.state).toBe('registered')
  })

  test('stores and emits the original error', () => {
    const registry = new PluginRegistry()
    registry.register(createManifest('broken-plugin'))
    const events: PluginRegistryEvent[] = []
    const error = new Error('broken')
    registry.subscribe(event => events.push(event))

    registry.markError('broken-plugin', error)

    const entry = registry.get('broken-plugin')
    expect(entry?.state).toBe('error')
    expect(entry?.error).toBe(error)
    expect(events).toEqual([
      { type: 'error', pluginId: 'broken-plugin', error },
    ])
  })

  test('ignores every mutation for an unknown id', () => {
    const registry = new PluginRegistry()
    const events: PluginRegistryEvent[] = []
    registry.subscribe(event => events.push(event))

    registry.updateState('missing-plugin', { loadedAt: 0 })
    registry.transition('missing-plugin', 'enabled')
    registry.markError('missing-plugin', new Error('missing'))
    registry.unregister('missing-plugin')

    expect(registry.size).toBe(0)
    expect(events).toEqual([])
  })
})
