import type { AppPlugin, ToolPlugin } from '@flow-tool/sdk'

import { PluginErrorBoundary, withWatchdog } from '@flow-tool/sdk'

import { createWebToolContext, WebPluginRuntimeProvider } from './ctx'

interface RunWebToolPluginOptions {
  signal?: AbortSignal
  timeoutMs?: number
}

/**
 * Wrap an app plugin panel with runtime ctx provider and error boundary.
 */
export function renderWebAppPlugin(plugin: AppPlugin) {
  const Panel = plugin.setup()

  return (
    <PluginErrorBoundary pluginId={plugin.meta.id}>
      <WebPluginRuntimeProvider
        mode="development"
        permissions={plugin.meta.permissions}
        storeShape={plugin.store}
        pluginId={plugin.meta.id}
        pluginType="app"
      >
        <Panel />
      </WebPluginRuntimeProvider>
    </PluginErrorBoundary>
  )
}

/**
 * Execute a tool plugin with web runtime ctx and watchdog.
 */
export function runWebToolPlugin<TPlugin extends ToolPlugin<never, unknown>>(
  plugin: TPlugin,
  input: Parameters<TPlugin['run']>[1],
  options?: RunWebToolPluginOptions
): ReturnType<TPlugin['run']> {
  const ctx = createWebToolContext({
    mode: 'development',
    permissions: plugin.meta.permissions,
    pluginId: plugin.meta.id,
    signal: options?.signal,
  })

  const guardedRun = withWatchdog(plugin, {
    timeoutMs: options?.timeoutMs,
  })

  return guardedRun(ctx, input) as ReturnType<TPlugin['run']>
}
