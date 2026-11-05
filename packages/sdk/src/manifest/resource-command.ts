import type { PluginManifest, CommandManifestV2 } from './v2'
/** The only v2 effect adapter admitted by NA1. Grants stay in the Host. */
export function isResourceCommand(
  command: PluginManifest['commands'][number]
): command is CommandManifestV2 {
  return (
    'capabilityProfile' in command &&
    command.capabilityProfile === 'flowtools-native-v2' &&
    command.effects.length === 1 &&
    command.effects[0] === 'file-read' &&
    command.permissions.length === 1 &&
    command.permissions[0]?.capability === 'fs' &&
    command.permissions[0].operations.every(operation =>
      ['read', 'stat', 'list'].includes(operation)
    ) &&
    command.permissions[0].scopes.every(
      scope => typeof scope === 'object' && scope.kind === 'resource'
    )
  )
}
