import type {
  CommandRegistry,
  PluginRegistry,
  RegisteredCommand,
} from '@flowtools/sdk'

export function registerPluginCommands(
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
    const pluginType = entry.plugin.type

    if (pluginType !== 'app' && pluginType !== 'tool') {
      continue
    }

    const command: RegisteredCommand = {
      id: `plugin:${pluginId}`,
      title: meta.name,
      description: meta.description,
      pluginId,
      mode: pluginType === 'app' ? 'panel' : 'headless',
      keywords: [meta.name, ...(meta.tags ?? []), meta.category ?? ''].filter(
        Boolean
      ),
      handler: () => navigate(`/tools/${pluginId}`),
    }

    commandRegistry.register(command)
  }
}
