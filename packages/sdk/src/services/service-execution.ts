import type {
  ExecutablePlugin,
  ExecutePluginOptions,
  ExecutionErrorCode,
} from '../execution'
import type { PluginManifestV1, ManifestTarget, JsonValue } from '../manifest'
import type { ToolContext } from '../types/ctx'

import { z } from 'zod'

import { dependencyIdSchema } from '../dependencies'
import { createExecutionFailure, executePlugin } from '../execution'
import {
  isJsonValue,
  parsePluginManifest,
  validateOperationValue,
} from '../manifest'

/** Logical selectors only; version, package, caller, grants and paths belong to Host. */
export const serviceTargetSchema = z.strictObject({
  publisher: dependencyIdSchema,
  id: dependencyIdSchema,
  service: dependencyIdSchema,
  operation: dependencyIdSchema,
})
export type ServiceTarget = z.infer<typeof serviceTargetSchema>
export interface ServiceCapability {
  call(target: ServiceTarget, input: JsonValue): Promise<JsonValue>
}
export type ServiceImplementations = Readonly<
  Record<string, Readonly<Record<string, ExecutablePlugin>>>
>

/** Execution helper only; possession of a handler never grants permission to run it. */
export async function executeManifestService(
  value: PluginManifestV1,
  serviceId: string,
  operationId: string,
  implementation: ExecutablePlugin,
  input: unknown,
  ctx: ToolContext,
  target: ManifestTarget,
  options: ExecutePluginOptions = {}
) {
  const fail = (code: ExecutionErrorCode) =>
    createExecutionFailure(
      implementation.meta.id,
      implementation.meta.version,
      input,
      {
        code,
        message: code,
      }
    )
  let manifest: PluginManifestV1
  try {
    manifest = parsePluginManifest(value, target)
  } catch {
    return fail('NOT_RUNNABLE')
  }
  const operation = manifest.services
    ?.find(service => service.id === serviceId)
    ?.operations.find(operation => operation.id === operationId)
  if (!operation || !operation.headless || operation.interaction !== 'none')
    return fail('NOT_RUNNABLE')
  if (
    implementation.meta.id !== manifest.id ||
    implementation.meta.version !== manifest.version
  )
    return fail('PLUGIN_ID_MISMATCH')
  if (
    (operation.runtimeValidation.input === 'required' &&
      !implementation.inputSchema) ||
    (operation.runtimeValidation.output === 'required' &&
      !implementation.outputSchema)
  )
    return fail('NOT_RUNNABLE')
  const bytes = (value: unknown) =>
    new TextEncoder().encode(JSON.stringify(value)).length
  if (!isJsonValue(input) || bytes(input) > operation.resources.maxInputBytes)
    return fail('INPUT_INVALID')
  const prepared = validateOperationValue(operation.inputSchema, input, true)
  if (
    !prepared.success ||
    bytes(prepared.data) > operation.resources.maxInputBytes
  )
    return fail('INPUT_INVALID')
  const result = await executePlugin(implementation, prepared.data, ctx, {
    ...options,
    timeoutMs: Math.min(
      options.timeoutMs ?? operation.resources.timeoutMs,
      operation.resources.timeoutMs
    ),
  })
  if (
    result.success &&
    (!isJsonValue(result.data) ||
      bytes(result.data) > operation.resources.maxOutputBytes ||
      !validateOperationValue(operation.outputSchema, result.data).success)
  )
    return fail('OUTPUT_INVALID')
  return result
}
