import type { ClipboardCapability } from './capabilities/clipboard'
import type { DBCapability } from './capabilities/db'
import type { DialogCapability } from './capabilities/dialog'
import type { FSCapability } from './capabilities/fs'
import type { NativeCapability } from './capabilities/native'
import type { NotificationCapability } from './capabilities/notification'
import type { RequestCapability } from './capabilities/request'
import type { StorageCapability } from './capabilities/storage'
import type { UICapability } from './ui'

/**
 * Runtime platform type.
 */
export type RuntimePlatform = 'desktop' | 'web' | 'unknown'

/**
 * Runtime environment mode.
 */
export type RuntimeMode = 'development' | 'production' | 'test'

/**
 * Environment information exposed to plugins.
 */
export interface PluginEnv {
  /**
   * Current plugin id.
   */
  pluginId: string
  /**
   * Current plugin category.
   */
  pluginType: 'app' | 'tool'
  /**
   * Host runtime platform.
   */
  platform: RuntimePlatform
  /**
   * Runtime mode.
   */
  mode: RuntimeMode
}

/**
 * Shared utility methods injected by host runtime.
 */
export interface RuntimeUtils {
  /**
   * Return current unix timestamp in milliseconds.
   */
  now: () => number
}

/**
 * Runtime context used by SDK hooks in app plugins.
 */
export interface PluginRuntimeContextValue {
  /**
   * Plugin environment metadata.
   */
  env: PluginEnv
  /**
   * UI interaction capability.
   */
  ui: UICapability
  /**
   * Optional filesystem capability.
   */
  fs?: FSCapability
  /**
   * Optional network request capability (`fetch` compatible).
   */
  fetch?: RequestCapability
  /**
   * Optional network request capability.
   * @deprecated Use `fetch` instead.
   */
  request?: RequestCapability
  /**
   * Optional clipboard capability.
   */
  clipboard?: ClipboardCapability
  /**
   * Optional dialog capability.
   */
  dialog?: DialogCapability
  /**
   * Optional notification capability.
   */
  notification?: NotificationCapability
  /**
   * Optional namespaced storage capability.
   */
  storage?: StorageCapability
  /**
   * Optional namespaced database capability.
   */
  db?: DBCapability
  /**
   * Optional native host capability.
   */
  native?: NativeCapability
  /**
   * Shared runtime helper methods.
   */
  utils: RuntimeUtils
}

/**
 * Log level supported by tool runtime logger.
 */
export type ToolLogLevel = 'debug' | 'info' | 'warn' | 'error'

/**
 * Tool runtime log function signature.
 */
export type ToolLogger = (
  level: ToolLogLevel,
  message: string,
  details?: Record<string, unknown>
) => void

/**
 * Tool execution context passed to `run(ctx, input)`.
 */
export interface ToolContext extends PluginRuntimeContextValue {
  /**
   * Abort signal for cancelable executions.
   */
  signal: AbortSignal
  /**
   * Structured logger for host-visible tool logs.
   */
  log: ToolLogger
}
