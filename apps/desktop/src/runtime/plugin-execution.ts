import type { ExecutePluginOptions } from '@flowtools/sdk/execution'
import type { FlowToolPlugin } from '@flowtools/sdk/types'

import {
  executeRuntimeJob,
  RuntimeExecutionError,
} from '@flowtools/runtime-client'
import { createExecutionFailure } from '@flowtools/sdk/execution'
import {
  parsePluginManifest,
  validateOperationValue,
  isJsonValue,
  type PluginManifestV1,
} from '@flowtools/sdk/manifest'

import { builtInManifestData } from '../plugin/manifests'

import { connectManagedRuntime } from './managed-client'

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
  const startedAt = Date.now()
  const fail = (
    code: 'INPUT_INVALID' | 'ABORTED' | 'TIMEOUT_INVALID',
    message: string
  ) =>
    createExecutionFailure(
      plugin.meta.id,
      plugin.meta.version,
      input,
      { code, message },
      startedAt
    )
  if (options.signal?.aborted) return fail('ABORTED', 'Execution was cancelled')
  const timeout = options.timeoutMs ?? 30000
  if (!Number.isInteger(timeout) || timeout < 1 || timeout > 2147483647)
    return fail('TIMEOUT_INVALID', 'Timeout is outside the supported range')
  const command = manifest.commands.find(command => command.id === 'run')!
  if (
    !isJsonValue(input) ||
    new TextEncoder().encode(JSON.stringify(input)).length >
      command.resources.maxInputBytes
  )
    return fail('INPUT_INVALID', 'Input exceeds the declared command budget')
  const parsed = validateOperationValue(command.inputSchema, input, true)
  if (
    !parsed.success ||
    new TextEncoder().encode(JSON.stringify(parsed.data)).length >
      command.resources.maxInputBytes
  )
    return fail(
      'INPUT_INVALID',
      'Input does not match the declared command schema'
    )
  try {
    const result = await executeRuntimeJob(
      connectManagedRuntime,
      {
        pluginId: manifest.id,
        commandId: 'run',
        input: parsed.data,
        idempotencyKey: crypto.randomUUID(),
        background: false,
        deadline: startedAt + (options.timeoutMs ?? 30000),
      },
      options.signal
    )
    if (result.success)
      return {
        ...result,
        success: true as const,
        data: result.data,
        startedAt: result.startedAt ?? startedAt,
        finishedAt: result.finishedAt ?? Date.now(),
        durationMs: result.durationMs ?? 0,
      }
    return createExecutionFailure(
      plugin.meta.id,
      plugin.meta.version,
      input,
      {
        code:
          result.error?.code === 'ABORTED'
            ? 'ABORTED'
            : result.error?.code === 'TIMEOUT'
              ? 'TIMEOUT'
              : 'CONTEXT_FAILED',
        message: result.error?.code ?? 'INVALID_RESPONSE',
      },
      startedAt
    )
  } catch (error) {
    return createExecutionFailure(
      plugin.meta.id,
      plugin.meta.version,
      input,
      {
        code: 'CONTEXT_FAILED',
        message:
          error instanceof Error && /^[A-Z_]+$/.test(error.message)
            ? error instanceof RuntimeExecutionError
              ? `${error.code}；保留幂等键 ${error.idempotencyKey}${error.runId ? `；任务 ${error.runId}` : ''}`
              : error.message
            : 'RUNTIME_DISCONNECTED',
      },
      startedAt
    )
  }
}
