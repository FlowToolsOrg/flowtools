import type { RegisteredCommand } from '@flow-tool/sdk'

import { CommandRegistry, PluginLoader, PluginRegistry } from '@flow-tool/sdk'

import { builtInManifests } from '@/plugin/manifests'
import {
  initPluginRegistryStore,
  pluginRegistryStore,
} from '@/stores/plugin-registry-store'

let _ready = false

function registerPluginCommands(
  registry: PluginRegistry,
  commandRegistry: CommandRegistry,
  navigate: (path: string) => void
): void {
  for (const entry of registry.getEnabled()) {
    if (!entry.plugin) {
      continue
    }

    const pluginId = entry.id
    const meta = entry.plugin.meta

    if (entry.plugin.type === 'app') {
      const cmd: RegisteredCommand = {
        id: `plugin:${pluginId}`,
        title: meta.name,
        description: meta.description,
        pluginId,
        mode: 'panel',
        keywords: [meta.name, ...(meta.tags ?? []), meta.category ?? ''].filter(
          Boolean
        ),
        handler: () => navigate(`/tools/${pluginId}`),
      }

      commandRegistry.register(cmd)
    } else if (entry.plugin.type === 'tool') {
      const cmd: RegisteredCommand = {
        id: `plugin:${pluginId}`,
        title: meta.name,
        description: meta.description,
        pluginId,
        mode: 'headless',
        keywords: [meta.name, ...(meta.tags ?? []), meta.category ?? ''].filter(
          Boolean
        ),
        handler: () => navigate(`/tools/${pluginId}`),
      }

      commandRegistry.register(cmd)
    }

    if (entry.plugin.commands) {
      for (const [cmdId, cmdDef] of Object.entries(entry.plugin.commands)) {
        const fullId = `${pluginId}:${cmdId}`

        const cmd: RegisteredCommand = {
          id: fullId,
          title: cmdDef.title,
          description: cmdDef.description,
          pluginId,
          mode: cmdDef.mode,
          keywords: [cmdDef.title, cmdDef.description ?? ''].filter(Boolean),
          handler: () => {
            if (cmdDef.mode === 'panel') {
              navigate(`/tools/${pluginId}`)
            }
          },
        }

        commandRegistry.register(cmd)
      }
    }
  }
}

export interface BootstrapResult {
  registry: PluginRegistry
  commandRegistry: CommandRegistry
  loader: PluginLoader
}

export async function bootstrap(
  navigate: (path: string) => void
): Promise<BootstrapResult> {
  if (_ready) {
    const state = pluginRegistryStore.getState()

    return state as unknown as BootstrapResult
  }

  const registry = new PluginRegistry()
  const commandRegistry = new CommandRegistry()
  const loader = new PluginLoader(registry)

  registry.registerAll(builtInManifests)

  await loader.loadAll()
  await loader.enableAll()

  initPluginRegistryStore(registry, commandRegistry, loader)

  registerPluginCommands(registry, commandRegistry, navigate)

  _ready = true

  return { registry, commandRegistry, loader }
}
