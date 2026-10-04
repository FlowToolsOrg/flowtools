import type {
  ExecutablePlugin,
  ExecutePluginOptions,
} from '../execution/executor'
import type { ToolContext } from '../types/ctx'
import type { ManifestTarget, PluginManifestV1 } from './schema'

import { createExecutionFailure, executePlugin } from '../execution/executor'

import { isJsonValue, validateOperationValue } from './json-schema'
import { parsePluginManifest } from './schema'

/** Explicit transitional adapter; a legacy function object is never serialized. */
export async function executeManifestCommand<T>(
  value: PluginManifestV1,
  commandId: string,
  plugin: ExecutablePlugin<T>,
  input: unknown,
  ctx: ToolContext,
  target: ManifestTarget,
  options: ExecutePluginOptions = {}
) {
  const startedAt = Date.now()
  const fail = (
    code:
      | 'INPUT_INVALID'
      | 'OUTPUT_INVALID'
      | 'NOT_RUNNABLE'
      | 'PLUGIN_ID_MISMATCH'
      | 'TIMEOUT_INVALID',
    message: string
  ) =>
    createExecutionFailure(
      plugin.meta.id,
      plugin.meta.version,
      input,
      { code, message },
      startedAt
    )
  let manifest: PluginManifestV1
  try {
    manifest = parsePluginManifest(value, target)
  } catch {
    return fail('NOT_RUNNABLE', 'Invalid or incompatible command manifest')
  }
  if (
    manifest.id !== plugin.meta.id ||
    manifest.version !== plugin.meta.version
  )
    return fail(
      'PLUGIN_ID_MISMATCH',
      'Manifest does not match executor identity'
    )
  const command = manifest.commands.find(item => item.id === commandId)
  if (!command || !command.headless || command.interaction === 'required')
    return fail(
      'NOT_RUNNABLE',
      'Command requires an available execution/interaction adapter'
    )
  if (
    command.runtimeValidation.input === 'required' &&
    typeof plugin.inputSchema?.safeParse !== 'function'
  )
    return fail('NOT_RUNNABLE', 'Required input runtime validator is missing')
  if (
    command.runtimeValidation.output === 'required' &&
    typeof plugin.outputSchema?.safeParse !== 'function'
  )
    return fail('NOT_RUNNABLE', 'Required output runtime validator is missing')
  if (
    !isJsonValue(input) ||
    new TextEncoder().encode(JSON.stringify(input)).length >
      command.resources.maxInputBytes
  )
    return fail('INPUT_INVALID', 'Input exceeds the command JSON budget')
  const parsed = validateOperationValue(command.inputSchema, input, true)
  if (!parsed.success)
    return fail('INPUT_INVALID', 'Input does not match command schema')
  if (
    new TextEncoder().encode(JSON.stringify(parsed.data)).length >
    command.resources.maxInputBytes
  )
    return fail(
      'INPUT_INVALID',
      'Defaulted input exceeds the command JSON budget'
    )
  if (
    options.timeoutMs !== undefined &&
    (!Number.isInteger(options.timeoutMs) ||
      options.timeoutMs < 1 ||
      options.timeoutMs > 2_147_483_647)
  )
    return fail(
      'TIMEOUT_INVALID',
      'Timeout must be a positive integer within the timer range'
    )
  const result = await executePlugin(plugin, parsed.data, ctx, {
    ...options,
    timeoutMs: Math.min(
      options.timeoutMs ?? command.resources.timeoutMs,
      command.resources.timeoutMs
    ),
  })
  if (!result.success) return result
  if (
    !isJsonValue(result.data) ||
    new TextEncoder().encode(JSON.stringify(result.data)).length >
      command.resources.maxOutputBytes ||
    !validateOperationValue(command.outputSchema, result.data).success
  )
    return createExecutionFailure(
      manifest.id,
      manifest.version,
      input,
      {
        code: 'OUTPUT_INVALID',
        message: 'Output does not match command schema or JSON budget',
      },
      result.startedAt
    )
  return result
}
