/**
 * Toast severity category.
 */
export type ToastLevel = 'info' | 'success' | 'warning' | 'error'

/**
 * Toast payload shown by host UI.
 */
export interface ToastInput {
  /**
   * Toast title text.
   */
  title: string
  /**
   * Optional body text.
   */
  message?: string
  /**
   * Optional display level.
   */
  level?: ToastLevel
}

/**
 * Panel open options.
 */
export interface OpenPanelOptions {
  /**
   * Optional command or panel id.
   */
  panelId?: string
  /**
   * Optional initial params.
   */
  params?: Record<string, unknown>
}

/**
 * UI capability contract provided by host runtime.
 */
export interface UICapability {
  /**
   * Show a toast in host UI.
   */
  toast: (input: ToastInput) => void
  /**
   * Open a plugin panel.
   */
  openPanel: (options?: OpenPanelOptions) => void
  /**
   * Close current or target panel.
   */
  closePanel: (panelId?: string) => void
}
