import type { FSCapability } from '../types/capabilities/fs'

import { createMissingCapabilityError } from '../runtime/errors'
import { useRuntime } from '../runtime/useRuntime'

/**
 * Read filesystem capability from runtime context.
 */
export function useFS(): FSCapability {
  const ctx = useRuntime()

  if (!ctx.fs) {
    throw createMissingCapabilityError('fs')
  }

  return ctx.fs
}
