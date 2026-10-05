import type { PluginRegistry } from './plugin-registry'

import { PluginLoader } from './plugin-loader'

/** Compatibility facade; no independent hooks, states or locks. */
export class PluginLifecycleManager {
  load(registry: PluginRegistry, pluginId: string): Promise<void> {
    return new PluginLoader(registry).load(pluginId)
  }

  unload(registry: PluginRegistry, pluginId: string): Promise<void> {
    return new PluginLoader(registry).unload(pluginId)
  }

  activate(registry: PluginRegistry, pluginId: string): Promise<void> {
    return new PluginLoader(registry).enable(pluginId)
  }

  deactivate(registry: PluginRegistry, pluginId: string): Promise<void> {
    return new PluginLoader(registry).disable(pluginId)
  }

  isHealthy(registry: PluginRegistry, pluginId: string): boolean {
    const entry = registry.get(pluginId)
    return entry !== undefined && entry.state !== 'error'
  }
}
