import * as sdk from '@flowtools/sdk'

import { builtInManifests } from '@/plugin/manifests'
import {
  initPluginRegistryStore,
  pluginRegistryStore,
} from '@/stores/plugin-registry-store'

import { unsafePluginPreviewEnabled } from './development-policy'
import { registerPluginCommands } from './plugin-commands'

let _ready = false

export interface BootstrapResult {
  registry: sdk.PluginRegistry
  commandRegistry: sdk.CommandRegistry
  loader: sdk.PluginLoader
  fileLoader: sdk.ExternalPluginLoader
}

export async function bootstrap(
  navigate: (path: string) => void
): Promise<BootstrapResult> {
  if (_ready) {
    const state = pluginRegistryStore.getState()

    return state as unknown as BootstrapResult
  }

  const registry = new sdk.PluginRegistry()
  const commandRegistry = new sdk.CommandRegistry()
  const loader = new sdk.PluginLoader(registry)
  let fileLoader: sdk.ExternalPluginLoader = new sdk.PluginFileLoader(
    registry,
    loader
  )
  if (import.meta.env?.DEV === true && unsafePluginPreviewEnabled) {
    const development = await import('@flowtools/sdk/development')
    development.setupImportMap(sdk)
    fileLoader = new development.DevelopmentPluginFileLoader(registry, loader)
  }

  registry.registerAll(builtInManifests)

  await loader.loadAll()

  const { disabledPluginIds } = pluginRegistryStore.getState()

  for (const entry of registry.getAll()) {
    if (entry.state === 'loaded' && !disabledPluginIds.includes(entry.id)) {
      await loader.enable(entry.id)
    }
  }

  initPluginRegistryStore(registry, commandRegistry, loader, fileLoader)

  registerPluginCommands(registry, commandRegistry, navigate)

  _ready = true

  return { registry, commandRegistry, loader, fileLoader }
}
