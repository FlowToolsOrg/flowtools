import type { PluginRuntimeContextValue } from '../types/ctx'

import { useRuntime } from '../runtime/useRuntime'

export type CapabilitySelector<T> = (ctx: PluginRuntimeContextValue) => T

/**
 * Read runtime capabilities with an optional selector.
 */
export function useCapability(): PluginRuntimeContextValue
export function useCapability<T>(selector: CapabilitySelector<T>): T
export function useCapability<T>(
  selector?: CapabilitySelector<T>
): PluginRuntimeContextValue | T {
  const ctx = useRuntime()

  if (!selector) {
    return ctx
  }

  return selector(ctx)
}
