import { expect, test } from 'bun:test'

import {
  encodeRequest,
  decodeResponse,
  encodeStorageAction,
  decodeStorageReport,
} from '../src/codec'

test('storage schema exposes logical backup identities and rejects paths and forged diagnostic payloads', () => {
  expect(() =>
    encodeStorageAction({
      operation: 'restore',
      parameters: { backupId: 'one', path: 'C:/private' },
    })
  ).toThrow('INVALID_REQUEST')
  expect(() =>
    encodeStorageAction({ operation: 'run', parameters: { argv: [] } })
  ).toThrow('INVALID_REQUEST')
  expect(() =>
    decodeStorageReport({
      formatVersion: 1,
      backups: [],
      recovery: null,
      token: 'private',
    })
  ).toThrow('INVALID_RESPONSE')
  expect(() =>
    decodeResponse({
      version: 1,
      requestId: 'one',
      outcome: {
        type: 'diagnostic',
        data: { runId: 'one', input: 'private', output: 'private' },
      },
    })
  ).toThrow('INVALID_RESPONSE')
})

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
