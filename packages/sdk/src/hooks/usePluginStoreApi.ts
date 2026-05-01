import type { PluginStoreCapability, PluginStoreState } from '../types/store'

import { createMissingPluginStoreError } from '../runtime/errors'
import { useRuntime } from '../runtime/useRuntime'

/**
 * Read host-managed plugin store API.
 */
export function usePluginStoreApi<
  TState extends PluginStoreState = PluginStoreState,
  TActions extends Record<string, (...args: any[]) => void> = Record<
    string,
    never
  >,
>(): PluginStoreCapability<TState, TActions> {
  const ctx = useRuntime()

  if (!ctx.store) {
    throw createMissingPluginStoreError()
  }

  return ctx.store as PluginStoreCapability<TState, TActions>
}
