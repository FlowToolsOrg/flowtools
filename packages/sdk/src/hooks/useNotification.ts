import { createMissingCapabilityError } from '../runtime/errors'
import { useRuntime } from '../runtime/useRuntime'
import type { NotificationCapability } from '../types/capabilities/notification'

/**
 * Read notification capability from runtime context.
 */
export function useNotification(): NotificationCapability {
  const ctx = useRuntime()

  if (!ctx.notification) {
    throw createMissingCapabilityError('notification')
  }

  return ctx.notification
}
