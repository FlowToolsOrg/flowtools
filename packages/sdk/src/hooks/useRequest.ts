import type { RequestCapability } from '../types/capabilities/request'

import { createMissingCapabilityError } from '../runtime/errors'
import { useRuntime } from '../runtime/useRuntime'

/**
 * Read request capability from runtime context.
 */
export function useRequest(): RequestCapability {
  const ctx = useRuntime()

  if (!ctx.request) {
    throw createMissingCapabilityError('network')
  }

  return ctx.request
}
