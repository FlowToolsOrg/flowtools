import type {
  PluginRegistryEvent,
  RegisteredCommand,
  RegisteredPlugin,
  ExternalPluginLoader,
} from '@flowtools/sdk'

import { ExternalCodeDisabledError, PluginLoader } from '@flowtools/sdk'
import { persist } from 'zustand/middleware'
import { createStore } from 'zustand/vanilla'

import { assertUnsafePluginPreview } from '../app/development-policy'
import { builtInManifests } from '../plugin/manifests'
import { removePluginFile, savePluginFile } from '../utils/plugin-storage'

const builtInPluginIds = new Set(builtInManifests.map(entry => entry.id))

function assertPluginEntryAllowed(pluginId: string): void {
  if (!builtInPluginIds.has(pluginId)) {
    assertUnsafePluginPreview()
  }
}

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
let _fileLoader: ExternalPluginLoader | null = null

function getLoader(): PluginLoader {
  if (!_loader) {
    throw new Error('[PluginRegistryStore] Store not initialized.')
  }

  return _loader
}

function getFileLoader(): ExternalPluginLoader {
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
        assertPluginEntryAllowed(pluginId)
        set(state => ({
          disabledPluginIds: state.disabledPluginIds.filter(
            id => id !== pluginId
          ),
        }))
        await getLoader().enable(pluginId)
        get().sync()
      },

      async disablePlugin(pluginId: string) {
        assertPluginEntryAllowed(pluginId)
        set(state => ({
          disabledPluginIds: [...state.disabledPluginIds, pluginId],
        }))
        await getLoader().disable(pluginId)
        get().sync()
      },

      async reloadPlugin(pluginId: string) {
        assertPluginEntryAllowed(pluginId)
        await getLoader().reload(pluginId)
        get().sync()
      },

      async loadPluginFromFile(file: File) {
        assertUnsafePluginPreview()
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
        assertUnsafePluginPreview()
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
  fileLoader: ExternalPluginLoader
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
 * Automatic source recovery is intentionally disabled in every host mode.
 * Keep the old database unchanged for deliberate future export/recovery.
 */
export async function restoreExternalPlugins(): Promise<void> {
  throw new ExternalCodeDisabledError()
}
