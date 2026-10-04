import type { RegisteredCommand, CommandRegistry } from './command-registry'
import type { PluginRegistry } from './plugin-registry'
import type { RegisteredPlugin } from './types'

import { PluginLifecycleError } from './lifecycle-state'
import { PluginLoader } from './plugin-loader'

/** Subscribe once; handlers are leased to the loaded generation that created them. */
export function projectPluginCommands(
  registry: PluginRegistry,
  commands: CommandRegistry,
  create: (entry: RegisteredPlugin) => readonly RegisteredCommand[]
): () => void {
  const projected = new Set<string>()
  const loader = new PluginLoader(registry)
  const sync = (pluginId: string) => {
    commands.unregisterByPlugin(pluginId)
    projected.delete(pluginId)
    const entry = registry.get(pluginId)
    if (
      entry?.state !== 'enabled' ||
      !entry.plugin ||
      entry.manifest.dependenciesSatisfied === false
    )
      return
    const generation = entry.generation
    for (const command of create(entry)) {
      if (command.pluginId !== pluginId)
        throw new PluginLifecycleError(
          pluginId,
          'command owner does not match.'
        )
      commands.register({
        ...command,
        handler: () =>
          loader.withPlugin(pluginId, () => {
            if (registry.get(pluginId)?.generation !== generation)
              throw new PluginLifecycleError(
                pluginId,
                'command generation has expired.'
              )
            return command.handler()
          }),
      })
    }
    projected.add(pluginId)
  }
  const unsubscribe = registry.subscribe(event => sync(event.pluginId))
  for (const entry of registry.getAll()) sync(entry.id)
  return () => {
    unsubscribe()
    for (const id of projected) commands.unregisterByPlugin(id)
    projected.clear()
  }
}
