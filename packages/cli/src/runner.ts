import type { OutputFormat } from './types'
import type { ExecutionResult } from '@flowtools/runtime-client'
import type {
  ExecutablePlugin,
  PluginExecutionResult,
} from '@flowtools/sdk/execution'
import type { Permission, ToolContext } from '@flowtools/sdk/types'

import { createExecutionFailure, executePlugin } from '@flowtools/sdk/execution'
import {
  executeManifestCommand,
  type PluginManifestV1,
} from '@flowtools/sdk/manifest'

import {
  loadPlugin,
  cliManifestTarget,
  getBuiltinPluginInfo,
} from './discovery'
import { formatRaw, formatResult } from './formatter'

export type RunResult =
  | PluginExecutionResult
  | (Omit<ExecutionResult, 'success' | 'data' | 'error'> &
      (
        | { success: true; data: unknown }
        | { success: false; error: { code: string } }
      ))
  | (Omit<PluginExecutionResult, 'error' | 'success'> & {
      success: false
      error: { code: string; message: string }
    })
interface CLIExecutablePlugin extends ExecutablePlugin {
  manifest?: PluginManifestV1
  type?: 'app' | 'tool'
  meta: ExecutablePlugin['meta'] & { permissions?: readonly Permission[] }
}
export interface PluginRunnerDependencies {
  loadPlugin: (pluginId: string) => Promise<CLIExecutablePlugin | null>
  createContext: (pluginId: string, plugin: CLIExecutablePlugin) => ToolContext
  startTimeout?: (callback: () => void, delayMs: number) => () => void
}
export interface PluginRunOptions {
  profile?: string
  commandId?: string
  timeout?: number
  signal?: AbortSignal
}

export function createPluginRunner(dependencies: PluginRunnerDependencies) {
  return async (
    pluginId: string,
    input: unknown,
    options: PluginRunOptions = {}
  ): Promise<PluginExecutionResult> => {
    const startedAt = Date.now()
    const failure = (
      code: 'PLUGIN_NOT_FOUND' | 'LOAD_FAILED' | 'CONTEXT_FAILED',
      message: string,
      version: string | null = null
    ) =>
      createExecutionFailure(
        pluginId,
        version,
        input,
        { code, message },
        startedAt
      )
    let plugin: CLIExecutablePlugin | null
    try {
      plugin = await dependencies.loadPlugin(pluginId)
    } catch (error) {
      return failure(
        'LOAD_FAILED',
        error instanceof Error ? error.message : String(error)
      )
    }
    if (!plugin)
      return failure('PLUGIN_NOT_FOUND', 'Plugin not found: ' + pluginId)
    if (!plugin.manifest && options.commandId && options.commandId !== 'run')
      return createExecutionFailure(
        pluginId,
        plugin.meta.version,
        input,
        {
          code: 'NOT_RUNNABLE',
          message: 'Legacy adapters expose only the run command',
        },
        startedAt
      )
    let ctx: ToolContext
    try {
      ctx = dependencies.createContext(pluginId, plugin)
    } catch (error) {
      return failure(
        'CONTEXT_FAILED',
        error instanceof Error ? error.message : String(error),
        plugin.meta.version
      )
    }
    const executionOptions = {
      timeoutMs: options.timeout,
      signal: options.signal,
      startTimeout: dependencies.startTimeout,
    }
    return plugin.manifest
      ? executeManifestCommand(
          plugin.manifest,
          options.commandId ?? 'run',
          plugin,
          input,
          ctx,
          cliManifestTarget,
          executionOptions
        )
      : executePlugin(plugin, input, ctx, executionOptions)
  }
}

export const runPlugin = async (
  pluginId: string,
  input: unknown,
  options: PluginRunOptions = {}
): Promise<RunResult> => {
  const info = getBuiltinPluginInfo(pluginId)
  if (!info)
    return createExecutionFailure(pluginId, null, input, {
      code: 'PLUGIN_NOT_FOUND',
      message: 'Plugin is not in the fixed inventory',
    })
  try {
    if (!(await loadPlugin(pluginId))) throw new Error('LOAD_FAILED')
  } catch {
    return createExecutionFailure(pluginId, info.version, input, {
      code: 'LOAD_FAILED',
      message: 'Compiled built-in contract unavailable',
    })
  }
  const { runManagedPlugin } = await import('./managed-run')
  return runManagedPlugin(pluginId, input, options)
}

/** JSON output is always the shared envelope, including failure metadata. */
export function printExecutionResult(
  result: RunResult,
  format: OutputFormat
): number {
  if (format === 'json') {
    let output: string
    try {
      output = JSON.stringify(result, null, 2)
    } catch {
      const failure = createExecutionFailure(
        result.pluginId,
        result.pluginVersion,
        null,
        {
          code: 'OUTPUT_INVALID',
          message: 'Plugin output is not JSON serializable',
        },
        result.startedAt ?? undefined
      )
      return printExecutionResult(
        { ...failure, inputSummary: result.inputSummary },
        format
      )
    }
    process.stdout.write(output + '\n')
  } else if (result.success) {
    const output =
      result.data && typeof result.data === 'object' && 'type' in result.data
        ? formatResult(
            result.data as Parameters<typeof formatResult>[0],
            format
          )
        : formatRaw(result.data, format)
    if (output) process.stdout.write(output + '\n')
  }
  if (!result.success) {
    process.stderr.write(
      '[' +
        (result.error?.code ?? 'INVALID_RESPONSE') +
        '] ' +
        (result.error && 'message' in result.error
          ? result.error.message
          : (result.error?.code ?? 'INVALID_RESPONSE')) +
        '\n'
    )
    return 1
  }
  return 0
}

export async function runPluginAndPrint(
  pluginId: string,
  input: unknown,
  format: OutputFormat = 'json',
  options?: PluginRunOptions
): Promise<number> {
  return printExecutionResult(await runPlugin(pluginId, input, options), format)
}
