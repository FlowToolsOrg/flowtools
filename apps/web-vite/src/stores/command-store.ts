import type { RegisteredCommand } from '@flow-tool/sdk'
import type { CommandPaletteItem } from '@flow-tool/ui'

import { pluginRegistryStore } from './plugin-registry-store'

let _executeHandler: ((commandId: string) => Promise<void>) | null = null

export function setCommandExecuteHandler(
  handler: (commandId: string) => Promise<void>
): void {
  _executeHandler = handler
}

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
  if (_executeHandler) {
    await _executeHandler(commandId)
  }
}
