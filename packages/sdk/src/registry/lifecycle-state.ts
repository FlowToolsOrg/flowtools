import type { PluginState } from './types'

/** Recovery is explicit: unload/reload must clean up before registered. */
export const pluginStateTransitions: Readonly<
  Record<PluginState, readonly PluginState[]>
> = {
  registered: ['loading', 'error'],
  loading: ['loaded', 'error'],
  loaded: ['enabled', 'registered', 'error'],
  enabled: ['disabled', 'error'],
  disabled: ['enabled', 'registered', 'error'],
  error: ['registered'],
}

export class PluginLifecycleError extends Error {
  readonly code = 'LIFECYCLE_CONFLICT'

  constructor(pluginId: string, reason: string) {
    super(`[PluginLoader] Plugin "${pluginId}" ${reason}`)
    this.name = 'PluginLifecycleError'
  }
}

export class PluginCleanupError extends Error {
  readonly code = 'LIFECYCLE_CLEANUP_FAILED'

  constructor(readonly errors: readonly Error[]) {
    super('Plugin cleanup failed')
    this.name = 'PluginCleanupError'
  }
}
