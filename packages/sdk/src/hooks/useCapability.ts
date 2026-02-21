import type { PluginRuntimeContextValue } from '../types/ctx'

import { useRuntime } from '../runtime/useRuntime'

export type FullCapability = {
  [K in keyof PluginRuntimeContextValue]-?: Exclude<
    PluginRuntimeContextValue[K],
    undefined
  >
}

export type SelectedCapability<T extends Partial<FullCapability>> = T & {
  [K in Exclude<keyof FullCapability, keyof T>]: never
}

export type CapabilitySelector<T extends Partial<FullCapability>> = (
  ctx: FullCapability
) => T

/**
 * Read runtime capabilities with an optional selector.
 */
export function useCapability(): FullCapability
export function useCapability<T extends Partial<FullCapability>>(
  selector: CapabilitySelector<T>
): SelectedCapability<T>
export function useCapability<T extends Partial<FullCapability>>(
  selector?: CapabilitySelector<T>
): FullCapability | SelectedCapability<T> {
  const ctx = useRuntime() as FullCapability

  if (!selector) {
    return ctx
  }

  return selector(ctx) as SelectedCapability<T>
}
