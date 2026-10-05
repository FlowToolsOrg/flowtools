import type {
  PluginManifestEntry,
  PluginRegistryEvent,
  PluginRegistryListener,
  PluginState,
  RegisteredPlugin,
} from './types'

import { PluginLifecycleError, pluginStateTransitions } from './lifecycle-state'
import { PluginResourceScope, type PluginResourceOwner } from './resource-scope'

/**
 * Central registry for all plugins.
 * Single source of truth for plugin state.
 */
export class PluginRegistry {
  private plugins = new Map<string, RegisteredPlugin>()
  private listeners = new Set<PluginRegistryListener>()
  private operations = new Map<string, Promise<unknown>>()
  private activeOperations = new Map<string, symbol>()
  private resources = new Map<
    string,
    Map<PluginResourceOwner, PluginResourceScope>
  >()

  resourceScope(
    pluginId: string,
    owner: PluginResourceOwner
  ): PluginResourceScope {
    const entry = this.plugins.get(pluginId)
    if (
      !entry ||
      (owner !== 'load' &&
        entry.state !== 'enabled' &&
        !(
          owner === 'activation' &&
          (entry.state === 'loaded' || entry.state === 'disabled')
        ))
    ) {
      throw new PluginLifecycleError(
        pluginId,
        'cannot acquire resources in this state.'
      )
    }
    let scopes = this.resources.get(pluginId)
    if (!scopes) {
      scopes = new Map()
      this.resources.set(pluginId, scopes)
    }
    let scope = scopes.get(owner)
    if (!scope) {
      scope = new PluginResourceScope()
      scopes.set(owner, scope)
    }
    return scope
  }

  /** Enabled UI modules do not imply a resident runner process. */
  isRunning(pluginId: string): boolean {
    return (this.resources.get(pluginId)?.get('runner')?.size ?? 0) > 0
  }

  async releaseResources(
    pluginId: string,
    owners: readonly PluginResourceOwner[]
  ): Promise<Error[]> {
    const scopes = this.resources.get(pluginId)
    const errors: Error[] = []
    for (const owner of owners) {
      const scope = scopes?.get(owner)
      if (!scope) continue
      const failures = await scope.close()
      errors.push(...failures)
      if (!failures.length) scopes!.delete(owner)
    }
    if (scopes?.size === 0) this.resources.delete(pluginId)
    return errors
  }

  replaceManifest(
    pluginId: string,
    manifest: PluginManifestEntry,
    lease: symbol
  ): void {
    const entry = this.plugins.get(pluginId)
    if (
      !entry ||
      entry.state !== 'registered' ||
      manifest.id !== pluginId ||
      this.activeOperations.get(pluginId) !== lease
    ) {
      throw new PluginLifecycleError(
        pluginId,
        'cannot replace a live manifest.'
      )
    }
    this.plugins.set(pluginId, Object.freeze({ ...entry, manifest }))
    this.emit({ type: 'registered', pluginId })
  }

  /** Shared by every loader/manager using this registry, including failures. */
  serialize<T>(
    pluginId: string,
    operation: (lease: symbol) => Promise<T>
  ): Promise<T> {
    const previous = this.operations.get(pluginId) ?? Promise.resolve()
    const next = previous
      .catch(() => {})
      .then(async () => {
        const lease = Symbol(pluginId)
        this.activeOperations.set(pluginId, lease)
        try {
          return await operation(lease)
        } finally {
          this.activeOperations.delete(pluginId)
        }
      })
    this.operations.set(pluginId, next)
    void next
      .finally(() => {
        if (this.operations.get(pluginId) === next)
          this.operations.delete(pluginId)
      })
      .catch(() => {})
    return next
  }

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
      generation: 0,
    }

    this.plugins.set(manifest.id, Object.freeze(entry))
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
  unregister(pluginId: string, lease?: symbol): void {
    const entry = this.plugins.get(pluginId)
    if (!entry) {
      return
    }

    if (
      entry.state !== 'registered' ||
      (this.operations.has(pluginId) &&
        (!lease || this.activeOperations.get(pluginId) !== lease))
    ) {
      throw new PluginLifecycleError(
        pluginId,
        'must be unloaded before removal.'
      )
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
  updateState(
    pluginId: string,
    update: Partial<Omit<RegisteredPlugin, 'id' | 'manifest' | 'state'>>
  ): void {
    const entry = this.plugins.get(pluginId)
    if (!entry) {
      return
    }

    if ('state' in update || 'manifest' in update || 'id' in update) {
      throw new PluginLifecycleError(
        pluginId,
        'cannot bypass the state machine.'
      )
    }
    this.plugins.set(pluginId, Object.freeze({ ...entry, ...update }))
  }

  /**
   * Transition a plugin to a new lifecycle state.
   */
  transition(pluginId: string, state: PluginState): void {
    const entry = this.plugins.get(pluginId)
    if (!entry || entry.state === state) {
      return
    }

    if (!pluginStateTransitions[entry.state].includes(state)) {
      throw new PluginLifecycleError(
        pluginId,
        `cannot transition from ${entry.state} to ${state}.`
      )
    }
    if (state === 'error') {
      throw new PluginLifecycleError(pluginId, 'requires an error cause.')
    }
    this.plugins.set(pluginId, Object.freeze({ ...entry, state }))

    switch (state) {
      case 'registered':
        this.emit({ type: 'registered', pluginId })
        break
      case 'loading':
        this.emit({ type: 'loading', pluginId })
        break
      case 'loaded':
        this.emit({ type: 'loaded', pluginId })
        break
      case 'enabled':
        this.emit({ type: 'enabled', pluginId })
        break
      case 'disabled':
        this.emit({ type: 'disabled', pluginId })
        break
    }
  }

  /**
   * Mark a plugin as errored.
   */
  markError(pluginId: string, error: Error): void {
    if (!this.plugins.has(pluginId)) {
      return
    }

    const entry = this.plugins.get(pluginId)!
    this.plugins.set(
      pluginId,
      Object.freeze({ ...entry, state: 'error', error })
    )
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
