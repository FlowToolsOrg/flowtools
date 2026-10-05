import type { PluginState } from '../src/registry/types'
import type { PluginLifecycle } from '../src/types/plugin'

import { describe, expect, test } from 'bun:test'

import { PluginLifecycleManager } from '../src/registry/lifecycle-manager'
import { PluginLoader } from '../src/registry/plugin-loader'
import { PluginRegistry } from '../src/registry/plugin-registry'

const states: PluginState[] = [
  'registered',
  'loading',
  'loaded',
  'enabled',
  'disabled',
  'error',
]

async function failure(promise: Promise<unknown>): Promise<unknown> {
  try {
    await promise
  } catch (error) {
    return error
  }
  throw new Error('Expected failure')
}
// Independent contract table. Error edges require markError(cause).
const edges: Record<PluginState, PluginState[]> = {
  registered: ['loading'],
  loading: ['loaded'],
  loaded: ['enabled', 'registered'],
  enabled: ['disabled'],
  disabled: ['enabled', 'registered'],
  error: ['registered'],
}

function fixture(lifecycle: PluginLifecycle = {}) {
  const registry = new PluginRegistry()
  let imports = 0
  registry.register({
    id: 'fixture',
    name: 'Fixture',
    type: 'tool',
    version: '1.0.0',
    loader: async () => {
      imports++
      return {
        default: {
          type: 'tool' as const,
          meta: { id: 'fixture', name: 'Fixture', version: '1.0.0' },
          run: () => undefined,
          lifecycle,
        },
      }
    },
  })
  return {
    registry,
    loader: new PluginLoader(registry),
    imports: () => imports,
  }
}

function reach(registry: PluginRegistry, state: PluginState) {
  if (state === 'registered') return
  if (state === 'error') {
    registry.markError('fixture', new Error('cause'))
    return
  }
  registry.transition('fixture', 'loading')
  if (state === 'loading') return
  registry.transition('fixture', 'loaded')
  if (state === 'loaded') return
  registry.transition('fixture', 'enabled')
  if (state === 'disabled') registry.transition('fixture', 'disabled')
}

describe('serialized lifecycle', () => {
  for (const from of states)
    for (const to of states) {
      test(`${from} -> ${to}`, () => {
        const { registry } = fixture()
        reach(registry, from)
        if (from === to || edges[from].includes(to)) {
          registry.transition('fixture', to)
          expect(registry.get('fixture')?.state).toBe(to)
        } else {
          expect(() => registry.transition('fixture', to)).toThrow()
          expect(registry.get('fixture')?.state).toBe(from)
        }
      })
    }

  test('100 concurrent enables across loader and manager share one load/activation', async () => {
    const calls: string[] = []
    const { registry, loader, imports } = fixture({
      onLoad: () => {
        calls.push('load')
      },
      onActivate: async () => {
        await Promise.resolve()
        calls.push('activate')
      },
      onDeactivate: () => {
        calls.push('deactivate')
      },
      onUnload: () => {
        calls.push('unload')
      },
    })
    const manager = new PluginLifecycleManager()
    await Promise.all(
      Array.from({ length: 100 }, (_, i) =>
        i % 2 ? loader.enable('fixture') : manager.activate(registry, 'fixture')
      )
    )
    expect(imports()).toBe(1)
    expect(calls).toEqual(['load', 'activate'])
    await Promise.all(
      Array.from({ length: 100 }, () => loader.disable('fixture'))
    )
    await loader.enable('fixture')
    expect(imports()).toBe(1)
    await manager.unload(registry, 'fixture')
    expect(calls).toEqual([
      'load',
      'activate',
      'deactivate',
      'activate',
      'deactivate',
      'unload',
    ])
    expect(registry.get('fixture')?.plugin).toBeUndefined()
  })

  test('activation compensation retains original error and requires explicit recovery', async () => {
    const calls: string[] = []
    const cause = new Error('activation failed')
    let failing = true
    const { registry, loader } = fixture({
      onLoad: () => {
        calls.push('load')
      },
      onActivate: () => {
        if (failing) throw cause
      },
      onDeactivate: () => {
        calls.push('deactivate')
      },
      onUnload: () => {
        calls.push('unload')
      },
    })
    expect(await failure(loader.enable('fixture'))).toBe(cause)
    expect(await failure(loader.enable('fixture'))).toHaveProperty(
      'code',
      'LIFECYCLE_CONFLICT'
    )
    expect(registry.get('fixture')?.error).toBe(cause)
    expect(calls).toEqual(['load', 'deactivate', 'unload'])
    failing = false
    await loader.reload('fixture')
    expect(registry.get('fixture')?.state).toBe('enabled')
    expect(registry.get('fixture')?.error).toBeUndefined()
  })

  test('cleanup failure prevents removal; recovery retries outstanding hook', async () => {
    const cause = new Error('partial load')
    const cleanup = new Error('cleanup failed')
    let failing = true
    const { registry, loader } = fixture({
      onLoad: () => {
        throw cause
      },
      onUnload: () => {
        if (failing) throw cleanup
      },
    })
    expect(await failure(loader.load('fixture'))).toBe(cause)
    expect(registry.get('fixture')?.cleanupErrors).toEqual([cleanup])
    expect(await failure(loader.uninstall('fixture'))).toHaveProperty(
      'code',
      'LIFECYCLE_CLEANUP_FAILED'
    )
    expect(registry.has('fixture')).toBe(true)
    expect(registry.get('fixture')?.error).toBe(cause)
    failing = false
    await loader.uninstall('fixture')
    await loader.uninstall('fixture')
    expect(registry.has('fixture')).toBe(false)
  })

  test('in-flight loading cannot be removed or enabled through a snapshot', async () => {
    const { registry, loader } = fixture()
    const pending = loader.enable('fixture')
    await Promise.resolve()
    expect(() => registry.unregister('fixture')).toThrow('unloaded')
    expect(() =>
      Object.assign(registry.get('fixture')!, { state: 'enabled' })
    ).toThrow()
    await pending
    await loader.reload('fixture')
    await loader.uninstall('fixture')
    expect(registry.size).toBe(0)
  })
})
