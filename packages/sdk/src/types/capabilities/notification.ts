/**
 * Notification severity category.
 */
export type NotificationLevel = 'info' | 'success' | 'warning' | 'error'

/**
 * Notification payload.
 */
export interface NotificationInput {
  /**
   * Notification title.
   */
  title: string
  /**
   * Optional detail message.
   */
  message?: string
  /**
   * Visual severity level.
   */
  level?: NotificationLevel
}

/**
 * Notification capability contract.
 */
export interface NotificationCapability {
  /**
   * Send a system notification.
   */
  notify: (input: NotificationInput) => Promise<void> | void
}
