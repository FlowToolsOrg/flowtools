import { createMissingCapabilityError } from '../runtime/errors'
import { useRuntime } from '../runtime/useRuntime'
import type { ClipboardCapability } from '../types/capabilities/clipboard'

/**
 * Read clipboard capability from runtime context.
 */
export function useClipboard(): ClipboardCapability {
  const ctx = useRuntime()

  if (!ctx.clipboard) {
    throw createMissingCapabilityError('clipboard')
  }

  return ctx.clipboard
}
