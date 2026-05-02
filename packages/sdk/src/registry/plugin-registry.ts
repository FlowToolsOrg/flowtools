import type {
  PluginManifestEntry,
  PluginRegistryEvent,
  PluginRegistryListener,
  PluginState,
  RegisteredPlugin,
} from './types'

/**
 * Central registry for all plugins.
 * Single source of truth for plugin state.
 */
export class PluginRegistry {
  private plugins = new Map<string, RegisteredPlugin>()
  private listeners = new Set<PluginRegistryListener>()

  /**
   * Register a plugin manifest without loading it.
   */
  register(manifest: PluginManifestEntry): void {
    if (this.plugins.has(manifest.id)) {
      return
    }

    const entry: RegisteredPlugin = {
      id: manifest.id,
      manifest,
      state: 'registered',
    }

    this.plugins.set(manifest.id, entry)
    this.emit({ type: 'registered', pluginId: manifest.id })
  }

  /**
   * Register multiple plugin manifests at once.
   */
  registerAll(manifests: PluginManifestEntry[]): void {
    for (const manifest of manifests) {
      this.register(manifest)
    }
  }

  /**
   * Unregister a plugin and remove it from the registry.
   */
  unregister(pluginId: string): void {
    const entry = this.plugins.get(pluginId)
    if (!entry) {
      return
    }

    this.plugins.delete(pluginId)
    this.emit({ type: 'unloaded', pluginId })
  }

  /**
   * Get a single registered plugin by id.
   */
  get(pluginId: string): RegisteredPlugin | undefined {
    return this.plugins.get(pluginId)
  }

  /**
   * Get all registered plugins.
   */
  getAll(): RegisteredPlugin[] {
    return [...this.plugins.values()]
  }

  /**
   * Get plugins filtered by type.
   */
  getByType(type: 'app' | 'tool'): RegisteredPlugin[] {
    return this.getAll().filter(p => p.manifest.type === type)
  }

  /**
   * Get only enabled plugins.
   */
  getEnabled(): RegisteredPlugin[] {
    return this.getAll().filter(p => p.state === 'enabled')
  }

  /**
   * Check if a plugin is registered.
   */
  has(pluginId: string): boolean {
    return this.plugins.has(pluginId)
  }

  /**
   * Get the count of registered plugins.
   */
  get size(): number {
    return this.plugins.size
  }

  /**
   * Update a plugin's internal state and emit event.
   */
  updateState(pluginId: string, update: Partial<RegisteredPlugin>): void {
    const entry = this.plugins.get(pluginId)
    if (!entry) {
      return
    }

    const next = { ...entry, ...update, id: pluginId }
    this.plugins.set(pluginId, next)
  }

  /**
   * Transition a plugin to a new lifecycle state.
   */
  transition(pluginId: string, state: PluginState): void {
    this.updateState(pluginId, { state })
  }

  /**
   * Mark a plugin as errored.
   */
  markError(pluginId: string, error: Error): void {
    this.updateState(pluginId, { state: 'error', error })
    this.emit({ type: 'error', pluginId, error })
  }

  /**
   * Subscribe to registry events.
   * Returns an unsubscribe function.
   */
  subscribe(listener: PluginRegistryListener): () => void {
    this.listeners.add(listener)
    return () => {
      this.listeners.delete(listener)
    }
  }

  /**
   * Emit an event to all listeners.
   */
  private emit(event: PluginRegistryEvent): void {
    for (const listener of this.listeners) {
      listener(event)
    }
  }
}
