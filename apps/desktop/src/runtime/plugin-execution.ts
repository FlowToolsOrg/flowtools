import type { ExecutePluginOptions } from '@flowtools/sdk/execution'
import type { FlowToolPlugin, ToolContext } from '@flowtools/sdk/types'

import { createExecutionFailure, executePlugin } from '@flowtools/sdk/execution'

import { createDesktopRuntimeContext } from './desktop-capabilities'

export async function runDesktopPlugin(
  plugin: FlowToolPlugin,
  input: unknown,
  options: ExecutePluginOptions = {}
) {
  let ctx: ToolContext
  try {
    ctx = {
      ...createDesktopRuntimeContext({
        pluginId: plugin.meta.id,
        pluginType: plugin.type,
        permissions: plugin.meta.permissions,
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
  return executePlugin(plugin, input, ctx, options)
}
