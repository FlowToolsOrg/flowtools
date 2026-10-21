import type { ErrorCode, RuntimeStatus } from '@flowtools/runtime-client'

import { expect, test } from 'bun:test'

import { createRuntimeStatusDiagnostic } from '../src/settings/runtime-status-diagnostic'

const at = '2026-10-21T01:02:03.000Z'
const versions = { appVersion: '0.1.0', tauriVersion: '2.11.0' }
const status: RuntimeStatus = {
  instanceId: 'fixture-diagnostic-host',
  mode: 'managed',
  jobs: 12,
  activeJobs: 3,
}

test('unqueried reports have explicit provenance and no invented Runtime status', () => {
  const report: unknown = JSON.parse(
    createRuntimeStatusDiagnostic(versions, { state: 'not-queried' }, at)
  )
  expect(report).toEqual({
    formatVersion: 1,
    scope: 'desktop-runtime-status',
    maturity: 'prototype',
    generatedAt: at,
    desktop: versions,
    observation: { state: 'not-queried' },
  })
})

test('observed reports project only bounded public fields without serializing unknown objects', () => {
  let serialized = false
  const hostile = {
    ...status,
    input: 'PRIVATE_INPUT_DONT_EXPORT',
    result: 'PRIVATE_RESULT_DONT_EXPORT',
    token: 'PRIVATE_TOKEN_DONT_EXPORT',
    resourcePath: 'C:\\PRIVATE_PATH_DONT_EXPORT',
    toJSON() {
      serialized = true
      throw new Error('PRIVATE_EXCEPTION_DONT_EXPORT')
    },
  }
  const text = createRuntimeStatusDiagnostic(
    versions,
    {
      state: 'observed',
      checkedAt: at,
      status: hostile,
    },
    at
  )
  expect(serialized).toBe(false)
  expect(text).not.toContain('PRIVATE_')
  const report: unknown = JSON.parse(text)
  expect(report).toHaveProperty('observation', {
    state: 'observed',
    checkedAt: at,
    status,
  })
  expect(new TextEncoder().encode(text).length).toBeLessThanOrEqual(4096)
})

test('unconfirmed reports contain a stable code and no previous snapshot', () => {
  const report: unknown = JSON.parse(
    createRuntimeStatusDiagnostic(
      versions,
      {
        state: 'unconfirmed',
        checkedAt: at,
        code: 'RUNTIME_DISCONNECTED',
      },
      at
    )
  )
  expect(report).toEqual({
    formatVersion: 1,
    scope: 'desktop-runtime-status',
    maturity: 'prototype',
    generatedAt: at,
    desktop: versions,
    observation: {
      state: 'unconfirmed',
      checkedAt: at,
      code: 'RUNTIME_DISCONNECTED',
    },
  })
})

test('unknown Host mode is normalized instead of exporting arbitrary text', () => {
  const text = createRuntimeStatusDiagnostic(
    versions,
    {
      state: 'observed',
      checkedAt: at,
      status: { ...status, mode: 'PRIVATE_MODE_DONT_EXPORT' },
    },
    at
  )
  const report: unknown = JSON.parse(text)
  expect(report).toHaveProperty('observation.status.mode', 'unknown')
  expect(text).not.toContain('PRIVATE_MODE')
})

test('invalid identifiers, counters and observation tags refuse the report', () => {
  for (const patch of [
    { instanceId: 'C:\\PRIVATE_PATH' },
    { instanceId: 'x'.repeat(129) },
    { jobs: -1 },
    { jobs: 2 ** 32 },
    { jobs: 1.5 },
    { activeJobs: 13 },
  ]) {
    expect(() =>
      createRuntimeStatusDiagnostic(
        versions,
        {
          state: 'observed',
          checkedAt: at,
          status: { ...status, ...patch },
        },
        at
      )
    ).toThrow('DIAGNOSTIC_INVALID')
  }
  expect(() =>
    createRuntimeStatusDiagnostic(
      versions,
      {
        state: 'PRIVATE_STATE' as 'not-queried',
      },
      at
    )
  ).toThrow('DIAGNOSTIC_INVALID')
})

test('private or oversized version strings and malformed timestamps refuse without echoing values', () => {
  for (const appVersion of [
    'PRIVATE_VERSION',
    'C:\\PRIVATE_PATH',
    '1.2.3+' + 'x'.repeat(200),
  ]) {
    expect(() =>
      createRuntimeStatusDiagnostic(
        { ...versions, appVersion },
        { state: 'not-queried' },
        at
      )
    ).toThrow('DIAGNOSTIC_INVALID')
  }
  for (const value of ['PRIVATE_TIME', '2026-02-31T01:02:03.000Z']) {
    expect(() =>
      createRuntimeStatusDiagnostic(versions, { state: 'not-queried' }, value)
    ).toThrow('DIAGNOSTIC_INVALID')
    expect(() =>
      createRuntimeStatusDiagnostic(
        versions,
        { state: 'observed', checkedAt: value, status },
        at
      )
    ).toThrow('DIAGNOSTIC_INVALID')
  }
})

test('unknown error codes cannot become arbitrary diagnostic text', () => {
  expect(() =>
    createRuntimeStatusDiagnostic(
      versions,
      {
        state: 'unconfirmed',
        checkedAt: at,
        code: 'PRIVATE_ERROR' as ErrorCode,
      },
      at
    )
  ).toThrow('DIAGNOSTIC_INVALID')
})
