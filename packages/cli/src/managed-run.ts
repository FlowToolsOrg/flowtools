import type { PluginRunOptions } from './runner'
import type { JobReceipt } from '@flowtools/runtime-client'

import { RuntimeClientError } from '@flowtools/runtime-client'
import { createExecutionFailure } from '@flowtools/sdk/execution'

import { getBuiltinPluginInfo } from './discovery'
import { connectHost } from './host'
import { userProfile } from './management'

export async function runManagedPlugin(
  pluginId: string,
  input: unknown,
  options: PluginRunOptions
) {
  const startedAt = Date.now()
  const idempotencyKey = crypto.randomUUID()
  const deadline = startedAt + (options.timeout ?? 30000)
  let client: Awaited<ReturnType<typeof connectHost>> | undefined
  try {
    client = await connectHost(userProfile(options.profile), false, true)
    let receipt: JobReceipt
    try {
      receipt = await client.submit({
        pluginId,
        commandId: options.commandId ?? 'run',
        input,
        idempotencyKey,
        background: false,
        deadline,
      })
    } catch (error) {
      if (!(error instanceof RuntimeClientError) || !error.acceptanceUnknown)
        throw error
      client.close()
      client = await connectHost(userProfile(options.profile))
      const outcome = await client.call({
        method: 'jobs.lookup',
        payload: { idempotencyKey },
      })
      if (outcome.type !== 'receipt') throw new Error('ACCEPTANCE_UNKNOWN')
      receipt = outcome.data
    }
    while (true) {
      const job = await client.job(receipt.runId)
      if (job.result) {
        if (job.result.success)
          return {
            ...job.result,
            success: true as const,
            data: job.result.data,
          }
        if (!job.result.error) throw new Error('INVALID_RESPONSE')
        return {
          ...job.result,
          success: false as const,
          error: job.result.error,
        }
      }
      if (
        ['succeeded', 'failed', 'cancelled', 'interrupted'].includes(job.state)
      )
        throw new Error('INVALID_RESPONSE')
      if (options.signal?.aborted || Date.now() > deadline)
        await client.cancel(receipt.runId)
      await new Promise(resolve => setTimeout(resolve, 25))
    }
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
    }
  } finally {
    client?.close()
  }
}
