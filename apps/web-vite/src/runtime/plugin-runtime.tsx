import type { AppPlugin, ToolPlugin } from '@flow-tool/sdk'

import { createWebToolContext, WebPluginRuntimeProvider } from './ctx'

interface RunWebToolPluginOptions {
  signal?: AbortSignal
}

/**
 * Wrap an app plugin panel with runtime ctx provider.
 */
export function renderWebAppPlugin(plugin: AppPlugin) {
  const Panel = plugin.setup()

  return (
    <WebPluginRuntimeProvider
      mode="development"
      permissions={plugin.meta.permissions}
      storeShape={plugin.store}
      pluginId={plugin.meta.id}
      pluginType="app"
    >
      <Panel />
    </WebPluginRuntimeProvider>
  )
}

/**
 * Execute a tool plugin with web runtime ctx.
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

  return plugin.run(ctx, input) as ReturnType<TPlugin['run']>
}
