import type {
  PluginRegistryEvent,
  RegisteredCommand,
  RegisteredPlugin,
} from '@flowtools/sdk'

import { PluginFileLoader, PluginLoader } from '@flowtools/sdk'
import { persist } from 'zustand/middleware'
import { createStore } from 'zustand/vanilla'

import {
  loadAllPluginFiles,
  removePluginFile,
  savePluginFile,
} from '../utils/plugin-storage'

interface PluginRegistryState {
  plugins: RegisteredPlugin[]
  commands: RegisteredCommand[]
  externalPluginIds: string[]
  disabledPluginIds: string[]
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

export const pluginRegistryStore = createStore<
  PluginRegistryState & PluginRegistryActions
>()(
  persist(
    (set, get) => ({
      plugins: [],
      commands: [],
      externalPluginIds: [],
      disabledPluginIds: [],

      sync() {
        const { _registry, _commandRegistry } = pluginRegistryInternals

        if (!_registry || !_commandRegistry) {
          return
        }

        set({
          plugins: _registry.getAll(),
          commands: _commandRegistry.getAll(),
          externalPluginIds: _fileLoader?.getExternalPluginIds() ?? [],
        })
      },

      async enablePlugin(pluginId: string) {
        set(state => ({
          disabledPluginIds: state.disabledPluginIds.filter(
            id => id !== pluginId
          ),
        }))
        await getLoader().enable(pluginId)
        get().sync()
      },

      async disablePlugin(pluginId: string) {
        set(state => ({
          disabledPluginIds: [...state.disabledPluginIds, pluginId],
        }))
        await getLoader().disable(pluginId)
        get().sync()
      },

      async reloadPlugin(pluginId: string) {
        await getLoader().reload(pluginId)
        get().sync()
      },

      async loadPluginFromFile(file: File) {
        const entry = await getFileLoader().loadFromFile(file)

        const buffer = await file.arrayBuffer()
        await savePluginFile({
          id: entry.id,
          name: entry.manifest.name,
          fileName: file.name,
          type: file.type || 'application/javascript',
          data: buffer,
        })

        get().sync()
        return entry
      },

      async unloadExternalPlugin(pluginId: string) {
        await getFileLoader().unloadExternalPlugin(pluginId)
        await removePluginFile(pluginId)
        get().sync()
      },
    }),
    {
      name: 'flowtools-plugin-registry',
      partialize: state => ({
        disabledPluginIds: state.disabledPluginIds,
      }),
    }
  )
)

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
    pluginRegistryStore.getState().sync()
  })

  commandRegistry.subscribe(() => {
    pluginRegistryStore.getState().sync()
  })

  pluginRegistryStore.getState().sync()
}

/**
 * Restore external plugins from IndexedDB.
 * Call after initPluginRegistryStore and import map setup.
 */
export async function restoreExternalPlugins(): Promise<void> {
  const stored = await loadAllPluginFiles()

  for (const entry of stored) {
    try {
      const file = new File([entry.data], entry.fileName, {
        type: entry.type,
      })
      await getFileLoader().loadFromFile(file)
    } catch {
      await removePluginFile(entry.id)
    }
  }

  pluginRegistryStore.getState().sync()
}
