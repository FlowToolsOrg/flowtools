import type { FlowToolPlugin } from '../types/plugin'
import type { PluginRegistry } from './plugin-registry'
import type { PluginManifestEntry, RegisteredPlugin } from './types'

import { PluginCleanupError, PluginLifecycleError } from './lifecycle-state'

interface HookOwnership {
  loaded: boolean
  active: boolean
}

const ownership = new WeakMap<PluginRegistry, Map<string, HookOwnership>>()
const asError = (error: unknown): Error =>
  error instanceof Error ? error : new Error(String(error))

/** One serialized execution path for loader and legacy lifecycle manager APIs. */
export class PluginLoader {
  private hooks: Map<string, HookOwnership>

  constructor(private registry: PluginRegistry) {
    let hooks = ownership.get(registry)
    if (!hooks) {
      hooks = new Map()
      ownership.set(registry, hooks)
    }
    this.hooks = hooks
  }

  load(pluginId: string): Promise<void> {
    return this.registry.serialize(pluginId, () => this.loadInternal(pluginId))
  }

  enable(pluginId: string): Promise<void> {
    return this.registry.serialize(pluginId, () =>
      this.enableInternal(pluginId)
    )
  }

  disable(pluginId: string): Promise<void> {
    return this.registry.serialize(pluginId, () =>
      this.disableInternal(pluginId)
    )
  }

  unload(pluginId: string): Promise<void> {
    return this.registry.serialize(pluginId, () =>
      this.unloadInternal(pluginId)
    )
  }

  uninstall(pluginId: string): Promise<void> {
    return this.registry.serialize(pluginId, async lease => {
      if (!this.registry.has(pluginId)) return
      await this.unloadInternal(pluginId)
      this.registry.unregister(pluginId, lease)
    })
  }

  reload(pluginId: string): Promise<void> {
    return this.registry.serialize(pluginId, async () => {
      this.getEntry(pluginId)
      await this.unloadInternal(pluginId)
      await this.enableInternal(pluginId)
    })
  }

  async loadAll(): Promise<void> {
    await Promise.allSettled(
      this.registry
        .getAll()
        .filter(e => e.state === 'registered')
        .map(e => this.load(e.id))
    )
  }

  update(pluginId: string, manifest: PluginManifestEntry): Promise<void> {
    return this.registry.serialize(pluginId, async lease => {
      const entry = this.getEntry(pluginId)
      if (manifest.id !== pluginId)
        throw new PluginLifecycleError(pluginId, 'identity cannot change.')
      const enabled = entry.state === 'enabled'
      await this.unloadInternal(pluginId)
      this.registry.replaceManifest(pluginId, manifest, lease)
      if (enabled) await this.enableInternal(pluginId)
    })
  }

  /** Updates/removal drain accepted calls; late calls reject after transition. */
  withPlugin<T>(
    pluginId: string,
    operation: (plugin: FlowToolPlugin) => Promise<T> | T
  ): Promise<T> {
    return this.registry.serialize(pluginId, async () => {
      const entry = this.getEntry(pluginId)
      if (
        entry.state !== 'enabled' ||
        entry.manifest.dependenciesSatisfied === false ||
        !entry.plugin
      ) {
        throw new PluginLifecycleError(pluginId, 'is unavailable for commands.')
      }
      return await operation(entry.plugin)
    })
  }

  async enableAll(): Promise<void> {
    for (const entry of this.registry.getAll()) {
      if (entry.state === 'loaded' || entry.state === 'disabled') {
        await this.enable(entry.id)
      }
    }
  }

  private getEntry(pluginId: string): RegisteredPlugin {
    const entry = this.registry.get(pluginId)
    if (!entry) {
      throw new PluginLifecycleError(pluginId, 'is not registered.')
    }
    return entry
  }

  private assertHealthy(entry: RegisteredPlugin): void {
    if (entry.state === 'error') {
      throw new PluginLifecycleError(
        entry.id,
        'requires explicit unload/reload recovery.'
      )
    }
  }

