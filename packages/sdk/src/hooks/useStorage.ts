import { createMissingCapabilityError } from '../runtime/errors'
import { useRuntime } from '../runtime/useRuntime'
import type { StorageCapability } from '../types/capabilities/storage'

/**
 * Read storage capability from runtime context.
 */
export function useStorage(): StorageCapability {
  const ctx = useRuntime()

  if (!ctx.storage) {
    throw createMissingCapabilityError('storage')
  }

  return ctx.storage
}
