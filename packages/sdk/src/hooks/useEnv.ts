import type { PluginEnv } from '../types/ctx'
import { useRuntime } from '../runtime/useRuntime'

/**
 * Read plugin environment from runtime context.
 */
export function useEnv(): PluginEnv {
  const ctx = useRuntime()

  return ctx.env
}
