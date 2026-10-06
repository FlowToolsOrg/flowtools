import type { PluginRunOptions } from './runner'

import {
  executeRuntimeJob,
  RuntimeExecutionError,
} from '@flowtools/runtime-client'
import { createExecutionFailure } from '@flowtools/sdk/execution'

import { getBuiltinPluginInfo } from './discovery'
import { connectHost } from './host'
import { userProfile } from './native-runtime'

export async function runManagedPlugin(
  pluginId: string,
  input: unknown,
  options: PluginRunOptions
) {
  const startedAt = Date.now()
  const idempotencyKey = crypto.randomUUID()
  const deadline = startedAt + (options.timeout ?? 30000)
  try {
    const result = await executeRuntimeJob(
      () => connectHost(userProfile(options.profile), false, true),
      {
        pluginId,
        commandId: options.commandId ?? 'run',
        input,
        idempotencyKey,
        background: false,
        deadline,
      },
      options.signal
    )
    if (result.success)
      return { ...result, success: true as const, data: result.data }
    if (!result.error) throw new Error('INVALID_RESPONSE')
    return { ...result, success: false as const, error: result.error }
  } catch (error) {
    const code =
      error instanceof Error && /^[A-Z_]+$/.test(error.message)
        ? error.message
        : 'SETUP_REQUIRED'
    const failure = createExecutionFailure(
      pluginId,
      getBuiltinPluginInfo(pluginId)?.version ?? null,
      input,
      {
        code: 'CONTEXT_FAILED',
        message: 'Runtime could not complete the command',
      },
      startedAt
    )
    return {
      ...failure,
      success: false as const,
      error: { code, message: code },
      idempotencyKey,
      runId: error instanceof RuntimeExecutionError ? error.runId : undefined,
    }
  }
}
