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
 * State setter passed to action factories.
 */
export type StoreSet<TState extends PluginStoreState> = (
  updater: PluginStoreUpdater<TState>,
  replace?: boolean
) => void

/**
 * State getter passed to action factories.
 */
export type StoreGet<TState extends PluginStoreState> = () => TState

/**
 * Action factory that receives bound set/get and returns named actions.
 */
export type PluginActionsFactory<
  TState extends PluginStoreState,
  TActions extends Record<string, (...args: any[]) => void>,
> = (set: StoreSet<TState>, get: StoreGet<TState>) => TActions

/**
 * Full store shape declared by a plugin: initial state + optional actions.
 */
export interface PluginStoreShape<
  TState extends PluginStoreState = PluginStoreState,
  TActions extends Record<string, (...args: any[]) => void> = Record<
    string,
    never
  >,
> {
  initialState: TState
  actions?: PluginActionsFactory<TState, TActions>
}

/**
 * Host-managed plugin store contract exposed to SDK hooks.
 */
export interface PluginStoreCapability<
  TState extends PluginStoreState = PluginStoreState,
  TActions extends Record<string, (...args: any[]) => void> = Record<
    string,
    never
  >,
> {
  getState: () => TState
  setState: (updater: PluginStoreUpdater<TState>, replace?: boolean) => void
  subscribe: (listener: () => void) => () => void
  reset: () => void
  actions: TActions
}
