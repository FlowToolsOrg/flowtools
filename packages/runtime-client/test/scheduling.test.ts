import type { CreateSchedule, ScheduleRecord, Outcome } from '../src/bindings'

import { expect, test } from 'bun:test'

import { RuntimeClient } from '../src/client'
import { encodeRequest, decodeResponse } from '../src/codec'
import { validScheduleDefinition, validScheduleRecord } from '../src/scheduling'

const create: CreateSchedule = {
  requestKey: '00000000-0000-4000-8000-000000000001',
  target: 'cli',
  pluginId: 'plugin-base64-encoder',
  commandId: 'run',
  input: { text: 'private' },
  timeoutMs: 1000,
  definition: {
    firstDueMs: 100000,
    intervalMs: 1000,
    occurrences: 3,
    graceMs: 500,
    missed: 'skip',
  },
}
const record: ScheduleRecord = {
  id: '00000000-0000-4000-8000-000000000002',
  revision: 1,
  definition: create.definition,
  binding: {
    identity: {
      caller: 'local-cli',
      publisher: 'flowtools',
      pluginId: create.pluginId,
      commandId: 'run',
      version: '0.1.0',
      packageDigest: 'a'.repeat(64),
    },
    inputDigest: 'b'.repeat(64),
    grantEpoch: 1,
    timeoutMs: 1000,
  },
  nextIndex: 0,
  lastObservedMs: null,
  cancelled: false,
  skipped: 0,
  lastClaim: null,
}
function client(outcome: Outcome) {
  return new RuntimeClient({
    async exchange(request) {
      return { version: 1, requestId: request.requestId, outcome }
    },
    close() {},
  })
}
test('Rust-derived schedule wire rejects identity, grant, path and init injection', () => {
  for (const extra of ['caller', 'grant', 'profile', 'path', 'initialize']) {
    expect(() =>
      encodeRequest({
        version: 1,
        requestId: 'schedule',
        session: null,
        call: {
          method: 'schedules.create',
          payload: { ...create, [extra]: true },
        },
      })
    ).toThrow('INVALID_REQUEST')
  }
  expect(
    encodeRequest({
      version: 1,
      requestId: 'schedule',
      session: null,
      call: { method: 'schedules.create', payload: create },
    })
  ).toContain('schedules.create')
})
test('bounded definitions reject fractional, excessive and overflow time values', () => {
  for (const definition of [
    { ...create.definition, occurrences: 0 },
    { ...create.definition, occurrences: 10001 },
    { ...create.definition, firstDueMs: Number.MAX_SAFE_INTEGER },
    { ...create.definition, firstDueMs: 0.5 },
    { ...create.definition, intervalMs: null },
    { ...create.definition, graceMs: 60001 },
  ])
    expect(validScheduleDefinition(definition)).toBe(false)
})
test('client validates schedules and rejects substituted positions and duplicated identities', async () => {
  expect(
    await client({ type: 'schedules', data: [record] }).schedules()
  ).toEqual([record])
  expect(
    await client({ type: 'schedule', data: record }).createSchedule(create)
  ).toEqual(record)
  expect(validScheduleRecord({ ...record, nextIndex: 4 })).toBe(false)
  await expect(
    client({ type: 'schedules', data: [record, record] }).schedules()
  ).rejects.toThrow('INVALID_RESPONSE')
  expect(() =>
    decodeResponse({
      version: 1,
      requestId: 'schedule',
      outcome: {
        type: 'schedule',
        data: {
          ...record,
          definition: { ...record.definition, firstDueMs: 9007199254740992 },
        },
      },
    })
  ).toThrow('INVALID_RESPONSE')
})
