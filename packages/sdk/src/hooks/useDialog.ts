import { createMissingCapabilityError } from '../runtime/errors'
import { useRuntime } from '../runtime/useRuntime'
import type { DialogCapability } from '../types/capabilities/dialog'

/**
 * Read dialog capability from runtime context.
 */
export function useDialog(): DialogCapability {
  const ctx = useRuntime()

  if (!ctx.dialog) {
    throw createMissingCapabilityError('dialog')
  }

  return ctx.dialog
}
