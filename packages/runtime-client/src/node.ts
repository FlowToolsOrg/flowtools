import type { Request, Response } from './bindings'
import type { RuntimeTransport } from './client'

import { createConnection } from 'node:net'

import { RuntimeClientError } from './client'
import { decodeResponse, encodeRequest, MAX_FRAME_BYTES } from './codec'

/** Windows local-only validation transport; no TCP, shell or cold-start fallback. */
export async function connectNamedPipe(
  path: string
): Promise<RuntimeTransport> {
  if (!/^\\\\\.\\pipe\\flowtools-validation-[a-f0-9]{64}$/.test(path))
    throw new RuntimeClientError('INVALID_REQUEST')
  const socket = createConnection(path)
  socket.on('error', () => {})
  await new Promise<void>((resolve, reject) => {
    const timer = setTimeout(() => {
      socket.destroy()
      reject(new RuntimeClientError('RUNTIME_DISCONNECTED'))
    }, 5000)
    socket.once('connect', () => {
      clearTimeout(timer)
      resolve()
    })
    socket.once('error', () => {
      clearTimeout(timer)
      reject(new RuntimeClientError('RUNTIME_DISCONNECTED'))
    })
  })
  let tail: Promise<unknown> = Promise.resolve()
  let queued = 0
  let closed = false
  let buffer = Buffer.alloc(0)
  let pending:
    | {
        resolve(response: Response): void
        reject(error: Error): void
        timer: ReturnType<typeof setTimeout>
      }
    | undefined
  const failed = () => {
    closed = true
    socket.destroy()
    if (pending) {
      clearTimeout(pending.timer)
      pending.reject(new RuntimeClientError('RUNTIME_DISCONNECTED'))
      pending = undefined
    }
    buffer = Buffer.alloc(0)
  }
  socket.on('close', failed)
  socket.on('error', failed)
  // Keep a single reader for the lifetime of the stream, including between requests.
  socket.on('data', (chunk: Buffer) => {
    if (!pending || buffer.length + chunk.length > MAX_FRAME_BYTES + 4) {
      failed()
      return
    }
    buffer = Buffer.concat([buffer, chunk])
    if (buffer.length < 4) return
    const size = buffer.readUInt32LE(0)
    if (size > MAX_FRAME_BYTES || buffer.length > size + 4) {
      failed()
      return
    }
    if (buffer.length < size + 4) return
    try {
      const response = decodeResponse(
        JSON.parse(buffer.subarray(4).toString('utf8'))
      )
      const operation = pending
      pending = undefined
      buffer = Buffer.alloc(0)
      clearTimeout(operation.timer)
      operation.resolve(response)
    } catch {
      failed()
    }
  })
  return {
    exchange(request: Request): Promise<Response> {
      if (closed || queued >= 64)
        return Promise.reject(
          new RuntimeClientError(
            closed ? 'RUNTIME_DISCONNECTED' : 'RUNTIME_BUSY'
          )
        )
      const body = Buffer.from(encodeRequest(request))
      queued++
      const operation = tail
        .catch(() => {})
        .then(
          () =>
            new Promise<Response>((resolve, reject) => {
              if (closed || socket.destroyed) {
                reject(new RuntimeClientError('RUNTIME_DISCONNECTED'))
                return
              }
              pending = { resolve, reject, timer: setTimeout(failed, 5000) }
              const header = Buffer.alloc(4)
              header.writeUInt32LE(body.length)
              socket.write(Buffer.concat([header, body]), error => {
                if (error) failed()
              })
            })
        )
        .finally(() => {
          queued--
        })
      tail = operation
      return operation
    },
    close: failed,
  }
}
