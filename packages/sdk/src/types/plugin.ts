import type { CommandResult } from '../result/types'
import type { CommandDef } from './command'
import type { ToolContext } from './ctx'
import type { Permission } from './permissions'
import type { PluginStoreShape } from './store'
import type { ComponentType } from 'react'
import type { z } from 'zod'

type IsTuple<T extends readonly unknown[]> = number extends T['length']
  ? false
  : true

type Includes<T extends readonly unknown[], Item> = T extends readonly [
  infer Head,
  ...infer Tail,
]
  ? [Head] extends [Item]
    ? true
    : Includes<Tail, Item>
  : false

type HasDuplicateItems<
  T extends readonly unknown[],
  Seen extends readonly unknown[] = [],
> =
  IsTuple<T> extends false
    ? false
    : T extends readonly [infer Head, ...infer Tail]
      ? Includes<Seen, Head> extends true
        ? true
        : HasDuplicateItems<Tail, [...Seen, Head]>
      : false

/**
 * Validate a permissions tuple and reject duplicate items.
 * For non-literal arrays, duplicate checks are skipped at type level.
 */
export type UniquePermissions<T extends readonly Permission[] | undefined> =
  T extends readonly Permission[]
    ? HasDuplicateItems<T> extends true
      ? never
      : T
    : T

/**
 * Enforce unique permission declarations in plugin metadata.
 */
export type EnforceUniquePluginPermissions<
  T extends { meta: { permissions?: readonly Permission[] } },
> = T & {
  meta: Omit<T['meta'], 'permissions'> & {
    permissions?: UniquePermissions<T['meta']['permissions']>
  }
}

/**
 * Supported plugin categories in Flow Tool.
 */
export type PluginType = 'app' | 'tool'

/**
 * Generic shape for plugin settings schema.
 */
export type PluginSettingsSchema = Record<string, unknown>

/**
 * Runtime lifecycle callbacks controlled by the host.
 */
export interface PluginLifecycle {
  /**
   * Called when the plugin is loaded.
   */
  onLoad?: () => void | Promise<void>
  /**
   * Called when the plugin is unloaded.
   */
  onUnload?: () => void | Promise<void>
  /**
   * Called when the plugin becomes active.
   */
  onActivate?: () => void | Promise<void>
  /**
   * Called when the plugin becomes inactive.
   */
  onDeactivate?: () => void | Promise<void>
}

/**
 * Basic metadata for every plugin package.
 */
export interface PluginMeta {
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
  /**
   * Optional plugin description.
   */
  description?: string
  /**
   * Optional plugin author.
   */
  author?: string
  /**
   * Optional plugin author link.
   */
  link?: string
  /**
   * Declared capability permissions.
   */
  permissions?: readonly Permission[]
  /**
   * Optional plugin group tags.
   */
  tags?: string[]
  /**
   * Plugin release status.
   */
  status?: 'stable' | 'beta' | 'experimental' | 'deprecated'
  /**
   * Plugin category label.
   */
  category?: string
}

/**
 * Map of command id to command definition.
 */
export type PluginCommands = Record<string, CommandDef<unknown, unknown>>

/**
 * Zod object schema for plugin input.
 * Used for runtime validation and CLI flag generation.
 */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type PluginInputSchema = z.ZodObject<any>

/**
 * Infer the input type from a PluginInputSchema.
 */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type InferInput<S extends PluginInputSchema> = z.infer<S>

/**
 * Shared fields for app and tool plugins.
 */
export interface PluginBase {
  /**
   * Plugin category.
   */
  type: PluginType
  /**
   * Plugin metadata.
   */
  meta: PluginMeta
  /**
   * Declarative Zod schema for plugin input.
   * When defined, CLI auto-generates typed flags and validates input at runtime.
   * When omitted, CLI falls back to `--input <json>`.
   */
  inputSchema?: PluginInputSchema
  /**
   * Optional command list exposed by this plugin.
   */
  commands?: PluginCommands
  /**
   * Optional plugin settings schema.
   */
  settingsSchema?: PluginSettingsSchema
  /**
   * Optional lifecycle handlers.
   */
  lifecycle?: PluginLifecycle
}

/**
 * Persistent panel plugin contract.
 */
export interface AppPlugin extends PluginBase {
  /**
   * App plugin type marker.
   */
  type: 'app'
  /**
   * Optional host-managed store shape with state and actions.
   */
  store?: PluginStoreShape<any, any>
  /**
   * Returns the React panel component rendered by host runtime.
   */
  setup: () => ComponentType
  /**
   * Optional execution entry for CLI and headless invocation.
   * When present, the plugin can be called without rendering its UI.
   * `setup()` can internally reuse the same core logic via shared helper functions.
   */
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  run?: (ctx: ToolContext, input: any) => Promise<CommandResult> | CommandResult
}

/**
 * Instant execution plugin contract.
 */
export interface ToolPlugin<In = never, Out = unknown> extends PluginBase {
  /**
   * Tool plugin type marker.
   */
  type: 'tool'
  /**
   * Tool execution entry.
   */
  run: (ctx: ToolContext, input: In) => Promise<Out> | Out
  /**
   * Tool plugins do not expose panel `setup`.
   */
  setup?: never
}

/**
 * Unified plugin contract.
 */
export type FlowToolPlugin<In = never, Out = unknown> =
  | AppPlugin
  | ToolPlugin<In, Out>
