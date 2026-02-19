import type { UICapability } from '../types/ui'

import { useRuntime } from '../runtime/useRuntime'

/**
 * Read UI capability from runtime context.
 */
export function useUI(): UICapability {
  const ctx = useRuntime()

  return ctx.ui
}
