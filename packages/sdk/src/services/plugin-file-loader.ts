import type { PluginLoader } from '../registry/plugin-loader'
import type { PluginRegistry } from '../registry/plugin-registry'
import type { RegisteredPlugin } from '../registry/types'
import type { FlowToolPlugin } from '../types/plugin'

export class ExternalCodeDisabledError extends Error {
  readonly code = 'EXTERNAL_CODE_DISABLED'

  constructor() {
    super(
      'External code execution is disabled. Signed-package admission and isolation are not implemented.'
    )
    this.name = 'ExternalCodeDisabledError'
  }
}

export interface ExternalPluginLoader {
  loadFromFile(file: File): Promise<RegisteredPlugin>
  loadFromFiles(files: File[]): Promise<RegisteredPlugin[]>
  registerExternalPlugin(plugin: FlowToolPlugin): Promise<RegisteredPlugin>
  unloadExternalPlugin(pluginId: string): Promise<void>
  getExternalPluginIds(): string[]
  isExternal(pluginId: string): boolean
}

/**
 * Compatibility-shaped, deny-only service. No mode/certification payload can
 * enable it. Trusted built-ins use PluginLoader and their compiled manifests.
 */
export class PluginFileLoader implements ExternalPluginLoader {
  constructor(_registry: PluginRegistry, _loader: PluginLoader) {}

  async loadFromFile(_file: File): Promise<RegisteredPlugin> {
    throw new ExternalCodeDisabledError()
  }

  async loadFromFiles(_files: File[]): Promise<RegisteredPlugin[]> {
    throw new ExternalCodeDisabledError()
  }

  async registerExternalPlugin(
    _plugin: FlowToolPlugin
  ): Promise<RegisteredPlugin> {
    throw new ExternalCodeDisabledError()
  }

  async unloadExternalPlugin(_pluginId: string): Promise<void> {
    throw new ExternalCodeDisabledError()
  }

  getExternalPluginIds(): string[] {
    return []
  }

  isExternal(_pluginId: string): boolean {
    return false
  }
}
