import type { ExecutePluginOptions } from '@flowtools/sdk/execution'
import type { FlowToolPlugin, ToolContext } from '@flowtools/sdk/types'

import { createExecutionFailure } from '@flowtools/sdk/execution'
import {
  executeManifestCommand,
  parsePluginManifest,
  type PluginManifestV1,
} from '@flowtools/sdk/manifest'

import { builtInManifestData } from '../plugin/manifests'

import { createDesktopRuntimeContext } from './desktop-capabilities'

export async function runDesktopPlugin(
  plugin: FlowToolPlugin,
  input: unknown,
  options: ExecutePluginOptions = {}
) {
  const target = {
    hostVersion: '0.1.0',
    sdkVersion: '0.0.0',
    platform: 'windows' as const,
    arch: 'x64' as const,
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
  let ctx: ToolContext
  try {
    ctx = {
      ...createDesktopRuntimeContext({
        pluginId: plugin.meta.id,
        pluginType: plugin.type,
        permissions: plugin.meta.permissions?.filter(permission =>
          manifest.commands[0]?.permissions.some(
            request => request.capability === permission
          )
        ),
        storeShape: plugin.type === 'app' ? plugin.store : undefined,
      }),
      signal: options.signal ?? new AbortController().signal,
      log: () => {},
    }
  } catch {
    return createExecutionFailure(plugin.meta.id, plugin.meta.version, input, {
      code: 'CONTEXT_FAILED',
      message: 'Desktop runtime context could not be created',
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
