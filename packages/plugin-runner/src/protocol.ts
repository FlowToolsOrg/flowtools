import type { JsonValue } from '@flowtools/sdk/manifest'
import type { ServiceCapability } from '@flowtools/sdk/services'

import { isJsonValue } from '@flowtools/sdk/manifest'
import { serviceTargetSchema } from '@flowtools/sdk/services'

const limit = 1_048_576
export interface ArtifactCapability {
  read(handle: string): Promise<JsonValue>
}
export class RunnerFrames {
  private iterator = process.stdin[Symbol.asyncIterator]()
  private buffer = Buffer.alloc(0)
  private queue: Promise<unknown> = Promise.resolve()
  private sequence = 0

  async read(): Promise<unknown> {
    const fill = async (length: number) => {
      while (this.buffer.length < length) {
        const chunk = await this.iterator.next()
        if (chunk.done) throw new Error('RUNTIME_DISCONNECTED')
        this.buffer = Buffer.concat([
          this.buffer,
          Buffer.from(chunk.value as Uint8Array),
        ])
        if (this.buffer.length > limit + 4) throw new Error('FRAME_TOO_LARGE')
      }
    }
    await fill(4)
    const length = this.buffer.readUInt32LE(0)
    if (!length || length > limit) throw new Error('FRAME_TOO_LARGE')
    await fill(length + 4)
    const bytes = this.buffer.subarray(4, length + 4)
    this.buffer = this.buffer.subarray(length + 4)
    return JSON.parse(bytes.toString('utf8')) as unknown
  }
  write(value: unknown): void {
    const body = Buffer.from(JSON.stringify(value))
    if (!body.length || body.length > limit) throw new Error('FRAME_TOO_LARGE')
    const header = Buffer.alloc(4)
    header.writeUInt32LE(body.length)
    process.stdout.write(Buffer.concat([header, body]))
  }
  private exchange(value: unknown): Promise<JsonValue> {
    // Snapshot and bound before entering the queue; callers cannot race mutation.
    const snapshot = JSON.stringify(value)
    if (Buffer.byteLength(snapshot) > limit - 1024)
      return Promise.reject(new Error('FRAME_TOO_LARGE'))
    const request = async () => {
      const id = ++this.sequence
      this.write({ ...(JSON.parse(snapshot) as object), id })
      const reply = await this.read()
      if (!reply || typeof reply !== 'object')
        throw new Error('INVALID_RESPONSE')
      const response = reply as Record<string, unknown>
      if (
        response.kind !== 'reply' ||
        response.id !== id ||
        typeof response.success !== 'boolean' ||
        Object.keys(response).sort().join(',') !==
          (response.success
            ? 'data,id,kind,success'
            : 'errorCode,id,kind,success')
      )
        throw new Error('INVALID_RESPONSE')
      if (!response.success) {
        if (
          typeof response.errorCode !== 'string' ||
          !/^[A-Z_]{1,64}$/.test(response.errorCode)
        )
          throw new Error('INVALID_RESPONSE')
        throw new Error(response.errorCode)
      }
      if (!isJsonValue(response.data)) throw new Error('INVALID_RESPONSE')
      return response.data
    }
    const result = this.queue.then(request)
    this.queue = result.catch(() => {})
    return result
  }
  readonly services: ServiceCapability = {
    call: (target, input) => {
      const parsed = serviceTargetSchema.parse(target)
      if (!isJsonValue(input)) return Promise.reject(new Error('INPUT_INVALID'))
      return this.exchange({ kind: 'service', target: parsed, input })
    },
  }
  readonly artifacts: ArtifactCapability = {
    read: handle => {
      if (!/^[a-zA-Z0-9_-]{1,128}$/.test(handle))
        return Promise.reject(new Error('SCOPE_DENIED'))
      return this.exchange({
        kind: 'capability',
        operation: { operation: 'file-read', parameters: { handle } },
      })
    },
  }
  async drained(): Promise<void> {
    await this.queue
  }
}

/** Only the Host-selected child pipe can deliver the first request. */
export async function serveRunner(
  execute: (value: unknown, frames: RunnerFrames) => Promise<unknown>
) {
  const frames = new RunnerFrames()
  const initial = await frames.read()
  if (
    !initial ||
    typeof initial !== 'object' ||
    Object.keys(initial).sort().join(',') !== 'kind,payload' ||
    (initial as { kind?: unknown }).kind !== 'start'
  )
    throw new Error('INVALID_REQUEST')
  const result = await execute(
    (initial as { payload: unknown }).payload,
    frames
  )
  await frames.drained()
  frames.write({ kind: 'result', ...(result as object) })
}
