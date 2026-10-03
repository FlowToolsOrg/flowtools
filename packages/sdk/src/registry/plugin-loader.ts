import type { PluginRegistry } from './plugin-registry'

/**
 * Plugin loader that handles loading, enabling, and disabling plugins.
 * Works with PluginRegistry to manage plugin lifecycle transitions.
 */
export class PluginLoader {
  constructor(private registry: PluginRegistry) {}

  /**
   * Load a single plugin by calling its manifest loader.
   * Transitions: registered → loading → loaded | error
   */
  async load(pluginId: string): Promise<void> {
    const entry = this.registry.get(pluginId)

    if (!entry) {
      throw new Error(`[PluginLoader] Plugin "${pluginId}" is not registered.`)
    }

    if (entry.state === 'loaded' || entry.state === 'enabled') {
      return
    }

    this.registry.transition(pluginId, 'loading')

    try {
      const module = await entry.manifest.loader()
      const plugin = module.default

      if (!plugin || !plugin.type || !plugin.meta) {
        throw new Error(
          `[PluginLoader] Plugin "${pluginId}" has invalid default export.`
        )
      }

      this.registry.updateState(pluginId, {
        plugin,
        loadedAt: Date.now(),
      })
      this.registry.transition(pluginId, 'loaded')
    } catch (error) {
      const err = error instanceof Error ? error : new Error(String(error))

      this.registry.markError(pluginId, err)
      throw err
    }
  }

  /**
   * Load all registered plugins.
   * Errors are caught per-plugin so one failure doesn't block others.
   */
  async loadAll(): Promise<void> {
    const entries = this.registry.getAll()
    await Promise.allSettled(
      entries.filter(e => e.state === 'registered').map(e => this.load(e.id))
    )
  }

  /**
   * Enable a plugin: load it if needed, then mark as enabled.
   * Transitions: loaded → enabled
   */
  async enable(pluginId: string): Promise<void> {
    const entry = this.registry.get(pluginId)

    if (!entry) {
      throw new Error(`[PluginLoader] Plugin "${pluginId}" is not registered.`)
    }

    if (entry.state === 'enabled') {
      return
    }

    if (entry.state !== 'loaded') {
      await this.load(pluginId)
    }

    const updated = this.registry.get(pluginId)

    if (!updated) {
      throw new Error(
        `[PluginLoader] Plugin "${pluginId}" disappeared during enable.`
      )
    }

    try {
      await updated.plugin?.lifecycle?.onActivate?.()
    } catch (error) {
      const err = error instanceof Error ? error : new Error(String(error))
      this.registry.markError(pluginId, err)
      throw err
    }

    this.registry.updateState(pluginId, {
      enabledAt: Date.now(),
    })
    this.registry.transition(pluginId, 'enabled')
  }

  /**
   * Disable a plugin: deactivate and mark as disabled.
   * Transitions: enabled → disabled
   */
  async disable(pluginId: string): Promise<void> {
    const entry = this.registry.get(pluginId)

    if (!entry) {
      throw new Error(`[PluginLoader] Plugin "${pluginId}" is not registered.`)
    }

    if (entry.state !== 'enabled') {
      return
    }

    try {
      await entry.plugin?.lifecycle?.onDeactivate?.()
    } catch (error) {
      const err = error instanceof Error ? error : new Error(String(error))
      this.registry.markError(pluginId, err)
      throw err
    }

    this.registry.transition(pluginId, 'disabled')
  }

  /**
   * Enable all loaded plugins.
   */
  async enableAll(): Promise<void> {
    const entries = this.registry.getAll()

    for (const entry of entries) {
      if (entry.state === 'loaded') {
        await this.enable(entry.id)
      }
    }
  }

  /**
   * Reload a plugin: disable, unload, then load and enable again.
   */
  async reload(pluginId: string): Promise<void> {
    const entry = this.registry.get(pluginId)

    if (!entry) {
      throw new Error(`[PluginLoader] Plugin "${pluginId}" is not registered.`)
    }

    if (entry.state === 'enabled') {
      await this.disable(pluginId)
    }

    this.registry.transition(pluginId, 'registered')
    this.registry.updateState(pluginId, {
      plugin: undefined,
      loadedAt: undefined,
      enabledAt: undefined,
      error: undefined,
    })

    await this.load(pluginId)
    await this.enable(pluginId)
  }
}
