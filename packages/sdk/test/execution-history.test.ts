import type { PluginExecutionResult } from '../src/execution/executor'

import { expect, test } from 'bun:test'

import { z } from 'zod'

import { createExecutionHistory } from '../src/execution/history'

const outcome: PluginExecutionResult = {
  success: true,
  pluginId: 'history-fixture',
  pluginVersion: '1.2.3',
  startedAt: 1,
  finishedAt: 4,
  durationMs: 3,
  inputSummary: { kind: 'object', size: 1 },
  data: { type: 'json', value: { result: 'secret-token' } },
}

test('history retains real metadata but neither output nor exception messages', () => {
  let raw = ''
  const history = createExecutionHistory(
    {
      getItem: () => null,
      setItem: (_key, value) => {
        raw = value
      },
    },
    'history-key'
  )
  history.record(outcome, 'Fixture')
  history.record(
    {
      ...outcome,
      success: false,
      error: { code: 'ABORTED', message: 'secret-token' },
    },
    'Fixture'
  )
  expect(raw).not.toContain('secret-token')
  expect(history.getSnapshot().entries[0]).toMatchObject({
    status: 'cancelled',
    errorCode: 'ABORTED',
    pluginVersion: '1.2.3',
    durationMs: 3,
  })
  expect(history.getSnapshot().entries[1]).toMatchObject({
    status: 'success',
    resultType: 'json',
  })
})

test('history reloads validated versioned metadata and excludes legacy/raw records', () => {
  let raw = ''
  const storage = {
    getItem: () => raw || null,
    setItem: (_key: string, value: string) => {
      raw = value
    },
  }
  createExecutionHistory(storage, 'new-history').record(outcome, 'Fixture')
  expect(
    createExecutionHistory(storage, 'new-history').getSnapshot().entries
  ).toHaveLength(1)
  raw = JSON.stringify({
    state: { entries: [{ input: 'secret-token', status: 'success' }] },
    version: 0,
  })
  expect(
    createExecutionHistory(storage, 'new-history').getSnapshot().entries
  ).toEqual([])
  raw = '{invalid'
  expect(
    createExecutionHistory(storage, 'new-history').getSnapshot().entries
  ).toEqual([])
})

test('history has stable snapshots, bounded entries, cleanup and visible persistence failure', () => {
  const history = createExecutionHistory(
    {
      getItem: () => null,
      setItem: () => {
        throw new Error('disk error')
      },
    },
    'fixture',
    2
  )
  expect(history.getSnapshot()).toBe(history.getSnapshot())
  let events = 0
  const unsubscribe = history.subscribe(() => {
    events += 1
  })
  history.record(outcome, 'Fixture')
  history.record(outcome, 'Fixture')
  history.record(outcome, 'Fixture')
  expect(history.getSnapshot().entries).toHaveLength(2)
  expect(history.getSnapshot().persistenceError).toBeTruthy()
  unsubscribe()
  history.clear('history-fixture')
  expect(history.getSnapshot().entries).toEqual([])
  expect(events).toBe(3)
})

test('history rejects contradictory restored outcomes rather than showing fake success', () => {
  let raw = ''
  const storage = {
    getItem: () => raw,
    setItem: (_key: string, value: string) => {
      raw = value
    },
  }
  createExecutionHistory(storage, 'fixture').record(outcome, 'Fixture')
  const valid = z
    .object({
      formatVersion: z.literal(1),
      entries: z.array(z.record(z.string(), z.unknown())),
    })
    .parse(JSON.parse(raw))
  const record = valid.entries[0]!
  for (const invalid of [
    { ...record, status: 'success', errorCode: 'ABORTED' },
    { ...record, status: 'success', resultType: undefined },
    {
      ...record,
      status: 'cancelled',
      resultType: undefined,
      errorCode: 'TIMEOUT',
    },
    { ...record, status: 'error', resultType: undefined, errorCode: 'ABORTED' },
  ]) {
    raw = JSON.stringify({ formatVersion: 1, entries: [invalid] })
    expect(
      createExecutionHistory(storage, 'fixture').getSnapshot().entries
    ).toEqual([])
  }
})
