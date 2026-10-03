import type { AppPlugin, FlowToolPlugin } from '@flowtools/sdk'
import type { ExecutePluginOptions } from '@flowtools/sdk/execution'

import { PluginErrorBoundary } from '@flowtools/sdk'
import { createExecutionFailure, executePlugin } from '@flowtools/sdk/execution'

import { createWebToolContext, WebPluginRuntimeProvider } from './ctx'

/**
 * Wrap an app plugin panel with runtime ctx provider and error boundary.
 */
export function renderWebAppPlugin(plugin: AppPlugin) {
  const Panel = plugin.setup()

  return (
    <PluginErrorBoundary pluginId={plugin.meta.id}>
      <WebPluginRuntimeProvider
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
 * Execute real app/tool run() with the shared SDK outcome boundary.
 */
export async function runWebPlugin(
  plugin: FlowToolPlugin,
  input: unknown,
  options: ExecutePluginOptions = {}
) {
  let ctx
  try {
    ctx = createWebToolContext({
      permissions: plugin.meta.permissions,
      pluginId: plugin.meta.id,
      pluginType: plugin.type,
      storeShape: plugin.type === 'app' ? plugin.store : undefined,
      signal: options.signal,
      log: () => {},
    })
  } catch {
    return createExecutionFailure(plugin.meta.id, plugin.meta.version, input, {
      code: 'CONTEXT_FAILED',
      message: 'Web runtime context could not be created',
    })
  }
  return executePlugin(plugin, input, ctx, options)
}

export const runWebToolPlugin = runWebPlugin
