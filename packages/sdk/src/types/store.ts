/**
 * Generic plugin store shape.
 */
export type PluginStoreState = Record<string, unknown>

/**
 * Store state updater signature.
 */
export type PluginStoreUpdater<TState extends PluginStoreState> =
  | TState
  | Partial<TState>
  | ((state: TState) => TState | Partial<TState>)

/**
 * Host-managed plugin store contract exposed to SDK hooks.
 */
export interface PluginStoreCapability<
  TState extends PluginStoreState = PluginStoreState,
> {
  /**
   * Read current store state snapshot.
   */
  getState: () => TState
  /**
   * Update state using partial patch / full state / updater function.
   */
  setState: (updater: PluginStoreUpdater<TState>, replace?: boolean) => void
  /**
   * Subscribe to store changes.
   */
  subscribe: (listener: () => void) => () => void
  /**
   * Reset state to host-defined initial state.
   */
  reset: () => void
}
