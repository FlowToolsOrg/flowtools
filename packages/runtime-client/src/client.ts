import type {
  Call,
  ErrorCode,
  JobEvent,
  JobReceipt,
  JobSnapshot,
  Outcome,
  Request,
  Response,
  SessionProof,
  SubmitJob,
} from './bindings'

import { decodeResponse, encodeRequest } from './codec'

export interface RuntimeTransport {
  exchange(request: Request): Promise<Response>
  close(): void
}

export class RuntimeClientError extends Error {
  constructor(
    readonly code: ErrorCode | 'INVALID_RESPONSE',
    readonly acceptanceUnknown = false
  ) {
    super(code)
    this.name = 'RuntimeClientError'
  }
}

/** No independent plugin/task/grant stores; queries are Runtime-owned facts. */
export class RuntimeClient {
  private proof: SessionProof | null = null
  constructor(private transport: RuntimeTransport) {}

  get instanceId(): string | undefined {
    return this.proof?.instanceId
  }

  async connect(
    token: string,
    expectedInstanceId: string | null = null
  ): Promise<void> {
    const outcome = await this.call({
      method: 'session.open',
      payload: { token, clientVersion: '0.1.0', expectedInstanceId },
    })
    if (outcome.type !== 'session')
      throw new RuntimeClientError('INVALID_RESPONSE')
    this.proof = outcome.data
  }

  async call(call: Call): Promise<Outcome> {
    const request: Request = {
      version: 1,
      requestId: crypto.randomUUID(),
      session: this.proof,
      call,
    }
    try {
      encodeRequest(request)
    } catch {
      throw new RuntimeClientError('INVALID_REQUEST')
    }
    let response: Response
    try {
      response = decodeResponse(await this.transport.exchange(request))
    } catch {
      throw new RuntimeClientError(
        'RUNTIME_DISCONNECTED',
        call.method === 'jobs.submit'
      )
    }
    if (response.version !== 1 || response.requestId !== request.requestId)
      throw new RuntimeClientError(
        'INVALID_RESPONSE',
        call.method === 'jobs.submit'
      )
    if (response.outcome.type === 'error')
      throw new RuntimeClientError(response.outcome.data.code)
    return response.outcome
  }

  async submit(job: SubmitJob): Promise<JobReceipt> {
    const outcome = await this.call({ method: 'jobs.submit', payload: job })
    if (
      outcome.type !== 'receipt' ||
      outcome.data.receiptType !== 'job' ||
      outcome.data.formatVersion !== 1 ||
      outcome.data.instanceId !== this.instanceId
    )
      throw new RuntimeClientError('INVALID_RESPONSE', true)
    return outcome.data
  }

  async job(runId: string): Promise<JobSnapshot> {
    const outcome = await this.call({
      method: 'jobs.status',
      payload: { runId },
    })
    if (outcome.type !== 'job' || outcome.data.runId !== runId)
      throw new RuntimeClientError('INVALID_RESPONSE')
    return outcome.data
  }

  async cancel(runId: string): Promise<JobSnapshot> {
    const outcome = await this.call({
      method: 'jobs.cancel',
      payload: { runId },
    })
    if (outcome.type !== 'job' || outcome.data.runId !== runId)
      throw new RuntimeClientError('INVALID_RESPONSE')
    return outcome.data
  }

  /** Explicit polling subscription; reconnect never resubmits a business operation. */
  async *watch(runId: string, signal?: AbortSignal): AsyncGenerator<JobEvent> {
    let afterSequence = 0
    while (!signal?.aborted) {
      const outcome = await this.call({
        method: 'jobs.events',
        payload: { runId, afterSequence },
      })
      if (outcome.type !== 'events')
        throw new RuntimeClientError('INVALID_RESPONSE')
      for (const event of outcome.data) {
        if (event.runId !== runId || event.sequence <= afterSequence)
          throw new RuntimeClientError('INVALID_RESPONSE')
        afterSequence = event.sequence
        yield event
        if (
          ['succeeded', 'failed', 'cancelled', 'interrupted'].includes(
            event.state
          )
        )
          return
      }
      await new Promise<void>(resolve => setTimeout(resolve, 25))
    }
  }

  async waitForResult(
    runId: string,
    signal?: AbortSignal
  ): Promise<JobSnapshot> {
    for await (const _event of this.watch(runId, signal)) {
      void _event
      /* Consume metadata-only progress. */
    }
    if (signal?.aborted) throw new RuntimeClientError('ABORTED')
    return this.job(runId)
  }

  close(): void {
    this.proof = null
    this.transport.close()
  }
}
