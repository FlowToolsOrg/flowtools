import type { PluginStoreState } from '../types/store'

import { useSyncExternalStore } from 'react'

import { usePluginStoreApi } from './usePluginStoreApi'

type StoreSelector<TState extends PluginStoreState, TSlice> = (
  state: TState
) => TSlice

/**
 * Read host-managed plugin store state.
 */
export function usePluginStore<
  TState extends PluginStoreState = PluginStoreState,
>(): TState
export function usePluginStore<TState extends PluginStoreState, TSlice>(
  selector: (state: TState) => TSlice
): TSlice
export function usePluginStore<TState extends PluginStoreState, TSlice>(
  selector?: (state: TState) => TSlice
): TSlice | TState {
  const store = usePluginStoreApi<TState>()
  const select = (selector ??
    ((state: TState) => state as unknown as TSlice)) as StoreSelector<
    TState,
    TSlice
  >

  return useSyncExternalStore(
    store.subscribe,
    () => select(store.getState()),
    () => select(store.getState())
  )
}
