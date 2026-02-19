import { createMissingCapabilityError } from '../runtime/errors'
import { useRuntime } from '../runtime/useRuntime'
import type { DBCapability } from '../types/capabilities/db'

/**
 * Read database capability from runtime context.
 */
export function useDB(): DBCapability {
  const ctx = useRuntime()

  if (!ctx.db) {
    throw createMissingCapabilityError('db')
  }

  return ctx.db
}
