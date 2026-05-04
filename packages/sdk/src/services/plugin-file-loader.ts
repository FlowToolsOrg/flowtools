import type { PluginLoader } from '../registry/plugin-loader'
import type { PluginRegistry } from '../registry/plugin-registry'
import type { PluginManifestEntry, RegisteredPlugin } from '../registry/types'
import type { FlowToolPlugin } from '../types/plugin'

import { needsTranspilation, transpile } from './transpile'

const VALID_EXTENSIONS = ['.js', '.mjs', '.jsx', '.ts', '.tsx']

interface LoadedPluginModule {
  default: FlowToolPlugin
}

/**
 * Service for loading plugins from external files at runtime.
 * Supports browser File objects and direct FlowToolPlugin instances.
 * TSX/TS/JSX files are transpiled with Sucrase at runtime.
 * Bare specifiers are resolved via the document's import map.
 */
export class PluginFileLoader {
  private externalPluginIds = new Set<string>()

  constructor(
    private registry: PluginRegistry,
    private loader: PluginLoader
  ) {}

  /**
   * Load a plugin from a browser File object.
   * The file must be a JS module that default-exports a FlowToolPlugin
   * (created via `definePlugin()`).
   */
  async loadFromFile(file: File): Promise<RegisteredPlugin> {
    this.validateFile(file)

    const source = await this.readFile(file)
    const code = needsTranspilation(file.name)
      ? transpile(source, file.name)
      : source

    const module = await this.importFromCode(code)
    return await this.registerAndEnable(module.default)
  }

  /**
   * Load multiple plugins from File objects.
   */
  async loadFromFiles(files: File[]): Promise<RegisteredPlugin[]> {
    return Promise.all(files.map(f => this.loadFromFile(f)))
  }

  /**
   * Register an already-resolved FlowToolPlugin instance directly.
   */
  async registerExternalPlugin(
    plugin: FlowToolPlugin
  ): Promise<RegisteredPlugin> {
    return await this.registerAndEnable(plugin)
  }

  /**
   * Unload and unregister an externally loaded plugin.
   */
  async unloadExternalPlugin(pluginId: string): Promise<void> {
    if (!this.externalPluginIds.has(pluginId)) {
      throw new Error(
        `[PluginFileLoader] Plugin "${pluginId}" is not an external plugin.`
      )
    }

    const entry = this.registry.get(pluginId)

    if (entry?.state === 'enabled') {
      await this.loader.disable(pluginId)
    }

    this.registry.unregister(pluginId)
    this.externalPluginIds.delete(pluginId)
  }

  /**
   * Get all externally loaded plugin ids.
   */
  getExternalPluginIds(): string[] {
    return [...this.externalPluginIds]
  }

  /**
   * Check if a plugin was loaded externally.
   */
  isExternal(pluginId: string): boolean {
    return this.externalPluginIds.has(pluginId)
  }

  private validateFile(file: File): void {
    const dotIndex = file.name.lastIndexOf('.')
    const ext = dotIndex >= 0 ? file.name.slice(dotIndex).toLowerCase() : ''

    if (!VALID_EXTENSIONS.includes(ext)) {
      throw new Error(
        `[PluginFileLoader] Invalid file extension "${ext || '(none)'}". Expected: ${VALID_EXTENSIONS.join(', ')}`
      )
    }

    if (file.size === 0) {
      throw new Error('[PluginFileLoader] File is empty.')
    }
  }

  private readFile(file: File): Promise<string> {
    return new Promise((resolve, reject) => {
      const reader = new FileReader()

      reader.onload = () => resolve(reader.result as string)
      reader.onerror = () =>
        reject(new Error('[PluginFileLoader] Failed to read file.'))
      reader.readAsText(file)
    })
  }

  /**
   * Import a module from source code.
   * Uses a <script type="module"> injected into the document so that
   * the document's import map can resolve bare specifiers.
   */
  private importFromCode(code: string): Promise<LoadedPluginModule> {
    return new Promise((resolve, reject) => {
      const blob = new Blob([code], { type: 'application/javascript' })
      const blobUrl = URL.createObjectURL(blob)

      const script = document.createElement('script')

      script.type = 'module'
      script.textContent = `
        import * as m from '${blobUrl}';
        window.dispatchEvent(new CustomEvent('__flowtools_plugin_loaded__', { detail: m }));
      `

      const timeout = setTimeout(() => {
        cleanup()
        reject(new Error('[PluginFileLoader] Module import timed out (10s).'))
      }, 10_000)

      function onLoaded(e: Event): void {
        cleanup()
        const module = (e as CustomEvent).detail as LoadedPluginModule

        URL.revokeObjectURL(blobUrl)

        if (!module || typeof module !== 'object' || !('default' in module)) {
          reject(
            new Error(
              '[PluginFileLoader] Module does not have a default export.'
            )
          )
          return
        }

        resolve(module)
      }

      function onError(e: Event): void {
        cleanup()
        URL.revokeObjectURL(blobUrl)
        const msg = e instanceof ErrorEvent ? e.message : 'Unknown import error'

        reject(new Error(`[PluginFileLoader] ${msg}`))
      }

      function cleanup(): void {
        clearTimeout(timeout)
        window.removeEventListener('__flowtools_plugin_loaded__', onLoaded)
        window.removeEventListener('error', onError)
        script.remove()
      }

      window.addEventListener('__flowtools_plugin_loaded__', onLoaded, {
        once: true,
      })
      window.addEventListener('error', onError, { once: true })
      document.head.appendChild(script)
    })
  }

  private validateModule(module: LoadedPluginModule): void {
    const plugin = module.default

    if (!plugin || typeof plugin !== 'object') {
      throw new Error(
        '[PluginFileLoader] Default export is not a valid plugin object.'
      )
    }

    if (!plugin.type || !plugin.meta) {
      throw new Error(
        '[PluginFileLoader] Plugin must have "type" and "meta" fields.'
      )
    }

    if (!plugin.meta.id || !plugin.meta.name || !plugin.meta.version) {
      throw new Error(
        '[PluginFileLoader] Plugin meta must include "id", "name", and "version".'
      )
    }
  }

  private async registerAndEnable(
    plugin: FlowToolPlugin
  ): Promise<RegisteredPlugin> {
    const { id, name, version, description, permissions, tags, category } =
      plugin.meta

    if (this.registry.has(id)) {
      throw new Error(
        `[PluginFileLoader] Plugin "${id}" is already registered. Unload it first.`
      )
    }

    const manifest: PluginManifestEntry = {
      id,
      name,
      version,
      description,
      type: plugin.type,
      permissions: permissions as PluginManifestEntry['permissions'],
      tags,
      category,
      cliAvailable: 'run' in plugin && typeof plugin.run === 'function',
      loader: () => Promise.resolve({ default: plugin }),
    }

    this.registry.register(manifest)
    this.externalPluginIds.add(id)

    try {
      await this.loader.load(id)
      await this.loader.enable(id)
    } catch (error) {
      this.externalPluginIds.delete(id)
      throw error
    }

    return this.registry.get(id)!
  }
}
