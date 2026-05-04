import type {
  PluginRegistryEvent,
  RegisteredCommand,
  RegisteredPlugin,
} from '@flowtools/sdk'

import { PluginFileLoader, PluginLoader } from '@flowtools/sdk'
import { createStore } from 'zustand/vanilla'

interface PluginRegistryState {
  plugins: RegisteredPlugin[]
  commands: RegisteredCommand[]
  externalPluginIds: string[]
}

interface PluginRegistryActions {
  sync: () => void
  enablePlugin: (pluginId: string) => Promise<void>
  disablePlugin: (pluginId: string) => Promise<void>
  reloadPlugin: (pluginId: string) => Promise<void>
  loadPluginFromFile: (file: File) => Promise<RegisteredPlugin>
  unloadExternalPlugin: (pluginId: string) => Promise<void>
}

export type PluginRegistryStore = PluginRegistryState & PluginRegistryActions

let _loader: PluginLoader | null = null
let _fileLoader: PluginFileLoader | null = null

function getLoader(): PluginLoader {
  if (!_loader) {
    throw new Error('[PluginRegistryStore] Store not initialized.')
  }

  return _loader
}

function getFileLoader(): PluginFileLoader {
  if (!_fileLoader) {
    throw new Error('[PluginRegistryStore] Store not initialized.')
  }

  return _fileLoader
}

export const pluginRegistryStore = createStore<PluginRegistryState>()(() => ({
  plugins: [],
  commands: [],
  externalPluginIds: [],
}))

function syncState(): void {
  const { _registry, _commandRegistry } = pluginRegistryInternals

  if (!_registry || !_commandRegistry) {
    return
  }

  pluginRegistryStore.setState({
    plugins: _registry.getAll(),
    commands: _commandRegistry.getAll(),
    externalPluginIds: _fileLoader?.getExternalPluginIds() ?? [],
  })
}

export const pluginRegistryInternals = {
  _registry: null as import('@flowtools/sdk').PluginRegistry | null,
  _commandRegistry: null as import('@flowtools/sdk').CommandRegistry | null,
}

export function initPluginRegistryStore(
  registry: import('@flowtools/sdk').PluginRegistry,
  commandRegistry: import('@flowtools/sdk').CommandRegistry,
  loader: PluginLoader,
  fileLoader: PluginFileLoader
): void {
  pluginRegistryInternals._registry = registry
  pluginRegistryInternals._commandRegistry = commandRegistry
  _loader = loader
  _fileLoader = fileLoader

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

  async loadPluginFromFile(file: File) {
    const entry = await getFileLoader().loadFromFile(file)
    syncState()
    return entry
  },

  async unloadExternalPlugin(pluginId: string) {
    await getFileLoader().unloadExternalPlugin(pluginId)
    syncState()
  },
}
