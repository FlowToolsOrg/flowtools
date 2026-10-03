import type { RegisteredCommand } from '@flowtools/sdk'
import type { CommandPaletteItem } from '@flowtools/ui'

import {
  pluginRegistryInternals,
  pluginRegistryStore,
} from './plugin-registry-store'

export function getCommandPaletteItems(): CommandPaletteItem[] {
  const { commands } = pluginRegistryStore.getState()

  return commands.map(
    (cmd: RegisteredCommand): CommandPaletteItem => ({
      id: cmd.id,
      title: cmd.title,
      description: cmd.description,
      source: cmd.pluginId,
      shortcut: cmd.shortcut,
      icon: cmd.icon,
    })
  )
}

export async function executeCommand(commandId: string): Promise<void> {
  const commandRegistry = pluginRegistryInternals._commandRegistry
  if (!commandRegistry) {
    throw new Error('Command registry not initialized')
  }

  await commandRegistry.execute(commandId)
}
