import type {
  PluginRegistryEvent,
  RegisteredCommand,
  RegisteredPlugin,
} from '@flowtools/sdk'

import { PluginLoader } from '@flowtools/sdk'
import { createStore } from 'zustand/vanilla'

interface PluginRegistryState {
  plugins: RegisteredPlugin[]
  commands: RegisteredCommand[]
}

interface PluginRegistryActions {
  sync: () => void
  enablePlugin: (pluginId: string) => Promise<void>
  disablePlugin: (pluginId: string) => Promise<void>
  reloadPlugin: (pluginId: string) => Promise<void>
}

export type PluginRegistryStore = PluginRegistryState & PluginRegistryActions

let _loader: PluginLoader | null = null

function getLoader(): PluginLoader {
  if (!_loader) {
    throw new Error('[PluginRegistryStore] Store not initialized.')
  }

  return _loader
}

export const pluginRegistryStore = createStore<PluginRegistryState>()(() => ({
  plugins: [],
  commands: [],
}))

function syncState(): void {
  const { _registry, _commandRegistry } = pluginRegistryInternals

  if (!_registry || !_commandRegistry) {
    return
  }

  pluginRegistryStore.setState({
    plugins: _registry.getAll(),
    commands: _commandRegistry.getAll(),
  })
}

export const pluginRegistryInternals = {
  _registry: null as import('@flowtools/sdk').PluginRegistry | null,
  _commandRegistry: null as import('@flowtools/sdk').CommandRegistry | null,
}

export function initPluginRegistryStore(
  registry: import('@flowtools/sdk').PluginRegistry,
  commandRegistry: import('@flowtools/sdk').CommandRegistry,
  loader: PluginLoader
): void {
  pluginRegistryInternals._registry = registry
  pluginRegistryInternals._commandRegistry = commandRegistry
  _loader = loader

  registry.subscribe((_event: PluginRegistryEvent) => {
    syncState()
  })

  commandRegistry.subscribe(() => {
    syncState()
  })

  syncState()
}

export const pluginRegistryActions: PluginRegistryActions = {
  sync() {
    syncState()
  },

  async enablePlugin(pluginId: string) {
    await getLoader().enable(pluginId)
    syncState()
  },

  async disablePlugin(pluginId: string) {
    await getLoader().disable(pluginId)
    syncState()
  },

  async reloadPlugin(pluginId: string) {
    await getLoader().reload(pluginId)
    syncState()
  },
}
