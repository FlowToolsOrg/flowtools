import type { PluginStoreCapability, PluginStoreState } from '../types/store'

import { createMissingPluginStoreError } from '../runtime/errors'
import { useRuntime } from '../runtime/useRuntime'

/**
 * Read host-managed plugin store API.
 */
export function usePluginStoreApi<
  TState extends PluginStoreState = PluginStoreState,
>(): PluginStoreCapability<TState> {
  const ctx = useRuntime()

  if (!ctx.store) {
    throw createMissingPluginStoreError()
  }

  return ctx.store as PluginStoreCapability<TState>
}
