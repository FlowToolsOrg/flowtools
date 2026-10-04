import type { AppPlugin, FlowToolPlugin } from '@flowtools/sdk'
import type { ExecutePluginOptions } from '@flowtools/sdk/execution'

import { PluginErrorBoundary, PluginLoader } from '@flowtools/sdk'
import { createExecutionFailure } from '@flowtools/sdk/execution'
import {
  executeManifestCommand,
  parsePluginManifest,
  type PluginManifestV1,
} from '@flowtools/sdk/manifest'

import { builtInManifestData } from '../plugin/manifests'
import { pluginRegistryInternals } from '../stores/plugin-registry-store'

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
  const target = {
    hostVersion: '0.1.0',
    sdkVersion: '0.0.0',
    platform: 'web' as const,
    arch: 'wasm32' as const,
  }
  let manifest: PluginManifestV1
  try {
    manifest = parsePluginManifest(
      builtInManifestData.find(item => item.id === plugin.meta.id),
      target
    )
  } catch {
    return createExecutionFailure(plugin.meta.id, plugin.meta.version, input, {
      code: 'NOT_RUNNABLE',
      message: 'Built-in command manifest is unavailable',
    })
  }
  let ctx
  try {
    ctx = createWebToolContext({
      permissions: plugin.meta.permissions?.filter(permission =>
        manifest.commands[0]?.permissions.some(
          request => request.capability === permission
        )
      ),
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
  return executeManifestCommand(
    manifest,
    'run',
    plugin,
    input,
    ctx,
    target,
    options
  )
}

export const runWebToolPlugin = runWebPlugin

/** GUI executions acquire the current enabled generation, never a captured UI instance. */
export async function runWebRegisteredPlugin(
  pluginId: string,
  input: unknown,
  options: ExecutePluginOptions = {}
) {
  const registry = pluginRegistryInternals._registry
  try {
    if (!registry) throw new Error('Registry unavailable')
    return await new PluginLoader(registry).withPlugin(pluginId, plugin =>
      runWebPlugin(plugin, input, options)
    )
  } catch {
    return createExecutionFailure(
      pluginId,
      registry?.get(pluginId)?.manifest.version ?? '0.0.0',
      input,
      {
        code: 'NOT_RUNNABLE',
        message: 'Plugin is unavailable for execution',
      }
    )
  }
}
