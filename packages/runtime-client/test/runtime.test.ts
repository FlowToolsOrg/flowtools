import { expect, test } from 'bun:test'

import { encodeRequest } from '../src/codec'

test('wire schema rejects identity injection and non-finite JSON before transport', () => {
  const request = {
    version: 1,
    requestId: 'test',
    session: null,
    call: {
      method: 'jobs.submit' as const,
      payload: {
        pluginId: 'plugin-base64-encoder',
        commandId: 'run',
        input: { text: 'hello' },
        idempotencyKey: 'test',
        background: false,
        deadline: Date.now() + 1000,
        agentId: 'admin',
      },
    },
  }
  expect(() => encodeRequest(request)).toThrow('INVALID_REQUEST')
  delete (request.call.payload as { agentId?: string }).agentId
  request.call.payload.deadline = Number.POSITIVE_INFINITY
  expect(() => encodeRequest(request)).toThrow('INVALID_REQUEST')
})