  private async loadInternal(pluginId: string): Promise<void> {
    const entry = this.getEntry(pluginId)
    this.assertHealthy(entry)
    if (entry.plugin) return
    this.registry.transition(pluginId, 'loading')
    try {
      const { default: plugin } = await entry.manifest.loader()
      if (
        !plugin ||
        plugin.type !== entry.manifest.type ||
        plugin.meta?.id !== pluginId ||
        plugin.meta.version !== entry.manifest.version
      ) {
        throw new Error(
          `[PluginLoader] Plugin "${pluginId}" has invalid default export.`
        )
      }
      this.registry.updateState(pluginId, {
        plugin,
        generation: entry.generation + 1,
      })
      this.hooks.set(pluginId, { loaded: true, active: false })
      await plugin.lifecycle?.onLoad?.(
        this.registry.resourceScope(pluginId, 'load')
      )
      this.registry.updateState(pluginId, { loadedAt: Date.now() })
      this.registry.transition(pluginId, 'loaded')
    } catch (error) {
      await this.fail(pluginId, error)
    }
  }

  private async enableInternal(pluginId: string): Promise<void> {
    const entry = this.getEntry(pluginId)
    this.assertHealthy(entry)
    if (entry.state === 'enabled') return
    await this.loadInternal(pluginId)
    const updated = this.getEntry(pluginId)
    try {
      this.hooks.get(pluginId)!.active = true
      if (updated.manifest.dependenciesSatisfied === false) {
        throw new PluginLifecycleError(
          pluginId,
          'dependencies are unavailable.'
        )
      }
      await updated.plugin?.lifecycle?.onActivate?.(
        this.registry.resourceScope(pluginId, 'activation')
      )
      this.registry.updateState(pluginId, { enabledAt: Date.now() })
      this.registry.transition(pluginId, 'enabled')
    } catch (error) {
      await this.fail(pluginId, error)
    }
  }

  private async disableInternal(pluginId: string): Promise<void> {
    const entry = this.getEntry(pluginId)
    this.assertHealthy(entry)
    if (entry.state !== 'enabled') return
    try {
      await entry.plugin?.lifecycle?.onDeactivate?.()
      this.hooks.get(pluginId)!.active = false
      const errors = await this.registry.releaseResources(pluginId, [
        'view',
        'runner',
        'activation',
      ])
      if (errors.length) throw new PluginCleanupError(errors)
      this.registry.updateState(pluginId, { enabledAt: undefined })
      this.registry.transition(pluginId, 'disabled')
    } catch (error) {
      // The failed hook remains owned and is retried once as compensation.
      await this.fail(pluginId, error)
    }
  }

  private async cleanup(pluginId: string): Promise<Error[]> {
    const plugin = this.getEntry(pluginId).plugin
    const hooks = this.hooks.get(pluginId)
    const errors: Error[] = []
    if (hooks?.active) {
      try {
        await plugin?.lifecycle?.onDeactivate?.()
        hooks.active = false
      } catch (error) {
        errors.push(asError(error))
      }
    }
    if (hooks?.loaded) {
      try {
        await plugin?.lifecycle?.onUnload?.()
        hooks.loaded = false
      } catch (error) {
        errors.push(asError(error))
      }
    }
    errors.push(
      ...(await this.registry.releaseResources(pluginId, [
        'view',
        'runner',
        'activation',
        'load',
      ]))
    )
    return errors
  }

  private async fail(pluginId: string, error: unknown): Promise<never> {
    const cause = asError(error)
    this.registry.markError(pluginId, cause)
    const cleanupErrors = await this.cleanup(pluginId)
    this.registry.updateState(pluginId, { cleanupErrors })
    throw cause
  }

  private async unloadInternal(pluginId: string): Promise<void> {
    const entry = this.getEntry(pluginId)
    if (entry.state === 'registered') return
    if (entry.state === 'enabled') await this.disableInternal(pluginId)
    const errors = await this.cleanup(pluginId)
    if (errors.length) {
      this.registry.markError(pluginId, entry.error ?? errors[0]!)
      this.registry.updateState(pluginId, { cleanupErrors: errors })
      throw new PluginCleanupError(errors)
    }
    this.hooks.delete(pluginId)
    this.registry.updateState(pluginId, {
      plugin: undefined,
      loadedAt: undefined,
      enabledAt: undefined,
      error: undefined,
      cleanupErrors: undefined,
    })
    this.registry.transition(pluginId, 'registered')
  }
}
