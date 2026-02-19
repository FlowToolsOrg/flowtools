import type { NativeCapability } from '../types/capabilities/native'

import { createMissingCapabilityError } from '../runtime/errors'
import { useRuntime } from '../runtime/useRuntime'

/**
 * Read native capability from runtime context.
 */
export function useNative(): NativeCapability {
  const ctx = useRuntime()

  if (!ctx.native) {
    throw createMissingCapabilityError('native')
  }

  return ctx.native
}
