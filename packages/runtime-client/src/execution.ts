import type { ExecutionResult, JobReceipt, SubmitJob } from './bindings'

import { RuntimeClient, RuntimeClientError } from './client'

export class RuntimeExecutionError extends RuntimeClientError {
  constructor(
    code: RuntimeClientError['code'],
    readonly idempotencyKey: string,
    readonly runId?: string
  ) {
    super(code, code === 'ACCEPTANCE_UNKNOWN')
  }
}

/** A lost acknowledgement is queried with the same key; never resubmit effects. */
export async function withSubmittedRuntimeJob<T>(
  connect: () => Promise<RuntimeClient>,
  job: SubmitJob,
  consume: (client: RuntimeClient, receipt: JobReceipt) => Promise<T>,
  signal?: AbortSignal
): Promise<T> {
  if (job.deadline === null || !Number.isFinite(job.deadline))
    throw new RuntimeExecutionError('INPUT_INVALID', job.idempotencyKey)
  if (signal?.aborted)
    throw new RuntimeExecutionError('ABORTED', job.idempotencyKey)
  let client: RuntimeClient | undefined
  let receipt: JobReceipt | undefined
  let uncertain = false
  try {
    client = await connect()
    try {
      receipt = await client.submit(job)
    } catch (error) {
      if (!(error instanceof RuntimeClientError) || !error.acceptanceUnknown)
        throw error
      uncertain = true
      client.close()
      client = await connect()
      const found = await client.call({
        method: 'jobs.lookup',
        payload: { idempotencyKey: job.idempotencyKey },
      })
      if (found.type !== 'receipt')
        throw new RuntimeClientError('ACCEPTANCE_UNKNOWN')
      receipt = found.data
      uncertain = false
    }
    return await consume(client, receipt)
  } catch (error) {
    // Once accepted, a broken connection cannot establish whether an effect committed.
    const code =
      uncertain || receipt
        ? 'ACCEPTANCE_UNKNOWN'
        : error instanceof RuntimeClientError
          ? error.code
          : 'RUNTIME_DISCONNECTED'
    throw new RuntimeExecutionError(code, job.idempotencyKey, receipt?.runId)
  } finally {
    client?.close()
  }
}

/** Own the foreground connection until a real terminal result is available. */
export function executeRuntimeJob(
  connect: () => Promise<RuntimeClient>,
  job: SubmitJob,
  signal?: AbortSignal
): Promise<ExecutionResult> {
  return withSubmittedRuntimeJob(
    connect,
    job,
    async (client, receipt) => {
      while (true) {
        const snapshot = await client.job(receipt.runId)
        if (snapshot.result) return snapshot.result
        if (
          ['succeeded', 'failed', 'cancelled', 'interrupted'].includes(
            snapshot.state
          )
        )
          throw new RuntimeClientError('INVALID_RESPONSE')
        if (signal?.aborted || Date.now() >= (job.deadline ?? 0))
          await client.cancel(receipt.runId)
        await new Promise<void>(resolve => setTimeout(resolve, 25))
      }
    },
    signal
  )
}
