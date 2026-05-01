import type { PluginStoreShape, PluginStoreState } from './types/store'

/**
 * Extract the state type from a PluginStoreShape.
 */
export type InferStoreState<T> =
  T extends PluginStoreShape<infer S, any> ? S : PluginStoreState

/**
 * Extract the actions type from a PluginStoreShape.
 */
export type InferStoreActions<T> =
  T extends PluginStoreShape<any, infer A> ? A : Record<string, never>
