import type { PluginRegistry } from './plugin-registry'
import type { RegisteredPlugin } from './types'

/**
 * Manages plugin lifecycle transitions.
 * Coordinates onLoad/onUnload/onActivate/onDeactivate hooks.
 */
export class PluginLifecycleManager {
  /**
   * Load a plugin: invoke its onLoad lifecycle hook.
   * Plugin must be in 'registered' state.
   */
  async load(registry: PluginRegistry, pluginId: string): Promise<void> {
    const entry = this.getEntry(registry, pluginId)

    if (entry.state !== 'registered') {
      return
    }

    try {
      await entry.plugin?.lifecycle?.onLoad?.()
    } catch (error) {
      registry.markError(
        pluginId,
        error instanceof Error ? error : new Error(String(error))
      )
    }
  }

  /**
   * Unload a plugin: invoke its onUnload lifecycle hook.
   */
  async unload(registry: PluginRegistry, pluginId: string): Promise<void> {
    const entry = this.getEntry(registry, pluginId)

    try {
      await entry.plugin?.lifecycle?.onUnload?.()
    } catch (error) {
      registry.markError(
        pluginId,
        error instanceof Error ? error : new Error(String(error))
      )
    }
  }

  /**
   * Activate a plugin: invoke its onActivate lifecycle hook.
   */
  async activate(registry: PluginRegistry, pluginId: string): Promise<void> {
    const entry = this.getEntry(registry, pluginId)

    try {
      await entry.plugin?.lifecycle?.onActivate?.()
    } catch (error) {
      registry.markError(
        pluginId,
        error instanceof Error ? error : new Error(String(error))
      )
    }
  }

  /**
   * Deactivate a plugin: invoke its onDeactivate lifecycle hook.
   */
  async deactivate(registry: PluginRegistry, pluginId: string): Promise<void> {
    const entry = this.getEntry(registry, pluginId)

    try {
      await entry.plugin?.lifecycle?.onDeactivate?.()
    } catch (error) {
      registry.markError(
        pluginId,
        error instanceof Error ? error : new Error(String(error))
      )
    }
  }

  /**
   * Check if a plugin is in a healthy state.
   */
  isHealthy(registry: PluginRegistry, pluginId: string): boolean {
    const entry = registry.get(pluginId)

    return entry !== undefined && entry.state !== 'error'
  }

  private getEntry(
    registry: PluginRegistry,
    pluginId: string
  ): RegisteredPlugin {
    const entry = registry.get(pluginId)

    if (!entry) {
      throw new Error(
        `[PluginLifecycleManager] Plugin "${pluginId}" is not registered.`
      )
    }

    return entry
  }
}
