import type { PluginMaturity } from '../types/maturity'
import type { Permission } from '../types/permissions'
import type { FlowToolPlugin, PluginType } from '../types/plugin'

/**
 * Runtime state of a registered plugin.
 */
export type PluginState =
  | 'registered'
  | 'loading'
  | 'loaded'
  | 'enabled'
  | 'disabled'
  | 'error'

/**
 * Plugin manifest entry used for registration.
 * Both built-in and external plugins share this shape.
 */
export interface PluginManifestEntry {
  /**
   * Stable plugin id in kebab-case.
   */
  id: string
  /**
   * Human readable plugin name.
   */
  name: string
  /**
   * Semver version string.
   */
  version: string
  /** Missing metadata is prototype; this is separate from compatibility evidence. */
  maturity?: PluginMaturity
  /**
   * Optional plugin description.
   */
  description?: string
  /**
   * Plugin category: 'app' (panel) or 'tool' (headless).
   */
  type: PluginType
  /**
   * Declared capability permissions.
   */
  permissions?: readonly Permission[]
  /**
   * Optional tags for search and categorization.
   */
  tags?: string[]
  /**
   * Optional category label.
   */
  category?: string
  /**
   * Whether this plugin has a `run()` function and is CLI-compatible.
   */
  cliAvailable?: boolean
  /** Host-resolved availability only; dependency resolution ships in G4. */
  dependenciesSatisfied?: boolean
  /**
   * Async loader that returns the plugin module.
   * Built-in plugins use relative import(), external plugins use URL import().
   */
  loader: () => Promise<{ default: FlowToolPlugin }>
}

/**
 * Wrapper for a plugin registered in the registry.
 */
export interface RegisteredPlugin {
  /**
   * Plugin id from manifest.
   */
  id: string
  /**
   * Original manifest entry.
   */
  manifest: PluginManifestEntry
  /**
   * Loaded plugin instance (available after loading).
   */
  plugin?: FlowToolPlugin
  /**
   * Current lifecycle state.
   */
  state: PluginState
  /** Monotonic loaded instance generation, even when imports reuse a singleton. */
  generation: number
  /**
   * Error if state is 'error'.
   */
  error?: Error
  /** Cleanup failures are separate from the original lifecycle failure. */
  cleanupErrors?: readonly Error[]
  /**
   * Timestamp when plugin was loaded.
   */
  loadedAt?: number
  /**
   * Timestamp when plugin was enabled.
   */
  enabledAt?: number
}

/**
 * Events emitted by the plugin registry.
 */
export type PluginRegistryEvent =
  | { type: 'registered'; pluginId: string }
  | { type: 'loading'; pluginId: string }
  | { type: 'loaded'; pluginId: string }
  | { type: 'enabled'; pluginId: string }
  | { type: 'disabled'; pluginId: string }
  | { type: 'unloaded'; pluginId: string }
  | { type: 'error'; pluginId: string; error: Error }

/**
 * Listener function for registry events.
 */
export type PluginRegistryListener = (event: PluginRegistryEvent) => void
